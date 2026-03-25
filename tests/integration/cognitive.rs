//! Integration tests for the cognitive memory system and coaching engine.
//!
//! Covers the full pipeline: DomainEvent → salience filter → extraction →
//! consolidation → retrieval, plus coaching trigger evaluation, rate limiting,
//! and feedback tracking.

use klyntbot::agent::cognitive_handlers::{
    HeuristicConsolidationHandler, HeuristicExtractionHandler,
};
use klyntbot::cognitive::consolidation::{execute_memory_ops, ConsolidationCandidate};
use klyntbot::cognitive::context_source::CognitiveContextSource;
use klyntbot::cognitive::extraction::to_semantic_fact;
use klyntbot::cognitive::repos::{cognitive_migrations, ProceduralRuleRepo, SemanticFactRepo};
use klyntbot::cognitive::retrieval::{retrieve_relevant_facts, RetrievalParams};
use klyntbot::cognitive::salience::evaluate_salience;
use klyntbot::cognitive::types::*;
use klyntbot::cognitive::{ConsolidationHandler, ExtractionHandler};

use super::common::test_pool;
use bus::{DomainEvent, FeedbackResponse};
use chrono::Utc;
use context_engine::source::{ContextSource, SourceContext};
use feature_coaching::feedback::FeedbackTracker;
use feature_coaching::reasoner::{CoachingDecision, InterventionType};
use feature_coaching::router::{
    DeliveredIntervention, InterventionRouter, RouterConfig, RoutingResult,
};
use feature_coaching::signal_accumulator::SignalAccumulator;

// ── Helpers ────────────────────────────────────────────────────

/// Create a test pool with cognitive migrations applied.
async fn cognitive_pool() -> (klyntbot::storage::StoragePool, sqlx::SqlitePool) {
    let pool = test_pool().await;
    let inner = pool.inner().clone();
    klyntbot::storage::StoragePool::run_feature_migrations(&inner, &cognitive_migrations())
        .await
        .expect("cognitive migrations");
    (pool, inner)
}

fn test_observation(source: &str, content: &str, importance: f64) -> Observation {
    Observation {
        domain: "productivity".into(),
        content: content.into(),
        importance,
        source_event: source.into(),
        timestamp: Utc::now(),
    }
}

fn test_fact(id: &str, predicate: &str, object: &str) -> SemanticFact {
    SemanticFact {
        id: id.into(),
        domain: "productivity".into(),
        subject: "user".into(),
        predicate: predicate.into(),
        object: object.into(),
        confidence: 0.8,
        source: "observed".into(),
        valid_from: "2026-03-01".into(),
        valid_until: None,
        recorded_at: "2026-03-06T10:00:00".into(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: DEFAULT_MEMORY_TYPE.to_string(),
        scope_type: "system".to_string(),
        scope_id: None,
    }
}

fn test_coaching_decision(intervene: bool, msg: &str) -> CoachingDecision {
    CoachingDecision {
        should_intervene: intervene,
        confidence: 0.8,
        message: Some(msg.into()),
        intervention_type: InterventionType::ChatMessage,
        reasoning: "test".into(),
        observations: vec![],
    }
}

fn test_delivered_intervention(trigger: &str) -> DeliveredIntervention {
    DeliveredIntervention {
        id: uuid::Uuid::new_v4().to_string(),
        intervention_type: InterventionType::ChatMessage,
        message: "test intervention".into(),
        delivered_at: Utc::now(),
        trigger_name: trigger.into(),
        action_url: None,
    }
}

// ── Salience filter ────────────────────────────────────────────

#[test]
fn test_salience_user_stated_fact_is_extract() {
    let verdict = evaluate_salience(&DomainEvent::UserStatedFact {
        fact: "I work best in mornings".into(),
        domain: "productivity".into(),
    });
    assert_eq!(verdict, SalienceVerdict::Extract);
}

#[test]
fn test_salience_budget_alert_is_extract() {
    let verdict = evaluate_salience(&DomainEvent::BudgetAlert {
        category: "food".into(),
        spent: 450.0,
        limit: 500.0,
    });
    assert_eq!(verdict, SalienceVerdict::Extract);
}

#[test]
fn test_salience_task_completed_is_accumulate() {
    let verdict = evaluate_salience(&DomainEvent::TaskCompleted {
        task_id: "t1".into(),
        actual_duration_mins: Some(30),
        estimated_duration_mins: Some(45),
        deviation_pct: None,
    });
    assert_eq!(verdict, SalienceVerdict::Accumulate);
}

#[test]
fn test_salience_productivity_score_is_accumulate() {
    let verdict = evaluate_salience(&DomainEvent::ProductivityScoreComputed {
        date: "2026-03-06".into(),
        score: 72.0,
    });
    assert_eq!(verdict, SalienceVerdict::Accumulate);
}

// ── Extraction with heuristic handler ──────────────────────────

#[tokio::test]
async fn test_heuristic_extraction_user_stated() {
    let handler = HeuristicExtractionHandler;
    let obs = test_observation("UserStatedFact", "I prefer mornings for deep work", 1.0);
    let result = handler.extract_facts_batch(&[obs]).await.unwrap();
    let facts: Vec<_> = result
        .extractions
        .into_iter()
        .flat_map(|e| e.facts)
        .collect();

    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].source, "user_stated");
    assert_eq!(facts[0].confidence, 1.0);
    assert!(facts[0].object.contains("mornings"));
}

