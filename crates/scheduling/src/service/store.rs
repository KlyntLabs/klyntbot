//! Persistence layer for the cron store (load/save to JSON on disk).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::error;

use crate::types::CronStore;
use common::{CronError, Result};

use super::CronService;

impl CronService {
    /// Load jobs from disk
    pub(crate) async fn load_store(&self) -> Result<()> {
        if !self.store_path.exists() {
            return Ok(());
        }

        let content = fs::read_to_string(&self.store_path).map_err(CronError::Io)?;

        let loaded_store: CronStore = serde_json::from_str(&content).map_err(CronError::Json)?;

        let mut store = self.store.write().await;
        *store = loaded_store;

        Ok(())
    }

    /// Save jobs to disk
    pub(crate) async fn save_store(&self) -> Result<()> {
        Self::save_store_static(&self.store, &self.store_path).await
    }

    /// Save store (static version for use in detached tasks)
    pub(crate) async fn save_store_static(
        store: &Arc<RwLock<CronStore>>,
        store_path: &PathBuf,
    ) -> Result<()> {
        if let Some(parent) = store_path.parent() {
            fs::create_dir_all(parent).map_err(CronError::Io)?;
        }

        let store = store.read().await;
        let content = serde_json::to_string_pretty(&*store).map_err(CronError::Json)?;
        fs::write(store_path, content).map_err(CronError::Io)?;

        Ok(())
    }

    /// Process all due jobs and save (static method for timer loop)
    pub(crate) async fn process_due_jobs(
        store: &Arc<RwLock<CronStore>>,
        store_path: &PathBuf,
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

        // Save store
        if let Err(e) = Self::save_store_static(store, store_path).await {
            error!("Failed to save cron store: {}", e);
        }
    }
}
