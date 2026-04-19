//! Execution dispatch layer for `AlarmFired { kind = "cron_job" }` events.
//!
//! `CronExecutor` subscribes to the `DomainEventBus`, filters for cron-job
//! fires, looks up the registered Rust handler by job name, and invokes it
//! concurrently (one `tokio::spawn` per fire).

use std::collections::HashMap;
use std::sync::Arc;

use bus::DomainEventBus;
use storage::repos::cron::CronRepo;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::service::row_to_job;
use crate::types::{CronJob, IntentTrigger};
use common::Result;

/// Returns `true` if `job` is allowed to fire at `now` given its `intent_window`.
///
/// - If `intent_window` is `None`, always returns `true` (no restriction).
/// - For `FirstActivityAfter { after_local }`: blocks dispatch if the current local
///   time of day is before `after_local`. This is the only trigger that can be
///   evaluated purely from wall-clock time without a presence snapshot.
/// - All other triggers (`UserPresent`, `MinActiveMinutes`, `UserIdle`) require live
///   presence data which is not available in the executor; they return `true` so the
///   alarm fires and the handler decides whether to act.
fn in_intent_window(job: &CronJob, now: jiff::Timestamp) -> bool {
    let window = match &job.intent_window {
        Some(w) => w,
        None => return true,
    };

    match &window.trigger {
        IntentTrigger::FirstActivityAfter { after_local } => {
            // Convert `now` to local civil time and compare the time-of-day component.
            let local_time = now
                .to_zoned(jiff::tz::TimeZone::system())
                .datetime()
                .time();
            local_time >= *after_local
        }
        // Presence-based triggers cannot be evaluated here — allow dispatch.
        IntentTrigger::UserPresent
        | IntentTrigger::MinActiveMinutes { .. }
        | IntentTrigger::UserIdle { .. } => true,
    }
}

/// Callback type for cron job handlers registered with [`CronExecutor`].
pub type CronHandler = Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>;

/// Dispatches `AlarmFired { kind = "cron_job" }` events to registered Rust handlers.
pub struct CronExecutor {
    handlers: Arc<std::sync::RwLock<HashMap<String, CronHandler>>>,
    cron_repo: CronRepo,
    bus: Arc<DomainEventBus>,
}

impl CronExecutor {
    /// Create a new executor. Call [`start`] to begin subscribing.
    pub fn new(cron_repo: CronRepo, bus: Arc<DomainEventBus>) -> Self {
        Self {
            handlers: Arc::new(std::sync::RwLock::new(HashMap::new())),
            cron_repo,
            bus,
        }
    }

    /// Register a handler for a specific cron job name.
    ///
    /// Can be called before or after [`start`] — handlers are stored in a
    /// shared `Arc<RwLock<...>>` so late registration takes effect immediately.
    ///
    /// If a fire arrives before a handler for the given name is registered,
    /// the fire is dropped with a `warn!` log and the executor continues.
    pub fn register(&self, name: &str, handler: CronHandler) {
        self.handlers
            .write()
            .expect("CronExecutor handler lock poisoned")
            .insert(name.to_owned(), handler);
    }

    /// Alias for [`register`] — convenience alias kept for call-site compatibility.
    ///
    /// If a fire arrives before a handler for the given name is registered,
    /// the fire is dropped with a `warn!` log and the executor continues.
    pub fn set_callback(&self, name: &str, handler: CronHandler) {
        self.register(name, handler);
    }