#[tokio::test]
async fn test_heuristic_extraction_low_importance_skipped() {
    let handler = HeuristicExtractionHandler;
    let obs = test_observation("ProductivityScoreComputed", "Score: 72", 0.5);
    let result = handler.extract_facts_batch(&[obs]).await.unwrap();
    let facts: Vec<_> = result
        .extractions
        .into_iter()
        .flat_map(|e| e.facts)
        .collect();

    assert!(
        facts.is_empty(),
        "Low-importance events should not produce facts"
    );
}

#[tokio::test]
async fn test_heuristic_extraction_accumulated_pattern() {
    let handler = HeuristicExtractionHandler;
    let obs = Observation {
        domain: "productivity".into(),
        content: "Pattern from 5 accumulated events".into(),
        importance: 0.5,
        source_event: "accumulated:ProductivityScoreComputed".into(),
        timestamp: Utc::now(),
    };
    let result = handler.extract_facts_batch(&[obs]).await.unwrap();
    let facts: Vec<_> = result
        .extractions
        .into_iter()
        .flat_map(|e| e.facts)
        .collect();

    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].source, "inferred");
    assert_eq!(facts[0].confidence, 0.7);
}

// ── Consolidation with heuristic handler ───────────────────────

#[tokio::test]
async fn test_consolidation_add_new_fact() {
    let (_, inner) = cognitive_pool().await;
    let repo = SemanticFactRepo::new(inner);
    let handler = HeuristicConsolidationHandler;

    let candidate = test_fact("f1", "peak_hours", "10am-12pm");
    let candidates = vec![ConsolidationCandidate {
        candidate: candidate.clone(),
        existing: vec![],
    }];
    let ops = handler.decide_batch(&candidates).await.unwrap();
    execute_memory_ops(&ops, &candidates, &repo, None).await;

    assert!(matches!(ops[0], MemoryOp::Add { ref id } if id == "f1"));

    let stored = repo.get("f1").await.unwrap().unwrap();
    assert_eq!(stored.object, "10am-12pm");
}

#[tokio::test]
async fn test_consolidation_update_on_changed_value() {
    let (_, inner) = cognitive_pool().await;
    let repo = SemanticFactRepo::new(inner);
    let handler = HeuristicConsolidationHandler;

    // Insert existing fact
    let old = test_fact("f1", "peak_hours", "10am-12pm");
    repo.upsert(&old).await.unwrap();

    // Candidate with updated value
    let candidate = test_fact("f2", "peak_hours", "2pm-4pm");
    let candidates = vec![ConsolidationCandidate {
        candidate: candidate.clone(),
        existing: vec![old.clone()],
    }];
    let ops = handler.decide_batch(&candidates).await.unwrap();
    execute_memory_ops(&ops, &candidates, &repo, None).await;

    assert!(matches!(ops[0], MemoryOp::Update { .. }));

    // Old fact should be superseded
    let old_fact = repo.get("f1").await.unwrap().unwrap();
    assert!(old_fact.superseded_at.is_some());
    assert_eq!(old_fact.superseded_by, Some("f2".into()));

    // New fact should exist
    let new_fact = repo.get("f2").await.unwrap().unwrap();
    assert_eq!(new_fact.object, "2pm-4pm");
}

