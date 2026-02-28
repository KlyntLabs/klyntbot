//! Integration tests for the Learning System.
//!
//! Covers outcome recording, enrichment feedback, adaptive thresholds,
//! cold-start protection, privacy guarantees, confidence evaluation,
//! LearningTool routing, strategy persistence, and goal metrics.

use super::common::*;
use chrono::Utc;
use feature_todo::EnrichmentFeedbackHandler;
use klyntbot::agent::confidence::prompt::confidence_prompt;
use klyntbot::agent::confidence::ConfidenceEvaluator;
use klyntbot::agent::learning::adaptive::AdaptiveThresholds;
use klyntbot::agent::learning::analyzer::LearningAnalyzer;
use klyntbot::agent::learning::recorder::OutcomeStore;
use klyntbot::agent::learning::recorder::OutcomeRecorder;
use klyntbot::agent::learning::types::{AnalysisResult, EnrichmentStats};
use klyntbot::agent::learning::{ExecutionMode, OutcomeRecord};
use klyntbot::agent::learning_handler::LearningHandlerImpl;
use klyntbot::agent::LearningService;
use klyntbot::{AgentLoop, MessageBus};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tools::learning_tool::{LearningHandler, LearningStatus, LearningTool, ThresholdEntry};
use tools::Tool;

// ── Helpers ────────────────────────────────────────────────────

fn make_feedback(
    task_id: &str,
    accepted: bool,
) -> feature_todo::enrichment::EnrichmentFeedbackEntry {
    feature_todo::enrichment::EnrichmentFeedbackEntry {
        task_id: task_id.to_string(),
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
    }
}

fn make_feedback_with_field(
    task_id: &str,
    accepted: bool,
    field: &str,
) -> feature_todo::enrichment::EnrichmentFeedbackEntry {
    feature_todo::enrichment::EnrichmentFeedbackEntry {
        task_id: task_id.to_string(),
        field: field.to_string(),
        suggested_value: "1".to_string(),
        actual_value: if accepted {
            None
        } else {
            Some("3".to_string())
        },
        accepted,
        confidence: 0.85,
        timestamp: Utc::now(),
    }
}

fn make_outcome(id: &str, tool: &str, success: bool, confidence: f32) -> OutcomeRecord {
    OutcomeRecord {
        id: id.to_string(),
        session_key: "e2e:feedback".to_string(),
        tool_name: tool.to_string(),
        success,
        error_category: if success {
            None
        } else {
            Some("validation".to_string())
        },
        duration_ms: 50,
        confidence_score: Some(confidence),
        confidence_dimensions: None,
        execution_mode: ExecutionMode::Chat,
        created_at: Utc::now(),
    }
}

fn make_routing_ctx() -> tools::RoutingContext {
    test_routing_ctx()
}

async fn make_handler() -> (LearningHandlerImpl, storage::StrategyRepo) {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let strategy_repo = storage::StrategyRepo::new(pool.inner().clone());
    let adaptive = Arc::new(RwLock::new(AdaptiveThresholds::new_in_memory(
        0.7, 0.4, 0.9, 50,
    )));
    let handler = LearningHandlerImpl::new(strategy_repo.clone(), adaptive);
    (handler, strategy_repo)
}

// ── Outcome Recording ─────────────────────────────────────────

#[tokio::test]
async fn outcome_records_on_successful_tool_call() {
    let store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let recorder = OutcomeRecorder::new(Arc::clone(&store));

    recorder
        .record_tool_outcome(
            "todo",
            true,
            None,
            42,
            None,
            ExecutionMode::Chat,
            "cli:test-session",
        )
        .await;

    let guard = store.read().await;
    let outcomes = guard.get_all_outcomes().await.unwrap();

    assert!(
        !outcomes.is_empty(),
        "at least one outcome must be recorded"
    );
    let rec = outcomes.iter().find(|o| o.tool_name == "todo").unwrap();
    assert!(rec.success, "outcome must record success=true");
    assert_eq!(rec.duration_ms, 42);
}

#[tokio::test]
async fn outcome_records_on_failed_tool_call() {
    let store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let recorder = OutcomeRecorder::new(Arc::clone(&store));

    recorder
        .record_tool_outcome(
            "shell",
            false,
            Some("validation"),
            15,
            None,
            ExecutionMode::Chat,
            "cli:test-session",
        )
        .await;

    let guard = store.read().await;
    let outcomes = guard.get_all_outcomes().await.unwrap();

    assert!(!outcomes.is_empty(), "failed tool call must be recorded");
    let rec = outcomes.iter().find(|o| o.tool_name == "shell").unwrap();
    assert!(!rec.success, "outcome must record success=false");
    assert!(
        rec.error_category.is_some(),
        "failed outcome must have an error_category"
    );
}

#[tokio::test]
async fn outcome_captures_duration_ms() {
    let store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let recorder = OutcomeRecorder::new(Arc::clone(&store));

    recorder
        .record_tool_outcome("todo", true, None, 123, None, ExecutionMode::Chat, "cli:s")
        .await;

    let guard = store.read().await;
    let outcomes = guard.get_all_outcomes().await.unwrap();

    let json = serde_json::to_string(&outcomes[0]).unwrap();
    assert!(
        json.contains("duration_ms"),
        "duration_ms field must be present in serialized outcome"
    );
    assert_eq!(outcomes[0].duration_ms, 123);
}

