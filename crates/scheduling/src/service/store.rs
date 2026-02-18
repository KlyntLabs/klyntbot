//! Persistence layer for the cron store (load/save to JSON on disk or SQL).

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::error;

use crate::types::CronStore;
use common::{CronError, Result};
use storage::CronRepo;

use super::CronService;

impl CronService {
    /// Load jobs from SQL or disk.
    pub(crate) async fn load_store(&self) -> Result<()> {
        if let Some(repo) = &self.sql_repo {
            let rows = repo
                .list()
                .await
                .map_err(|e| CronError::ExecutionFailed(e.to_string()))?;
            let jobs = rows.into_iter().map(Self::row_to_job).collect();
            let mut store = self.store.write().await;
            *store = CronStore { version: 1, jobs };
            return Ok(());
        }

        if !tokio::fs::try_exists(&self.store_path)
            .await
            .unwrap_or(false)
        {
            return Ok(());
        }

        let content = tokio::fs::read_to_string(&self.store_path)
            .await
            .map_err(CronError::Io)?;

        let loaded_store: CronStore = serde_json::from_str(&content).map_err(CronError::Json)?;

        let mut store = self.store.write().await;
        *store = loaded_store;

        Ok(())
    }

    /// Save jobs to SQL or disk.
    pub(crate) async fn save_store(&self) -> Result<()> {
        if let Some(repo) = &self.sql_repo {
            return Self::save_store_sql_static(&self.store, repo).await;
        }
        Self::save_store_file_static(&self.store, &self.store_path).await
    }

    /// Save store to JSONL file (static version for use in detached tasks).
    pub(crate) async fn save_store_file_static(
        store: &Arc<RwLock<CronStore>>,
        store_path: &PathBuf,
    ) -> Result<()> {
        if let Some(parent) = store_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(CronError::Io)?;
        }

        let store = store.read().await;
        let content = serde_json::to_string_pretty(&*store).map_err(CronError::Json)?;
        tokio::fs::write(store_path, content)
            .await
            .map_err(CronError::Io)?;

        Ok(())
    }

    /// Save in-memory store to SQL (static version for use in detached tasks).
    pub(crate) async fn save_store_sql_static(
        store: &Arc<RwLock<CronStore>>,
        repo: &CronRepo,
    ) -> Result<()> {
        let store = store.read().await;
        let current_ids: Vec<String> = store.jobs.iter().map(|j| j.id.clone()).collect();

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
            if !current_ids.contains(&row.id) {
                let _ = repo.delete(&row.id).await;
            }
        }

        Ok(())
    }

    /// Process all due jobs and save (static method for timer loop).
    pub(crate) async fn process_due_jobs(
        store: &Arc<RwLock<CronStore>>,
        store_path: &PathBuf,
        sql_repo: &Option<CronRepo>,
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
            super::executor::execute_job_static(store, on_job, &job).await;
        }

        // Save store via SQL or file
        let save_result = if let Some(repo) = sql_repo {
            Self::save_store_sql_static(store, repo).await
        } else {
            Self::save_store_file_static(store, store_path).await
        };
        if let Err(e) = save_result {
            error!("Failed to save cron store: {}", e);
        }
    }
}