#[tokio::test]
async fn test_consolidation_noop_on_duplicate() {
    let (_, inner) = cognitive_pool().await;
    let repo = SemanticFactRepo::new(inner);
    let handler = HeuristicConsolidationHandler;

    let old = test_fact("f1", "peak_hours", "10am-12pm");
    repo.upsert(&old).await.unwrap();

    // Same predicate + same object = NOOP
    let candidate = test_fact("f2", "peak_hours", "10am-12pm");
    let candidates = vec![ConsolidationCandidate {
        candidate: candidate.clone(),
        existing: vec![old.clone()],
    }];
    let ops = handler.decide_batch(&candidates).await.unwrap();

    assert_eq!(ops[0], MemoryOp::Noop);
}

// ── Full pipeline: extraction → consolidation → retrieval ──────

#[tokio::test]
async fn test_full_pipeline_event_to_retrieval() {
    let (_, inner) = cognitive_pool().await;
    let repo = SemanticFactRepo::new(inner);
    let extraction = HeuristicExtractionHandler;
    let consolidation = HeuristicConsolidationHandler;

    // 1. Extract from a user-stated fact observation
    let obs = test_observation("UserStatedFact", "I work best between 10am and noon", 1.0);
    let observations = [obs];
    let batch_result = extraction.extract_facts_batch(&observations).await.unwrap();
    let facts: Vec<_> = batch_result
        .extractions
        .iter()
        .flat_map(|e| {
            e.facts
                .iter()
                .map(|f| to_semantic_fact(f, &observations[e.observation_index]))
        })
        .collect();
    assert!(!facts.is_empty());

    // 2. Build consolidation candidates (no existing facts yet → direct ADD)
    let candidates: Vec<_> = facts
        .iter()
        .map(|f| ConsolidationCandidate {
            candidate: f.clone(),
            existing: vec![],
        })
        .collect();
    let ops = consolidation.decide_batch(&candidates).await.unwrap();
    assert!(ops.iter().any(|op| matches!(op, MemoryOp::Add { .. })));

    // 3. Execute ops into storage
    execute_memory_ops(&ops, &candidates, &repo, None).await;

    // 4. Retrieve the stored fact
    let results = retrieve_relevant_facts(
        &repo,
        None,
        "",
        &["productivity"],
        &RetrievalParams {
            limit: 10,
            situational_boost: 0.5,
            ..RetrievalParams::new(0)
        },
    )
    .await
    .unwrap();
    assert!(!results.is_empty());
    assert!(results[0].fact.object.contains("10am"));
    assert!(results[0].score > 0.0);
}