#[tokio::test]
async fn plan_step_outcomes_tagged_with_execution_mode() {
    let mode = ExecutionMode::PlanStep {
        plan_id: "plan-test-001".to_string(),
        step_index: 2,
    };
    let json = serde_json::to_string(&mode).unwrap();
    let back: ExecutionMode = serde_json::from_str(&json).unwrap();

    assert!(
        matches!(back, ExecutionMode::PlanStep { step_index: 2, .. }),
        "PlanStep execution mode must round-trip correctly"
    );

    let chat = ExecutionMode::Chat;
    let json2 = serde_json::to_string(&chat).unwrap();
    let back2: ExecutionMode = serde_json::from_str(&json2).unwrap();
    assert!(matches!(back2, ExecutionMode::Chat));
}

#[tokio::test]
async fn outcomes_persist_and_are_analyzable() {
    let store = OutcomeStore::new_in_memory();
    for i in 0..30 {
        store
            .record(make_outcome(&format!("o{}", i), "todo", i % 4 != 0, 0.75))
            .await
            .unwrap();
    }
    let outcomes = store.get_all_outcomes().await.unwrap();
    assert_eq!(outcomes.len(), 30, "all 30 outcomes must be present");
    let analysis = LearningAnalyzer::analyze(&outcomes, &[]);
    assert_eq!(analysis.total_outcomes, 30);
    assert!(analysis.per_tool_stats.contains_key("todo"));
}

// ── Learning Analysis ─────────────────────────────────────────

#[tokio::test]
async fn analyzer_produces_five_confidence_bands() {
    let confidences = [0.1_f32, 0.4, 0.6, 0.75, 0.9];
    let outcomes: Vec<OutcomeRecord> = confidences
        .iter()
        .enumerate()
        .map(|(i, &c)| OutcomeRecord {
            id: format!("r{}", i),
            session_key: "test:hash".to_string(),
            tool_name: "todo".to_string(),
            success: true,
            error_category: None,
            duration_ms: 10,
            confidence_score: Some(c),
            confidence_dimensions: None,
            execution_mode: ExecutionMode::Chat,
            created_at: Utc::now(),
        })
        .collect();

    let result = LearningAnalyzer::analyze(&outcomes, &[]);
    let stats = result
        .per_tool_stats
        .get("todo")
        .expect("todo stats must exist");

    assert_eq!(
        stats.success_rate_by_confidence_band.len(),
        5,
        "must produce exactly 5 confidence bands"
    );

    let bands = &stats.success_rate_by_confidence_band;
    assert!(
        (bands[0].lower - 0.0).abs() < f32::EPSILON && (bands[0].upper - 0.3).abs() < f32::EPSILON
    );
    assert!(
        (bands[1].lower - 0.3).abs() < f32::EPSILON && (bands[1].upper - 0.5).abs() < f32::EPSILON
    );
    assert!(
        (bands[2].lower - 0.5).abs() < f32::EPSILON && (bands[2].upper - 0.7).abs() < f32::EPSILON
    );
    assert!(
        (bands[3].lower - 0.7).abs() < f32::EPSILON && (bands[3].upper - 0.85).abs() < f32::EPSILON
    );
    assert!(
        (bands[4].lower - 0.85).abs() < f32::EPSILON
            && (bands[4].upper - 1.0).abs() < f32::EPSILON
    );
}

#[tokio::test]
async fn analyzer_suggested_threshold_within_bounds() {
    let test_cases: Vec<(&str, bool, f32)> = vec![
        ("all-success-high", true, 0.9),
        ("all-fail-low", false, 0.1),
        ("mixed-mid", true, 0.5),
    ];

    for (desc, success, confidence) in &test_cases {
        let outcomes: Vec<OutcomeRecord> = (0..60)
            .map(|i| OutcomeRecord {
                id: format!("{}-{}", desc, i),
                session_key: "test:hash".to_string(),
                tool_name: "todo".to_string(),
                success: *success,
                error_category: None,
                duration_ms: 10,
                confidence_score: Some(*confidence),
                confidence_dimensions: None,
                execution_mode: ExecutionMode::Chat,
                created_at: Utc::now(),
            })
            .collect();

        let result = LearningAnalyzer::analyze(&outcomes, &[]);
        assert!(
            result.suggested_threshold >= 0.4 && result.suggested_threshold <= 0.9,
            "suggested_threshold out of [0.4, 0.9] for '{}': {}",
            desc,
            result.suggested_threshold
        );
    }
}

#[tokio::test]
async fn analyzer_handles_mixed_success_distribution() {
    let outcomes: Vec<OutcomeRecord> = (0..60)
        .map(|i| {
            let success = i < 45;
            let confidence = if success { 0.8 } else { 0.3 };
            make_outcome(&format!("o{}", i), "todo", success, confidence)
        })
        .collect();

    let result = LearningAnalyzer::analyze(&outcomes, &[]);

    assert_eq!(result.total_outcomes, 60, "must count all 60 outcomes");
    assert!(
        result.per_tool_stats.contains_key("todo"),
        "must have stats for 'todo'"
    );

    let todo_stats = &result.per_tool_stats["todo"];
    assert_eq!(todo_stats.total_calls, 60);
    assert_eq!(todo_stats.success_count, 45);

    assert!(
        result.suggested_threshold >= 0.4 && result.suggested_threshold <= 0.9,
        "suggested_threshold must be in [0.4, 0.9], got {}",
        result.suggested_threshold
    );
}

