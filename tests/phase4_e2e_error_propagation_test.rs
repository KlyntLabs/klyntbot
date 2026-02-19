//! E2E test: Error handling and backtracking
//!
//! AC-E2E.3: Error conditions propagate correctly through the plan subsystem.
//!
//! Scenarios:
//!   - Invalid state transitions return appropriate errors
//!   - BacktrackEntry correctly records failure metadata
//!   - PlanCompletionHandler records failure outcome in goal metadata
//!   - OutcomeRecorder captures error categories for failed tool calls
//!   - A plan that fails notifies the completion handler with success=false
//!
//! Run: `cargo nextest run --test phase4_e2e_error_propagation_test`

use chrono::Utc;
use goal::{Goal, GoalStatus, GoalStore};
use klyntbot::agent::learning::{ExecutionMode, OutcomeRecord, OutcomeRecorder, OutcomeStore};
use klyntbot::PlanCompletionHandlerImpl;
use plan::{BacktrackEntry, Plan, PlanStatus, PlanStep, PlanStore, StepStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tools::plan_tool::PlanCompletionHandler;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────

fn make_goal_with_store(title: &str) -> Goal {
    let now = Utc::now();
    Goal {
        id: Uuid::new_v4(),
        title: title.to_string(),
        description: "Error propagation test goal".to_string(),
        status: GoalStatus::Active,
        priority: 2,
        target_date: None,
        created_at: now,
        updated_at: now,
        metrics: vec![],
        linked_project_ids: vec![],
        metadata: HashMap::new(),
    }
}

fn make_plan_with_failing_step() -> Plan {
    let now = Utc::now();
    Plan {
        id: Uuid::new_v4(),
        session_key: "e2e:error-test".to_string(),
        goal_id: None,
        title: "Error Test Plan".to_string(),
        description: "Plan with a failing step".to_string(),
        status: PlanStatus::Executing,
        steps: vec![
            PlanStep {
                id: Uuid::new_v4(),
                index: 0,
                description: "This step will fail".to_string(),
                reasoning: "Simulate failure".to_string(),
                expected_tools: vec!["shell".to_string()],
                status: StepStatus::Failed,
                attempt_count: 3,
                max_attempts: 3,
                result: Some("Tool execution failed: command not found".to_string()),
                started_at: Some(now),
                completed_at: Some(now),
            },
            PlanStep {
                id: Uuid::new_v4(),
                index: 1,
                description: "This step never ran".to_string(),
                reasoning: "Blocked by step 0 failure".to_string(),
                expected_tools: vec!["todo".to_string()],
                status: StepStatus::Pending,
                attempt_count: 0,
                max_attempts: 3,
                result: None,
                started_at: None,
                completed_at: None,
            },
        ],
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    }
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.3.1 — Invalid transitions from final states return errors
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_invalid_transitions_from_completed() {
    for target in [
        PlanStatus::Draft,
        PlanStatus::Approved,
        PlanStatus::Executing,
        PlanStatus::Failed,
        PlanStatus::Abandoned,
    ] {
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Completed, &target).is_err(),
            "Completed → {:?} must fail",
            target
        );
    }
}

#[tokio::test]
async fn test_e2e_invalid_transitions_from_failed() {
    for target in [
        PlanStatus::Draft,
        PlanStatus::Approved,
        PlanStatus::Executing,
        PlanStatus::Completed,
        PlanStatus::Abandoned,
    ] {
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Failed, &target).is_err(),
            "Failed → {:?} must fail",
            target
        );
    }
}

#[tokio::test]
async fn test_e2e_invalid_transitions_from_abandoned() {
    for target in [
        PlanStatus::Draft,
        PlanStatus::Approved,
        PlanStatus::Executing,
        PlanStatus::Completed,
        PlanStatus::Failed,
    ] {
        assert!(
            PlanStatus::validate_transition(&PlanStatus::Abandoned, &target).is_err(),
            "Abandoned → {:?} must fail",
            target
        );
    }
}