#[tokio::test]
async fn test_full_pipeline_update_replaces_old_fact() {
    let (_, inner) = cognitive_pool().await;
    let repo = SemanticFactRepo::new(inner);
    let extraction = HeuristicExtractionHandler;
    let consolidation = HeuristicConsolidationHandler;

    // First fact — extract and store
    let obs1 = test_observation("UserStatedFact", "I prefer mornings", 1.0);
    let observations1 = [obs1];
    let batch1 = extraction
        .extract_facts_batch(&observations1)
        .await
        .unwrap();
    let facts1: Vec<_> = batch1
        .extractions
        .iter()
        .flat_map(|e| {
            e.facts
                .iter()
                .map(|f| to_semantic_fact(f, &observations1[e.observation_index]))
        })
        .collect();
    let candidates1: Vec<_> = facts1
        .iter()
        .map(|f| ConsolidationCandidate {
            candidate: f.clone(),
            existing: vec![],
        })
        .collect();
    let ops1 = consolidation.decide_batch(&candidates1).await.unwrap();
    execute_memory_ops(&ops1, &candidates1, &repo, None).await;

    // Second fact (same predicate "stated" → triggers UPDATE in heuristic handler)
    let obs2 = test_observation("UserStatedFact", "Actually I prefer afternoons", 1.0);
    let observations2 = [obs2];
    let batch2 = extraction
        .extract_facts_batch(&observations2)
        .await
        .unwrap();
    let facts2: Vec<_> = batch2
        .extractions
        .iter()
        .flat_map(|e| {
            e.facts
                .iter()
                .map(|f| to_semantic_fact(f, &observations2[e.observation_index]))
        })
        .collect();

    // Look up existing facts for consolidation candidates
    let mut candidates2 = Vec::new();
    for fact in &facts2 {
        let existing = repo
            .find_similar(&fact.subject, &fact.predicate)
            .await
            .unwrap_or_default();
        candidates2.push(ConsolidationCandidate {
            candidate: fact.clone(),
            existing,
        });
    }
    let ops2 = consolidation.decide_batch(&candidates2).await.unwrap();

    // Should be an UPDATE since the predicate "stated" matches
    assert!(ops2.iter().any(|op| matches!(op, MemoryOp::Update { .. })));

    // Execute the update
    execute_memory_ops(&ops2, &candidates2, &repo, None).await;

    // Only the new fact should be active
    let results = retrieve_relevant_facts(
        &repo,
        None,
        "",
        &["productivity"],
        &RetrievalParams {
            limit: 10,
            situational_boost: 0.5,
            ..RetrievalParams::new(0)
        },
    )
    .await
    .unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].fact.object.contains("afternoons"));
}

// ── Live batch pipeline: BackgroundConsolidationService E2E ────

#[tokio::test]
async fn test_batch_pipeline_processes_domain_events_end_to_end() {
    use klyntbot::cognitive::background::{BackgroundConsolidationService, PipelineEvent};
    use klyntbot::cognitive::{
        AccumulatedObservationRepo, EpisodicMemoryRepo, FailedObservationRepo,
    };
    use tokio_util::sync::CancellationToken;

    let (_, inner) = cognitive_pool().await;
    let repo = SemanticFactRepo::new(inner.clone());
    let episodic_repo = EpisodicMemoryRepo::new(inner.clone());
    let accum_repo = AccumulatedObservationRepo::new(inner.clone());
    let failed_obs_repo = FailedObservationRepo::new(inner.clone());

    // Wire up broadcast channels (same as agent_loop/builder.rs)
    let (domain_tx, domain_rx) = tokio::sync::broadcast::channel::<DomainEvent>(64);
    let (pipeline_tx, mut pipeline_rx) = tokio::sync::broadcast::channel::<PipelineEvent>(64);

    let cancel = CancellationToken::new();

    // Start the real background service with heuristic handlers
    let extraction: std::sync::Arc<dyn ExtractionHandler> =
        std::sync::Arc::new(HeuristicExtractionHandler);
    let consolidation: std::sync::Arc<dyn ConsolidationHandler> =
        std::sync::Arc::new(HeuristicConsolidationHandler);

    let _service = BackgroundConsolidationService::start(
        klyntbot::cognitive::background::BackgroundServiceConfig {
            event_rx: domain_rx,
            extraction,
            consolidation,
            repo: repo.clone(),
            episodic_repo: Some(episodic_repo),
            embedder: None, // no embedder needed for heuristic
            cancel: cancel.clone(),
            pipeline_tx: Some(pipeline_tx),
            accum_repo: Some(accum_repo),
            failed_obs_repo: Some(failed_obs_repo),
            promote_threshold: 3,
            min_days: 2,
            domain_bus: None,
            context_update_queue: None,
        },
    );

    // Verify DB is empty before we begin
    let before = retrieve_relevant_facts(
        &repo,
        None,
        "",
        &["productivity"],
        &RetrievalParams {
            limit: 10,
            situational_boost: 0.5,
            ..RetrievalParams::new(0)
        },
    )
    .await
    .unwrap();
    assert!(before.is_empty(), "DB should be empty before pipeline runs");

    // Send two UserStatedFact events (will be batched together)
    domain_tx
        .send(DomainEvent::UserStatedFact {
            fact: "I work best between 10am and noon".into(),
            domain: "productivity".into(),
        })
        .unwrap();
    domain_tx
        .send(DomainEvent::UserStatedFact {
            fact: "I prefer dark mode in my IDE".into(),
            domain: "productivity".into(),
        })
        .unwrap();

    // Wait for pipeline events — we should see BatchStarted + Extractions + Consolidations
    let mut saw_batch_started = false;
    let mut extraction_count = 0;
    let mut consolidation_count = 0;
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(10);

    loop {
        tokio::select! {
            result = pipeline_rx.recv() => {
                match result {
                    Ok(PipelineEvent::BatchStarted { observation_count }) => {
                        saw_batch_started = true;
                        assert!(observation_count > 0, "Batch should have observations");
                    }
                    Ok(PipelineEvent::Extraction { facts_extracted, .. }) => {
                        extraction_count += facts_extracted;
                    }
                    Ok(PipelineEvent::Consolidation { .. }) => {
                        consolidation_count += 1;
                    }
                    Ok(_) => {} // DeadLetter events etc.
                    Err(_) => break,
                }
                // Once we've seen extraction + consolidation, give execute_memory_ops
                // time to flush to SQLite (events emit before DB write)
                if saw_batch_started && extraction_count > 0 && consolidation_count > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    break;
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                panic!(
                    "Timed out waiting for pipeline events (batch={}, extractions={}, consolidations={})",
                    saw_batch_started, extraction_count, consolidation_count
                );
            }
        }
    }

    // Verify facts were actually stored in the database
    let after = retrieve_relevant_facts(
        &repo,
        None,
        "",
        &["productivity"],
        &RetrievalParams {
            limit: 10,
            situational_boost: 0.5,
            ..RetrievalParams::new(0)
        },
    )
    .await
    .unwrap();
    assert!(
        !after.is_empty(),
        "Facts should be stored after pipeline processes events"
    );
    assert!(
        after.len() >= 2,
        "Should have at least 2 facts from 2 events, got {}",
        after.len()
    );

    // Shut down cleanly
    cancel.cancel();
}

