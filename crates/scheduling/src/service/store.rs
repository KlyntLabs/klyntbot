//! Persistence layer for the cron store (SQL-only via CronRepo).

use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::error;

use crate::types::CronStore;
use crate::CronError;
use common::Result;
use storage::CronRepo;

use super::CronService;

impl CronService {
    /// Load jobs from SQL. No-op when repo is absent (test-only path).
    pub(crate) async fn load_store(&self) -> Result<()> {
        let Some(repo) = &self.repo else {
            return Ok(());
        };
        let rows = repo
            .list()
            .await
            .map_err(|e| CronError::ExecutionFailed(e.to_string()))?;
        let jobs = rows.into_iter().map(Self::row_to_job).collect();
        let mut store = self.store.write().await;
        *store = CronStore { version: 1, jobs };
        Ok(())
    }

    /// Save in-memory store to SQL. No-op when repo is absent (test-only path).
    pub(crate) async fn save_store(&self) -> Result<()> {
        let Some(repo) = &self.repo else {
            return Ok(());
        };
        Self::save_store_sql_static(&self.store, repo).await
    }

    /// Save in-memory store to SQL (static version for use in detached tasks).
    pub(crate) async fn save_store_sql_static(
        store: &Arc<RwLock<CronStore>>,
        repo: &CronRepo,
    ) -> Result<()> {
        let store = store.read().await;
        let current_ids: HashSet<&str> = store.jobs.iter().map(|j| j.id.as_str()).collect();

        // Upsert all current jobs
        for job in &store.jobs {
            let row = CronService::job_to_row(job);
            repo.upsert(&row)
                .await
                .map_err(|e| CronError::ExecutionFailed(e.to_string()))?;
        }

        // Delete SQL rows that are no longer in memory (handles remove_job)
        let all_rows = repo
            .list()
            .await
            .map_err(|e| CronError::ExecutionFailed(e.to_string()))?;
        for row in all_rows {
            if !current_ids.contains(row.id.as_str()) {
                if let Err(e) = repo.delete(&row.id).await {
                    error!("Failed to delete orphaned cron row '{}': {}", row.id, e);
                }
            }
        }

        Ok(())
    }

    /// Process all due jobs and save (static method for timer loop).
    pub(crate) async fn process_due_jobs(
        store: &Arc<RwLock<CronStore>>,
        repo: &Option<CronRepo>,
        handlers: &std::collections::HashMap<String, super::JobCallback>,
        on_job: &Option<super::JobCallback>,
    ) {
        let now = super::now_ms();

        // Get due jobs
        let due_jobs = {
            let store = store.read().await;
            store
                .jobs
                .iter()
                .filter(|j| {
                    j.enabled
                        && j.state.next_run_at_ms.is_some()
                        && now >= j.state.next_run_at_ms.unwrap()
                })
                .cloned()
                .collect::<Vec<_>>()
        };

        // Execute due jobs
        for job in due_jobs {
            super::executor::execute_job_static(store, handlers, on_job, &job).await;
        }

        // Save store via SQL (skip if no repo, e.g. in tests)
        if let Some(repo) = repo {
            if let Err(e) = Self::save_store_sql_static(store, repo).await {
                error!("Failed to save cron store: {}", e);
            }
        }
    }
}
