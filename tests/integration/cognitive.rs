//! Integration tests for the cognitive memory system and coaching engine.
//!
//! Covers the full pipeline: DomainEvent → extraction → consolidation →
//! retrieval, plus coaching trigger evaluation, rate limiting, and feedback
//! tracking. The old salience filter has been removed; SignalRouter now
//! handles event classification upstream.

use klyntbot::agent::cognitive_handlers::{
    HeuristicConsolidationHandler, HeuristicExtractionHandler,
};
use klyntbot::cognitive::consolidation::{execute_memory_ops, ConsolidationCandidate};
use klyntbot::cognitive::context_source::CognitiveContextSource;
use klyntbot::cognitive::extraction::to_semantic_fact;
use klyntbot::cognitive::repos::{
    cognitive_migrations, MetricRepo, ProceduralRuleRepo, SemanticFactRepo,
};
use klyntbot::cognitive::retrieval::{retrieve_relevant_facts, RetrievalParams};
use klyntbot::cognitive::types::*;
use klyntbot::cognitive::{ConsolidationHandler, ExtractionHandler};

use super::common::test_pool;
use ai_core::{AiMetrics, AiSignal, MetricRegistry, RecallDomain, SalienceVerdict};
use bus::{DomainEvent, FeedbackResponse};
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
        timestamp: jiff::Timestamp::now(),
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
        convergence_score: 0.0,
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
        delivered_at: jiff::Timestamp::now(),
        trigger_name: trigger.into(),
        action_url: None,
    }
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
        timestamp: jiff::Timestamp::now(),
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
    execute_memory_ops(&ops, &candidates, &repo, None, None).await;

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
    execute_memory_ops(&ops, &candidates, &repo, None, None).await;

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
    execute_memory_ops(&ops, &candidates, &repo, None, None).await;

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
        None,
        None,
        None,
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
    execute_memory_ops(&ops1, &candidates1, &repo, None, None).await;

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
    execute_memory_ops(&ops2, &candidates2, &repo, None, None).await;

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
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].fact.object.contains("afternoons"));
}

// ── Live AI pipeline: SignalRouter -> IngestionConsumer E2E ────
//
// v1.5 migration replaced the BackgroundConsolidationService's
// `event_to_observation` match with a SignalRouter that translates each
// DomainEvent into an AiSignal and fans it out to SignalConsumers. This test
// asserts that contract end-to-end: publishing a DomainEvent on the bus
// causes IngestionConsumer to persist an `accumulated_observations` row.
// Extraction/consolidation/retrieval is no longer the responsibility of this
// boundary and is covered by the cognitive unit tests above.

