//! Job execution logic — runs callbacks and updates job state.

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::types::{CronJob, CronSchedule, CronStore};

use super::{compute_next_run, now_ms, JobCallback};

/// Execute a single job and update its state in the store.
///
/// Named handlers are checked first by job name; `on_job` is the fallback.
pub(crate) async fn execute_job_static(
    store: &Arc<RwLock<CronStore>>,
    handlers: &HashMap<String, JobCallback>,
    on_job: &Option<JobCallback>,
    job: &CronJob,
) {
    let start_ms = now_ms();
    info!("Cron: executing job '{}' ({})", job.name, job.id);

    let status;
    let error_msg;

    // Look up named handler first, then fall back to the generic callback.
    let callback = handlers.get(&job.name).or(on_job.as_ref());

    if let Some(callback) = callback {
        match callback(job) {
            Ok(_response) => {
                status = "ok".to_string();
                error_msg = None;
                info!("Cron: job '{}' completed", job.name);
            }
            Err(e) => {
                status = "error".to_string();
                error_msg = Some(e.to_string());
                error!("Cron: job '{}' failed: {}", job.name, e);
            }
        }
    } else {
        status = "skipped".to_string();
        error_msg = Some("No callback configured".to_string());
        warn!("Cron: job '{}' skipped (no callback)", job.name);
    }

    // Update job state
    let mut store = store.write().await;
    let should_delete = if let Some(job) = store.jobs.iter_mut().find(|j| j.id == job.id) {
        job.state.last_status = Some(status);
        job.state.last_error = error_msg;
        job.state.last_run_at_ms = Some(start_ms);
        job.updated_at_ms = now_ms();

        // Handle one-shot jobs
        match &job.schedule {
            CronSchedule::At { .. } => {
                if job.delete_after_run {
                    true
                } else {
                    job.enabled = false;
                    job.state.next_run_at_ms = None;
                    false
                }
            }
            _ => {
                // Compute next run
                job.state.next_run_at_ms = compute_next_run(&job.schedule, now_ms());
                false
            }
        }
    } else {
        false
    };

    // Delete job if needed
    if should_delete {
        store.jobs.retain(|j| j.id != job.id);
    }
}

impl super::CronService {
    /// Execute a single job
    pub(crate) async fn execute_job(&self, job: &CronJob) {
        let handlers = {
            let guard = self.handlers.read().expect("handler lock poisoned");
            guard.clone()
        };
        execute_job_static(&self.store, &handlers, &self.on_job, job).await;
    }
}