#[tokio::test]
async fn analyzer_multi_tool_per_tool_stats() {
    let outcomes: Vec<OutcomeRecord> = vec![
        make_outcome("t0", "todo", true, 0.8),
        make_outcome("t1", "todo", true, 0.8),
        make_outcome("t2", "todo", true, 0.8),
        make_outcome("t3", "todo", false, 0.3),
        make_outcome("s0", "shell", true, 0.75),
        make_outcome("s1", "shell", false, 0.2),
        make_outcome("w0", "web_search", true, 0.9),
        make_outcome("w1", "web_search", true, 0.9),
        make_outcome("w2", "web_search", true, 0.9),
    ];

    let result = LearningAnalyzer::analyze(&outcomes, &[]);

    assert_eq!(result.total_outcomes, 9);
    assert_eq!(result.per_tool_stats["todo"].total_calls, 4);
    assert_eq!(result.per_tool_stats["todo"].success_count, 3);
    assert_eq!(result.per_tool_stats["shell"].total_calls, 2);
    assert_eq!(result.per_tool_stats["shell"].success_count, 1);
    assert_eq!(result.per_tool_stats["web_search"].total_calls, 3);
    assert_eq!(result.per_tool_stats["web_search"].success_count, 3);
}

#[tokio::test]
async fn analyzer_now_returns_immediate_result() {
    let store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let adaptive = Arc::new(RwLock::new(AdaptiveThresholds::new_in_memory(
        0.7, 0.4, 0.9, 50,
    )));

    let service = LearningService::new(store, adaptive, None, Duration::from_secs(9999));
    let result = service.analyze_now().await;
    assert!(
        result.is_ok(),
        "analyze_now must return Ok: {:?}",
        result.err()
    );
}

#[tokio::test]
async fn combined_outcomes_and_feedback_analysis() {
    let outcomes: Vec<OutcomeRecord> = (0..50)
        .map(|i| make_outcome(&format!("o{}", i), "todo", i % 5 != 0, 0.7))
        .collect();

    let feedback: Vec<feature_todo::enrichment::EnrichmentFeedbackEntry> = (0..10)
        .map(|i| make_feedback_with_field(&format!("f{}", i), i < 8, "priority"))
        .collect();

    let result = LearningAnalyzer::analyze(&outcomes, &feedback);

    assert_eq!(result.total_outcomes, 50);
    assert_eq!(result.per_tool_stats["todo"].total_calls, 50);

    assert_eq!(result.enrichment_stats.total_suggestions, 10);
    assert_eq!(result.enrichment_stats.accepted_count, 8);
    assert_eq!(result.enrichment_stats.overridden_count, 2);
    assert!(
        (result.enrichment_stats.acceptance_rate - 0.8).abs() < 0.001,
        "acceptance rate must be 0.8"
    );
}

// ── Adaptive Thresholds ───────────────────────────────────────

#[tokio::test]
async fn threshold_never_below_min_bound() {
    let mut adaptive = AdaptiveThresholds::new_in_memory(0.42, 0.4, 0.9, 50);

    let outcomes: Vec<OutcomeRecord> = (0..60)
        .map(|i| OutcomeRecord {
            id: format!("r{}", i),
            session_key: "test:abc".to_string(),
            tool_name: "todo".to_string(),
            success: true,
            error_category: None,
            duration_ms: 10,
            confidence_score: Some(0.05),
            confidence_dimensions: None,
            execution_mode: ExecutionMode::Chat,
            created_at: Utc::now(),
        })
        .collect();

    let analysis = LearningAnalyzer::analyze(&outcomes, &[]);
    if let Some(new_threshold) = adaptive.apply_analysis(&analysis) {
        assert!(
            new_threshold >= 0.4,
            "threshold must stay >= 0.4, got {}",
            new_threshold
        );
    }
    assert!(
        adaptive.current_threshold() >= 0.4,
        "current threshold must be >= 0.4, got {}",
        adaptive.current_threshold()
    );
}

#[tokio::test]
async fn threshold_never_above_max_bound() {
    let mut adaptive = AdaptiveThresholds::new_in_memory(0.88, 0.4, 0.9, 50);

    let outcomes: Vec<OutcomeRecord> = (0..60)
        .map(|i| OutcomeRecord {
            id: format!("r{}", i),
            session_key: "test:abc".to_string(),
            tool_name: "todo".to_string(),
            success: i >= 55,
            error_category: if i < 55 {
                Some("validation".to_string())
            } else {
                None
            },
            duration_ms: 10,
            confidence_score: Some(i as f32 / 60.0),
            confidence_dimensions: None,
            execution_mode: ExecutionMode::Chat,
            created_at: Utc::now(),
        })
        .collect();

    let analysis = LearningAnalyzer::analyze(&outcomes, &[]);
    if let Some(new_threshold) = adaptive.apply_analysis(&analysis) {
        assert!(
            new_threshold <= 0.9,
            "threshold must stay <= 0.9, got {}",
            new_threshold
        );
    }
    assert!(
        adaptive.current_threshold() <= 0.9,
        "current threshold must be <= 0.9, got {}",
        adaptive.current_threshold()
    );
}