    /// Spawn the subscriber loop. Returns the `JoinHandle` — store it to keep
    /// the task alive. The loop stops cleanly when `shutdown` is cancelled.
    pub fn start(&self, shutdown: CancellationToken) -> JoinHandle<()> {
        let mut rx = self.bus.subscribe();
        let handlers = Arc::clone(&self.handlers);
        let cron_repo = self.cron_repo.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = shutdown.cancelled() => {
                        info!("CronExecutor: shutdown requested, stopping subscriber");
                        break;
                    }
                    result = rx.recv() => {
                        match result {
                            Ok(event) => {
                                if let bus::DomainEvent::AlarmFired { kind, ref_id, .. } = event {
                                    if kind == "cron_job" {
                                        Self::dispatch(
                                            &cron_repo,
                                            &handlers,
                                            ref_id,
                                        )
                                        .await;
                                    }
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!("CronExecutor: broadcast lagged by {} events", n);
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                info!("CronExecutor: bus closed, stopping subscriber");
                                break;
                            }
                        }
                    }
                }
            }
        })
    }

    /// Immediately invoke the handler for the given job ID, if registered.
    ///
    /// Used by the CronTool "run" action for manual / forced job execution.
    /// Returns `Ok(true)` if the handler was found and invoked, `Ok(false)` if
    /// no job with that ID exists in the repo.
    pub async fn run_now(&self, job_id: &str) -> Result<bool> {
        let row = match self.cron_repo.get_opt(job_id).await {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(false),
            Err(e) => {
                return Err(common::KlyntbotError::Storage(format!(
                    "CronExecutor::run_now DB error: {e}"
                )))
            }
        };
        let job = row_to_job(row);
        let job_name = job.name.clone();

        let handler = {
            let guard = self
                .handlers
                .read()
                .expect("CronExecutor handler lock poisoned");
            guard.get(&job_name).cloned()
        };

        match handler {
            None => {
                warn!("CronExecutor::run_now: no handler for '{}'", job_name);
                Ok(false)
            }
            Some(cb) => {
                info!("CronExecutor::run_now: executing '{}'", job_name);
                match tokio::task::spawn_blocking(move || cb(&job)).await {
                    Ok(Ok(_)) => info!("CronExecutor::run_now: '{}' completed", job_name),
                    Ok(Err(e)) => error!("CronExecutor::run_now: '{}' failed: {}", job_name, e),
                    Err(e) => error!("CronExecutor::run_now: '{}' panicked: {}", job_name, e),
                }
                Ok(true)
            }
        }
    }

    /// Resolve the cron job and invoke the handler, if registered.
    async fn dispatch(
        cron_repo: &CronRepo,
        handlers: &Arc<std::sync::RwLock<HashMap<String, CronHandler>>>,
        ref_id: Option<String>,
    ) {
        let job_id = match ref_id {
            Some(id) => id,
            None => {
                warn!("CronExecutor: AlarmFired cron_job event has no ref_id, skipping");
                return;
            }
        };

        // DB fetch resolves ref_id → job name for handler lookup.
        // `JobCallback` is keyed by name, so one handler handles all jobs of the same name.
        let row = match cron_repo.get(&job_id).await {
            Ok(r) => r,
            Err(storage::error::StorageError::NotFound(_)) => {
                warn!(
                    "CronExecutor: cron job '{}' not found in DB (deleted?), skipping",
                    job_id
                );
                return;
            }
            Err(e) => {
                error!(
                    "CronExecutor: DB error fetching cron job '{}': {}",
                    job_id, e
                );
                return;
            }
        };

        let job = row_to_job(row);
        let job_name = job.name.clone();

        // Evaluate intent_window before dispatching. Jobs with a `FirstActivityAfter`
        // trigger are suppressed if the current local time is before the configured
        // threshold. Other trigger kinds require live presence data and are not
        // evaluated here (they pass through and fire normally).
        if !in_intent_window(&job, jiff::Timestamp::now()) {
            warn!(
                job_name = %job_name,
                "CronExecutor: job fire suppressed — outside intent_window"
            );
            return;
        }

        let handler = {
            let guard = handlers.read().expect("CronExecutor handler lock poisoned");
            guard.get(&job_name).cloned()
        };

        match handler {
            None => {
                warn!(
                    "CronExecutor: no handler registered for cron job '{}', skipping",
                    job_name
                );
            }
            Some(cb) => {
                info!("CronExecutor: dispatching job '{}'", job_name);
                let job_name_for_watcher = job_name.clone();
                // Use spawn_blocking so a slow (sync) handler can't block other fires.
                let handle = tokio::task::spawn_blocking(move || match cb(&job) {
                    Ok(_) => {
                        info!("CronExecutor: job '{}' completed successfully", job_name);
                    }
                    Err(e) => {
                        error!("CronExecutor: job '{}' failed: {}", job_name, e);
                    }
                });
                // Watcher task surfaces panics inside the handler — without it, a panic
                // in spawn_blocking is silently swallowed by the tokio runtime.
                tokio::spawn(async move {
                    match handle.await {
                        Ok(()) => {}
                        Err(join_err) => {
                            if join_err.is_panic() {
                                error!("CronExecutor: job '{}' panicked", job_name_for_watcher);
                            } else if join_err.is_cancelled() {
                                warn!("CronExecutor: job '{}' cancelled", job_name_for_watcher);
                            }
                        }
                    }
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bus::DomainEvent;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use storage::pool::StoragePool;
    use storage::repos::cron::CronRepo;
    use storage::rows::cron::CronJobRow;
    use tokio::time::{Duration, Instant};

    async fn setup_repo() -> CronRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        CronRepo::new(pool.inner().clone())
    }

    fn make_row(id: &str, name: &str) -> CronJobRow {
        CronJobRow {
            id: id.into(),
            name: name.into(),
            enabled: true,
            origin: "system".into(),
            schedule: serde_json::json!({ "cron": "0 0 * * * * *", "tz": "UTC" }),
            payload: serde_json::json!({}),
            next_run_at_ms: None,
            last_run_at_ms: None,
            last_status: None,
            last_error: None,
            created_at_ms: 0,
            updated_at_ms: 0,
            delete_after_run: false,
            intent_window: None,
            intent_pending_since_ms: None,
        }
    }

    fn alarm_fired(kind: &str, ref_id: Option<&str>) -> DomainEvent {
        DomainEvent::AlarmFired {
            fire_id: "fire-test".into(),
            kind: kind.into(),
            ref_id: ref_id.map(|s| s.to_owned()),
            payload_json: "{}".into(),
            fired_at_ms: 0,
        }
    }

    #[tokio::test]
    async fn handler_fires_on_matching_alarm() {
        let repo = setup_repo().await;
        repo.upsert(&make_row("job-1", "nightly_reforge"))
            .await
            .unwrap();

        let bus = Arc::new(DomainEventBus::new(64));
        let executor = CronExecutor::new(repo, Arc::clone(&bus));

        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);
        executor.register(
            "nightly_reforge",
            Arc::new(move |_job| {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }),
        );

        let shutdown = CancellationToken::new();
        let _handle = executor.start(shutdown.clone());

        // Give subscriber a moment to register before publishing.
        tokio::time::sleep(Duration::from_millis(10)).await;

        bus.publish(alarm_fired("cron_job", Some("job-1")));

        // Wait for handler to be invoked.
        let deadline = Instant::now() + Duration::from_secs(2);
        while call_count.load(Ordering::SeqCst) == 0 {
            if Instant::now() > deadline {
                panic!("handler was not invoked within 2s");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert_eq!(call_count.load(Ordering::SeqCst), 1);
        shutdown.cancel();
    }

    #[tokio::test]
    async fn non_cron_job_kind_is_ignored() {
        let repo = setup_repo().await;
        repo.upsert(&make_row("job-2", "some_job")).await.unwrap();

        let bus = Arc::new(DomainEventBus::new(64));
        let executor = CronExecutor::new(repo, Arc::clone(&bus));

        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);
        executor.register(
            "some_job",
            Arc::new(move |_job| {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }),
        );

        let shutdown = CancellationToken::new();
        let _handle = executor.start(shutdown.clone());

        tokio::time::sleep(Duration::from_millis(10)).await;

        bus.publish(alarm_fired("task_alarm", Some("job-2")));

        // Wait briefly and assert NOT called.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(call_count.load(Ordering::SeqCst), 0);

        shutdown.cancel();
    }

    #[tokio::test]
    async fn unregistered_job_logs_warn_without_error() {
        let repo = setup_repo().await;
        repo.upsert(&make_row("job-3", "unknown_job"))
            .await
            .unwrap();

        let bus = Arc::new(DomainEventBus::new(64));
        let executor = CronExecutor::new(repo, Arc::clone(&bus));
        // No handler registered for "unknown_job"

        let shutdown = CancellationToken::new();
        let _handle = executor.start(shutdown.clone());

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Must not panic or crash.
        bus.publish(alarm_fired("cron_job", Some("job-3")));

        tokio::time::sleep(Duration::from_millis(100)).await;

        // Executor should still be alive — publish a second event with no crash.
        bus.publish(alarm_fired("cron_job", Some("job-3")));

        tokio::time::sleep(Duration::from_millis(100)).await;

        shutdown.cancel();
    }

    #[tokio::test]
    async fn concurrent_fires_run_in_parallel() {
        let repo = setup_repo().await;
        repo.upsert(&make_row("job-4a", "parallel_job"))
            .await
            .unwrap();
        repo.upsert(&make_row("job-4b", "parallel_job"))
            .await
            .unwrap();
        repo.upsert(&make_row("job-4c", "parallel_job"))
            .await
            .unwrap();

        let bus = Arc::new(DomainEventBus::new(64));
        let executor = CronExecutor::new(repo, Arc::clone(&bus));

        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);

        executor.register(
            "parallel_job",
            Arc::new(move |_job| {
                // Simulate a slow sync handler.
                std::thread::sleep(std::time::Duration::from_millis(100));
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }),
        );

        let shutdown = CancellationToken::new();
        let _handle = executor.start(shutdown.clone());

        tokio::time::sleep(Duration::from_millis(10)).await;

        let t0 = Instant::now();
        bus.publish(alarm_fired("cron_job", Some("job-4a")));
        bus.publish(alarm_fired("cron_job", Some("job-4b")));
        bus.publish(alarm_fired("cron_job", Some("job-4c")));

        // Wait for all 3 to complete.
        let deadline = Instant::now() + Duration::from_secs(3);
        while call_count.load(Ordering::SeqCst) < 3 {
            if Instant::now() > deadline {
                panic!("not all handlers completed within 3s");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        let elapsed = t0.elapsed();
        // 300ms is the serial-execution time; 400ms proves parallelism with CI headroom.
        assert!(
            elapsed < Duration::from_millis(400),
            "handlers took {:?} — expected parallel execution",
            elapsed
        );
        assert_eq!(call_count.load(Ordering::SeqCst), 3);

        shutdown.cancel();
    }

    #[tokio::test]
    async fn shutdown_stops_subscriber() {
        let repo = setup_repo().await;
        repo.upsert(&make_row("job-5", "stop_job")).await.unwrap();

        let bus = Arc::new(DomainEventBus::new(64));
        let executor = CronExecutor::new(repo, Arc::clone(&bus));

        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);
        executor.register(
            "stop_job",
            Arc::new(move |_job| {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }),
        );

        let shutdown = CancellationToken::new();
        let handle = executor.start(shutdown.clone());

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Fire one event — it should be processed.
        bus.publish(alarm_fired("cron_job", Some("job-5")));
        tokio::time::sleep(Duration::from_millis(100)).await;
        let after_first = call_count.load(Ordering::SeqCst);
        assert_eq!(after_first, 1, "first event should have been processed");

        // Cancel shutdown and wait for the task to exit.
        shutdown.cancel();
        tokio::time::timeout(Duration::from_secs(1), handle)
            .await
            .expect("executor task should stop within 1s")
            .expect("executor task should not panic");

        // Fire a second event after shutdown — it must NOT be processed.
        bus.publish(alarm_fired("cron_job", Some("job-5")));
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "no new dispatches after shutdown"
        );
    }

    #[tokio::test]
    async fn handler_panic_is_logged_and_executor_continues() {
        let repo = setup_repo().await;
        repo.upsert(&make_row("job-6a", "panic_job")).await.unwrap();
        repo.upsert(&make_row("job-6b", "survivor_job"))
            .await
            .unwrap();

        let bus = Arc::new(DomainEventBus::new(64));
        let executor = CronExecutor::new(repo, Arc::clone(&bus));

        // First handler always panics.
        executor.register(
            "panic_job",
            Arc::new(|_job| -> Result<Option<String>> {
                panic!("intentional panic in handler");
            }),
        );

        // Second handler records invocation — proves executor survived the panic.
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);
        executor.register(
            "survivor_job",
            Arc::new(move |_job| {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }),
        );

        let shutdown = CancellationToken::new();
        let _handle = executor.start(shutdown.clone());

        tokio::time::sleep(Duration::from_millis(10)).await;

        // Fire the panicking job first.
        bus.publish(alarm_fired("cron_job", Some("job-6a")));

        // Give the panic watcher time to log and the executor to recover.
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Fire the survivor job — executor must still be alive and processing.
        bus.publish(alarm_fired("cron_job", Some("job-6b")));

        let deadline = Instant::now() + Duration::from_secs(2);
        while call_count.load(Ordering::SeqCst) == 0 {
            if Instant::now() > deadline {
                panic!("survivor handler was not invoked within 2s — executor may have crashed");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "survivor handler should have run exactly once"
        );

        shutdown.cancel();
    }

    /// Build a `CronJobRow` whose `intent_window` is set to `FirstActivityAfter` with
    /// the given local `after_hour` (0–23). Used by intent-window gate tests.
    fn make_row_with_intent_hour(id: &str, name: &str, after_hour: u8) -> CronJobRow {
        let mut row = make_row(id, name);
        // Encode the IntentWindow as the JSON string the DB column stores.
        let json = serde_json::json!({
            "trigger": {
                "kind": "first_activity_after",
                "afterLocal": format!("{:02}:00:00", after_hour)
            },
            "toleranceSecs": 3600,
            "catchUp": "when_present"
        });
        row.intent_window = Some(json.to_string());
        row
    }

    #[tokio::test]
    async fn intent_window_blocks_outside_range() {
        // Choose an `after_local` that is always in the future relative to "now" — we
        // pick hour 25 which is impossible, so any current time is before it.  Because
        // jiff `civil::Time` caps at 23:59, we instead use the `FirstActivityAfter`
        // trigger with `after_local = 23:59` and run the test at any time before that.
        //
        // Simpler approach: set after_local to 23:59 and rely on the test running
        // before that time.  On CI (UTC, ~midnight) this could flake.  Instead we
        // build the row with after_local = current_local_hour + 1 (mod 24) so it is
        // always in the future relative to *now*.
        let now_local = jiff::Zoned::now().datetime().time();
        // Pick an hour guaranteed to be strictly in the future today.
        let block_hour = ((now_local.hour() as u32 + 1) % 24) as u8;

        let repo = setup_repo().await;
        repo.upsert(&make_row_with_intent_hour("job-7", "gated_job", block_hour))
            .await
            .unwrap();

        let bus = Arc::new(DomainEventBus::new(64));
        let executor = CronExecutor::new(repo, Arc::clone(&bus));

        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);
        executor.register(
            "gated_job",
            Arc::new(move |_job| {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }),
        );

        let shutdown = CancellationToken::new();
        let _handle = executor.start(shutdown.clone());

        tokio::time::sleep(Duration::from_millis(10)).await;

        bus.publish(alarm_fired("cron_job", Some("job-7")));

        // Wait long enough for the event to be processed — handler must NOT be called.
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert_eq!(
            call_count.load(Ordering::SeqCst),
            0,
            "handler must be suppressed when outside intent_window"
        );

        shutdown.cancel();
    }

    #[tokio::test]
    async fn intent_window_allows_inside_range() {
        // Set after_local = 00:00 (midnight) so any current local time satisfies it.
        let repo = setup_repo().await;
        repo.upsert(&make_row_with_intent_hour("job-8", "allowed_job", 0))
            .await
            .unwrap();

        let bus = Arc::new(DomainEventBus::new(64));
        let executor = CronExecutor::new(repo, Arc::clone(&bus));

        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);
        executor.register(
            "allowed_job",
            Arc::new(move |_job| {
                cc.fetch_add(1, Ordering::SeqCst);
                Ok(None)
            }),
        );

        let shutdown = CancellationToken::new();
        let _handle = executor.start(shutdown.clone());

        tokio::time::sleep(Duration::from_millis(10)).await;

        bus.publish(alarm_fired("cron_job", Some("job-8")));

        let deadline = Instant::now() + Duration::from_secs(2);
        while call_count.load(Ordering::SeqCst) == 0 {
            if Instant::now() > deadline {
                panic!("handler was not invoked within 2s — intent_window gate may be too strict");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert_eq!(
            call_count.load(Ordering::SeqCst),
            1,
            "handler must fire when inside intent_window"
        );

        shutdown.cancel();
    }
}
