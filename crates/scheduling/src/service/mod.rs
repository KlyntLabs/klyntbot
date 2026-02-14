//! Cron service for scheduling agent tasks.
//!
//! Split into submodules:
//! - `executor`: Job execution and state updates
//! - `store`: Persistence (load/save to JSON on disk)

mod executor;
mod store;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::info;
use uuid::Uuid;

use crate::types::{CronJob, CronSchedule, CronStore};
use common::Result;

/// Get current time in milliseconds
fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/// Compute next run time in ms
fn compute_next_run(schedule: &CronSchedule, now_ms: i64) -> Option<i64> {
    match schedule {
        CronSchedule::At { at_ms } => {
            if *at_ms > now_ms {
                Some(*at_ms)
            } else {
                None
            }
        }
        CronSchedule::Every { every_ms } => {
            if *every_ms > 0 {
                Some(now_ms + *every_ms as i64)
            } else {
                None
            }
        }
        CronSchedule::Cron { expr, .. } => {
            // Use cron crate to compute next run
            match cron::Schedule::try_from(expr.as_str()) {
                Ok(schedule) => schedule
                    .upcoming(Utc)
                    .next()
                    .map(|dt| dt.timestamp_millis()),
                Err(_) => None,
            }
        }
    }
}

/// Callback type for job execution
pub type JobCallback = Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>;

/// Service for managing and executing scheduled jobs
pub struct CronService {
    pub(crate) store_path: PathBuf,
    pub(crate) store: Arc<RwLock<CronStore>>,
    pub(crate) on_job: Option<JobCallback>,
    pub(crate) running: Arc<RwLock<bool>>,
    pub(crate) timer_task: Arc<RwLock<Option<JoinHandle<()>>>>,
}

impl CronService {
    /// Create a new cron service
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
            store: Arc::new(RwLock::new(CronStore::default())),
            on_job: None,
            running: Arc::new(RwLock::new(false)),
            timer_task: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the job execution callback
    pub fn set_callback(&mut self, callback: JobCallback) {
        self.on_job = Some(callback);
    }

    /// Recompute next run times for all enabled jobs
    async fn recompute_next_runs(&self) {
        let mut store = self.store.write().await;
        let now = now_ms();

        for job in &mut store.jobs {
            if job.enabled {
                job.state.next_run_at_ms = compute_next_run(&job.schedule, now);
            }
        }
    }

    /// Get the earliest next run time across all jobs
    async fn get_next_wake_ms(&self) -> Option<i64> {
        let store = self.store.read().await;
        store
            .jobs
            .iter()
            .filter(|j| j.enabled && j.state.next_run_at_ms.is_some())
            .filter_map(|j| j.state.next_run_at_ms)
            .min()
    }