#[tokio::test]
async fn threshold_step_limit_enforced() {
    let mut adaptive = AdaptiveThresholds::new_in_memory(0.7, 0.4, 0.9, 50);
    let old_threshold = adaptive.current_threshold();

    let analysis = AnalysisResult {
        computed_at: Utc::now(),
        total_outcomes: 100,
        per_tool_stats: Default::default(),
        suggested_threshold: 0.1,
        threshold_confidence: 0.99,
        enrichment_stats: EnrichmentStats::default(),
    };

    if let Some(new_threshold) = adaptive.apply_analysis(&analysis) {
        let change = (new_threshold - old_threshold).abs();
        assert!(
            change <= 0.05 + f32::EPSILON,
            "single step must not exceed 0.05; change was {:.4}",
            change
        );
    }
}

#[tokio::test]
async fn cold_start_blocks_adaptation_below_minimum() {
    let mut adaptive = AdaptiveThresholds::new_in_memory(0.7, 0.4, 0.9, 50);

    let analysis = AnalysisResult {
        computed_at: Utc::now(),
        total_outcomes: 49,
        per_tool_stats: Default::default(),
        suggested_threshold: 0.5,
        threshold_confidence: 0.9,
        enrichment_stats: EnrichmentStats::default(),
    };

    let result = adaptive.apply_analysis(&analysis);
    assert!(result.is_none(), "must not adapt with < 50 outcomes");
    assert!(
        (adaptive.current_threshold() - 0.7).abs() < f32::EPSILON,
        "threshold must remain unchanged"
    );
}

#[tokio::test]
async fn adaptation_triggers_at_minimum_outcomes() {
    let mut adaptive = AdaptiveThresholds::new_in_memory(0.7, 0.4, 0.9, 50);

    let analysis = AnalysisResult {
        computed_at: Utc::now(),
        total_outcomes: 50,
        per_tool_stats: Default::default(),
        suggested_threshold: 0.5,
        threshold_confidence: 0.9,
        enrichment_stats: EnrichmentStats::default(),
    };

    let result = adaptive.apply_analysis(&analysis);
    assert!(
        result.is_some() || (adaptive.current_threshold() - 0.7).abs() < f32::EPSILON,
        "at 50 outcomes, adaptation is permitted (not blocked by cold-start)"
    );
    assert!(
        result.is_some(),
        "50 outcomes with suggested=0.5 should produce a threshold change"
    );
}

#[tokio::test]
async fn threshold_history_tracked_after_change() {
    let mut adaptive = AdaptiveThresholds::new_in_memory(0.7, 0.4, 0.9, 50);

    let analysis = AnalysisResult {
        computed_at: Utc::now(),
        total_outcomes: 100,
        per_tool_stats: Default::default(),
        suggested_threshold: 0.5,
        threshold_confidence: 0.9,
        enrichment_stats: EnrichmentStats::default(),
    };
    let result = adaptive.apply_analysis(&analysis);
    assert!(result.is_some(), "threshold should change");

    let saved_threshold = adaptive.current_threshold();
    assert!(
        (saved_threshold - 0.7).abs() > f32::EPSILON,
        "threshold must have changed from initial 0.7"
    );
    assert!(
        !adaptive.state().threshold_history.is_empty(),
        "threshold history must record the change"
    );
    let change = &adaptive.state().threshold_history[0];
    assert!((change.from - 0.7).abs() < f32::EPSILON);
    assert!((change.to - saved_threshold).abs() < f32::EPSILON);
}

// ── Enrichment Feedback ───────────────────────────────────────

#[tokio::test]
async fn enrichment_feedback_records_override() {
    use feature_todo::EnrichmentFeedbackEntry;

    let store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let recorder = Arc::new(OutcomeRecorder::new(Arc::clone(&store)));

    let feedback = EnrichmentFeedbackEntry {
        task_id: "todo-001".to_string(),
        field: "priority".to_string(),
        suggested_value: "1".to_string(),
        actual_value: Some("3".to_string()),
        accepted: false,
        confidence: 0.85,
        timestamp: Utc::now(),
    };
    recorder.record_feedback(feedback).await.unwrap();

    let guard = store.read().await;
    let all_fb = guard.get_all_feedback().await.unwrap();
    assert_eq!(all_fb.len(), 1, "one feedback entry must be stored");
    assert_eq!(all_fb[0].task_id, "todo-001");
    assert!(
        !all_fb[0].accepted,
        "override must be recorded as not accepted"
    );
    assert_eq!(all_fb[0].actual_value.as_deref(), Some("3"));
}

#[tokio::test]
async fn enrichment_feedback_records_acceptance() {
    use feature_todo::EnrichmentFeedbackEntry;

    let store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let recorder = Arc::new(OutcomeRecorder::new(Arc::clone(&store)));

    let feedback = EnrichmentFeedbackEntry {
        task_id: "todo-002".to_string(),
        field: "estimated_minutes".to_string(),
        suggested_value: "30".to_string(),
        actual_value: None,
        accepted: true,
        confidence: 0.90,
        timestamp: Utc::now(),
    };
    recorder.record_feedback(feedback).await.unwrap();

    let guard = store.read().await;
    let all_fb = guard.get_all_feedback().await.unwrap();
    assert_eq!(all_fb.len(), 1);
    assert!(
        all_fb[0].accepted,
        "accepted suggestion must be recorded as accepted"
    );
    assert!(all_fb[0].actual_value.is_none());
}