// ── CognitiveContextSource integration ─────────────────────────

#[tokio::test]
async fn test_cognitive_context_source_with_facts() {
    let (_, inner) = cognitive_pool().await;
    let fact_repo = SemanticFactRepo::new(inner.clone());
    let rule_repo = ProceduralRuleRepo::new(inner);

    // Insert a fact in the "energy" domain
    let mut fact = test_fact("f1", "peak_hours", "10am-12pm");
    fact.domain = "energy".into();
    fact_repo.upsert(&fact).await.unwrap();

    // Insert a procedural rule
    let rule = ProceduralRule {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "productivity".into(),
        rule_text: "User works best after morning exercise".into(),
        confidence: 0.8,
        source: "reflected".into(),
        signal_count: 5,
        created_at: "2026-03-06".into(),
        updated_at: "2026-03-06".into(),
        active: true,
        project_id: None,
        scope_type: "system".to_string(),
        scope_id: None,
    };
    rule_repo.upsert(&rule).await.unwrap();

    let source = CognitiveContextSource::new(fact_repo, rule_repo);
    let ctx = SourceContext {
        channel: "test".into(),
        chat_id: "c1".into(),
        message: None,
        intent_summary: None,
        project_id: None,
    };

    let result = source.provide(&ctx).await;
    assert!(result.is_some());
    let text = result.unwrap();
    assert!(text.contains("User Understanding"), "Should have header");
    assert!(text.contains("peak_hours"), "Should include fact");
    assert!(text.contains("morning exercise"), "Should include rule");
}

#[tokio::test]
async fn test_cognitive_context_source_empty_returns_none() {
    let (_, inner) = cognitive_pool().await;
    let source = CognitiveContextSource::new(
        SemanticFactRepo::new(inner.clone()),
        ProceduralRuleRepo::new(inner),
    );
    let ctx = SourceContext {
        channel: "test".into(),
        chat_id: "c1".into(),
        message: None,
        intent_summary: None,
        project_id: None,
    };
    assert!(source.provide(&ctx).await.is_none());
}

#[tokio::test]
async fn test_cognitive_context_source_priority() {
    let (_, inner) = cognitive_pool().await;
    let source = CognitiveContextSource::new(
        SemanticFactRepo::new(inner.clone()),
        ProceduralRuleRepo::new(inner),
    );
    assert_eq!(source.priority(), 60, "Cognitive priority should be 60");
}

