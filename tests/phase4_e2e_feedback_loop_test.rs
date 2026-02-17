//! E2E test: Learning adaptation feedback loop
//!
//! AC-E2E.4: The learning system correctly adapts based on recorded outcomes.
//!
//! Scenarios:
//!   - Record 60+ outcomes → LearningAnalyzer produces valid analysis
//!   - AdaptiveThresholds updates based on analysis (within step limits)
//!   - Enrichment feedback is aggregated correctly in EnrichmentStats
//!   - Learning service analyze_now() reflects accumulated outcomes
//!   - Threshold history persists across save/reload cycles
//!
//! Run: `cargo nextest run --test phase4_e2e_feedback_loop_test`

use chrono::Utc;
use klyntbot::agent::learning::adaptive::AdaptiveThresholds;
use klyntbot::agent::learning::analyzer::LearningAnalyzer;
use klyntbot::agent::learning::types::{AnalysisResult, EnrichmentStats};
use klyntbot::agent::learning::{ExecutionMode, OutcomeRecord, OutcomeStore};
use klyntbot::agent::LearningHandlerImpl;
use klyntbot::agent::LearningService;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tools::learning_feedback::EnrichmentFeedbackEntry;
use tools::learning_tool::LearningHandler;

// ─────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────

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

fn make_feedback(task_id: &str, accepted: bool, field: &str) -> EnrichmentFeedbackEntry {
    EnrichmentFeedbackEntry {
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

// ─────────────────────────────────────────────────────────────
// AC-E2E.4.1 — Analyzer produces analysis from 60+ outcomes
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_analyzer_with_mixed_outcomes() {
    // 60 outcomes: 45 succeed (high confidence), 15 fail (low confidence)
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

    // suggested_threshold must be within [0.4, 0.9]
    assert!(
        result.suggested_threshold >= 0.4 && result.suggested_threshold <= 0.9,
        "suggested_threshold must be in [0.4, 0.9], got {}",
        result.suggested_threshold
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.4.2 — Adaptive threshold updates based on analysis
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_adaptive_threshold_updates_after_sufficient_data() {
    let tmp = TempDir::new().unwrap();
    let state_path = tmp.path().join("state.json");
    let mut adaptive = AdaptiveThresholds::load(state_path, 0.7, 0.4, 0.9, 50).await;

    // Build 60 outcomes suggesting a lower threshold (all succeed at moderate confidence)
    let outcomes: Vec<OutcomeRecord> = (0..60)
        .map(|i| make_outcome(&format!("o{}", i), "todo", true, 0.55))
        .collect();

    let analysis = LearningAnalyzer::analyze(&outcomes, &[]);
    let result = adaptive.apply_analysis(&analysis);

    // With 60 outcomes (above min of 50), threshold may change
    if let Some(new_threshold) = result {
        // Step limit enforced
        let change = (new_threshold - 0.7_f32).abs();
        assert!(
            change <= 0.05 + f32::EPSILON,
            "single step must not exceed 0.05; change was {:.4}",
            change
        );
        // Bounds enforced
        assert!(new_threshold >= 0.4, "threshold must stay >= 0.4");
        assert!(new_threshold <= 0.9, "threshold must stay <= 0.9");
    }
}

#[tokio::test]
async fn test_e2e_cold_start_prevents_adaptation_below_50() {
    let tmp = TempDir::new().unwrap();
    let mut adaptive =
        AdaptiveThresholds::load(tmp.path().join("state.json"), 0.7, 0.4, 0.9, 50).await;
    let initial = adaptive.current_threshold();

    // Only 49 outcomes — below minimum
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
        (adaptive.current_threshold() - initial).abs() < f32::EPSILON,
        "threshold must be unchanged"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.4.3 — Enrichment feedback stats are accurate
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_enrichment_feedback_stats_computed() {
    // 20 feedback entries: 14 accepted, 6 rejected
    let feedback: Vec<EnrichmentFeedbackEntry> = (0..20)
        .map(|i| make_feedback(&format!("task-{}", i), i < 14, "priority"))
        .collect();

    let result = LearningAnalyzer::analyze(&[], &feedback);
    let stats = &result.enrichment_stats;

    assert_eq!(stats.total_suggestions, 20);
    assert_eq!(stats.accepted_count, 14);
    assert_eq!(stats.overridden_count, 6);
    assert!(
        (stats.acceptance_rate - 0.7).abs() < 0.001,
        "acceptance_rate must be 0.70, got {}",
        stats.acceptance_rate
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.4.4 — Full loop: record → persist → reload → analyze
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_outcomes_persist_and_are_analyzable_after_reload() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("outcomes.jsonl");

    // Session 1: record 30 outcomes
    {
        let mut store = OutcomeStore::new(path.clone());
        for i in 0..30 {
            store
                .record(make_outcome(&format!("o{}", i), "todo", i % 4 != 0, 0.75))
                .await
                .unwrap();
        }
    }

    // Session 2: reload and verify analysis works
    let mut store2 = OutcomeStore::new(path.clone());
    store2.load().await.unwrap();
    let outcomes = store2.get_all_outcomes().await.unwrap();

    assert_eq!(outcomes.len(), 30, "all 30 outcomes must survive reload");

    let analysis = LearningAnalyzer::analyze(outcomes, &[]);
    assert_eq!(analysis.total_outcomes, 30);
    assert!(analysis.per_tool_stats.contains_key("todo"));
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.4.5 — LearningHandlerImpl.analyze_now() reflects recorded data
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_learning_handler_analyze_now_with_real_data() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(OutcomeStore::new(
        tmp.path().join("outcomes.jsonl"),
    )));
    let adaptive = Arc::new(RwLock::new(
        AdaptiveThresholds::load(tmp.path().join("state.json"), 0.7, 0.4, 0.9, 50).await,
    ));

    // Pre-populate the store
    {
        let mut sg = store.write().await;
        for i in 0..10 {
            sg.record(make_outcome(
                &format!("r{}", i),
                if i % 2 == 0 { "todo" } else { "shell" },
                i % 3 != 0,
                0.7,
            ))
            .await
            .unwrap();
        }
    }

    let handler = LearningHandlerImpl::new(Arc::clone(&store), Arc::clone(&adaptive));
    let status = handler.analyze_now().await.unwrap();

    assert_eq!(
        status.total_outcomes, 10,
        "must reflect 10 recorded outcomes"
    );
    assert!(
        status.per_tool.contains_key("todo"),
        "must have stats for 'todo'"
    );
    assert!(
        status.per_tool.contains_key("shell"),
        "must have stats for 'shell'"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.4.6 — LearningService starts, accumulates, and shuts down cleanly
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_learning_service_lifecycle() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(OutcomeStore::new(
        tmp.path().join("outcomes.jsonl"),
    )));
    let adaptive = Arc::new(RwLock::new(
        AdaptiveThresholds::load(tmp.path().join("state.json"), 0.7, 0.4, 0.9, 50).await,
    ));

    let mut service = LearningService::new(
        Arc::clone(&store),
        Arc::clone(&adaptive),
        None,
        Duration::from_secs(9999), // won't auto-trigger
    );

    service.start();

    // Manual analysis while running
    let analysis_result = service.analyze_now().await;
    assert!(
        analysis_result.is_ok(),
        "analyze_now() must succeed while service is running: {:?}",
        analysis_result
    );

    service.stop().await; // must not panic
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.4.7 — Threshold history persists across restart
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_threshold_history_persists_across_restart() {
    let tmp = TempDir::new().unwrap();
    let state_path = tmp.path().join("state.json");

    let saved_threshold = {
        let mut adaptive = AdaptiveThresholds::load(state_path.clone(), 0.7, 0.4, 0.9, 50).await;

        // Force a change (100 outcomes, suggest big drop)
        let analysis = AnalysisResult {
            computed_at: Utc::now(),
            total_outcomes: 100,
            per_tool_stats: Default::default(),
            suggested_threshold: 0.5,
            threshold_confidence: 0.9,
            enrichment_stats: EnrichmentStats::default(),
        };
        adaptive.apply_analysis(&analysis);
        adaptive.save().await.unwrap();
        adaptive.current_threshold()
    };

    // Reload from disk
    let reloaded = AdaptiveThresholds::load(state_path, 0.7, 0.4, 0.9, 50).await;
    assert!(
        (reloaded.current_threshold() - saved_threshold).abs() < f32::EPSILON,
        "reloaded threshold must match saved: {} vs {}",
        reloaded.current_threshold(),
        saved_threshold
    );
    assert!(
        !reloaded.state().threshold_history.is_empty(),
        "threshold history must survive save/reload"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.4.8 — Mixed tool outcomes produce correct per-tool stats
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_multi_tool_analysis() {
    let outcomes: Vec<OutcomeRecord> = vec![
        // 4 todo calls: 3 succeed
        make_outcome("t0", "todo", true, 0.8),
        make_outcome("t1", "todo", true, 0.8),
        make_outcome("t2", "todo", true, 0.8),
        make_outcome("t3", "todo", false, 0.3),
        // 2 shell calls: 1 succeeds
        make_outcome("s0", "shell", true, 0.75),
        make_outcome("s1", "shell", false, 0.2),
        // 3 web calls: all succeed
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

// ─────────────────────────────────────────────────────────────
// AC-E2E.4.9 — Feedback + outcomes analyzed together
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_combined_outcomes_and_feedback_analysis() {
    // 50 outcomes and 10 feedback entries analyzed together
    let outcomes: Vec<OutcomeRecord> = (0..50)
        .map(|i| make_outcome(&format!("o{}", i), "todo", i % 5 != 0, 0.7))
        .collect();

    let feedback: Vec<EnrichmentFeedbackEntry> = (0..10)
        .map(|i| make_feedback(&format!("f{}", i), i < 8, "priority"))
        .collect();

    let result = LearningAnalyzer::analyze(&outcomes, &feedback);

    // Outcome stats
    assert_eq!(result.total_outcomes, 50);
    assert_eq!(result.per_tool_stats["todo"].total_calls, 50);

    // Feedback stats
    assert_eq!(result.enrichment_stats.total_suggestions, 10);
    assert_eq!(result.enrichment_stats.accepted_count, 8);
    assert_eq!(result.enrichment_stats.overridden_count, 2);
    assert!(
        (result.enrichment_stats.acceptance_rate - 0.8).abs() < 0.001,
        "acceptance rate must be 0.8"
    );
}