#[tokio::test]
async fn enrichment_acceptance_rate_computed_correctly() {
    let feedback: Vec<_> = (0..10)
        .map(|i| make_feedback(&format!("task-{}", i), i < 7))
        .collect();

    let result = LearningAnalyzer::analyze(&[], &feedback);
    let stats = &result.enrichment_stats;

    assert_eq!(stats.total_suggestions, 10);
    assert_eq!(stats.accepted_count, 7);
    assert_eq!(stats.overridden_count, 3);
    assert!(
        (stats.acceptance_rate - 0.7).abs() < 0.001,
        "acceptance_rate must be 0.7, got {}",
        stats.acceptance_rate
    );
}

// ── Privacy ───────────────────────────────────────────────────

#[tokio::test]
async fn privacy_no_args_or_messages_in_outcomes() {
    let record = OutcomeRecord {
        id: "privacy-test".to_string(),
        session_key: "test:hash".to_string(),
        tool_name: "shell".to_string(),
        success: true,
        error_category: None,
        duration_ms: 100,
        confidence_score: None,
        confidence_dimensions: None,
        execution_mode: ExecutionMode::Chat,
        created_at: Utc::now(),
    };
    let json = serde_json::to_string(&record).unwrap();

    assert!(
        !json.contains("\"args\""),
        "args must not appear in serialized outcome"
    );
    assert!(
        !json.contains("\"arguments\""),
        "arguments must not appear in serialized outcome"
    );
    assert!(
        !json.contains("\"user_message\""),
        "user_message must not appear in serialized outcome"
    );
    let obj: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(obj.get("tool_args").is_none());
    assert!(obj.get("user_message").is_none());
}

#[tokio::test]
async fn privacy_session_key_is_hashed() {
    let store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let recorder = OutcomeRecorder::new(Arc::clone(&store));

    recorder
        .record_tool_outcome(
            "todo",
            true,
            None,
            42,
            None,
            ExecutionMode::Chat,
            "telegram:user99999",
        )
        .await;

    let guard = store.read().await;
    let outcomes = guard.get_all_outcomes().await.unwrap();
    assert_eq!(outcomes.len(), 1);

    let stored_key = &outcomes[0].session_key;
    assert!(
        stored_key.starts_with("telegram:"),
        "channel prefix must be preserved: {}",
        stored_key
    );
    assert!(
        !stored_key.contains("user99999"),
        "raw user suffix must not appear in stored key: {}",
        stored_key
    );
}

// ── Confidence Evaluation ─────────────────────────────────────

#[tokio::test]
async fn confidence_evaluator_uses_dynamic_threshold() {
    let evaluator = ConfidenceEvaluator::new(0.65);
    assert!(
        (evaluator.threshold() - 0.65).abs() < f32::EPSILON,
        "initial threshold must be 0.65"
    );

    let handle = evaluator.threshold_handle();
    handle.store(0.72_f32.to_bits(), Ordering::SeqCst);

    assert!(
        (evaluator.threshold() - 0.72).abs() < f32::EPSILON,
        "evaluator must reflect external update via handle, expected 0.72 got {}",
        evaluator.threshold()
    );
    assert!(
        (evaluator.threshold() - 0.7).abs() > f32::EPSILON,
        "threshold must NOT be hardcoded to 0.7"
    );
}

#[tokio::test]
async fn confidence_prompt_has_no_hardcoded_threshold() {
    let prompt_50 = confidence_prompt(0.5);
    assert!(
        prompt_50.contains("0.50") || prompt_50.contains("0.5"),
        "prompt must contain the given threshold 0.50, got: {}",
        &prompt_50[..100.min(prompt_50.len())]
    );

    let lines_with_threshold: Vec<_> = prompt_50
        .lines()
        .filter(|l| l.contains("score") && l.contains(':'))
        .collect();
    for line in lines_with_threshold {
        assert!(
            !line.contains("0.70") && !line.contains("0.7:"),
            "threshold guideline line must not hardcode 0.7: '{}'",
            line
        );
    }

    let prompt_80 = confidence_prompt(0.8);
    assert!(
        prompt_80.contains("0.80") || prompt_80.contains("0.8"),
        "prompt must contain threshold 0.80 for dynamic variant"
    );
}

// ── Learning Service Lifecycle ────────────────────────────────

#[tokio::test]
async fn learning_disabled_no_recording() {
    let temp_dir = TempDir::new().unwrap();
    let provider = Arc::new(MockProvider::new("Hello!"));
    let mut config = test_config(&temp_dir);
    config.learning.enabled = false;

    let bus = Arc::new(MessageBus::new(10));
    let agent = AgentLoop::builder()
        .with_bus(bus)
        .with_provider(provider.clone())
        .with_config(config)
        .build()
        .await
        .unwrap();

    let response = agent
        .process_direct("Hi".to_string(), "test:sess".to_string())
        .await
        .unwrap();
    assert_eq!(response, "Hello!");
}