// ── Coaching: signal accumulator + triggers ────────────────────

fn distraction_situation() -> cognitive::situation::UserSituation {
    cognitive::situation::UserSituation {
        energy_level: 0.5,
        focus_state: 0.3,
        deadline_pressure: 0.2,
        distraction_risk: 0.8,
        coaching_receptivity: 0.7,
        task_avoidance_detected: false,
        hours_active_today: 4.0,
        mins_since_break: 45.0,
        hour_of_day: 10,
        recent_context_switches: 3,
    }
}

#[test]
fn test_signal_accumulator_distraction_streak_removed() {
    let mut acc = SignalAccumulator::new();

    // distraction_streak condition was removed — events are tracked for
    // pattern detection but no longer trigger coaching popups.
    for _ in 0..3 {
        acc.push_event(&DomainEvent::DistractionDetected {
            app: "reddit".into(),
            duration_secs: None,
            context: "browsing".into(),
        });
    }

    let fired = acc.evaluate(&distraction_situation());
    assert!(
        !fired
            .iter()
            .any(|t| t.condition_name == "distraction_streak"),
        "distraction_streak should no longer fire as a coaching trigger"
    );
}

#[test]
fn test_signal_accumulator_cooldown_prevents_refire() {
    let mut acc = SignalAccumulator::new();

    // Use budget_warning trigger (distraction_streak was removed as a coaching trigger).
    // Push enough budget alerts to fire the condition.
    for _ in 0..3 {
        acc.push_event(&DomainEvent::BudgetAlert {
            category: "dining".into(),
            spent: 280.0,
            limit: 300.0,
        });
    }

    let situation = distraction_situation();

    // First eval fires budget_warning
    let fired1 = acc.evaluate(&situation);
    assert!(
        fired1.iter().any(|t| t.condition_name == "budget_warning"),
        "First eval should fire budget_warning"
    );

    // Push another alert immediately
    acc.push_event(&DomainEvent::BudgetAlert {
        category: "dining".into(),
        spent: 290.0,
        limit: 300.0,
    });

    // Second eval — cooldown blocks re-fire
    let fired2 = acc.evaluate(&situation);
    let has_budget = fired2.iter().any(|t| t.condition_name == "budget_warning");
    assert!(!has_budget, "Cooldown should prevent re-fire");
}

// ── Coaching: intervention routing + rate limiting ─────────────

#[test]
fn test_intervention_router_delivers() {
    let mut router = InterventionRouter::default();
    let decision = test_coaching_decision(true, "Take a break!");
    let result = router.route(&decision, "distraction_streak");
    assert!(matches!(result, RoutingResult::Delivered(_)));
}

#[test]
fn test_intervention_router_rate_limits() {
    let config = RouterConfig {
        max_per_hour: 2,
        ..Default::default()
    };
    let mut router = InterventionRouter::new(config);
    let decision = test_coaching_decision(true, "msg");

    assert!(matches!(
        router.route(&decision, "t1"),
        RoutingResult::Delivered(_)
    ));
    assert!(matches!(
        router.route(&decision, "t2"),
        RoutingResult::Delivered(_)
    ));
    assert!(
        matches!(
            router.route(&decision, "t3"),
            RoutingResult::RateLimited { .. }
        ),
        "Third intervention should be rate limited"
    );
}

#[test]
fn test_intervention_router_dismissal_backoff() {
    let mut router = InterventionRouter::default();

    router.record_dismissal("distraction_streak");

    let decision = test_coaching_decision(true, "msg");
    let result = router.route(&decision, "distraction_streak");
    assert!(
        matches!(result, RoutingResult::RateLimited { .. }),
        "Dismissed trigger should be in cooldown"
    );

    // Different trigger should still work
    let result = router.route(&decision, "budget_warning");
    assert!(matches!(result, RoutingResult::Delivered(_)));
}

// ── Coaching: feedback tracking ────────────────────────────────

