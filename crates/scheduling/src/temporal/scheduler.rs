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

use std::sync::Arc;
use std::time::Duration;

use bus::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use storage::rows::scheduled_fire::ScheduledFireRow;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::error::SchedulerError;
use crate::temporal::fire_store::FireStore;
use crate::temporal::misfire::{Decision, MisfirePolicy};

/// Max time the loop will sleep without checking wall clock. Keep short enough
/// that macOS system-sleep resume leaves us at most this far behind.
const MAX_SLEEP: Duration = Duration::from_secs(30);
const DEFAULT_GRACE_SECS: u64 = 3600;

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
}

impl TemporalScheduler {
    pub fn new(store: FireStore, bus: Arc<DomainEventBus>, config: SchedulerConfig) -> Self {
        Self {
            store,
            bus,
            config,
            wake: Arc::new(Notify::new()),
            shutdown: CancellationToken::new(),
        }
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
    pub fn start_background(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = self.run().await {
                warn!(error = %e, "TemporalScheduler exited with error");
            }
        })
    }

    /// Main loop. Returns only when shutdown is cancelled.
    pub async fn run(self) -> Result<(), SchedulerError> {
        info!("TemporalScheduler starting");
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
        for row in rows {
            self.dispatch(row, Timestamp::now()).await?;
        }
        Ok(())
    }

    async fn process_due(&self, now: Timestamp) -> Result<(), SchedulerError> {
        let due = self.store.list_due(now).await?;
        let mut missed: Vec<ScheduledFireRow> = Vec::new();
        for row in due {
            let (policy, grace) = self.extract_misfire_params(&row);
            let fire_at = Timestamp::from_millisecond(row.fire_at_ms)
                .map_err(|_| SchedulerError::InvalidState("bad fire_at_ms".into()))?;
            match Decision::classify(policy, grace, fire_at, now) {
                Decision::Fire => self.dispatch(row, now).await?,
                Decision::SkipStale => {
                    if self.store.begin_firing(&row.id, now).await? {
                        self.store.mark_fired(&row.id, now).await?;
                        missed.push(row);
                    }
                }
                Decision::CoalesceLater => self.dispatch(row, now).await?,
            }
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
            .and_then(|s| serde_json::from_str::<MisfirePolicy>(&format!("\"{s}\"")).ok())
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
}