#[tokio::test]
async fn learning_service_shutdown_is_clean() {
    let store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let adaptive = Arc::new(RwLock::new(AdaptiveThresholds::new_in_memory(
        0.7, 0.4, 0.9, 50,
    )));

    let mut service = LearningService::new(store, adaptive, None, Duration::from_secs(3600));
    service.start();
    service.stop().await;
}

// ── Strategy Persistence ──────────────────────────────────────

#[tokio::test]
async fn strategy_record_round_trips_with_satisfaction_backfill() {
    let pool = klyntbot::StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::Repos::from_pool(&pool);

    let record = klyntbot::storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: Utc::now(),
        request_id: "integration-test-1".to_string(),
        predicted_strategy: "DirectResponse".to_string(),
        actual_strategy: "ToolAssisted".to_string(),
        escalation_count: 1,
        iterations_used: 3,
        max_iterations: 5,
        success: true,
        user_satisfaction: None,
        response_time_ms: 1500,
        chat_id: Some("tg:user123".to_string()),
        tool_name: None,
        tool_success: None,
        tool_duration_ms: None,
    };
    repos.strategies.create(&record).await.unwrap();

    let since = Utc::now() - chrono::Duration::minutes(5);
    let updated = repos
        .strategies
        .set_satisfaction_for_chat("tg:user123", since, 1.0)
        .await
        .unwrap();
    assert!(updated, "Expected satisfaction to be set");

    let fetched = repos.strategies.get(record.id).await.unwrap();
    assert_eq!(fetched.user_satisfaction, Some(1.0));
    assert_eq!(fetched.chat_id, Some("tg:user123".to_string()));
    assert_eq!(fetched.predicted_strategy, "DirectResponse");
    assert_eq!(fetched.actual_strategy, "ToolAssisted");
    assert_eq!(fetched.iterations_used, 3);
}

#[tokio::test]
async fn goal_plan_completion_metrics_end_to_end() {
    let pool = klyntbot::StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::Repos::from_pool(&pool);

    let goal_id = uuid::Uuid::new_v4();
    let row = klyntbot::storage::GoalRow {
        id: goal_id,
        title: "Ship v1.0".to_string(),
        description: "Release the first version".to_string(),
        status: "Active".to_string(),
        priority: 1,
        target_date: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        plans_completed: 0,
        plans_failed: 0,
        avg_duration_ms: None,
        last_plan_at: None,
    };
    repos.goals.create(&row).await.unwrap();

    repos
        .goals
        .increment_completed(goal_id, 60_000)
        .await
        .unwrap();
    repos
        .goals
        .increment_completed(goal_id, 120_000)
        .await
        .unwrap();
    repos.goals.increment_failed(goal_id).await.unwrap();

    let g = repos.goals.get(goal_id).await.unwrap();
    assert_eq!(g.plans_completed, 2);
    assert_eq!(g.plans_failed, 1);
    assert_eq!(g.avg_duration_ms, Some(90_000));
    assert!(g.last_plan_at.is_some(), "Expected last_plan_at to be set");
    assert_eq!(g.title, "Ship v1.0");
}

#[tokio::test]
async fn handler_reads_strategy_records_with_tool_stats() {
    let pool = klyntbot::StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::Repos::from_pool(&pool);

    let row = klyntbot::storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: Utc::now(),
        request_id: "req-learn".to_string(),
        predicted_strategy: "ToolAssisted".to_string(),
        actual_strategy: "ToolAssisted".to_string(),
        escalation_count: 0,
        iterations_used: 2,
        max_iterations: 5,
        success: true,
        user_satisfaction: Some(1.0),
        response_time_ms: 1500,
        chat_id: Some("tg:test".to_string()),
        tool_name: Some("todo".to_string()),
        tool_success: Some(true),
        tool_duration_ms: Some(45),
    };
    repos.strategies.create(&row).await.unwrap();

    let count = repos.strategies.count_all().await.unwrap();
    assert_eq!(count, 1);

    let stats = repos.strategies.get_overall_stats().await.unwrap();
    assert_eq!(stats.total_records, 1);
    assert!((stats.accuracy - 1.0).abs() < 0.01);

    let tool_stats = repos.strategies.get_tool_stats().await.unwrap();
    assert_eq!(tool_stats.len(), 1);
    assert_eq!(tool_stats[0].tool_name, "todo");
    assert_eq!(tool_stats[0].success_count, 1);
}

#[tokio::test]
async fn satisfaction_no_match_returns_false() {
    let pool = klyntbot::StoragePool::connect_in_memory().await.unwrap();
    let repos = klyntbot::Repos::from_pool(&pool);

    let since = Utc::now() - chrono::Duration::minutes(5);
    let updated = repos
        .strategies
        .set_satisfaction_for_chat("nonexistent:chat", since, 0.0)
        .await
        .unwrap();
    assert!(!updated, "Expected no record to be updated");
}

// ── LearningHandlerImpl ──────────────────────────────────────

#[tokio::test]
async fn handler_impl_returns_none_for_empty_store() {
    let (handler, _repo) = make_handler().await;

    let status = handler.get_status().await.unwrap();
    assert!(
        status.is_none(),
        "Expected None when no strategy records have been recorded"
    );
}

