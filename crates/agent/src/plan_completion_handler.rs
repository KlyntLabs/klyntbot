//! PlanCompletionHandlerImpl — records plan outcomes on linked goals.
//!
//! When a plan finishes (success or failure) and it has a `goal_id`,
//! this handler writes structured metadata into the goal so the
//! LLM and ContextBuilder can review what happened.
//!
//! Follows the same dependency-inversion pattern as GoalHandlerImpl:
//! the trait lives in `tools` (Layer 3), the impl lives here (Layer 5).

use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use goal::GoalStore;
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::plan_tool::PlanCompletionHandler;
use uuid::Uuid;

/// Implements PlanCompletionHandler by updating goal metadata in GoalStore.
pub struct PlanCompletionHandlerImpl {
    store: Arc<RwLock<GoalStore>>,
}

impl PlanCompletionHandlerImpl {
    pub fn new(store: Arc<RwLock<GoalStore>>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl PlanCompletionHandler for PlanCompletionHandlerImpl {
    async fn on_plan_completed(
        &self,
        plan_id: &Uuid,
        goal_id: Option<Uuid>,
        success: bool,
        summary: &str,
    ) -> Result<()> {
        let Some(gid) = goal_id else {
            // Not linked to a goal — nothing to update.
            return Ok(());
        };

        let mut store = self.store.write().await;
        let Some(goal) = store.get(&gid).await? else {
            // Goal was deleted — treat as no-op.
            return Ok(());
        };

        let mut updated = goal;
        let plans_completed: u64 = updated
            .metadata
            .get("plans_completed")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let outcome = if success { "completed" } else { "failed" };

        updated
            .metadata
            .insert("last_completed_plan_id".into(), plan_id.to_string());
        updated
            .metadata
            .insert("last_plan_outcome".into(), outcome.into());
        updated
            .metadata
            .insert("last_plan_summary".into(), summary.to_string());
        updated
            .metadata
            .insert("last_plan_at".into(), Utc::now().to_rfc3339());

        if success {
            updated
                .metadata
                .insert("plans_completed".into(), (plans_completed + 1).to_string());
        }

        updated.updated_at = Utc::now();
        store.update(updated).await?;
        Ok(())
    }
}
