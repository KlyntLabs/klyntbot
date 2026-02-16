//! PlanHandlerImpl — Implements PlanHandler trait for agent crate (Layer 5).
//!
//! Bridges the PlanHandler trait (defined in tools at Layer 3) with the
//! PlanStore (defined in plan at Layer 2), following the dependency
//! inversion pattern used by GoalHandlerImpl.

use async_trait::async_trait;
use chrono::Utc;
use common::{PlanError, Result};
use plan::{Plan, PlanStatus, PlanStore};
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::plan_tool::PlanHandler;
use uuid::Uuid;

/// Implements PlanHandler by delegating to PlanStore.
pub struct PlanHandlerImpl {
    store: Arc<RwLock<PlanStore>>,
}

impl PlanHandlerImpl {
    pub fn new(store: Arc<RwLock<PlanStore>>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl PlanHandler for PlanHandlerImpl {
    async fn create_plan(
        &self,
        title: &str,
        description: &str,
        session_key: &str,
        goal_id: Option<Uuid>,
    ) -> Result<Plan> {
        let now = Utc::now();
        let plan = Plan {
            id: Uuid::new_v4(),
            session_key: session_key.to_string(),
            goal_id,
            title: title.to_string(),
            description: description.to_string(),
            status: PlanStatus::Draft,
            steps: vec![],
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            created_at: now,
            updated_at: now,
            completed_at: None,
        };

        let mut store = self.store.write().await;
        store.upsert(plan).await
    }

    async fn get_plan(&self, id: &Uuid) -> Result<Option<Plan>> {
        let mut store = self.store.write().await;
        store.get(id).await
    }

    async fn get_active_plan(&self, session_key: &str) -> Result<Option<Plan>> {
        let mut store = self.store.write().await;
        store.get_active_plan(session_key).await
    }

    async fn approve_plan(&self, id: &Uuid) -> Result<Plan> {
        let mut store = self.store.write().await;
        let mut plan = store
            .get(id)
            .await?
            .ok_or_else(|| PlanError::NotFound(id.to_string()))?;

        // Strict validation: only Draft → Approved is allowed
        if plan.status != PlanStatus::Draft {
            return Err(PlanError::InvalidState(format!(
                "Cannot approve plan in {:?} state, must be Draft",
                plan.status
            ))
            .into());
        }

        plan.status = PlanStatus::Approved;
        plan.updated_at = Utc::now();

        store.upsert(plan).await
    }

    async fn abandon_plan(&self, id: &Uuid) -> Result<()> {
        let mut store = self.store.write().await;
        let mut plan = store
            .get(id)
            .await?
            .ok_or_else(|| PlanError::NotFound(id.to_string()))?;

        plan.status = PlanStatus::Abandoned;
        plan.updated_at = Utc::now();

        store.upsert(plan).await?;
        Ok(())
    }

    async fn get_step_context(&self, id: &Uuid) -> Result<String> {
        let mut store = self.store.write().await;
        let plan = store
            .get(id)
            .await?
            .ok_or_else(|| PlanError::NotFound(id.to_string()))?;

        // Build context window: current step + next 3
        let idx = plan.current_step_index;
        let window_end = (idx + 4).min(plan.steps.len());

        if idx >= plan.steps.len() {
            return Ok("Plan completed - no active step".to_string());
        }

        let steps_window = &plan.steps[idx..window_end];

        let mut ctx = format!("## Active Plan: {}\n", plan.title);
        ctx.push_str(&format!(
            "Progress: step {}/{}\n\n",
            idx + 1,
            plan.steps.len()
        ));

        for (i, step) in steps_window.iter().enumerate() {
            let marker = if i == 0 {
                ">>> CURRENT"
            } else {
                match i {
                    1 => "    NEXT 1",
                    2 => "    NEXT 2",
                    3 => "    NEXT 3",
                    _ => "    NEXT",
                }
            };
            ctx.push_str(&format!(
                "{}: {}\n  Reasoning: {}\n",
                marker, step.description, step.reasoning
            ));
        }
        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use plan::PlanStatus;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_plan_handler_create_and_get() {
        // Test 14
        // Given: a PlanHandlerImpl with an empty store
        // When: create_plan() is called with title, description, session_key, goal_id
        // Then:
        //   - a new Plan is created in the store
        //   - get_plan() retrieves the Plan
        //   - the Plan has status Draft
        // Maps to: US-1 (AC-1.2), Handler delegation

        let dir = tempdir().unwrap();
        let store = Arc::new(RwLock::new(PlanStore::new(
            dir.path().join("plans.jsonl"),
        )));
        let handler = PlanHandlerImpl::new(store);

        let plan = handler
            .create_plan("Test Plan", "Test description", "test-session", None)
            .await
            .unwrap();

        assert_eq!(plan.title, "Test Plan");
        assert_eq!(plan.status, PlanStatus::Draft);

        let retrieved = handler.get_plan(&plan.id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Plan");
    }

    #[tokio::test]
    async fn test_plan_handler_approve_transitions_status() {
        // Test 15
        // Given: a Plan with status Draft
        // When: approve_plan() is called
        // Then:
        //   - Plan status transitions to Approved
        //   - updated_at is refreshed
        // Maps to: US-1 (AC-1.2)

        let dir = tempdir().unwrap();
        let store = Arc::new(RwLock::new(PlanStore::new(
            dir.path().join("plans.jsonl"),
        )));
        let handler = PlanHandlerImpl::new(store);

        let plan = handler
            .create_plan("Plan to Approve", "Test", "test-session", None)
            .await
            .unwrap();

        let original_updated_at = plan.updated_at;

        // Small delay to ensure updated_at changes
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let approved = handler.approve_plan(&plan.id).await.unwrap();
        assert_eq!(approved.status, PlanStatus::Approved);
        assert!(approved.updated_at > original_updated_at);
    }

    #[tokio::test]
    async fn test_plan_handler_approve_invalid_state() {
        // Test 15b
        // Given: a Plan with status Executing (not Draft)
        // When: approve_plan() is called
        // Then: PlanError::InvalidState is returned
        // And: Plan status remains unchanged
        // Maps to: US-1 (AC-1.2) - state machine validation
        // Decision: Strict validation - return error if not in Draft status

        let dir = tempdir().unwrap();
        let store_arc = Arc::new(RwLock::new(PlanStore::new(
            dir.path().join("plans.jsonl"),
        )));
        let handler = PlanHandlerImpl::new(Arc::clone(&store_arc));

        let mut plan = handler
            .create_plan("Invalid Approve Test", "Test", "test-session", None)
            .await
            .unwrap();

        // Manually set to Executing (skip Draft → Approved)
        plan.status = PlanStatus::Executing;
        {
            let mut store = store_arc.write().await;
            store.upsert(plan.clone()).await.unwrap();
        }

        let result = handler.approve_plan(&plan.id).await;
        assert!(result.is_err());

        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Invalid plan state"));
        assert!(err_msg.contains("must be Draft"));

        // Verify status unchanged
        let unchanged = handler.get_plan(&plan.id).await.unwrap().unwrap();
        assert_eq!(unchanged.status, PlanStatus::Executing);
    }

    #[tokio::test]
    async fn test_plan_handler_abandon() {
        // Test 16
        // Given: a Plan with status Executing
        // When: abandon_plan() is called
        // Then:
        //   - Plan status transitions to Abandoned
        // Maps to: US-1 (AC-1.2)

        let dir = tempdir().unwrap();
        let store_arc = Arc::new(RwLock::new(PlanStore::new(
            dir.path().join("plans.jsonl"),
        )));
        let handler = PlanHandlerImpl::new(Arc::clone(&store_arc));

        let mut plan = handler
            .create_plan("Plan to Abandon", "Test", "test-session", None)
            .await
            .unwrap();

        // Manually set to Executing
        plan.status = PlanStatus::Executing;
        {
            let mut store = store_arc.write().await;
            store.upsert(plan.clone()).await.unwrap();
        }

        handler.abandon_plan(&plan.id).await.unwrap();

        let abandoned = handler.get_plan(&plan.id).await.unwrap().unwrap();
        assert_eq!(abandoned.status, PlanStatus::Abandoned);
    }
}
