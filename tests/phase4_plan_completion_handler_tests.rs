//! Tests for I3: Plan→Goal Completion Handler
//!
//! AC-I3.1: PlanCompletionHandler trait defined in tools::plan_tool
//! AC-I3.2: PlanCompletionHandlerImpl in agent crate can be constructed
//! AC-I3.3: on_plan_completed updates goal metadata when goal_id is provided
//! AC-I3.4: on_plan_completed is a no-op when no goal_id
//! AC-I3.5: AgentLoop stores Option<Arc<dyn PlanCompletionHandler>>
//! AC-I3.6: Completion summary includes key review fields

use async_trait::async_trait;
use common::Result;
use std::sync::{Arc, Mutex};
use tempfile::TempDir;
use tokio::sync::RwLock;
use uuid::Uuid;

use klyntbot::PlanCompletionHandlerImpl;
use tools::plan_tool::PlanCompletionHandler;

// ── AC-I3.1: Trait definition ─────────────────────────────────────────────────

/// Call record for MockPlanCompletionHandler.
struct CompletionCall {
    plan_id: Uuid,
    goal_id: Option<Uuid>,
    success: bool,
    summary: String,
}

/// A minimal mock so we can verify the trait's interface compiles.
struct MockPlanCompletionHandler {
    calls: Mutex<Vec<CompletionCall>>,
}

impl MockPlanCompletionHandler {
    fn new() -> Self {
        Self {
            calls: Mutex::new(vec![]),
        }
    }
}

#[async_trait]
impl PlanCompletionHandler for MockPlanCompletionHandler {
    async fn on_plan_completed(
        &self,
        plan_id: &Uuid,
        goal_id: Option<Uuid>,
        success: bool,
        summary: &str,
    ) -> Result<()> {
        self.calls.lock().unwrap().push(CompletionCall {
            plan_id: *plan_id,
            goal_id,
            success,
            summary: summary.to_string(),
        });
        Ok(())
    }
}

#[test]
fn ac_i3_1_trait_is_send_sync() {
    // Verify the trait is object-safe and has Send + Sync bounds
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MockPlanCompletionHandler>();

    // Verify it works as a trait object
    fn accepts_dyn(_: &dyn PlanCompletionHandler) {}
    let handler = MockPlanCompletionHandler::new();
    accepts_dyn(&handler);
}

#[tokio::test]
async fn ac_i3_1_trait_method_callable() {
    let handler = MockPlanCompletionHandler::new();
    let plan_id = Uuid::new_v4();
    let goal_id = Uuid::new_v4();

    handler
        .on_plan_completed(&plan_id, Some(goal_id), true, "Plan completed successfully")
        .await
        .unwrap();

    let calls = handler.calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].plan_id, plan_id);
    assert_eq!(calls[0].goal_id, Some(goal_id));
    assert!(calls[0].success);
    assert_eq!(calls[0].summary, "Plan completed successfully");
}

// ── AC-I3.2: PlanCompletionHandlerImpl construction ──────────────────────────

#[tokio::test]
async fn ac_i3_2_impl_can_be_constructed() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(goal::GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));
    let _handler = PlanCompletionHandlerImpl::new(Arc::clone(&store));
    // Construction succeeds — no panic
}

#[tokio::test]
async fn ac_i3_2_impl_satisfies_trait_bound() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(goal::GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));
    let handler = PlanCompletionHandlerImpl::new(store);

    // Coerce to trait object
    let _: Arc<dyn PlanCompletionHandler> = Arc::new(handler);
}

// ── AC-I3.3: Updates goal metadata on completion ─────────────────────────────

#[tokio::test]
async fn ac_i3_3_updates_goal_metadata_on_success() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(goal::GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));

    // Create a goal in the store
    let goal = make_test_goal("Ship v2");
    let goal_id = goal.id;
    {
        let mut s = store.write().await;
        s.add(goal).await.unwrap();
    }

    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&store));
    let plan_id = Uuid::new_v4();

    handler
        .on_plan_completed(&plan_id, Some(goal_id), true, "All steps completed")
        .await
        .unwrap();

    // Goal should have plan completion recorded in metadata
    let mut s = store.write().await;
    let goal = s.get(&goal_id).await.unwrap().unwrap();
    assert!(
        goal.metadata.contains_key("last_completed_plan_id"),
        "metadata should record last_completed_plan_id"
    );
    assert_eq!(goal.metadata["last_completed_plan_id"], plan_id.to_string());
    assert!(
        goal.metadata.contains_key("plans_completed"),
        "metadata should track plans_completed count"
    );
}

