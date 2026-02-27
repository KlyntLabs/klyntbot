//! Plan visibility auto-cleanup service.
//!
//! Runs hourly to delete stale plans based on their visibility level:
//! - `silent` plans: removed 24 hours after reaching a terminal state
//! - `on_failure` plans: removed 7 days after successful completion

use std::time::Duration;

use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

/// Default cleanup interval (1 hour).
const CLEANUP_INTERVAL_SECS: u64 = 3600;
/// Silent plans are removed after 24 hours.
const SILENT_AGE_HOURS: i64 = 24;
/// Successful on_failure plans are removed after 7 days.
const ON_FAILURE_AGE_HOURS: i64 = 168;

/// Background service that periodically cleans up stale plans.
pub struct PlanCleanupService {
    plan_repo: storage::PlanRepo,
    cancel: CancellationToken,
}

impl PlanCleanupService {
    pub fn new(plan_repo: storage::PlanRepo, cancel: CancellationToken) -> Self {
        Self { plan_repo, cancel }
    }

    /// Spawn the cleanup service as a background task.
    pub fn spawn(self) {
        tokio::spawn(async move {
            self.run().await;
        });
    }

    async fn run(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));
        // Skip the first immediate tick
        interval.tick().await;

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    self.cleanup().await;
                }
                _ = self.cancel.cancelled() => {
                    debug!("Plan cleanup service shutting down");
                    break;
                }
            }
        }
    }

    async fn cleanup(&self) {
        match self
            .plan_repo
            .delete_stale_plans(SILENT_AGE_HOURS, ON_FAILURE_AGE_HOURS)
            .await
        {
            Ok(0) => {
                debug!("Plan cleanup: no stale plans to delete");
            }
            Ok(count) => {
                info!("Plan cleanup: deleted {} stale plan(s)", count);
            }
            Err(e) => {
                warn!("Plan cleanup failed: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn deletes_stale_silent_plans() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = storage::PlanRepo::new(pool.inner().clone());

        // Create a silent plan that's "completed"
        let now = chrono::Utc::now();
        let mut row = storage::PlanRow {
            id: uuid::Uuid::new_v4(),
            session_key: "test".to_string(),
            goal_id: None,
            title: "Silent plan".to_string(),
            description: String::new(),
            status: "completed".to_string(),
            current_step_index: 0,
            iteration_limit: 10,
            backtrack_history: serde_json::json!([]),
            visibility: "silent".to_string(),
            task_id: None,
            created_at: now,
            updated_at: now,
            completed_at: Some(now),
        };
        repo.create(&row).await.unwrap();

        // Create a transparent plan (should NOT be deleted)
        row.id = uuid::Uuid::new_v4();
        row.visibility = "transparent".to_string();
        row.title = "Transparent plan".to_string();
        repo.create(&row).await.unwrap();

        // age=0 means delete all completed silent plans regardless of age
        let deleted = repo.delete_stale_plans(0, 0).await.unwrap();
        assert_eq!(deleted, 1);

        // Transparent plan should still exist
        let remaining = repo.list(None, None, None, Some("all")).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].visibility, "transparent");
    }

    #[tokio::test]
    async fn does_not_delete_fresh_silent_plans() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = storage::PlanRepo::new(pool.inner().clone());

        let now = chrono::Utc::now();
        let row = storage::PlanRow {
            id: uuid::Uuid::new_v4(),
            session_key: "test".to_string(),
            goal_id: None,
            title: "Fresh silent plan".to_string(),
            description: String::new(),
            status: "completed".to_string(),
            current_step_index: 0,
            iteration_limit: 10,
            backtrack_history: serde_json::json!([]),
            visibility: "silent".to_string(),
            task_id: None,
            created_at: now,
            updated_at: now,
            completed_at: Some(now),
        };
        repo.create(&row).await.unwrap();

        // 24-hour age means the just-created plan is NOT stale
        let deleted = repo.delete_stale_plans(24, 168).await.unwrap();
        assert_eq!(deleted, 0);
    }
}
