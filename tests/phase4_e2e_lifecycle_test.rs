//! E2E test: Full Goal → Plan → Execute → Learn pipeline
//!
//! AC-E2E.1: End-to-end integration across goal, plan, and learning subsystems.
//!
//! Scenario:
//!   1. Create a goal
//!   2. Create a plan linked to the goal
//!   3. Walk the plan through its full lifecycle (Draft → Approved → Executing → Completed)
//!   4. Notify PlanCompletionHandler — goal metadata reflects completion
//!   5. Record learning outcomes for the steps
//!   6. Verify the outcome store captures plan-step telemetry
//!
//! Run: `cargo nextest run --test phase4_e2e_lifecycle_test`

use chrono::Utc;
use goal::{Goal, GoalStatus, GoalStore};
use klyntbot::agent::learning::{ExecutionMode, OutcomeRecorder, OutcomeStore};
use klyntbot::PlanCompletionHandlerImpl;
use plan::{Plan, PlanStatus, PlanStep, PlanStore, StepStatus};
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::RwLock;
use tools::plan_tool::PlanCompletionHandler;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────

fn make_goal(title: &str) -> Goal {
    let now = Utc::now();
    Goal {
        id: Uuid::new_v4(),
        title: title.to_string(),
        description: format!("E2E test goal: {}", title),
        status: GoalStatus::Active,
        priority: 1,
        target_date: None,
        created_at: now,
        updated_at: now,
        metrics: vec![],
        linked_project_ids: vec![],
        metadata: HashMap::new(),
    }
}

