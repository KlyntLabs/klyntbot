//! Cron service for scheduling agent tasks.
//!
//! Split into submodules:
//! - `executor`: Job execution and state updates
//! - `store`: Persistence (SQL-only via CronRepo)

mod executor;
mod store;

use std::sync::Arc;

use chrono::Utc;
use chrono_tz::Tz;
use tokio::sync::{Notify, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant};
use tracing::info;
use uuid::Uuid;

use crate::types::{CronJob, CronJobState, CronSchedule, CronStore};
use common::Result;
use storage::CronJobRow;

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
        CronSchedule::Cron { expr, tz } => {
            // Use cron crate to compute next run, optionally in the specified timezone.
            let cron_schedule = match cron::Schedule::try_from(expr.as_str()) {
                Ok(s) => s,
                Err(_) => return None,
            };

            match tz.as_deref() {
                None => {
                    // No timezone specified — compute directly in UTC.
                    cron_schedule
                        .upcoming(Utc)
                        .next()
                        .map(|dt| dt.timestamp_millis())
                }
                Some(tz_str) => {
                    // Parse the timezone string; fall back to UTC with a warning on failure.
                    match tz_str.parse::<Tz>() {
                        Ok(tz) => cron_schedule
                            .upcoming(tz)
                            .next()
                            .map(|dt| dt.timestamp_millis()),
                        Err(_) => {
                            tracing::warn!(
                                "CronSchedule: unrecognised timezone {:?}, falling back to UTC",
                                tz_str
                            );
                            cron_schedule
                                .upcoming(Utc)
                                .next()
                                .map(|dt| dt.timestamp_millis())
                        }
                    }
                }
            }
        }
    }
}

/// Callback type for job execution
pub type JobCallback = Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>;

/// Service for managing and executing scheduled jobs (SQL-only via CronRepo).
pub struct CronService {
    pub(crate) store: Arc<RwLock<CronStore>>,
    pub(crate) on_job: Option<JobCallback>,
    pub(crate) running: Arc<RwLock<bool>>,
    pub(crate) timer_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// SQL backend for persistence. Always `Some` in production; `None` only in
    /// unit tests where save/load become no-ops (in-memory only).
    pub(crate) repo: Option<storage::CronRepo>,
    /// Signals the timer loop to re-evaluate when jobs are added/modified/removed.
    pub(crate) wake: Arc<Notify>,
}

impl CronService {
    /// Create a new cron service backed by a SQL CronRepo.
    pub fn new(repo: storage::CronRepo) -> Self {
        Self {
            store: Arc::new(RwLock::new(CronStore::default())),
            on_job: None,
            running: Arc::new(RwLock::new(false)),
            timer_task: Arc::new(RwLock::new(None)),
            repo: Some(repo),
            wake: Arc::new(Notify::new()),
        }
    }

