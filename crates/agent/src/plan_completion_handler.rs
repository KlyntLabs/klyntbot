//! PlanCompletionHandlerImpl — records plan outcomes on linked goals.
//!
//! When a plan finishes (success or failure) and it has a `goal_id`,
//! this handler updates the goal's typed plan-completion columns
//! (`plans_completed`, `plans_failed`, `avg_duration_ms`, `last_plan_at`)
//! directly via GoalRepo atomic increment methods.
//!
//! Follows the same dependency-inversion pattern as GoalHandlerImpl:
//! the trait lives in `tools` (Layer 3), the impl lives here (Layer 5).

use async_trait::async_trait;
use common::Result;
use tools::plan_tool::PlanCompletionHandler;
use tracing::warn;
use uuid::Uuid;

/// Implements PlanCompletionHandler by updating typed goal columns in GoalRepo.
pub struct PlanCompletionHandlerImpl {
    repo: storage::GoalRepo,
}

impl PlanCompletionHandlerImpl {
    pub fn new(repo: storage::GoalRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl PlanCompletionHandler for PlanCompletionHandlerImpl {
    async fn on_plan_completed(
        &self,
        _plan_id: &Uuid,
        goal_id: Option<Uuid>,
        success: bool,
        _summary: &str,
    ) -> Result<()> {
        let Some(gid) = goal_id else {
            // Not linked to a goal — nothing to update.
            return Ok(());
        };

        if success {
            // Use a default duration of 0 when we don't have timing info.
            // The plan runner will call increment_completed with the real
            // duration once Task 10 wires up timing — for now, just record
            // the success count and last_plan_at timestamp.
            if let Err(e) = self.repo.increment_completed(gid, 0).await {
                warn!(
                    "Failed to increment plans_completed for goal {}: {}",
                    gid, e
                );
            }
        } else if let Err(e) = self.repo.increment_failed(gid).await {
            warn!("Failed to increment plans_failed for goal {}: {}", gid, e);
        }

        Ok(())
    }
}