fn make_plan_with_steps(goal_id: Option<Uuid>) -> Plan {
    let now = Utc::now();
    let steps = vec![
        PlanStep {
            id: Uuid::new_v4(),
            index: 0,
            description: "Analyse requirements".to_string(),
            reasoning: "Understand scope".to_string(),
            expected_tools: vec!["todo".to_string()],
            status: StepStatus::Pending,
            attempt_count: 0,
            max_attempts: 3,
            result: None,
            started_at: None,
            completed_at: None,
        },
        PlanStep {
            id: Uuid::new_v4(),
            index: 1,
            description: "Implement core feature".to_string(),
            reasoning: "Deliver value".to_string(),
            expected_tools: vec!["shell".to_string()],
            status: StepStatus::Pending,
            attempt_count: 0,
            max_attempts: 3,
            result: None,
            started_at: None,
            completed_at: None,
        },
    ];

    Plan {
        id: Uuid::new_v4(),
        session_key: "e2e:lifecycle-test".to_string(),
        goal_id,
        title: "Lifecycle E2E Plan".to_string(),
        description: "Created by E2E lifecycle test".to_string(),
        status: PlanStatus::Draft,
        steps,
        current_step_index: 0,
        iteration_limit: 50,
        backtrack_history: vec![],
        created_at: now,
        updated_at: now,
        completed_at: None,
    }
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.1.1 — Goal creation persists correctly
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_goal_created_and_persists() {
    let tmp = TempDir::new().unwrap();
    let mut goal_store = GoalStore::new(tmp.path().join("goals.jsonl"));

    let goal = make_goal("Launch product v1");
    let id = goal.id;
    goal_store.add(goal).await.unwrap();

    // Reload to verify persistence (GoalStore lazy-loads on first access)
    let mut goal_store2 = GoalStore::new(tmp.path().join("goals.jsonl"));
    let retrieved = goal_store2.get(&id).await.unwrap();

    assert!(retrieved.is_some(), "goal must survive store reload");
    assert_eq!(retrieved.unwrap().title, "Launch product v1");
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.1.2 — Plan linked to goal, lifecycle transitions succeed
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_plan_lifecycle_draft_to_completed() {
    let tmp = TempDir::new().unwrap();
    let plan_store = Arc::new(RwLock::new(PlanStore::new(tmp.path().join("plans.jsonl"))));

    let goal_id = Uuid::new_v4();
    let mut plan = make_plan_with_steps(Some(goal_id));
    let plan_id = plan.id;

    // Draft → persisted
    {
        let mut s = plan_store.write().await;
        s.upsert(plan.clone()).await.unwrap();
    }

    // Draft → Approved
    {
        let mut s = plan_store.write().await;
        let p = s.get(&plan_id).await.unwrap().unwrap();
        assert_eq!(p.status, PlanStatus::Draft);
    }
    plan.status = PlanStatus::Approved;
    {
        let mut s = plan_store.write().await;
        s.upsert(plan.clone()).await.unwrap();
    }

    // Approved → Executing
    plan.status = PlanStatus::Executing;
    {
        let mut s = plan_store.write().await;
        s.upsert(plan.clone()).await.unwrap();
    }

    // Mark steps completed
    for step in plan.steps.iter_mut() {
        step.status = StepStatus::Completed;
        step.result = Some("Step executed successfully".to_string());
        step.completed_at = Some(Utc::now());
    }

    // Executing → Completed
    plan.status = PlanStatus::Completed;
    plan.completed_at = Some(Utc::now());
    {
        let mut s = plan_store.write().await;
        s.upsert(plan.clone()).await.unwrap();
    }

    // Verify final state
    let final_plan = {
        let mut s = plan_store.write().await;
        s.get(&plan_id).await.unwrap().unwrap()
    };

    assert_eq!(final_plan.status, PlanStatus::Completed);
    assert!(
        final_plan.completed_at.is_some(),
        "completed_at must be set"
    );
    assert_eq!(
        final_plan.goal_id,
        Some(goal_id),
        "goal link must be preserved"
    );
    assert!(
        final_plan
            .steps
            .iter()
            .all(|s| s.status == StepStatus::Completed),
        "all steps must be completed"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.1.3 — PlanCompletionHandler updates goal metadata
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_completion_handler_updates_goal() {
    let tmp = TempDir::new().unwrap();
    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));

    // Create goal
    let goal = make_goal("Ship feature X");
    let goal_id = goal.id;
    {
        let mut s = goal_store.write().await;
        s.add(goal).await.unwrap();
    }

    // Create plan linked to goal
    let plan_id = Uuid::new_v4();

    // Invoke completion handler
    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&goal_store));
    handler
        .on_plan_completed(
            &plan_id,
            Some(goal_id),
            true,
            "All steps completed successfully",
        )
        .await
        .unwrap();

    // Verify goal metadata
    let updated = {
        let mut s = goal_store.write().await;
        s.get(&goal_id).await.unwrap().unwrap()
    };

    assert!(
        updated.metadata.contains_key("last_completed_plan_id"),
        "goal must record last completed plan"
    );
    assert_eq!(
        updated.metadata["last_completed_plan_id"],
        plan_id.to_string()
    );
    assert_eq!(updated.metadata["last_plan_outcome"], "completed");
    assert!(
        updated.metadata.contains_key("plans_completed"),
        "plans_completed counter must exist"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.1.4 — Learning outcomes captured during plan execution
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_plan_step_outcomes_recorded() {
    let outcome_store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));
    let recorder = OutcomeRecorder::new(Arc::clone(&outcome_store));

    let plan_id = Uuid::new_v4().to_string();

    // Simulate recording outcomes for two plan steps
    for step_index in 0..2 {
        recorder
            .record_tool_outcome(
                "todo",
                true,
                None,
                50,
                None,
                ExecutionMode::PlanStep {
                    plan_id: plan_id.clone(),
                    step_index,
                },
                "e2e:lifecycle-test",
            )
            .await;
    }

    let s = outcome_store.read().await;
    let outcomes = s.get_all_outcomes().await.unwrap();

    assert_eq!(outcomes.len(), 2, "two step outcomes must be recorded");
    assert!(
        outcomes
            .iter()
            .all(|o| matches!(&o.execution_mode, ExecutionMode::PlanStep { .. })),
        "all outcomes must have PlanStep execution mode"
    );
    assert!(
        outcomes.iter().all(|o| o.success),
        "all outcomes must be success=true"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.1.5 — Full pipeline: goal + plan + completion + learning
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_full_pipeline_goal_plan_learn() {
    let tmp = TempDir::new().unwrap();

    // Setup stores
    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));
    let plan_store = Arc::new(RwLock::new(PlanStore::new(tmp.path().join("plans.jsonl"))));
    let outcome_store = Arc::new(RwLock::new(OutcomeStore::new_in_memory()));

    // 1. Create a goal
    let goal = make_goal("Full Pipeline Goal");
    let goal_id = goal.id;
    {
        let mut s = goal_store.write().await;
        s.add(goal).await.unwrap();
    }

    // 2. Create a plan linked to the goal
    let mut plan = make_plan_with_steps(Some(goal_id));
    let plan_id = plan.id;
    {
        let mut s = plan_store.write().await;
        s.upsert(plan.clone()).await.unwrap();
    }

    // 3. Walk through lifecycle
    for status in [PlanStatus::Approved, PlanStatus::Executing] {
        plan.status = status;
        let mut s = plan_store.write().await;
        s.upsert(plan.clone()).await.unwrap();
    }

    // 4. Record outcomes for each step
    let recorder = OutcomeRecorder::new(Arc::clone(&outcome_store));
    for (i, _step) in plan.steps.iter().enumerate() {
        recorder
            .record_tool_outcome(
                "todo",
                true,
                None,
                30,
                None,
                ExecutionMode::PlanStep {
                    plan_id: plan_id.to_string(),
                    step_index: i,
                },
                "e2e:pipeline",
            )
            .await;
    }

    // 5. Complete the plan
    plan.status = PlanStatus::Completed;
    plan.completed_at = Some(Utc::now());
    {
        let mut s = plan_store.write().await;
        s.upsert(plan.clone()).await.unwrap();
    }

    // 6. Notify completion handler
    let completion_handler = PlanCompletionHandlerImpl::new(Arc::clone(&goal_store));
    completion_handler
        .on_plan_completed(&plan_id, Some(goal_id), true, "Pipeline completed")
        .await
        .unwrap();

    // Assertions — clone owned values before asserting
    let goal_final = {
        let mut s = goal_store.write().await;
        s.get(&goal_id).await.unwrap().unwrap()
    };
    let plan_final = {
        let mut s = plan_store.write().await;
        s.get(&plan_id).await.unwrap().unwrap()
    };
    // Keep guard alive while using borrowed slice
    let os = outcome_store.read().await;
    let outcomes = os.get_all_outcomes().await.unwrap();

    assert_eq!(
        plan_final.status,
        PlanStatus::Completed,
        "plan must be Completed"
    );
    assert_eq!(goal_final.metadata["last_plan_outcome"], "completed");
    assert_eq!(
        goal_final.metadata["last_completed_plan_id"],
        plan_id.to_string()
    );
    assert_eq!(outcomes.len(), 2, "one outcome per step");
    assert!(
        outcomes.iter().all(|o| o.success),
        "all step outcomes must be successful"
    );
}

// ─────────────────────────────────────────────────────────────
// AC-E2E.1.6 — Multiple plans across same goal accumulate counter
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_e2e_multiple_plans_increment_goal_counter() {
    let tmp = TempDir::new().unwrap();
    let goal_store = Arc::new(RwLock::new(GoalStore::new(tmp.path().join("goals.jsonl"))));

    let goal = make_goal("Multi-plan goal");
    let goal_id = goal.id;
    {
        let mut s = goal_store.write().await;
        s.add(goal).await.unwrap();
    }

    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&goal_store));

    // Complete 3 plans against same goal
    for i in 0..3 {
        handler
            .on_plan_completed(
                &Uuid::new_v4(),
                Some(goal_id),
                true,
                &format!("Plan {} done", i + 1),
            )
            .await
            .unwrap();
    }

    let goal_final = {
        let mut s = goal_store.write().await;
        s.get(&goal_id).await.unwrap().unwrap()
    };

    assert_eq!(
        goal_final.metadata["plans_completed"], "3",
        "counter must be 3 after three completions"
    );
}
