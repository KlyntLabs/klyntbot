//! E2E test: Enrichment integration
//!
//! AC-E2E.5: The enrichment engine works end-to-end with the todo store and
//! learning feedback loop.
//!
//! Scenarios:
//!   - Urgent task is enriched with high-priority suggestion
//!   - Enriched task is stored in TodoStore
//!   - User acceptance is recorded via OutcomeRecorder
//!   - User override is recorded as rejected feedback
//!   - LearningAnalyzer reflects feedback acceptance/rejection
//!   - Enrichment is skipped when fields are already set
//!   - Disabled enrichment config returns None
//!
//! Run: `cargo nextest run --test phase4_e2e_enrichment_test`

use chrono::Utc;
use klyntbot::agent::enrichment::EnrichmentEngine;
use klyntbot::agent::learning::analyzer::LearningAnalyzer;
use klyntbot::agent::learning::{ExecutionMode, OutcomeRecorder, OutcomeStore};
use klyntbot::config::TodoEnrichmentConfig;
use klyntbot::tools::enrichment::EnrichmentHandler;
use klyntbot::tools::todo_store::TodoStore;
use klyntbot::tools::todo_types::{Todo, TodoFilter};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tools::learning_feedback::EnrichmentFeedbackEntry;
use tools::EnrichmentFeedbackHandler;

// ─────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────

fn make_urgent_todo() -> Todo {
    let mut t = Todo::default_instance();
    t.title = "URGENT: Fix critical auth bug in production".to_string();
    t
}