#[tokio::test]
async fn handler_impl_returns_status_with_outcomes() {
    let (handler, repo) = make_handler().await;

    let row = storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: Utc::now(),
        request_id: "req-1".to_string(),
        predicted_strategy: "DirectResponse".to_string(),
        actual_strategy: "DirectResponse".to_string(),
        escalation_count: 0,
        iterations_used: 1,
        max_iterations: 1,
        success: true,
        user_satisfaction: Some(1.0),
        response_time_ms: 50,
        chat_id: None,
        tool_name: Some("todo".to_string()),
        tool_success: Some(true),
        tool_duration_ms: Some(25),
    };
    repo.create(&row).await.unwrap();

    let status = handler.get_status().await.unwrap();
    assert!(
        status.is_some(),
        "Expected Some when strategy records exist"
    );

    let s = status.unwrap();
    assert_eq!(s.total_strategy_records, 1);
    assert!(
        s.per_tool.contains_key("todo"),
        "Expected 'todo' in per_tool stats"
    );
    assert_eq!(s.per_tool["todo"].total_calls, 1);
    assert_eq!(s.per_tool["todo"].success_count, 1);
}

#[tokio::test]
async fn handler_impl_analyze_now_empty() {
    let (handler, _repo) = make_handler().await;

    let status = handler.analyze_now().await.unwrap();
    assert_eq!(status.total_strategy_records, 0);
    assert!(
        (status.current_threshold - 0.7).abs() < 0.001,
        "Expected initial threshold of 0.7, got {}",
        status.current_threshold
    );
}

#[tokio::test]
async fn handler_impl_analyze_now_with_multi_tool_aggregation() {
    let (handler, repo) = make_handler().await;

    for i in 0..2 {
        let row = storage::StrategyRecordRow {
            id: uuid::Uuid::new_v4(),
            timestamp: Utc::now(),
            request_id: format!("req-todo-{}", i),
            predicted_strategy: "ToolAssisted".to_string(),
            actual_strategy: "ToolAssisted".to_string(),
            escalation_count: 0,
            iterations_used: 1,
            max_iterations: 5,
            success: true,
            user_satisfaction: None,
            response_time_ms: 100,
            chat_id: None,
            tool_name: Some("todo".to_string()),
            tool_success: Some(true),
            tool_duration_ms: Some(30),
        };
        repo.create(&row).await.unwrap();
    }
    let shell_row = storage::StrategyRecordRow {
        id: uuid::Uuid::new_v4(),
        timestamp: Utc::now(),
        request_id: "req-shell-0".to_string(),
        predicted_strategy: "ToolAssisted".to_string(),
        actual_strategy: "ToolAssisted".to_string(),
        escalation_count: 0,
        iterations_used: 1,
        max_iterations: 5,
        success: true,
        user_satisfaction: None,
        response_time_ms: 5000,
        chat_id: None,
        tool_name: Some("shell".to_string()),
        tool_success: Some(false),
        tool_duration_ms: Some(5000),
    };
    repo.create(&shell_row).await.unwrap();

    let status = handler.analyze_now().await.unwrap();
    assert_eq!(status.total_strategy_records, 3);
    assert_eq!(status.per_tool["todo"].total_calls, 2);
    assert_eq!(status.per_tool["todo"].success_count, 2);
    assert_eq!(status.per_tool["shell"].total_calls, 1);
    assert_eq!(status.per_tool["shell"].success_count, 0);
}

#[tokio::test]
async fn handler_impl_threshold_history_empty() {
    let (handler, _repo) = make_handler().await;

    let history = handler.get_threshold_history(10).await.unwrap();
    assert!(
        history.is_empty(),
        "Expected empty threshold history on fresh AdaptiveThresholds"
    );
}

// ── LearningTool Routing ──────────────────────────────────────

#[test]
fn learning_tool_name_is_learning() {
    let tool = LearningTool::new(None);
    assert_eq!(tool.name(), "learning");
}

#[test]
fn learning_tool_description_nonempty() {
    let tool = LearningTool::new(None);
    assert!(!tool.description().is_empty());
}

#[test]
fn learning_tool_parameters_include_action_enum() {
    let tool = LearningTool::new(None);
    let params = tool.parameters();

    let action_enum = &params["properties"]["action"]["enum"];
    let variants: Vec<&str> = action_enum
        .as_array()
        .expect("action.enum should be an array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    assert!(
        variants.contains(&"status"),
        "Parameters schema must include 'status' action, got: {:?}",
        variants
    );
    assert!(
        variants.contains(&"analyze"),
        "Parameters schema must include 'analyze' action, got: {:?}",
        variants
    );
    assert!(
        variants.contains(&"history"),
        "Parameters schema must include 'history' action, got: {:?}",
        variants
    );
}

#[test]
fn learning_tool_parameters_action_required() {
    let tool = LearningTool::new(None);
    let params = tool.parameters();

    let required: Vec<&str> = params["required"]
        .as_array()
        .expect("required should be an array")
        .iter()
        .filter_map(|v| v.as_str())
        .collect();

    assert!(
        required.contains(&"action"),
        "'action' must be in required fields, got: {:?}",
        required
    );
}