    /// Test-only constructor: in-memory store with no SQL persistence.
    #[cfg(test)]
    fn new_for_test() -> Self {
        Self {
            store: Arc::new(RwLock::new(CronStore::default())),
            on_job: None,
            running: Arc::new(RwLock::new(false)),
            timer_task: Arc::new(RwLock::new(None)),
            repo: None,
            wake: Arc::new(Notify::new()),
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
        Self::next_wake_ms_static(&self.store).await
    }

    /// Compute the earliest next run time from a store handle (avoids &self).
    async fn next_wake_ms_static(store: &Arc<RwLock<CronStore>>) -> Option<i64> {
        let store = store.read().await;
        store
            .jobs
            .iter()
            .filter(|j| j.enabled && j.state.next_run_at_ms.is_some())
            .filter_map(|j| j.state.next_run_at_ms)
            .min()
    }

    /// Start the continuous timer loop.
    ///
    /// Sleeps until the exact next-job deadline using `tokio::time::sleep_until`,
    /// and wakes early via `Notify` when jobs are added, modified, or removed.
    async fn start_timer_loop(&self) {
        let store = self.store.clone();
        let repo = self.repo.clone();
        let on_job = self.on_job.clone();
        let running = self.running.clone();
        let wake = self.wake.clone();

        let task = tokio::spawn(async move {
            loop {
                // Check if still running
                if !*running.read().await {
                    break;
                }

                // Get next wake time
                let next_wake_ms = CronService::next_wake_ms_static(&store).await;

                let sleep_duration = match next_wake_ms {
                    Some(next_wake) => {
                        let delay_ms = (next_wake - now_ms()).max(0);
                        Duration::from_millis(delay_ms as u64)
                    }
                    // No jobs scheduled — sleep until woken by a Notify
                    None => Duration::from_secs(86400),
                };

                let deadline = Instant::now() + sleep_duration;

                // Sleep until deadline OR early wake from Notify
                tokio::select! {
                    _ = tokio::time::sleep_until(deadline) => {}
                    _ = wake.notified() => {}
                }

                // Check if still running after wake
                if !*running.read().await {
                    break;
                }

                // Process any due jobs
                let next_wake_ms = CronService::next_wake_ms_static(&store).await;
                if let Some(next_wake) = next_wake_ms {
                    if now_ms() >= next_wake {
                        CronService::process_due_jobs(&store, &repo, &on_job).await;
                    }
                }
            }
        });

        // Store the task handle synchronously to avoid race with stop()
        let mut timer_task = self.timer_task.write().await;
        *timer_task = Some(task);
    }

    // ========== Public API ==========

    /// Start the cron service
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;

        self.load_store().await?;
        self.recompute_next_runs().await;
        self.save_store().await?;
        self.start_timer_loop().await;

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
        self.wake.notify_one();

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
            self.wake.notify_one();
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
            self.wake.notify_one();

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

    /// Convert a domain CronJob to a CronJobRow for SQL storage.
    fn job_to_row(job: &CronJob) -> CronJobRow {
        CronJobRow {
            id: job.id.clone(),
            name: job.name.clone(),
            enabled: job.enabled,
            schedule: serde_json::to_value(&job.schedule).unwrap_or_default(),
            payload: serde_json::to_value(&job.payload).unwrap_or_default(),
            next_run_at_ms: job.state.next_run_at_ms,
            last_run_at_ms: job.state.last_run_at_ms,
            last_status: job.state.last_status.clone(),
            last_error: job.state.last_error.clone(),
            created_at_ms: job.created_at_ms,
            updated_at_ms: job.updated_at_ms,
            delete_after_run: job.delete_after_run,
        }
    }

    /// Convert a CronJobRow from SQL back to a domain CronJob.
    fn row_to_job(row: CronJobRow) -> CronJob {
        let schedule =
            serde_json::from_value(row.schedule).unwrap_or(CronSchedule::Every { every_ms: 0 });
        let payload = serde_json::from_value(row.payload).unwrap_or_default();
        CronJob {
            id: row.id,
            name: row.name,
            enabled: row.enabled,
            schedule,
            payload,
            state: CronJobState {
                next_run_at_ms: row.next_run_at_ms,
                last_run_at_ms: row.last_run_at_ms,
                last_status: row.last_status,
                last_error: row.last_error,
            },
            created_at_ms: row.created_at_ms,
            updated_at_ms: row.updated_at_ms,
            delete_after_run: row.delete_after_run,
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

    #[tokio::test]
    async fn test_create_cron_service() {
        let service = CronService::new_for_test();
        assert!(!*service.running.read().await);
    }

    #[tokio::test]
    async fn test_add_job_every_schedule() {
        let service = CronService::new_for_test();

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
        let service = CronService::new_for_test();

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
        let service = CronService::new_for_test();

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
        let service = CronService::new_for_test();

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
        let service = CronService::new_for_test();

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
        let service = CronService::new_for_test();

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
        let service = CronService::new_for_test();

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
        let mut service = CronService::new_for_test();

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
        let mut service = CronService::new_for_test();

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
        let mut service = CronService::new_for_test();

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
        let mut service = CronService::new_for_test();

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
        let mut service = CronService::new_for_test();

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
        let mut service = CronService::new_for_test();

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
    async fn test_service_status() {
        let service = CronService::new_for_test();

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

    // ---- Timezone-aware cron tests ----

    #[test]
    fn test_compute_next_run_cron_utc_explicit() {
        let now = now_ms();
        let schedule = CronSchedule::Cron {
            expr: "0 0 0 * * *".to_string(),
            tz: Some("UTC".to_string()),
        };

        let next = compute_next_run(&schedule, now);
        assert!(next.is_some());
        assert!(next.unwrap() > now);
    }

    #[test]
    fn test_compute_next_run_cron_named_timezone() {
        let now = now_ms();
        let schedule = CronSchedule::Cron {
            expr: "0 0 9 * * *".to_string(), // 09:00 daily
            tz: Some("America/New_York".to_string()),
        };

        let next = compute_next_run(&schedule, now);
        // Should produce a valid future timestamp
        assert!(next.is_some());
        assert!(next.unwrap() > now);
    }

    #[test]
    fn test_compute_next_run_cron_timezone_differs_from_utc() {
        // Compute next run for the same hour spec in two different timezones.
        // The results must differ because the timezones have different UTC offsets.
        let now = now_ms();
        let expr = "0 0 12 * * *".to_string(); // Noon

        let utc_schedule = CronSchedule::Cron {
            expr: expr.clone(),
            tz: Some("UTC".to_string()),
        };
        let tokyo_schedule = CronSchedule::Cron {
            expr,
            tz: Some("Asia/Tokyo".to_string()), // UTC+9
        };

        let utc_next = compute_next_run(&utc_schedule, now).unwrap();
        let tokyo_next = compute_next_run(&tokyo_schedule, now).unwrap();

        // The two timestamps should differ by UTC offset (9 hours = 32_400_000 ms).
        // We just assert they are not equal; both must be in the future.
        assert!(utc_next > now);
        assert!(tokyo_next > now);
        assert_ne!(utc_next, tokyo_next);
    }

    #[test]
    fn test_compute_next_run_cron_invalid_timezone_falls_back_to_utc() {
        let now = now_ms();
        let schedule_invalid_tz = CronSchedule::Cron {
            expr: "0 0 0 * * *".to_string(),
            tz: Some("Not/A/Timezone".to_string()),
        };
        let schedule_utc = CronSchedule::Cron {
            expr: "0 0 0 * * *".to_string(),
            tz: None,
        };

        let next_invalid = compute_next_run(&schedule_invalid_tz, now);
        let next_utc = compute_next_run(&schedule_utc, now);

        // Both should return Some (fallback to UTC on invalid tz)
        assert!(next_invalid.is_some());
        assert!(next_utc.is_some());
        // Results should be equal since both end up using UTC
        assert_eq!(next_invalid, next_utc);
    }

    #[test]
    fn test_compute_next_run_cron_none_timezone_same_as_utc() {
        let now = now_ms();
        let schedule_none = CronSchedule::Cron {
            expr: "0 0 6 * * *".to_string(),
            tz: None,
        };
        let schedule_utc = CronSchedule::Cron {
            expr: "0 0 6 * * *".to_string(),
            tz: Some("UTC".to_string()),
        };

        let next_none = compute_next_run(&schedule_none, now);
        let next_utc = compute_next_run(&schedule_utc, now);

        assert_eq!(next_none, next_utc);
    }
}