    /// Start the continuous timer loop
    fn start_timer_loop(&self) {
        let store = self.store.clone();
        let store_path = self.store_path.clone();
        let on_job = self.on_job.clone();
        let running = self.running.clone();
        let timer_task_ref = self.timer_task.clone();

        let task = tokio::spawn(async move {
            loop {
                // Check if still running
                if !*running.read().await {
                    break;
                }

                // Get next wake time
                let next_wake_ms = {
                    let store = store.read().await;
                    store
                        .jobs
                        .iter()
                        .filter(|j| j.enabled && j.state.next_run_at_ms.is_some())
                        .filter_map(|j| j.state.next_run_at_ms)
                        .min()
                };

                if let Some(next_wake) = next_wake_ms {
                    let now = now_ms();
                    let delay_ms = (next_wake - now).max(0);

                    // Sleep until next wake time (or check every 100ms if it's far away)
                    let check_interval = Duration::from_millis(100);
                    let sleep_duration = Duration::from_millis(delay_ms as u64).min(check_interval);

                    tokio::time::sleep(sleep_duration).await;

                    // Check if any jobs are due
                    if now_ms() >= next_wake {
                        CronService::process_due_jobs(&store, &store_path, &on_job).await;
                    }
                } else {
                    // No jobs scheduled, sleep for a bit then check again
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        });

        // Store the task handle
        tokio::spawn(async move {
            let mut timer_task = timer_task_ref.write().await;
            *timer_task = Some(task);
        });
    }

    // ========== Public API ==========

    /// Start the cron service
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;

        self.load_store().await?;
        self.recompute_next_runs().await;
        self.save_store().await?;
        self.start_timer_loop();

        let job_count = self.store.read().await.jobs.len();
        info!("Cron service started with {} jobs", job_count);

        Ok(())
    }

    /// Stop the cron service
    pub async fn stop(&self) {
        *self.running.write().await = false;

        let mut timer_task = self.timer_task.write().await;
        if let Some(task) = timer_task.take() {
            task.abort();
        }
    }

    /// List all jobs
    pub async fn list_jobs(&self, include_disabled: bool) -> Vec<CronJob> {
        let store = self.store.read().await;
        let mut jobs: Vec<CronJob> = if include_disabled {
            store.jobs.clone()
        } else {
            store.jobs.iter().filter(|j| j.enabled).cloned().collect()
        };

        // Sort by next run time
        jobs.sort_by_key(|j| j.state.next_run_at_ms.unwrap_or(i64::MAX));
        jobs
    }

    /// Add a new job
    #[allow(clippy::too_many_arguments)]
    pub async fn add_job(
        &self,
        name: impl Into<String>,
        schedule: CronSchedule,
        message: impl Into<String>,
        deliver: bool,
        channel: Option<String>,
        to: Option<String>,
        delete_after_run: bool,
    ) -> Result<CronJob> {
        let name = name.into();
        let message = message.into();
        let now = now_ms();

        let job_id = Uuid::new_v4().to_string()[..8].to_string();

        let mut job = CronJob::new(job_id.clone(), name.clone(), schedule.clone(), message);
        job.payload.deliver = deliver;
        job.payload.channel = channel;
        job.payload.to = to;
        job.delete_after_run = delete_after_run;
        job.state.next_run_at_ms = compute_next_run(&schedule, now);

        let mut store = self.store.write().await;
        store.jobs.push(job.clone());
        drop(store);

        self.save_store().await?;

        info!("Cron: added job '{}' ({})", name, job_id);
        Ok(job)
    }

    /// Remove a job by ID
    pub async fn remove_job(&self, job_id: impl AsRef<str>) -> Result<bool> {
        let job_id = job_id.as_ref();
        let mut store = self.store.write().await;

        let before = store.jobs.len();
        store.jobs.retain(|j| j.id != job_id);
        let removed = store.jobs.len() < before;

        drop(store);

        if removed {
            self.save_store().await?;
            info!("Cron: removed job {}", job_id);
        }

        Ok(removed)
    }

    /// Enable or disable a job
    pub async fn enable_job(
        &self,
        job_id: impl AsRef<str>,
        enabled: bool,
    ) -> Result<Option<CronJob>> {
        let job_id = job_id.as_ref();
        let mut store = self.store.write().await;

        let job = store.jobs.iter_mut().find(|j| j.id == job_id);

        if let Some(job) = job {
            job.enabled = enabled;
            job.updated_at_ms = now_ms();

            if enabled {
                job.state.next_run_at_ms = compute_next_run(&job.schedule, now_ms());
            } else {
                job.state.next_run_at_ms = None;
            }

            let job_clone = job.clone();
            drop(store);

            self.save_store().await?;

            Ok(Some(job_clone))
        } else {
            Ok(None)
        }
    }

    /// Manually run a job
    pub async fn run_job(&self, job_id: impl AsRef<str>, force: bool) -> Result<bool> {
        let job_id = job_id.as_ref();
        let store = self.store.read().await;

        let job = store.jobs.iter().find(|j| j.id == job_id);

        if let Some(job) = job {
            if !force && !job.enabled {
                return Ok(false);
            }

            let job = job.clone();
            drop(store);

            self.execute_job(&job).await;
            self.save_store().await?;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get service status
    pub async fn status(&self) -> serde_json::Value {
        let store = self.store.read().await;
        let running = *self.running.read().await;
        let next_wake_ms = self.get_next_wake_ms().await;

        serde_json::json!({
            "enabled": running,
            "jobs": store.jobs.len(),
            "nextWakeAtMs": next_wake_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::CronError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_cron_service() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let service = CronService::new(&store_path);

        assert_eq!(service.store_path, store_path);
        assert!(!*service.running.read().await);
    }

    #[tokio::test]
    async fn test_add_job_every_schedule() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let service = CronService::new(&store_path);

        let schedule = CronSchedule::Every { every_ms: 60000 };
        let job = service
            .add_job("test", schedule, "Test message", false, None, None, false)
            .await
            .unwrap();

        assert_eq!(job.name, "test");
        assert_eq!(job.payload.message, "Test message");
        assert!(!job.payload.deliver);
        assert!(job.state.next_run_at_ms.is_some());

        // Verify it was saved
        let jobs = service.list_jobs(true).await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].name, "test");
    }

    #[tokio::test]
    async fn test_add_job_at_schedule() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let service = CronService::new(&store_path);

        let future_time = now_ms() + 3600000; // 1 hour from now
        let schedule = CronSchedule::At { at_ms: future_time };
        let job = service
            .add_job("once", schedule, "One-time task", false, None, None, false)
            .await
            .unwrap();

        assert_eq!(job.name, "once");
        assert_eq!(job.state.next_run_at_ms, Some(future_time));
    }

    #[tokio::test]
    async fn test_add_job_cron_schedule() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let service = CronService::new(&store_path);

        let schedule = CronSchedule::Cron {
            expr: "0 0 0 * * *".to_string(), // Daily at midnight (sec min hour day month dow)
            tz: None,
        };
        let job = service
            .add_job("daily", schedule, "Daily task", false, None, None, false)
            .await
            .unwrap();

        assert_eq!(job.name, "daily");
        assert!(job.state.next_run_at_ms.is_some());
    }

