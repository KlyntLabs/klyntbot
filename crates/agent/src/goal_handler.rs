//! GoalHandlerImpl — Implements GoalHandler trait for agent crate (Layer 5).
//!
//! Bridges the GoalHandler trait (defined in tools at Layer 3) with the
//! GoalRepo (defined in storage at Layer 1.5), following the dependency
//! inversion pattern used by CalendarSyncAdapter.

use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use goal::GoalError;
use goal::{conversions, Goal, GoalProgress, GoalStatus};
use tools::goal_tool::GoalHandler;
use uuid::Uuid;

/// Implements GoalHandler by delegating to GoalRepo.
pub struct GoalHandlerImpl {
    repo: storage::GoalRepo,
}

impl GoalHandlerImpl {
    pub fn new(repo: storage::GoalRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl GoalHandler for GoalHandlerImpl {
    async fn create_goal(&self, mut goal: Goal) -> Result<Uuid> {
        goal.updated_at = Utc::now();
        let row = conversions::goal_to_row(&goal);
        self.repo.create(&row).await?;
        for pid in &goal.linked_project_ids {
            self.repo.link_project(goal.id, &pid.to_string()).await?;
        }
        Ok(goal.id)
    }

    async fn get_goal(&self, id: &Uuid) -> Result<Option<Goal>> {
        conversions::load_goal(&self.repo, id).await
    }

    async fn list_goals(&self, status: Option<GoalStatus>) -> Result<Vec<Goal>> {
        let status_str = status.as_ref().map(|s| s.to_string());
        let rows = self.repo.list(status_str.as_deref()).await?;
        let mut goals = Vec::with_capacity(rows.len());
        for row in rows {
            let links = self.repo.get_project_links(row.id).await?;
            let project_ids = links
                .into_iter()
                .filter_map(|l| Uuid::parse_str(&l.project_id).ok())
                .collect();
            goals.push(conversions::row_to_goal(row, project_ids));
        }
        Ok(goals)
    }

    async fn update_goal(&self, mut goal: Goal) -> Result<()> {
        // Validate status transition
        match self.repo.get(goal.id).await {
            Ok(existing) => {
                let old_status = std::str::FromStr::from_str(&existing.status).unwrap_or_default();
                GoalStatus::validate_transition(&old_status, &goal.status)?;
            }
            Err(storage::StorageError::NotFound(_)) => {
                return Err(GoalError::NotFound(goal.id.to_string()).into());
            }
            Err(e) => return Err(e.into()),
        }

        goal.updated_at = Utc::now();
        conversions::save_goal(&self.repo, &goal).await?;
        Ok(())
    }

    async fn delete_goal(&self, id: &Uuid) -> Result<()> {
        let deleted = self.repo.delete(*id).await?;
        if !deleted {
            return Err(GoalError::NotFound(id.to_string()).into());
        }
        Ok(())
    }

    async fn calculate_progress(&self, id: &Uuid) -> Result<GoalProgress> {
        let goal = conversions::load_goal(&self.repo, id)
            .await?
            .ok_or_else(|| GoalError::NotFound(id.to_string()))?;
        Ok(conversions::compute_progress(&goal))
    }
}