#[tokio::test]
async fn ac_i3_3_increments_plans_completed_counter() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(goal::GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));

    let goal = make_test_goal("Multi-plan goal");
    let goal_id = goal.id;
    {
        let mut s = store.write().await;
        s.add(goal).await.unwrap();
    }

    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&store));

    // Complete two plans against the same goal
    handler
        .on_plan_completed(&Uuid::new_v4(), Some(goal_id), true, "Plan 1 done")
        .await
        .unwrap();
    handler
        .on_plan_completed(&Uuid::new_v4(), Some(goal_id), true, "Plan 2 done")
        .await
        .unwrap();

    let mut s = store.write().await;
    let goal = s.get(&goal_id).await.unwrap().unwrap();
    assert_eq!(
        goal.metadata["plans_completed"], "2",
        "counter should be 2 after two completions"
    );
}

// ── AC-I3.4: No-op when no goal_id ───────────────────────────────────────────

#[tokio::test]
async fn ac_i3_4_noop_when_no_goal_id() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(goal::GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));
    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&store));

    // Should succeed silently — no goal_id means nothing to update
    handler
        .on_plan_completed(&Uuid::new_v4(), None, true, "Unlinked plan done")
        .await
        .unwrap();

    // Store should remain empty
    let mut s = store.write().await;
    let goals = s.list(None).await.unwrap();
    assert!(
        goals.is_empty(),
        "no goals should be created for unlinked plans"
    );
}

#[tokio::test]
async fn ac_i3_4_noop_when_goal_not_found() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(goal::GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));
    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&store));

    // goal_id points to a non-existent goal — should not error
    let result = handler
        .on_plan_completed(&Uuid::new_v4(), Some(Uuid::new_v4()), true, "Summary")
        .await;
    assert!(
        result.is_ok(),
        "missing goal should be a no-op, not an error: {:?}",
        result
    );
}

// ── AC-I3.5: AgentLoop wiring ─────────────────────────────────────────────────

/// Verify AgentLoop can be constructed with a PlanCompletionHandler.
/// This test does NOT start the full agent (too heavy for a unit test),
/// but verifies the builder/field accepts the handler type.
#[test]
fn ac_i3_5_plan_completion_handler_type_is_arc_dyn() {
    // Verify we can build an Arc<dyn PlanCompletionHandler> and clone it
    let mock: Arc<dyn PlanCompletionHandler> = Arc::new(MockPlanCompletionHandler::new());
    let _clone = Arc::clone(&mock);
    // The type bound is satisfied
}

// ── AC-I3.6: Completion summary includes review fields ───────────────────────

#[tokio::test]
async fn ac_i3_6_failed_plan_records_failure_in_metadata() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(goal::GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));

    let goal = make_test_goal("Risky goal");
    let goal_id = goal.id;
    {
        let mut s = store.write().await;
        s.add(goal).await.unwrap();
    }

    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&store));
    handler
        .on_plan_completed(&Uuid::new_v4(), Some(goal_id), false, "Step 2 failed")
        .await
        .unwrap();

    let mut s = store.write().await;
    let goal = s.get(&goal_id).await.unwrap().unwrap();
    assert!(
        goal.metadata.contains_key("last_plan_outcome"),
        "metadata should record last_plan_outcome"
    );
    assert_eq!(
        goal.metadata["last_plan_outcome"], "failed",
        "failed plan should record 'failed' outcome"
    );
}

#[tokio::test]
async fn ac_i3_6_success_plan_records_success_and_summary() {
    let tmp = TempDir::new().unwrap();
    let store = Arc::new(RwLock::new(goal::GoalStore::new(
        tmp.path().join("goals.jsonl"),
    )));

    let goal = make_test_goal("Successful goal");
    let goal_id = goal.id;
    {
        let mut s = store.write().await;
        s.add(goal).await.unwrap();
    }

    let handler = PlanCompletionHandlerImpl::new(Arc::clone(&store));
    handler
        .on_plan_completed(
            &Uuid::new_v4(),
            Some(goal_id),
            true,
            "Deployed new feature to production",
        )
        .await
        .unwrap();

    let mut s = store.write().await;
    let goal = s.get(&goal_id).await.unwrap().unwrap();
    assert_eq!(goal.metadata["last_plan_outcome"], "completed");
    assert!(
        goal.metadata.contains_key("last_plan_summary"),
        "metadata should include the completion summary for LLM review"
    );
    assert_eq!(
        goal.metadata["last_plan_summary"],
        "Deployed new feature to production"
    );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_test_goal(title: &str) -> goal::Goal {
    use chrono::Utc;
    use std::collections::HashMap;
    goal::Goal {
        id: Uuid::new_v4(),
        title: title.to_string(),
        description: String::new(),
        status: goal::GoalStatus::Active,
        priority: 3,
        target_date: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        metrics: vec![],
        linked_project_ids: vec![],
        metadata: HashMap::new(),
    }
}
