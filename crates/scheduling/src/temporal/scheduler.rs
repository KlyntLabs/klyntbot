//! Unified wall-clock-anchored scheduler.
//!
//! Design:
//! - Single tokio loop; sleeps at most 30s at a time. Guarantees re-evaluation of
//!   wall-clock time after macOS sleep, without platform-specific code.
//! - Two-phase fire commit: begin_firing (claim) → publish event → mark_fired.
//!   Crash between phases leaves an in-flight row; on restart we re-dispatch.
//! - Wake signal: external mutations call `wake()` to jump out of sleep early.
//! - Emits events on the `DomainEventBus` — no synchronous caller closure.
//!
//! Subscribers (Phase 3 dispatcher, UI) listen for `AlarmFired` / `MissedAlarms`.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bus::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use storage::rows::scheduled_fire::ScheduledFireRow;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::error::SchedulerError;
use crate::temporal::cron_bridge::CronBridge;
use crate::temporal::fire_store::FireStore;
use crate::temporal::misfire::{Decision, MisfirePolicy};
use crate::temporal::recurrence::RecurrenceEngine;

/// Max time the loop will sleep without checking wall clock. Keep short enough
/// that macOS system-sleep resume leaves us at most this far behind.
const MAX_SLEEP: Duration = Duration::from_secs(30);
const DEFAULT_GRACE_SECS: u64 = 3600;

/// Canonical kind string for recurrence-spawn fires.
/// Used in `dispatch` and `recover_in_flight` so a rename is caught at one site.
pub(crate) const RECURRENCE_SPAWN_KIND: &str = "recurrence_spawn";

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub max_sleep: Duration,
    pub default_grace_secs: u64,
    pub default_misfire_policy: MisfirePolicy,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_sleep: MAX_SLEEP,
            default_grace_secs: DEFAULT_GRACE_SECS,
            default_misfire_policy: MisfirePolicy::default(),
        }
    }
}

#[derive(Clone)]
pub struct TemporalScheduler {
    store: FireStore,
    bus: Arc<DomainEventBus>,
    config: SchedulerConfig,
    wake: Arc<Notify>,
    shutdown: CancellationToken,
    started: Arc<AtomicBool>,
    cron_bridge: Option<Arc<CronBridge>>,
    recurrence_engine: Option<Arc<RecurrenceEngine>>,
}

impl TemporalScheduler {
    pub fn new(store: FireStore, bus: Arc<DomainEventBus>, config: SchedulerConfig) -> Self {
        Self {
            store,
            bus,
            config,
            wake: Arc::new(Notify::new()),
            shutdown: CancellationToken::new(),
            started: Arc::new(AtomicBool::new(false)),
            cron_bridge: None,
            recurrence_engine: None,
        }
    }

    /// Attach a CronBridge so the scheduler can advance cron jobs after each
    /// `kind="cron_job"` fire. Without this, cron jobs will fire once but never
    /// re-schedule.
    pub fn with_cron_bridge(mut self, bridge: CronBridge) -> Self {
        self.cron_bridge = Some(Arc::new(bridge));
        self
    }

    /// Attach a `RecurrenceEngine` so the scheduler materialises task instances
    /// when a `kind="recurrence_spawn"` fire triggers. Without this the fire is
    /// still emitted on the bus; the engine is simply not called.
    pub fn with_recurrence_engine(mut self, engine: Arc<RecurrenceEngine>) -> Self {
        self.recurrence_engine = Some(engine);
        self
    }

    pub fn store(&self) -> &FireStore {
        &self.store
    }

    /// External mutations call this after inserting/cancelling fires to avoid
    /// waiting up to `max_sleep` for the next loop iteration.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    /// Graceful shutdown. Loop exits after current iteration.
    pub fn shutdown(&self) {
        self.shutdown.cancel();
    }

    /// Spawn the loop on the current runtime.
    ///
    /// Panics if called more than once across any clone of this scheduler —
    /// double-start is always a programmer error (two loops sharing the same
    /// FireStore would still be safe thanks to `begin_firing` claim semantics,
    /// but would waste resources and race on the wake signal).
    pub fn start_background(self) -> tokio::task::JoinHandle<()> {
        if self.started.swap(true, Ordering::SeqCst) {
            panic!("TemporalScheduler::start_background called more than once");
        }
        tokio::spawn(async move {
            if let Err(e) = self.run().await {
                warn!(error = %e, "TemporalScheduler exited with error");
            }
        })
    }