#[test]
fn test_feedback_tracker_records_delivery() {
    let mut tracker = FeedbackTracker::new();
    let intervention = test_delivered_intervention("distraction_streak");
    tracker.record_delivery(&intervention);

    let strategy = tracker.get_strategy("distraction_streak").unwrap();
    assert_eq!(strategy.times_used, 1);
}

#[test]
fn test_feedback_tracker_explicit_helpful() {
    let mut tracker = FeedbackTracker::new();
    let intervention = test_delivered_intervention("distraction_streak");
    let id = intervention.id.clone();
    tracker.record_delivery(&intervention);

    tracker.record_explicit(&id, &FeedbackResponse::Helpful);

    let strategy = tracker.get_strategy("distraction_streak").unwrap();
    assert_eq!(strategy.times_accepted, 1);
    assert_eq!(strategy.acceptance_rate(), 1.0);
}

#[test]
fn test_feedback_tracker_explicit_dismissed() {
    let mut tracker = FeedbackTracker::new();
    let intervention = test_delivered_intervention("distraction_streak");
    let id = intervention.id.clone();
    tracker.record_delivery(&intervention);

    tracker.record_explicit(&id, &FeedbackResponse::Dismissed);

    let strategy = tracker.get_strategy("distraction_streak").unwrap();
    assert_eq!(strategy.times_dismissed, 1);
    assert_eq!(strategy.acceptance_rate(), 0.0);
}

#[test]
fn test_feedback_effectiveness_positive() {
    let mut tracker = FeedbackTracker::new();

    for _ in 0..5 {
        let intervention = test_delivered_intervention("good_strategy");
        let id = intervention.id.clone();
        tracker.record_delivery(&intervention);
        tracker.record_explicit(&id, &FeedbackResponse::Helpful);
    }

    let strategy = tracker.get_strategy("good_strategy").unwrap();
    assert!(
        strategy.effectiveness() > 0.5,
        "Good strategy should have positive effectiveness"
    );
}

#[test]
fn test_feedback_receptivity_adjustment() {
    let mut tracker = FeedbackTracker::new();

    // Mostly helpful feedback → positive adjustment
    for _ in 0..4 {
        let intervention = test_delivered_intervention("helpful");
        let id = intervention.id.clone();
        tracker.record_delivery(&intervention);
        tracker.record_explicit(&id, &FeedbackResponse::Helpful);
    }

    assert!(
        tracker.receptivity_adjustment() > 0.0,
        "Mostly helpful feedback should increase receptivity"
    );

    // Now add dismissals
    let mut tracker2 = FeedbackTracker::new();
    for _ in 0..4 {
        let intervention = test_delivered_intervention("annoying");
        let id = intervention.id.clone();
        tracker2.record_delivery(&intervention);
        tracker2.record_explicit(&id, &FeedbackResponse::Dismissed);
    }

    assert!(
        tracker2.receptivity_adjustment() < 0.0,
        "Mostly dismissed feedback should decrease receptivity"
    );
}

// ── Coaching: feedback persistence round-trip ─────────────────

#[tokio::test]
async fn test_feedback_tracker_persist_and_load() {
    use storage::CoachingStrategyRepo;

    let pool = test_pool().await;
    // Run cognitive migrations so coaching_strategies table exists.
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive_migrations())
        .await
        .unwrap();

    let repo = CoachingStrategyRepo::new(pool.inner().clone());

    // 1. Create tracker with repo, record some feedback, persist.
    {
        let mut tracker = FeedbackTracker::new().with_repo(repo.clone());
        let intervention = test_delivered_intervention("persist_test");
        let id = intervention.id.clone();
        tracker.record_delivery(&intervention);
        tracker.record_explicit(&id, &FeedbackResponse::Helpful);
        tracker.persist().await;
    }

    // 2. Create a fresh tracker, load from DB, verify state survived.
    {
        let mut tracker = FeedbackTracker::new().with_repo(repo);
        assert!(
            tracker.get_strategy("persist_test").is_none(),
            "Fresh tracker should have no in-memory state"
        );

        tracker.load_from_db().await;

        let strategy = tracker
            .get_strategy("persist_test")
            .expect("Strategy should be loaded from DB");
        assert_eq!(strategy.times_used, 1);
        assert_eq!(strategy.times_accepted, 1);
    }
}