fn make_feature_todo() -> Todo {
    let mut t = Todo::default_instance();
    t.title = "Implement new dashboard feature".to_string();
    t
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.1 — Urgent task is enriched with high priority
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_urgent_task_enriched_with_priority_1() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let task = make_urgent_todo();
    let result = engine.enrich_task(&task).await.unwrap();

    assert!(result.is_some(), "urgent task must receive enrichment");
    let enrichment = result.unwrap();

    let priority_sug = enrichment
        .priority
        .expect("priority must be inferred from 'urgent'");
    assert_eq!(priority_sug.value, 1, "urgent task must get priority 1");
    assert!(
        priority_sug.confidence >= 0.80,
        "confidence must be >= 0.80 for 'urgent' keyword, got {}",
        priority_sug.confidence
    );
    assert!(
        !priority_sug.reasoning.is_empty(),
        "reasoning must be provided"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.2 — Feature task gets medium priority and duration
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_feature_task_enriched_with_medium_priority() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let task = make_feature_todo();
    let result = engine.enrich_task(&task).await.unwrap();

    assert!(result.is_some(), "feature task must receive enrichment");
    let enrichment = result.unwrap();

    if let Some(priority_sug) = enrichment.priority {
        assert!(
            priority_sug.value >= 2 && priority_sug.value <= 4,
            "feature task should get medium priority (2-4), got {}",
            priority_sug.value
        );
    }
    // Duration should be estimated for "implement"
    assert!(
        enrichment.estimated_minutes.is_some(),
        "feature task must get duration estimate"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.3 — Enriched task is stored in TodoStore
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_enriched_task_persists_in_todo_store() {
    let tmp = TempDir::new().unwrap();
    let mut todo_store = TodoStore::new(tmp.path().join("todos.jsonl"));
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let mut task = make_urgent_todo();

    // Apply enrichment
    if let Some(enrichment) = engine.enrich_task(&task).await.unwrap() {
        if let Some(ps) = enrichment.priority {
            task.priority = Some(ps.value);
        }
        if let Some(ds) = enrichment.estimated_minutes {
            task.estimated_minutes = Some(ds.value);
        }
    }

    // Persist
    let stored = todo_store.add(task).await.unwrap();
    let retrieved = todo_store.get(&stored.id).await.unwrap().unwrap();

    assert_eq!(
        retrieved.priority,
        Some(1),
        "enriched priority must persist"
    );
    assert!(
        retrieved.estimated_minutes.is_some(),
        "enriched duration must persist"
    );
    assert!(
        retrieved.title.contains("URGENT"),
        "task title must be preserved"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.4 — Accepted enrichment is recorded as feedback
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_accepted_enrichment_recorded_as_feedback() {
    let tmp = TempDir::new().unwrap();
    let outcome_store = Arc::new(RwLock::new(OutcomeStore::new(
        tmp.path().join("outcomes.jsonl"),
    )));
    let recorder = Arc::new(OutcomeRecorder::new(Arc::clone(&outcome_store)));

    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);
    let task = make_urgent_todo();

    // Enrich the task
    let enrichment = engine.enrich_task(&task).await.unwrap().unwrap();
    let priority_sug = enrichment.priority.unwrap();

    // User accepts the suggestion (keeps priority=1)
    let feedback = EnrichmentFeedbackEntry {
        task_id: task.id.to_string(),
        field: "priority".to_string(),
        suggested_value: priority_sug.value.to_string(),
        actual_value: None, // accepted as-is
        accepted: true,
        confidence: priority_sug.confidence,
        timestamp: Utc::now(),
    };
    recorder.record_feedback(feedback).await.unwrap();

    // Verify feedback is stored (keep guard alive while using borrowed slice)
    let mut s = outcome_store.write().await;
    let all_feedback = s.get_all_feedback().await.unwrap();

    assert_eq!(all_feedback.len(), 1, "one feedback entry must be recorded");
    assert!(
        all_feedback[0].accepted,
        "feedback must be recorded as accepted"
    );
    assert_eq!(all_feedback[0].field, "priority");
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.5 — Rejected enrichment (override) is recorded correctly
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_rejected_enrichment_override_recorded() {
    let tmp = TempDir::new().unwrap();
    let outcome_store = Arc::new(RwLock::new(OutcomeStore::new(
        tmp.path().join("outcomes.jsonl"),
    )));
    let recorder = Arc::new(OutcomeRecorder::new(Arc::clone(&outcome_store)));

    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);
    let task = make_urgent_todo();

    let enrichment = engine.enrich_task(&task).await.unwrap().unwrap();
    let priority_sug = enrichment.priority.unwrap();

    // User overrides to priority=3 (medium)
    let feedback = EnrichmentFeedbackEntry {
        task_id: task.id.to_string(),
        field: "priority".to_string(),
        suggested_value: priority_sug.value.to_string(),
        actual_value: Some("3".to_string()),
        accepted: false,
        confidence: priority_sug.confidence,
        timestamp: Utc::now(),
    };
    recorder.record_feedback(feedback).await.unwrap();

    let mut s = outcome_store.write().await;
    let all_feedback = s.get_all_feedback().await.unwrap();

    assert_eq!(all_feedback.len(), 1);
    assert!(
        !all_feedback[0].accepted,
        "override must be recorded as not accepted"
    );
    assert_eq!(
        all_feedback[0].actual_value.as_deref(),
        Some("3"),
        "user's actual value must be stored"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.6 — Multiple feedbacks reflected in LearningAnalyzer stats
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_feedback_loop_reflected_in_analysis() {
    let tmp = TempDir::new().unwrap();
    let outcome_store = Arc::new(RwLock::new(OutcomeStore::new(
        tmp.path().join("outcomes.jsonl"),
    )));
    let recorder = Arc::new(OutcomeRecorder::new(Arc::clone(&outcome_store)));

    // Record 10 feedback entries: 7 accepted, 3 overridden
    for i in 0..10 {
        let accepted = i < 7;
        let feedback = EnrichmentFeedbackEntry {
            task_id: format!("task-{}", i),
            field: "priority".to_string(),
            suggested_value: "1".to_string(),
            actual_value: if accepted {
                None
            } else {
                Some("3".to_string())
            },
            accepted,
            confidence: 0.85,
            timestamp: Utc::now(),
        };
        recorder.record_feedback(feedback).await.unwrap();
    }

    // Read feedback and run analysis (keep guard alive for borrowed slice)
    let mut s = outcome_store.write().await;
    let feedback_entries = s.get_all_feedback().await.unwrap();

    assert_eq!(feedback_entries.len(), 10);

    let analysis = LearningAnalyzer::analyze(&[], feedback_entries);
    let stats = &analysis.enrichment_stats;

    assert_eq!(stats.total_suggestions, 10);
    assert_eq!(stats.accepted_count, 7);
    assert_eq!(stats.overridden_count, 3);
    assert!(
        (stats.acceptance_rate - 0.7).abs() < 0.001,
        "acceptance_rate must be 0.7, got {}",
        stats.acceptance_rate
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.7 — Disabled enrichment returns None
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_disabled_enrichment_returns_none() {
    let config = TodoEnrichmentConfig {
        enabled: false,
        ..Default::default()
    };
    let engine = EnrichmentEngine::new(config);

    let task = make_urgent_todo();
    let result = engine.enrich_task(&task).await.unwrap();

    assert!(
        result.is_none(),
        "disabled enrichment must return None even for urgent tasks"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.8 — Already-set fields are not overridden by enrichment
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_enrichment_skips_already_set_fields() {
    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    let mut task = make_urgent_todo();
    task.priority = Some(3); // already set to medium
    task.estimated_minutes = Some(120); // already set

    let result = engine.enrich_task(&task).await.unwrap();

    if let Some(enrichment) = result {
        // Already-set fields must not be overridden
        assert!(
            enrichment.priority.is_none(),
            "enrichment must not override already-set priority"
        );
        assert!(
            enrichment.estimated_minutes.is_none(),
            "enrichment must not override already-set duration"
        );
    }
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.9 — Full enrichment pipeline: enrich → store → feedback → analyze
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_full_enrichment_pipeline() {
    let tmp = TempDir::new().unwrap();
    let mut todo_store = TodoStore::new(tmp.path().join("todos.jsonl"));
    let outcome_store = Arc::new(RwLock::new(OutcomeStore::new(
        tmp.path().join("outcomes.jsonl"),
    )));
    let recorder = Arc::new(OutcomeRecorder::new(Arc::clone(&outcome_store)));

    let config = TodoEnrichmentConfig::default();
    let engine = EnrichmentEngine::new(config);

    // Create 5 tasks, enrich them, store them, record mixed feedback
    let mut accepted_count = 0;
    let mut rejected_count = 0;

    for i in 0..5 {
        let mut task = Todo::default_instance();
        task.title = if i < 3 {
            format!("URGENT: Fix bug #{}", i)
        } else {
            format!("Implement feature #{}", i)
        };

        // Enrich
        if let Some(enrichment) = engine.enrich_task(&task).await.unwrap() {
            if let Some(ps) = &enrichment.priority {
                let accepted = i % 2 == 0; // alternate accept/reject
                let feedback = EnrichmentFeedbackEntry {
                    task_id: task.id.to_string(),
                    field: "priority".to_string(),
                    suggested_value: ps.value.to_string(),
                    actual_value: if accepted {
                        None
                    } else {
                        Some("3".to_string())
                    },
                    accepted,
                    confidence: ps.confidence,
                    timestamp: Utc::now(),
                };
                recorder.record_feedback(feedback).await.unwrap();
                if accepted {
                    task.priority = Some(ps.value);
                    accepted_count += 1;
                } else {
                    task.priority = Some(3);
                    rejected_count += 1;
                }
            }
        }

        todo_store.add(task).await.unwrap();
    }

    // Verify todos were stored
    let all_todos = todo_store.list(&TodoFilter::default()).await.unwrap();
    assert_eq!(all_todos.len(), 5, "all 5 todos must be stored");

    // Verify feedback was recorded (keep guard alive)
    let mut s = outcome_store.write().await;
    let feedback_entries = s.get_all_feedback().await.unwrap();
    let feedback_len = feedback_entries.len();
    assert_eq!(
        feedback_len,
        accepted_count + rejected_count,
        "all feedback must be recorded"
    );

    // Run analysis
    let analysis = LearningAnalyzer::analyze(&[], feedback_entries);
    assert_eq!(analysis.enrichment_stats.total_suggestions, feedback_len);
    assert_eq!(analysis.enrichment_stats.accepted_count, accepted_count);
    assert_eq!(analysis.enrichment_stats.overridden_count, rejected_count);
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.5.10 — Enrichment outcome recorded via OutcomeRecorder
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_enrichment_tool_outcome_recorded() {
    let tmp = TempDir::new().unwrap();
    let outcome_store = Arc::new(RwLock::new(OutcomeStore::new(
        tmp.path().join("outcomes.jsonl"),
    )));
    let recorder = OutcomeRecorder::new(Arc::clone(&outcome_store));

    // Simulate the agent recording a tool outcome for the "todo" tool
    // after enrichment was applied to a new task creation
    recorder
        .record_tool_outcome(
            "todo",
            true,
            None,
            75,
            None,
            ExecutionMode::Chat,
            "e2e:enrichment-test",
        )
        .await;

    let mut s = outcome_store.write().await;
    let outcomes = s.get_all_outcomes().await.unwrap();

    assert_eq!(outcomes.len(), 1);
    assert_eq!(outcomes[0].tool_name, "todo");
    assert!(outcomes[0].success);
    assert_eq!(outcomes[0].duration_ms, 75);
}