    /// Main loop. Returns only when shutdown is cancelled.
    pub async fn run(self) -> Result<(), SchedulerError> {
        info!("TemporalScheduler starting");
        // Recover crashed in-flight rows BEFORE the main loop begins. These rows
        // have `firing_started_at_ms IS NOT NULL` and will also appear in `list_due`
        // on subsequent iterations, but their `begin_firing` call returns false
        // so they're never double-dispatched.
        self.recover_in_flight().await?;

        loop {
            let next = self.store.next_pending_fire_at().await?;
            let now = Timestamp::now();
            let sleep = match next {
                None => self.config.max_sleep,
                Some(t) => {
                    let diff_ms = (t.as_millisecond() - now.as_millisecond()).max(0) as u64;
                    Duration::from_millis(diff_ms).min(self.config.max_sleep)
                }
            };

            tokio::select! {
                _ = tokio::time::sleep(sleep) => {}
                _ = self.wake.notified() => {}
                _ = self.shutdown.cancelled() => {
                    info!("TemporalScheduler shutting down");
                    return Ok(());
                }
            }

            self.process_due(Timestamp::now()).await?;
        }
    }

    async fn recover_in_flight(&self) -> Result<(), SchedulerError> {
        let rows = self.store.recover_in_flight().await?;
        if rows.is_empty() {
            return Ok(());
        }
        warn!(
            count = rows.len(),
            "recovering in-flight fires after restart"
        );
        let now = Timestamp::now();
        for row in rows {
            let id = row.id.clone();
            let kind = row.kind.clone();
            let ref_id = row.ref_id.clone();
            // Row is already claimed (firing_started_at_ms IS NOT NULL) from a
            // prior session. Publish the event and mark fired; skip begin_firing
            // because begin_firing rejects rows already in firing state.
            self.bus.publish(DomainEvent::AlarmFired {
                fire_id: row.id.clone(),
                kind: row.kind.clone(),
                ref_id: row.ref_id.clone(),
                payload_json: row.payload.to_string(),
                fired_at_ms: now.as_millisecond(),
            });
            if let Err(e) = self.store.mark_fired(&id, now).await {
                warn!(
                    error = %e,
                    fire_id = %id,
                    "failed to mark in-flight fire as fired",
                );
                continue;
            }
            if kind == "cron_job" {
                if let (Some(bridge), Some(ref_id)) = (&self.cron_bridge, &ref_id) {
                    if let Err(e) = bridge.advance(ref_id).await {
                        warn!(error = %e, job = %ref_id, "cron bridge advance failed during recovery");
                    }
                }
            }
            if kind == RECURRENCE_SPAWN_KIND {
                if let (Some(engine), Some(ref_id)) = (&self.recurrence_engine, &ref_id) {
                    if let Err(e) = engine.on_spawn(ref_id, now).await {
                        warn!(error = %e, template_id = %ref_id, "recurrence_spawn engine failed during recovery");
                    }
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn process_due(&self, now: Timestamp) -> Result<(), SchedulerError> {
        let due = self.store.list_due(now).await?;
        let mut missed: Vec<ScheduledFireRow> = Vec::new();

        // Partition rows into three buckets in a single pass.
        let mut fire: Vec<ScheduledFireRow> = Vec::new();
        let mut skip: Vec<ScheduledFireRow> = Vec::new();
        // Key: (ref_id, kind) — two different kinds for the same ref are independent.
        let mut coalesce: HashMap<(String, String), Vec<ScheduledFireRow>> = HashMap::new();

        for row in due {
            let (policy, grace) = self.extract_misfire_params(&row);
            let fire_at = match Timestamp::from_millisecond(row.fire_at_ms) {
                Ok(t) => t,
                Err(_) => {
                    warn!(
                        id = %row.id,
                        fire_at_ms = row.fire_at_ms,
                        "scheduled_fires row has invalid fire_at_ms — skipping",
                    );
                    continue;
                }
            };
            match Decision::classify(policy, grace, fire_at, now) {
                Decision::Fire => fire.push(row),
                Decision::SkipStale => skip.push(row),
                Decision::CoalesceLater => {
                    let key = (
                        row.ref_id.clone().unwrap_or_default(),
                        row.kind.clone(),
                    );
                    coalesce.entry(key).or_default().push(row);
                }
            }
        }

        // Process Fire rows.
        for row in fire {
            self.dispatch(row, now).await?;
        }

        // Process SkipStale rows.
        for row in skip {
            // If begin_firing returns false, the row was already claimed
            // (e.g. by recover_in_flight) — skipping quietly is correct.
            if self.store.begin_firing(&row.id, now).await? {
                self.store.mark_fired(&row.id, now).await?;
                missed.push(row);
            }
        }

        // Process Coalesce groups: fire only the most recent per (ref_id, kind).
        for (_key, mut group) in coalesce {
            // Sort ascending by fire_at_ms so the last element is the most recent.
            group.sort_unstable_by_key(|r| r.fire_at_ms);
            let winner = group.pop().expect("coalesce group is never empty");
            for loser in &group {
                self.store
                    .mark_suppressed(&loser.id, &winner.id, now)
                    .await?;
            }
            self.dispatch(winner, now).await?;
        }

        if !missed.is_empty() {
            self.emit_missed(missed);
        }
        Ok(())
    }

    fn extract_misfire_params(&self, row: &ScheduledFireRow) -> (MisfirePolicy, Duration) {
        let policy = row
            .payload
            .get("misfire_policy")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                serde_json::from_value::<MisfirePolicy>(serde_json::Value::String(s.to_owned()))
                    .ok()
            })
            .unwrap_or(self.config.default_misfire_policy);
        let grace_secs = row
            .payload
            .get("grace_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.config.default_grace_secs);
        (policy, Duration::from_secs(grace_secs))
    }

    async fn dispatch(&self, row: ScheduledFireRow, now: Timestamp) -> Result<(), SchedulerError> {
        if !self.store.begin_firing(&row.id, now).await? {
            return Ok(()); // already claimed
        }
        self.bus.publish(DomainEvent::AlarmFired {
            fire_id: row.id.clone(),
            kind: row.kind.clone(),
            ref_id: row.ref_id.clone(),
            payload_json: row.payload.to_string(),
            fired_at_ms: now.as_millisecond(),
        });
        self.store.mark_fired(&row.id, now).await?;

        // Cron jobs re-schedule themselves via the bridge after each fire.
        if row.kind == "cron_job" {
            if let (Some(bridge), Some(ref_id)) = (&self.cron_bridge, &row.ref_id) {
                if let Err(e) = bridge.advance(ref_id).await {
                    warn!(error = %e, job = %ref_id, "cron bridge advance failed");
                }
            }
        }

        // Recurrence spawn: materialise task instances via the engine.
        if row.kind == RECURRENCE_SPAWN_KIND {
            if let (Some(engine), Some(ref_id)) = (&self.recurrence_engine, &row.ref_id) {
                if let Err(e) = engine.on_spawn(ref_id, now).await {
                    warn!(error = %e, template_id = %ref_id, "recurrence_spawn engine failed");
                }
            }
        }

        Ok(())
    }

    fn emit_missed(&self, rows: Vec<ScheduledFireRow>) {
        let fire_ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        let oldest = rows.iter().map(|r| r.fire_at_ms).min().unwrap_or(0);
        let newest = rows.iter().map(|r| r.fire_at_ms).max().unwrap_or(0);
        self.bus.publish(DomainEvent::MissedAlarms {
            fire_ids,
            oldest_fire_at_ms: oldest,
            newest_fire_at_ms: newest,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::FireSpec;
    use bus::{DomainEvent, DomainEventBus};
    use jiff::Timestamp;
    use std::sync::Arc;
    use std::time::Duration;
    use storage::pool::StoragePool;

    async fn setup() -> (TemporalScheduler, Arc<DomainEventBus>) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
            }],
        )
        .await
        .unwrap();
        let store = FireStore::new(storage::repos::scheduled_fires::ScheduledFiresRepo::new(
            pool.inner().clone(),
        ));
        let bus = Arc::new(DomainEventBus::new(32));
        let scheduler = TemporalScheduler::new(store, bus.clone(), SchedulerConfig::default());
        (scheduler, bus)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fires_due_alarm_and_emits_event() {
        let (scheduler, bus) = setup().await;
        let mut rx = bus.subscribe();
        let _handle = scheduler.clone().start_background();

        // Schedule a fire 50ms in the future.
        let fire_at = Timestamp::now()
            .checked_add(jiff::Span::new().milliseconds(50))
            .unwrap();
        scheduler
            .store()
            .schedule(FireSpec {
                fire_at,
                kind: "test".into(),
                ref_id: Some("r1".into()),
                payload: serde_json::json!({}),
                dedup_prefix: None,
            })
            .await
            .unwrap();
        scheduler.wake();

        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("event not received in time")
            .unwrap();
        assert!(matches!(event, DomainEvent::AlarmFired { .. }));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn skip_if_stale_emits_missed_alarms() {
        let (scheduler, bus) = setup().await;
        let mut rx = bus.subscribe();
        let _handle = scheduler.clone().start_background();

        // Fire 2 hours in the past; grace 1 hour (default config); policy skip_if_stale.
        let fire_at = Timestamp::now()
            .checked_sub(jiff::Span::new().hours(2))
            .unwrap();
        scheduler
            .store()
            .schedule(FireSpec {
                fire_at,
                kind: "test".into(),
                ref_id: None,
                payload: serde_json::json!({ "misfire_policy": "skip_if_stale", "grace_secs": 3600 }),
                dedup_prefix: None,
            })
            .await
            .unwrap();
        scheduler.wake();

        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("event not received")
            .unwrap();
        assert!(matches!(event, DomainEvent::MissedAlarms { .. }));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn recovers_in_flight_rows_on_restart() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
            }],
        )
        .await
        .unwrap();
        let store = FireStore::new(storage::repos::scheduled_fires::ScheduledFiresRepo::new(
            pool.inner().clone(),
        ));

        // Simulate a crash: schedule a row, claim it via begin_firing, do NOT mark_fired.
        let fire_at = Timestamp::now()
            .checked_sub(jiff::Span::new().seconds(1))
            .unwrap();
        let id = store
            .schedule(FireSpec {
                fire_at,
                kind: "test".into(),
                ref_id: None,
                payload: serde_json::json!({}),
                dedup_prefix: None,
            })
            .await
            .unwrap();
        assert!(store.begin_firing(&id, Timestamp::now()).await.unwrap());
        // (no mark_fired — simulates a process crash between phases)

        // "Restart": new scheduler, fresh bus, same storage pool.
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();
        let scheduler = TemporalScheduler::new(store, bus.clone(), SchedulerConfig::default());
        let _h = scheduler.start_background();

        let ev = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("no recovery event within timeout")
            .unwrap();
        match ev {
            DomainEvent::AlarmFired { fire_id, .. } => assert_eq!(fire_id, id),
            other => panic!("expected AlarmFired, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cron_job_fire_triggers_next_schedule() {
        // Setup: in-memory pool with storage initial + scheduling feature migrations.
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
            }],
        )
        .await
        .unwrap();

        let cron = storage::repos::cron::CronRepo::new(pool.inner().clone());
        let sf_repo =
            storage::repos::scheduled_fires::ScheduledFiresRepo::new(pool.inner().clone());
        let store = FireStore::new(sf_repo.clone());
        let bridge = crate::temporal::cron_bridge::CronBridge::new(cron.clone(), store.clone());

        // Every-second cron job.
        cron.upsert(&storage::rows::cron::CronJobRow {
            id: "j1".into(),
            name: "every-sec".into(),
            enabled: true,
            origin: "user".into(),
            schedule: serde_json::json!({ "cron": "* * * * * * *", "tz": "UTC" }),
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
        })
        .await
        .unwrap();
        bridge.reconcile_all().await.unwrap();

        let bus = Arc::new(DomainEventBus::new(32));
        let scheduler =
            TemporalScheduler::new(store.clone(), bus.clone(), SchedulerConfig::default())
                .with_cron_bridge(bridge);
        let _h = scheduler.start_background();

        // Wait for a few fires.
        tokio::time::sleep(Duration::from_secs(3)).await;

        let fired_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM scheduled_fires WHERE fired = 1 AND kind = 'cron_job'",
        )
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert!(
            fired_count >= 2,
            "expected ≥2 cron fires in 3s, got {fired_count}"
        );
    }

    /// Coalesce: 3 stale rows with same (ref_id, kind). Only the most recent (c3)
    /// should dispatch; c1 and c2 should be marked fired with suppressed_by = c3.
    #[tokio::test]
    async fn coalesce_fires_only_most_recent_per_ref() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
            }],
        )
        .await
        .unwrap();
        let store = FireStore::new(storage::repos::scheduled_fires::ScheduledFiresRepo::new(
            pool.inner().clone(),
        ));
        let bus = Arc::new(DomainEventBus::new(32));
        let mut rx = bus.subscribe();

        let now = Timestamp::now();
        let minus_30m = now.checked_sub(jiff::Span::new().minutes(30)).unwrap();
        let minus_25m = now.checked_sub(jiff::Span::new().minutes(25)).unwrap();
        let minus_20m = now.checked_sub(jiff::Span::new().minutes(20)).unwrap();

        let payload = serde_json::json!({ "misfire_policy": "coalesce" });

        let c1 = store
            .schedule(FireSpec {
                fire_at: minus_30m,
                kind: "task_alarm".into(),
                ref_id: Some("task:abc:reminder".into()),
                payload: payload.clone(),
                dedup_prefix: None,
            })
            .await
            .unwrap();
        let c2 = store
            .schedule(FireSpec {
                fire_at: minus_25m,
                kind: "task_alarm".into(),
                ref_id: Some("task:abc:reminder".into()),
                payload: payload.clone(),
                dedup_prefix: None,
            })
            .await
            .unwrap();
        let c3 = store
            .schedule(FireSpec {
                fire_at: minus_20m,
                kind: "task_alarm".into(),
                ref_id: Some("task:abc:reminder".into()),
                payload: payload.clone(),
                dedup_prefix: None,
            })
            .await
            .unwrap();

        let config = SchedulerConfig {
            default_misfire_policy: crate::temporal::misfire::MisfirePolicy::Coalesce,
            ..SchedulerConfig::default()
        };
        let scheduler = TemporalScheduler::new(store.clone(), bus.clone(), config);

        // Run a single process_due pass directly (no background loop).
        scheduler.process_due(now).await.unwrap();

        // Exactly one AlarmFired event should have been emitted (for c3).
        let event = tokio::time::timeout(Duration::from_millis(200), rx.recv())
            .await
            .expect("expected an AlarmFired event")
            .unwrap();
        match &event {
            DomainEvent::AlarmFired { fire_id, .. } => {
                assert_eq!(fire_id, &c3, "winner should be c3 (most recent)");
            }
            other => panic!("expected AlarmFired, got {other:?}"),
        }
        // No second event should arrive within a short window.
        let second = tokio::time::timeout(Duration::from_millis(100), rx.recv()).await;
        assert!(
            second.is_err(),
            "expected no second event, but got one: {second:?}"
        );

        // c1 and c2 must be marked fired with suppressed_by = c3.
        let c1_row: (i64, Option<String>) =
            sqlx::query_as("SELECT fired, suppressed_by FROM scheduled_fires WHERE id = ?1")
                .bind(&c1)
                .fetch_one(pool.inner())
                .await
                .unwrap();
        assert_eq!(c1_row.0, 1, "c1 should be fired");
        assert_eq!(
            c1_row.1.as_deref(),
            Some(c3.as_str()),
            "c1 suppressed_by should be c3"
        );

        let c2_row: (i64, Option<String>) =
            sqlx::query_as("SELECT fired, suppressed_by FROM scheduled_fires WHERE id = ?1")
                .bind(&c2)
                .fetch_one(pool.inner())
                .await
                .unwrap();
        assert_eq!(c2_row.0, 1, "c2 should be fired");
        assert_eq!(
            c2_row.1.as_deref(),
            Some(c3.as_str()),
            "c2 suppressed_by should be c3"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    #[should_panic(expected = "start_background called more than once")]
    async fn double_start_background_panics() {
        let (scheduler, _bus) = setup().await;
        let s2 = scheduler.clone();
        let _h1 = scheduler.start_background();
        // second start on the sibling clone must panic
        let _h2 = s2.start_background();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_stops_the_loop() {
        let (scheduler, _bus) = setup().await;
        let shutdown_handle = scheduler.clone();
        let h = scheduler.start_background();
        // Let the loop enter its first select.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        shutdown_handle.shutdown();
        // Should terminate quickly.
        tokio::time::timeout(std::time::Duration::from_secs(2), h)
            .await
            .expect("loop did not exit within timeout")
            .unwrap();
    }
}