    #[tokio::test]
    async fn test_add_job_with_delivery() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let service = CronService::new(&store_path);

        let schedule = CronSchedule::Every { every_ms: 60000 };
        let job = service
            .add_job(
                "notify",
                schedule,
                "Notification",
                true,
                Some("telegram".to_string()),
                Some("chat123".to_string()),
                false,
            )
            .await
            .unwrap();

        assert!(job.payload.deliver);
        assert_eq!(job.payload.channel, Some("telegram".to_string()));
        assert_eq!(job.payload.to, Some("chat123".to_string()));
    }

    #[tokio::test]
    async fn test_remove_job() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let service = CronService::new(&store_path);

        let schedule = CronSchedule::Every { every_ms: 60000 };
        let job = service
            .add_job("test", schedule, "Test", false, None, None, false)
            .await
            .unwrap();

        let job_id = job.id.clone();

        // Verify job exists
        let jobs = service.list_jobs(true).await;
        assert_eq!(jobs.len(), 1);

        // Remove job
        let removed = service.remove_job(&job_id).await.unwrap();
        assert!(removed);

        // Verify job is gone
        let jobs = service.list_jobs(true).await;
        assert_eq!(jobs.len(), 0);

        // Try to remove again
        let removed = service.remove_job(&job_id).await.unwrap();
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_enable_disable_job() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let service = CronService::new(&store_path);

        let schedule = CronSchedule::Every { every_ms: 60000 };
        let job = service
            .add_job("test", schedule, "Test", false, None, None, false)
            .await
            .unwrap();

        let job_id = job.id.clone();

        // Disable job
        let result = service.enable_job(&job_id, false).await.unwrap();
        assert!(result.is_some());
        assert!(!result.unwrap().enabled);

        // Verify it's disabled
        let jobs = service.list_jobs(false).await;
        assert_eq!(jobs.len(), 0); // Should not appear in enabled-only list

        let jobs = service.list_jobs(true).await;
        assert_eq!(jobs.len(), 1);
        assert!(!jobs[0].enabled);
        assert!(jobs[0].state.next_run_at_ms.is_none()); // Next run cleared

        // Enable job
        let result = service.enable_job(&job_id, true).await.unwrap();
        assert!(result.is_some());
        assert!(result.unwrap().enabled);

        let jobs = service.list_jobs(false).await;
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].state.next_run_at_ms.is_some()); // Next run computed
    }

    #[tokio::test]
    async fn test_list_jobs_filtering() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let service = CronService::new(&store_path);

        let schedule = CronSchedule::Every { every_ms: 60000 };

        // Add enabled job
        service
            .add_job(
                "enabled",
                schedule.clone(),
                "Test",
                false,
                None,
                None,
                false,
            )
            .await
            .unwrap();

        // Add disabled job
        let job2 = service
            .add_job("disabled", schedule, "Test", false, None, None, false)
            .await
            .unwrap();
        service.enable_job(&job2.id, false).await.unwrap();

        // List all jobs
        let all_jobs = service.list_jobs(true).await;
        assert_eq!(all_jobs.len(), 2);

        // List only enabled
        let enabled_jobs = service.list_jobs(false).await;
        assert_eq!(enabled_jobs.len(), 1);
        assert_eq!(enabled_jobs[0].name, "enabled");
    }

    #[tokio::test]
    async fn test_job_execution_callback() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let mut service = CronService::new(&store_path);

        // Track callback invocations
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        service.set_callback(Arc::new(move |job| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            assert_eq!(job.name, "test");
            Ok(Some("completed".to_string()))
        }));

        let schedule = CronSchedule::Every { every_ms: 60000 };
        let job = service
            .add_job("test", schedule, "Test", false, None, None, false)
            .await
            .unwrap();

        // Manually run the job
        let ran = service.run_job(&job.id, false).await.unwrap();
        assert!(ran);

        // Verify callback was called
        assert_eq!(call_count.load(Ordering::SeqCst), 1);

        // Verify job state was updated
        let jobs = service.list_jobs(true).await;
        assert_eq!(jobs.len(), 1);
        assert!(jobs[0].state.last_run_at_ms.is_some());
        assert_eq!(jobs[0].state.last_status, Some("ok".to_string()));
    }

    #[tokio::test]
    async fn test_job_execution_error_handling() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let mut service = CronService::new(&store_path);

        service.set_callback(Arc::new(move |_job| {
            Err(CronError::ExecutionFailed("Test error".to_string()).into())
        }));

        let schedule = CronSchedule::Every { every_ms: 60000 };
        let job = service
            .add_job("test", schedule, "Test", false, None, None, false)
            .await
            .unwrap();

        // Run the job
        service.run_job(&job.id, false).await.unwrap();

        // Verify error was recorded
        let jobs = service.list_jobs(true).await;
        assert_eq!(jobs[0].state.last_status, Some("error".to_string()));
        assert!(jobs[0].state.last_error.is_some());
    }

    #[tokio::test]
    async fn test_one_shot_job_deletes_after_run() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let mut service = CronService::new(&store_path);

        service.set_callback(Arc::new(move |_job| Ok(None)));

        let future_time = now_ms() + 100;
        let schedule = CronSchedule::At { at_ms: future_time };
        let job = service
            .add_job("once", schedule, "One-time", false, None, None, true)
            .await
            .unwrap();

        assert!(job.delete_after_run);

        // Run the job
        service.run_job(&job.id, true).await.unwrap();

        // Verify it was deleted
        let jobs = service.list_jobs(true).await;
        assert_eq!(jobs.len(), 0);
    }

    #[tokio::test]
    async fn test_one_shot_job_disables_without_delete() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let mut service = CronService::new(&store_path);

        service.set_callback(Arc::new(move |_job| Ok(None)));

        let future_time = now_ms() + 100;
        let schedule = CronSchedule::At { at_ms: future_time };
        let job = service
            .add_job("once", schedule, "One-time", false, None, None, false)
            .await
            .unwrap();

        // Run the job
        service.run_job(&job.id, true).await.unwrap();

        // Verify it was disabled but not deleted
        let jobs = service.list_jobs(true).await;
        assert_eq!(jobs.len(), 1);
        assert!(!jobs[0].enabled);
        assert!(jobs[0].state.next_run_at_ms.is_none());
    }

    #[tokio::test]
    async fn test_recurring_job_computes_next_run() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let mut service = CronService::new(&store_path);

        service.set_callback(Arc::new(move |_job| Ok(None)));

        let schedule = CronSchedule::Every { every_ms: 60000 };
        let job = service
            .add_job("recurring", schedule, "Repeat", false, None, None, false)
            .await
            .unwrap();

        assert!(job.state.next_run_at_ms.is_some());

        // Run the job
        service.run_job(&job.id, false).await.unwrap();

        // Verify next run is still scheduled (recurring job stays active)
        let jobs = service.list_jobs(true).await;
        assert!(jobs[0].state.next_run_at_ms.is_some());
        assert!(jobs[0].enabled);
        // Next run should be in the future
        let next_run = jobs[0].state.next_run_at_ms.unwrap();
        assert!(next_run > 0);
    }

    #[tokio::test]
    async fn test_force_run_disabled_job() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let mut service = CronService::new(&store_path);

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();
        service.set_callback(Arc::new(move |_job| {
            call_count_clone.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }));

        let schedule = CronSchedule::Every { every_ms: 60000 };
        let job = service
            .add_job("test", schedule, "Test", false, None, None, false)
            .await
            .unwrap();

        // Disable the job
        service.enable_job(&job.id, false).await.unwrap();

        // Try to run without force
        let ran = service.run_job(&job.id, false).await.unwrap();
        assert!(!ran);
        assert_eq!(call_count.load(Ordering::SeqCst), 0);

        // Run with force
        let ran = service.run_job(&job.id, true).await.unwrap();
        assert!(ran);
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_persistence_across_restarts() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");

        // Create service and add jobs
        {
            let service = CronService::new(&store_path);
            let schedule = CronSchedule::Every { every_ms: 60000 };
            service
                .add_job("job1", schedule.clone(), "Test 1", false, None, None, false)
                .await
                .unwrap();
            service
                .add_job("job2", schedule, "Test 2", false, None, None, false)
                .await
                .unwrap();
        }

        // Create new service and load
        let service = CronService::new(&store_path);
        service.load_store().await.unwrap();
        let jobs = service.list_jobs(true).await;

        assert_eq!(jobs.len(), 2);
        assert!(jobs.iter().any(|j| j.name == "job1"));
        assert!(jobs.iter().any(|j| j.name == "job2"));
    }

    #[tokio::test]
    async fn test_service_status() {
        let temp_dir = TempDir::new().unwrap();
        let store_path = temp_dir.path().join("cron.json");
        let service = CronService::new(&store_path);

        let schedule = CronSchedule::Every { every_ms: 60000 };
        service
            .add_job("test", schedule, "Test", false, None, None, false)
            .await
            .unwrap();

        let status = service.status().await;
        assert_eq!(status["jobs"], 1);
        assert!(status["nextWakeAtMs"].is_number());
    }

    #[test]
    fn test_compute_next_run_at_schedule() {
        let now = now_ms();
        let future = now + 3600000; // 1 hour from now
        let schedule = CronSchedule::At { at_ms: future };

        let next = compute_next_run(&schedule, now);
        assert_eq!(next, Some(future));

        // Past time should return None
        let past = now - 3600000;
        let schedule = CronSchedule::At { at_ms: past };
        let next = compute_next_run(&schedule, now);
        assert_eq!(next, None);
    }

    #[test]
    fn test_compute_next_run_every_schedule() {
        let now = now_ms();
        let schedule = CronSchedule::Every { every_ms: 60000 };

        let next = compute_next_run(&schedule, now);
        assert!(next.is_some());
        assert!(next.unwrap() >= now);
        assert!(next.unwrap() <= now + 60000);
    }

    #[test]
    fn test_compute_next_run_cron_schedule() {
        let now = now_ms();
        let schedule = CronSchedule::Cron {
            expr: "0 0 0 * * *".to_string(), // Daily at midnight (sec min hour day month dow)
            tz: None,
        };

        let next = compute_next_run(&schedule, now);
        assert!(next.is_some());
        assert!(next.unwrap() > now);
    }

    #[test]
    fn test_compute_next_run_invalid_cron() {
        let now = now_ms();
        let schedule = CronSchedule::Cron {
            expr: "invalid cron".to_string(),
            tz: None,
        };

        let next = compute_next_run(&schedule, now);
        assert!(next.is_none());
    }
}