#[tokio::test]
async fn test_signal_router_persists_observation_via_ingestion_consumer() {
    use ai_core::{SignalConsumer, SignalRouter};
    use bus::DomainEventBus;
    use klyntbot::cognitive::{AccumulatedObservationRepo, EpisodicMemoryRepo};

    let (_, inner) = cognitive_pool().await;
    let observation_repo = AccumulatedObservationRepo::new(inner.clone());
    let entity_repo = klyntbot::cognitive::repos::EntityRepo::new(inner.clone());
    let episodic_repo = EpisodicMemoryRepo::new(inner.clone());

    let bus = std::sync::Arc::new(DomainEventBus::new(64));

    // Heuristic extraction is spawned async by IngestionConsumer for
    // Extract-salience signals; ChatTurnCompleted is Accumulate so it just
    // exercises the observation-write path.
    let extraction: std::sync::Arc<dyn ExtractionHandler> =
        std::sync::Arc::new(HeuristicExtractionHandler);

    let ingestion: std::sync::Arc<dyn SignalConsumer> =
        std::sync::Arc::new(klyntbot::cognitive::consumers::IngestionConsumer::new(
            observation_repo.clone(),
            entity_repo,
            episodic_repo,
            Some(extraction),
        ));

    let _router = SignalRouter::start(
        std::sync::Arc::clone(&bus),
        vec![ingestion],
        app_core::init::ai_pipeline::translate,
    );

    bus.publish(DomainEvent::ChatTurnCompleted {
        session_key: "test-session".into(),
        user_message: Some("I prefer to work in dark mode in the mornings".into()),
    });

    // Poll for the observation row — the router fans out asynchronously.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let entries = observation_repo.load_all().await;
        if let Some(entry) = entries.get("ChatTurnCompleted") {
            assert!(
                !entry.observations.is_empty(),
                "ChatTurnCompleted entry should have at least one observation"
            );
            let obs = &entry.observations[0];
            assert!(
                obs.content.contains("dark mode"),
                "observation content lost the user message: {:?}",
                obs.content
            );
            assert_eq!(obs.source_event, "ChatTurnCompleted");
            return;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "Timed out waiting for IngestionConsumer to persist a \
                 ChatTurnCompleted observation; saw keys: {:?}",
                entries.keys().collect::<Vec<_>>()
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
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
        acc.push_event(&AiSignal {
            domain: RecallDomain::General,
            event_kind: "DistractionDetected",
            importance: 0.3,
            salience: SalienceVerdict::Accumulate,
            content: "reddit".into(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(DomainEvent::DistractionDetected {
                app: "reddit".into(),
                duration_secs: None,
                context: "browsing".into(),
            }),
            metrics: AiMetrics {
                app: Some("reddit".into()),
                ..AiMetrics::default()
            },
            coaching_signal: true,
            coaching_rule: None,
            metric_samples: vec![],
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
        acc.push_event(&AiSignal {
            domain: RecallDomain::Finance,
            event_kind: "BudgetAlert",
            importance: 0.9,
            salience: SalienceVerdict::Extract,
            content: "Budget alert: dining spent 280 of 300".into(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(DomainEvent::BudgetAlert {
                category: "dining".into(),
                spent: 280.0,
                limit: 300.0,
            }),
            metrics: AiMetrics {
                category: Some("dining".into()),
                amount: Some(280.0),
                ..AiMetrics::default()
            },
            coaching_signal: true,
            coaching_rule: Some("Review spending patterns when budget pressure is detected".into()),
            metric_samples: vec![],
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
    acc.push_event(&AiSignal {
        domain: RecallDomain::Finance,
        event_kind: "BudgetAlert",
        importance: 0.9,
        salience: SalienceVerdict::Extract,
        content: "Budget alert: dining spent 290 of 300".into(),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(DomainEvent::BudgetAlert {
            category: "dining".into(),
            spent: 290.0,
            limit: 300.0,
        }),
        metrics: AiMetrics {
            category: Some("dining".into()),
            amount: Some(290.0),
            ..AiMetrics::default()
        },
        coaching_signal: true,
        coaching_rule: Some("Review spending patterns when budget pressure is detected".into()),
        metric_samples: vec![],
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

// ── Reforge cycle end-to-end ───────────────────────────────────

struct MockReforgeHandler;

#[async_trait::async_trait]
impl klyntbot::cognitive::services::reforge::ReforgeHandler for MockReforgeHandler {
    async fn synthesize(
        &self,
        _input: &klyntbot::cognitive::services::reforge::types::SynthesizeInput,
    ) -> common::Result<klyntbot::cognitive::services::reforge::types::SynthesizeOutput> {
        Ok(
            klyntbot::cognitive::services::reforge::types::SynthesizeOutput {
                fact_updates: vec![klyntbot::cognitive::services::reforge::types::FactUpdate {
                    action: klyntbot::cognitive::services::reforge::types::FactAction::Add,
                    subject: "user".into(),
                    predicate: "prefers".into(),
                    object: "morning work".into(),
                    domain: "productivity".into(),
                    confidence: 0.8,
                    reason: "Consistent across sessions".into(),
                }],
                rule_updates: vec![],
                stale_facts: vec![],
                cross_session_patterns: vec![],
                extraction_quality_flag: None,
            },
        )
    }

    async fn review(
        &self,
        _input: &klyntbot::cognitive::services::reforge::types::ReviewInput,
    ) -> common::Result<klyntbot::cognitive::services::reforge::types::ReviewOutput> {
        Ok(
            klyntbot::cognitive::services::reforge::types::ReviewOutput {
                skill_edits: vec![],
                routing_insights: vec![],
                context_priority_suggestions: vec![],
                trial_suggestions: vec![],
            },
        )
    }

    async fn narrate(
        &self,
        _input: &klyntbot::cognitive::services::reforge::types::NarrateInput,
    ) -> common::Result<String> {
        Ok("Test Reforge narrative.".into())
    }
}

#[tokio::test]
async fn test_reforge_cycle_end_to_end() {
    use klyntbot::cognitive::repos::{EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo};
    use klyntbot::cognitive::services::reforge::service::run_reforge;
    use klyntbot::cognitive::services::reforge::skill_files::SkillFileManager;
    use klyntbot::cognitive::types::EpisodicMemory;
    use klyntbot::storage::repos::{ReforgeStateRepo, SkillVersionRepo};
    use klyntbot::storage::SessionMemoryRepo;
    use tempfile::TempDir;

    // 1. Setup: in-memory pool + cognitive migrations
    let (pool, inner) = cognitive_pool().await;

    // 2. Construct repos
    let reforge_state_repo = ReforgeStateRepo::new(inner.clone());
    let skill_version_repo = SkillVersionRepo::new(inner.clone());
    let session_memory_repo = SessionMemoryRepo::new(inner.clone());
    let fact_repo = SemanticFactRepo::new(inner.clone());
    let episodic_repo = EpisodicMemoryRepo::new(inner.clone());
    let rule_repo = ProceduralRuleRepo::new(inner.clone());

    // Verify initial reforge state has no runs yet
    let initial_state: klyntbot::storage::rows::ReforgeStateRow =
        reforge_state_repo.get().await.unwrap();
    assert_eq!(initial_state.run_count, 0);
    assert!(initial_state.last_run_at.is_none());

    // 3. Seed a session scratchpad so the collector finds data.
    //    session_memory has a FK to sessions — insert a session row first.
    sqlx::query("INSERT INTO sessions (key) VALUES ('reforge-test-session')")
        .execute(&inner)
        .await
        .unwrap();
    session_memory_repo
        .upsert(
            "reforge-test-session",
            "User mentioned they prefer working in the mornings.",
            5,
        )
        .await
        .unwrap();

    // 4. Seed an episodic memory so the collector also finds episodics.
    let episodic = EpisodicMemory {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "productivity".into(),
        content: "User completed a deep-work block before 10am.".into(),
        summary: Some("Morning deep work".into()),
        importance: 0.8,
        occurred_at: jiff::Timestamp::now().to_string(),
        recorded_at: jiff::Timestamp::now().to_string(),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: "system".into(),
        scope_id: None,
    };
    episodic_repo.insert(&episodic).await.unwrap();

    // 5. Temp skills directory with a test SKILL.md
    let skills_tmp = TempDir::new().expect("skills tempdir");
    let skill_mgr = SkillFileManager::new(skills_tmp.path().to_path_buf());
    skill_mgr
        .write_file(
            "test-skill",
            "SKILL.md",
            "---\nname: test-skill\nversion: 1\n---\n# Test Skill\nThis is a test skill.",
        )
        .expect("write test SKILL.md");

    // 6. Run the full Reforge cycle with the mock handler
    let handler = MockReforgeHandler;
    let result = run_reforge(
        &reforge_state_repo,
        &skill_version_repo,
        &session_memory_repo,
        &fact_repo,
        &episodic_repo,
        &rule_repo,
        &handler,
        &skill_mgr,
        None,
        None,
        None,
        None, // no autotuner bridge
        None, // no autotuner context
        None, // no feedback sources
        None, // no graph enrichment handler
        None, // no density repo
        None, // no entity repo
        None, // no snapshot repo
        None, // no community intelligence handler
        None, // no community repo
        None, // no co_activation repo for split
    )
    .await;

    // 7. Assert the cycle ran and produced expected results
    assert!(
        result.is_some(),
        "run_reforge should return Some when there is new data"
    );
    let r = result.unwrap();

    assert!(
        r.facts_added > 0,
        "Expected at least 1 fact added, got {}",
        r.facts_added
    );
    assert!(
        r.phase_errors.is_empty(),
        "Expected no phase errors, got: {:?}",
        r.phase_errors
    );
    assert_eq!(r.narrative, "Test Reforge narrative.");

    // 8. Verify reforge_state was updated
    let state: klyntbot::storage::rows::ReforgeStateRow = reforge_state_repo.get().await.unwrap();
    assert_eq!(state.run_count, 1, "run_count should be 1 after one cycle");
    assert!(
        state.last_run_at.is_some(),
        "last_run_at should be set after a run"
    );

    // Keep TempDir alive until end of test
    drop(skills_tmp);
    drop(pool);
}

// ---------------------------------------------------------------------------
// Mock AutotunerBridge for Phase 6 integration tests
// ---------------------------------------------------------------------------

struct MockAutotunerBridge {
    promoted: bool,
}

impl MockAutotunerBridge {
    fn new(promoted: bool) -> Self {
        Self { promoted }
    }
}

#[async_trait::async_trait]
impl klyntbot::cognitive::services::reforge::AutotunerBridge for MockAutotunerBridge {
    async fn run_evaluation(
        &self,
    ) -> common::Result<klyntbot::cognitive::services::reforge::Phase6Result> {
        Ok(klyntbot::cognitive::services::reforge::Phase6Result {
            promoted: self.promoted,
            promotion_summary: if self.promoted {
                Some("Mock trial promoted".into())
            } else {
                None
            },
            regression: false,
            evaluated_count: 2,
            failed_constraints: vec![],
        })
    }

    async fn create_trials(
        &self,
        suggestions: Vec<klyntbot::cognitive::services::reforge::ValidatedTrial>,
    ) -> common::Result<u32> {
        Ok(suggestions.len() as u32)
    }

    fn champion_params_map(&self) -> std::collections::HashMap<String, f64> {
        let mut map = std::collections::HashMap::new();
        map.insert("relevance_weight_semantic".to_string(), 0.30);
        map.insert("min_similarity".to_string(), 0.55);
        map.insert("vector_top_k".to_string(), 30.0);
        map
    }

    async fn active_trial_count(&self) -> u32 {
        2
    }

    async fn expire_stale_trials(&self) -> u32 {
        0
    }
}

#[tokio::test]
async fn test_reforge_phase6_with_autotuner_bridge() {
    use klyntbot::cognitive::repos::{EpisodicMemoryRepo, ProceduralRuleRepo, SemanticFactRepo};
    use klyntbot::cognitive::services::reforge::service::run_reforge;
    use klyntbot::cognitive::services::reforge::skill_files::SkillFileManager;
    use klyntbot::cognitive::types::EpisodicMemory;
    use klyntbot::storage::repos::{ReforgeStateRepo, SkillVersionRepo};
    use klyntbot::storage::SessionMemoryRepo;
    use tempfile::TempDir;

    // 1. Setup: in-memory pool + cognitive migrations
    let (pool, inner) = cognitive_pool().await;

    // 2. Construct repos
    let reforge_state_repo = ReforgeStateRepo::new(inner.clone());
    let skill_version_repo = SkillVersionRepo::new(inner.clone());
    let session_memory_repo = SessionMemoryRepo::new(inner.clone());
    let fact_repo = SemanticFactRepo::new(inner.clone());
    let episodic_repo = EpisodicMemoryRepo::new(inner.clone());
    let rule_repo = ProceduralRuleRepo::new(inner.clone());

    // 3. Seed data
    sqlx::query("INSERT INTO sessions (key) VALUES ('phase6-test-session')")
        .execute(&inner)
        .await
        .unwrap();
    session_memory_repo
        .upsert(
            "phase6-test-session",
            "User discussed routing optimization.",
            3,
        )
        .await
        .unwrap();

    let episodic = EpisodicMemory {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "system".into(),
        content: "Autotuner integration test episodic memory.".into(),
        summary: Some("Phase 6 test".into()),
        importance: 0.8,
        occurred_at: jiff::Timestamp::now().to_string(),
        recorded_at: jiff::Timestamp::now().to_string(),
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: "system".into(),
        scope_id: None,
    };
    episodic_repo.insert(&episodic).await.unwrap();

    // 4. Skills directory
    let skills_tmp = TempDir::new().expect("skills tempdir");
    let skill_mgr = SkillFileManager::new(skills_tmp.path().to_path_buf());
    skill_mgr
        .write_file(
            "test-skill",
            "SKILL.md",
            "---\nname: test-skill\nversion: 1\n---\n# Test Skill\nBody content.",
        )
        .expect("write test SKILL.md");

    // 5. Run with MockAutotunerBridge (promoted=true)
    let handler = MockReforgeHandler;
    let bridge = MockAutotunerBridge::new(true);

    let result = run_reforge(
        &reforge_state_repo,
        &skill_version_repo,
        &session_memory_repo,
        &fact_repo,
        &episodic_repo,
        &rule_repo,
        &handler,
        &skill_mgr,
        None,
        None,
        None,
        Some(&bridge),
        None, // no autotuner context for this test
        None, // no feedback sources
        None, // no graph enrichment handler
        None, // no density repo
        None, // no entity repo
        None, // no snapshot repo
        None, // no community intelligence handler
        None, // no community repo
        None, // no co_activation repo for split
    )
    .await;

    // 6. Assertions
    assert!(result.is_some(), "run_reforge should return Some with data");
    let r = result.unwrap();

    // Phase 6 ran: champion was promoted
    assert!(
        r.champion_promoted,
        "Expected champion_promoted=true from mock bridge"
    );
    assert!(
        !r.regression_detected,
        "Expected no regression from mock bridge"
    );
    // No trial suggestions from the MockReforgeHandler's review, so trials_created=0
    assert_eq!(
        r.trials_created, 0,
        "Expected 0 trials created (mock review returns empty trial_suggestions)"
    );
    assert!(
        r.phase_errors.is_empty(),
        "Expected no phase errors, got: {:?}",
        r.phase_errors
    );

    drop(skills_tmp);
    drop(pool);
}

#[tokio::test]
async fn test_reforge_with_feedback_signals() {
    use klyntbot::cognitive::repos::{
        CoActivationRepo, EpisodicMemoryRepo, EventLogRepo, ProceduralRuleRepo, SemanticFactRepo,
    };
    use klyntbot::cognitive::services::reforge::collector::FeedbackSources;
    use klyntbot::cognitive::services::reforge::service::run_reforge;
    use klyntbot::cognitive::services::reforge::skill_files::SkillFileManager;
    use klyntbot::cognitive::types::EpisodicMemory;
    use klyntbot::storage::repos::{ReforgeStateRepo, SkillVersionRepo};
    use klyntbot::storage::{ReforgeSuggestionRepo, SessionMemoryRepo};
    use tempfile::TempDir;

    // 1. Setup: in-memory pool + cognitive migrations
    let (pool, inner) = cognitive_pool().await;

    // 2. Construct repos
    let reforge_state_repo = ReforgeStateRepo::new(inner.clone());
    let skill_version_repo = SkillVersionRepo::new(inner.clone());
    let session_memory_repo = SessionMemoryRepo::new(inner.clone());
    let fact_repo = SemanticFactRepo::new(inner.clone());
    let episodic_repo = EpisodicMemoryRepo::new(inner.clone());
    let rule_repo = ProceduralRuleRepo::new(inner.clone());
    let outcome_repo = klyntbot::storage::OutcomeRepo::new(inner.clone());
    let event_log_repo = EventLogRepo::new(inner.clone());
    let co_activation_repo = CoActivationRepo::new(inner.clone());
    let suggestion_repo = ReforgeSuggestionRepo::new(inner.clone());

    // 3. Seed session data
    sqlx::query("INSERT INTO sessions (key) VALUES ('feedback-test-session')")
        .execute(&inner)
        .await
        .unwrap();
    session_memory_repo
        .upsert(
            "feedback-test-session",
            "User mentioned they use finance tracking daily.",
            3,
        )
        .await
        .unwrap();

    let episodic = EpisodicMemory {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "finance".into(),
        content: "User tracked multiple expenses today.".into(),
        summary: Some("Finance tracking activity".into()),
        importance: 0.7,
        occurred_at: jiff::Timestamp::now().to_string(),
        recorded_at: jiff::Timestamp::now().to_string(),
        stability: 3.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        scope_type: "system".into(),
        scope_id: None,
    };
    episodic_repo.insert(&episodic).await.unwrap();

    // 4. Seed a tool failure in learning_outcomes
    sqlx::query(
        "INSERT INTO learning_outcomes (id, session_key, tool_name, success, error_category, duration_ms, created_at)
         VALUES ('tf1', 'feedback-test-session', 'finance:tx_add', 0, 'InvalidParams', 100, datetime('now'))",
    )
    .execute(&inner)
    .await
    .unwrap();

    // 5. Seed a correction event in domain_event_log
    sqlx::query(
        "INSERT INTO domain_event_log (id, event_type, domain, salience, payload, timestamp)
         VALUES ('corr1', 'UserCorrectedAI', 'chat', 'extract',
                 '{\"original\":\"wrong\",\"correction\":\"right\",\"kind\":\"KeywordPrefix\",\"strength\":0.8,\"session_key\":\"test\",\"active_skill\":\"finance-management\"}',
                 datetime('now'))",
    )
    .execute(&inner)
    .await
    .unwrap();

    // 6. Create skill directory and file
    let skills_tmp = TempDir::new().unwrap();
    let skill_dir = skills_tmp.path().join("test-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: test-skill\nversion: 1\n---\n# Test Skill\nBody content.",
    )
    .expect("write test SKILL.md");
    let skill_mgr = SkillFileManager::new(skills_tmp.path().to_path_buf());

    // 7. Run Reforge with feedback sources
    let handler = MockReforgeHandler;
    let metric_repo = MetricRepo::new(inner.clone());
    let metric_registry = MetricRegistry::new();
    let feedback_sources = FeedbackSources {
        outcome_repo: Some(&outcome_repo),
        event_log_repo: Some(&event_log_repo),
        co_activation_repo: Some(&co_activation_repo),
        suggestion_repo: Some(&suggestion_repo),
        pool: Some(&inner),
        density_repo: None,
        metric_repo: Some(&metric_repo),
        metric_registry: Some(&metric_registry),
    };

    let result = run_reforge(
        &reforge_state_repo,
        &skill_version_repo,
        &session_memory_repo,
        &fact_repo,
        &episodic_repo,
        &rule_repo,
        &handler,
        &skill_mgr,
        None,
        None,
        None,
        None,
        None,
        Some(&feedback_sources),
        None, // no graph enrichment handler
        None, // no density repo
        None, // no entity repo
        None, // no snapshot repo
        None, // no community intelligence handler
        None, // no community repo
        None, // no co_activation repo for split
    )
    .await;

    // 8. Assert
    assert!(
        result.is_some(),
        "run_reforge should return Some when there is new data"
    );
    let r = result.unwrap();
    assert!(
        r.phase_errors.is_empty(),
        "Expected no phase errors, got: {:?}",
        r.phase_errors
    );

    drop(skills_tmp);
    drop(pool);
}

// ── Pending memory approve flow ────────────────────────────────

#[tokio::test]
async fn test_pending_memory_approve_flow() {
    use klyntbot::cognitive::repos::PendingMemoryRepo;

    // Setup: in-memory pool with cognitive migrations
    let (_, inner) = cognitive_pool().await;
    let fact_repo = SemanticFactRepo::new(inner.clone());
    let pending_repo = PendingMemoryRepo::new(inner.clone());
    pending_repo.migrate().await.unwrap();

    // Create a test fact with low confidence (pending candidate)
    let base = test_fact("pending-test-1", "might_prefer", "dark mode");
    let fact = SemanticFact {
        id: "pending-test-1".into(),
        confidence: 0.3,
        domain: "test".into(),
        subject: "user".into(),
        predicate: "might_prefer".into(),
        object: "dark mode".into(),
        ..base
    };

    // Insert to pending
    pending_repo.insert(&fact, "low_confidence").await.unwrap();

    // Verify it's pending
    let pending = pending_repo.list_pending(10).await;
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "pending-test-1");
    assert_eq!(pending[0].reason, "low_confidence");

    // Simulate approval: deserialize, upsert into semantic_facts, remove from pending
    let approved: SemanticFact = serde_json::from_str(&pending[0].fact_json).unwrap();
    fact_repo.upsert(&approved).await.unwrap();
    pending_repo.remove("pending-test-1").await.unwrap();

    // Verify it's now in semantic_facts
    let stored = fact_repo.get("pending-test-1").await.unwrap();
    assert!(
        stored.is_some(),
        "Approved fact should be in semantic_facts"
    );
    assert_eq!(stored.unwrap().object, "dark mode");

    // Verify pending queue is empty
    let remaining = pending_repo.list_pending(10).await;
    assert!(
        remaining.is_empty(),
        "Pending queue should be empty after approval"
    );
}