#[test]
fn learning_tool_schema_is_valid() {
    let tool = LearningTool::new(None);
    let schema = tool.to_schema();
    assert_eq!(schema["type"], "function");
    assert_eq!(schema["function"]["name"], "learning");
}

#[tokio::test]
async fn learning_tool_routes_status_with_data() {
    let mock = Arc::new(MockLearningHandler::with_status(LearningStatus {
        current_threshold: 0.75,
        total_strategy_records: 42,
        strategy_accuracy: 0.85,
        avg_response_time_ms: 300,
        avg_satisfaction: Some(0.9),
        suggested_threshold: 0.72,
        per_tool: HashMap::new(),
    }));

    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "status"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert_eq!(parsed["total_strategy_records"], 42);
    assert!(
        (parsed["current_threshold"].as_f64().unwrap() as f32 - 0.75).abs() < 0.001,
        "Expected current_threshold ~0.75, got {}",
        parsed["current_threshold"]
    );
}

#[tokio::test]
async fn learning_tool_routes_status_no_data() {
    let mock = Arc::new(MockLearningHandler::empty());

    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "status"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    assert!(
        result.contains("No learning data") || result.contains("message"),
        "Expected no-data message, got: {}",
        result
    );
}

#[tokio::test]
async fn learning_tool_routes_analyze() {
    let mock = Arc::new(MockLearningHandler::empty());

    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "analyze"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    assert!(
        parsed["total_strategy_records"].is_number(),
        "Expected total_strategy_records in response"
    );
    assert!(
        parsed["current_threshold"].is_number(),
        "Expected current_threshold in response"
    );
}

#[tokio::test]
async fn learning_tool_routes_history_empty() {
    let mock = Arc::new(MockLearningHandler::empty());
    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "history"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert!(
        parsed.as_array().map(|a| a.is_empty()).unwrap_or(false),
        "Expected empty JSON array, got: {}",
        result
    );
}

#[tokio::test]
async fn learning_tool_routes_history_with_entries() {
    let mock = Arc::new(MockLearningHandler::with_history(vec![
        ThresholdEntry {
            from: 0.70,
            to: 0.72,
            reason: "initial".to_string(),
            timestamp: Utc::now(),
        },
        ThresholdEntry {
            from: 0.72,
            to: 0.75,
            reason: "increased".to_string(),
            timestamp: Utc::now(),
        },
    ]));
    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "history"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    let arr = parsed.as_array().expect("Expected JSON array");
    assert_eq!(arr.len(), 2, "Expected 2 history entries");
    assert_eq!(arr[0]["reason"], "initial");
    assert_eq!(arr[1]["reason"], "increased");
}

#[tokio::test]
async fn learning_tool_routes_history_explicit_limit() {
    let mock = Arc::new(MockLearningHandler::with_history(vec![
        ThresholdEntry {
            from: 0.70,
            to: 0.72,
            reason: "r1".to_string(),
            timestamp: Utc::now(),
        },
        ThresholdEntry {
            from: 0.72,
            to: 0.74,
            reason: "r2".to_string(),
            timestamp: Utc::now(),
        },
        ThresholdEntry {
            from: 0.74,
            to: 0.76,
            reason: "r3".to_string(),
            timestamp: Utc::now(),
        },
    ]));
    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "history", "limit": 2});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

    let arr = parsed.as_array().expect("Expected JSON array");
    assert_eq!(arr.len(), 2, "Expected 2 entries with limit=2");
}

#[tokio::test]
async fn threshold_history_respects_limit() {
    let mock = MockLearningHandler::with_history(vec![
        ThresholdEntry {
            from: 0.70,
            to: 0.72,
            reason: "r1".to_string(),
            timestamp: Utc::now(),
        },
        ThresholdEntry {
            from: 0.72,
            to: 0.74,
            reason: "r2".to_string(),
            timestamp: Utc::now(),
        },
        ThresholdEntry {
            from: 0.74,
            to: 0.76,
            reason: "r3".to_string(),
            timestamp: Utc::now(),
        },
    ]);

    let all = mock.get_threshold_history(10).await.unwrap();
    assert_eq!(all.len(), 3, "Expected all 3 entries with limit=10");

    let limited = mock.get_threshold_history(2).await.unwrap();
    assert_eq!(limited.len(), 2, "Expected last 2 entries with limit=2");
    assert!(
        (limited[0].from - 0.72).abs() < 0.001,
        "First entry in limited result should be r2"
    );
    assert!(
        (limited[1].from - 0.74).abs() < 0.001,
        "Second entry should be r3"
    );
}

// ── Edge Cases ────────────────────────────────────────────────

#[tokio::test]
async fn learning_tool_errors_without_handler() {
    let tool = LearningTool::new(None);
    let args = serde_json::json!({"action": "status"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await;
    assert!(
        result.is_err(),
        "Expected error when no handler configured, got: {:?}",
        result
    );
}

#[tokio::test]
async fn learning_tool_errors_on_unknown_action() {
    let mock = Arc::new(MockLearningHandler::empty());
    let tool = LearningTool::new(Some(mock as Arc<dyn LearningHandler>));
    let args = serde_json::json!({"action": "nonexistent"});
    let ctx = make_routing_ctx();

    let result = tool.execute(args, &ctx).await;
    assert!(
        result.is_err(),
        "Expected error for unknown action, got: {:?}",
        result
    );
}