#[tokio::test]
async fn test_e2e_skip_state_transitions_rejected() {
    // Cannot skip states
    assert!(
        PlanStatus::validate_transition(&PlanStatus::Draft, &PlanStatus::Executing).is_err(),
        "Draft → Executing (skip Approved) must fail"
    );
    assert!(
        PlanStatus::validate_transition(&PlanStatus::Draft, &PlanStatus::Completed).is_err(),
        "Draft → Completed must fail"
    );
    assert!(
        PlanStatus::validate_transition(&PlanStatus::Approved, &PlanStatus::Completed).is_err(),
        "Approved → Completed (skip Executing) must fail"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.3.2 — BacktrackEntry records failure metadata
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_backtrack_entry_captures_failure_info() {
    let now = Utc::now();
    let entry = BacktrackEntry {
        step_index: 2,
        attempt: 3,
        failure_reason: "Tool returned exit code 1: permission denied".to_string(),
        timestamp: now,
    };

    // Serialize / deserialize round-trip
    let json = serde_json::to_string(&entry).unwrap();
    let back: BacktrackEntry = serde_json::from_str(&json).unwrap();

    assert_eq!(back.step_index, 2);
    assert_eq!(back.attempt, 3);
    assert!(
        back.failure_reason.contains("permission denied"),
        "failure reason must survive round-trip"
    );
}

#[tokio::test]
async fn test_e2e_plan_stores_multiple_backtrack_entries() {
    let tmp = TempDir::new().unwrap();
    let plan_store = Arc::new(RwLock::new(PlanStore::new(tmp.path().join("plans.jsonl"))));

    let now = Utc::now();
    let mut plan = make_plan_with_failing_step();
    let plan_id = plan.id;

    // Simulate three backtrack attempts
    plan.backtrack_history = vec![
        BacktrackEntry {
            step_index: 0,
            attempt: 1,
            failure_reason: "First attempt: tool timeout".to_string(),
            timestamp: now,
        },
        BacktrackEntry {
            step_index: 0,
            attempt: 2,
            failure_reason: "Second attempt: invalid args".to_string(),
            timestamp: now,
        },
        BacktrackEntry {
            step_index: 0,
            attempt: 3,
            failure_reason: "Third attempt: file not found".to_string(),
            timestamp: now,
        },
    ];
    plan.status = PlanStatus::Failed;

    {
        let mut s = plan_store.write().await;
        s.upsert(plan.clone()).await.unwrap();
    }

    let stored = {
        let mut s = plan_store.write().await;
        s.get(&plan_id).await.unwrap().unwrap()
    };

    assert_eq!(stored.status, PlanStatus::Failed, "plan must be Failed");
    assert_eq!(
        stored.backtrack_history.len(),
        3,
        "three backtrack entries must be stored"
    );
    assert_eq!(stored.backtrack_history[0].attempt, 1);
    assert_eq!(stored.backtrack_history[1].attempt, 2);
    assert_eq!(stored.backtrack_history[2].attempt, 3);
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.3.3 — Failed plan propagates failure to goal metadata
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_failed_plan_updates_goal_failure_metadata() {
    let tmp = TempDir::new().unwrap();
    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));

    let goal = make_goal_with_store("Goal with failing plan");
    let goal_id = goal.id;
    {
        let mut s = goal_store.write().await;
        s.add(goal).await.unwrap();
    }

    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&goal_store));
    let plan_id = Uuid::new_v4();

    // Notify completion handler with success=false
    handler
        .on_plan_completed(
            &plan_id,
            Some(goal_id),
            false,
            "Step 0 failed after 3 attempts",
        )
        .await
        .unwrap();

    let goal_final = {
        let mut s = goal_store.write().await;
        s.get(&goal_id).await.unwrap().unwrap()
    };

    assert_eq!(
        goal_final.metadata["last_plan_outcome"], "failed",
        "failed plan must record 'failed' outcome in goal metadata"
    );
    assert_eq!(
        goal_final.metadata["last_completed_plan_id"],
        plan_id.to_string(),
        "plan ID must still be recorded even on failure"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.3.4 — Error categories recorded in outcomes
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_error_categories_captured_in_outcomes() {
    let outcome_store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let recorder = OutcomeRecorder::new(Arc::clone(&outcome_store));

    // Record failures with distinct error categories
    let error_cases = [
        ("shell", "timeout"),
        ("todo", "validation"),
        ("web_search", "network"),
        ("file_write", "permission"),
    ];

    for (tool, category) in &error_cases {
        recorder
            .record_tool_outcome(
                tool,
                false,
                Some(category),
                100,
                None,
                ExecutionMode::PlanStep {
                    plan_id: Uuid::new_v4().to_string(),
                    step_index: 0,
                },
                "e2e:error-test",
            )
            .await;
    }

    let s = outcome_store.read().await;
    let outcomes = s.get_all_outcomes().await.unwrap();

    assert_eq!(outcomes.len(), 4, "four error outcomes must be recorded");
    assert!(outcomes.iter().all(|o| !o.success), "all must be failures");
    assert!(
        outcomes.iter().all(|o| o.error_category.is_some()),
        "all must have error categories"
    );

    // Verify specific categories preserved (collect to owned Strings)
    let categories: Vec<String> = outcomes
        .iter()
        .filter_map(|o| o.error_category.clone())
        .collect();
    assert!(
        categories.iter().any(|c| c == "timeout"),
        "timeout must be recorded"
    );
    assert!(
        categories.iter().any(|c| c == "validation"),
        "validation must be recorded"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.3.5 — Plan failure after max backtracks persists correctly
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_plan_failed_state_is_terminal() {
    let tmp = TempDir::new().unwrap();
    let plan_store = Arc::new(RwLock::new(PlanStore::new(tmp.path().join("plans.jsonl"))));

    let mut plan = make_plan_with_failing_step();
    plan.status = PlanStatus::Failed;
    let plan_id = plan.id;

    {
        let mut s = plan_store.write().await;
        s.upsert(plan.clone()).await.unwrap();
    }

    // Attempt to transition from Failed — must be rejected by state machine
    let should_fail = PlanStatus::validate_transition(&PlanStatus::Failed, &PlanStatus::Executing);
    assert!(should_fail.is_err(), "Failed → Executing must be rejected");

    // Verify plan remains in Failed state
    let stored = {
        let mut s = plan_store.write().await;
        s.get(&plan_id).await.unwrap().unwrap()
    };
    assert_eq!(stored.status, PlanStatus::Failed, "plan must remain Failed");
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.3.6 — No-op when goal not found (graceful degradation)
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_completion_handler_missing_goal_is_noop() {
    let tmp = TempDir::new().unwrap();
    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));
    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&goal_store));

    // goal_id does not exist in store
    let result = handler
        .on_plan_completed(&Uuid::new_v4(), Some(Uuid::new_v4()), false, "Failed plan")
        .await;

    assert!(
        result.is_ok(),
        "missing goal must be a no-op, not an error: {:?}",
        result
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.3.7 — Privacy: outcome records do not contain sensitive data
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_outcome_record_no_sensitive_fields() {
    let record = OutcomeRecord {
        id: "e2e-privacy-check".to_string(),
        session_key: "e2e:abc123".to_string(),
        tool_name: "shell".to_string(),
        success: false,
        error_category: Some("timeout".to_string()),
        duration_ms: 5000,
        confidence_score: Some(0.4),
        confidence_dimensions: None,
        execution_mode: ExecutionMode::PlanStep {
            plan_id: "plan-xyz".to_string(),
            step_index: 1,
        },
        created_at: Utc::now(),
    };

    let json = serde_json::to_string(&record).unwrap();

    // Must not contain tool arguments or user messages
    assert!(!json.contains("\"arguments\""), "args must not leak");
    assert!(
        !json.contains("\"user_message\""),
        "user message must not leak"
    );
    assert!(!json.contains("\"tool_args\""), "tool_args must not leak");

    // Must contain expected structural fields
    assert!(json.contains("\"tool_name\""), "tool_name must be present");
    assert!(json.contains("\"success\""), "success must be present");
    assert!(
        json.contains("\"error_category\""),
        "error_category must be present"
    );
    assert!(
        json.contains("\"duration_ms\""),
        "duration_ms must be present"
    );
}
