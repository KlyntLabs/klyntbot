//! [`TemporalAlarmBridge`] — real [`DndScheduler`] impl backed by [`FireStore`].
//!
//! Schedules a `focus_end` fire in the `scheduled_fires` table via the existing
//! `FireStore` service. Cancellation uses `cancel_by_prefix` with a stable prefix
//! so stale rows are swept even if a previous run crashed between schedule + cancel.

use async_trait::async_trait;
use jiff::Timestamp;

use common::{KlyntbotError, Result};
use scheduling::temporal::fire_store::{FireSpec, FireStore};

use crate::manager::DndScheduler;

/// Dedup prefix for focus-end fires. Because at most one DND session is active
/// at a time the prefix does not need to embed a session id.
const DEDUP_PREFIX: &str = "focus_end:dnd:";

/// Bridges `DndScheduler` to the real `FireStore` (TemporalScheduler backend).
#[derive(Debug, Clone)]
pub struct TemporalAlarmBridge {
    store: FireStore,
}

impl TemporalAlarmBridge {
    pub fn new(store: FireStore) -> Self {
        Self { store }
    }
}

#[async_trait]
impl DndScheduler for TemporalAlarmBridge {
    async fn schedule(&self, fire_at: Timestamp) -> Result<String> {
        // Sweep any stale focus_end row before inserting so we never have two
        // pending rows for the same DND session (e.g. after a crash during extend).
        let _ = self.store.cancel_by_prefix(DEDUP_PREFIX).await;

        self.store
            .schedule(FireSpec {
                fire_at,
                kind: "focus_end".into(),
                ref_id: None,
                payload: serde_json::json!({ "mode": "dnd" }),
                dedup_prefix: Some(DEDUP_PREFIX.into()),
            })
            .await
            .map_err(|e| KlyntbotError::Storage(format!("TemporalAlarmBridge.schedule: {e}")))
    }

    async fn cancel(&self, _alarm_id: &str) -> Result<()> {
        // We use a stable prefix-cancel rather than per-id cancel because
        // FireStore does not expose a cancel-by-id method. The dedup prefix
        // guarantees only the single pending focus_end:dnd: row is removed.
        self.store
            .cancel_by_prefix(DEDUP_PREFIX)
            .await
            .map(|_| ())
            .map_err(|e| KlyntbotError::Storage(format!("TemporalAlarmBridge.cancel: {e}")))
    }
}

// ── Integration tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use scheduling::temporal::fire_store::FireStore;
    use storage::repos::scheduled_fires::ScheduledFiresRepo;
    use storage::StoragePool;
    use tools_core::FeatureMigration;

    async fn make_store() -> FireStore {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../scheduling/migrations/001_scheduled_fires.sql")
                    .into(),
            }],
        )
        .await
        .unwrap();
        FireStore::new(ScheduledFiresRepo::new(pool.inner().clone()))
    }

    #[tokio::test]
    async fn schedule_inserts_a_pending_row() {
        let store = make_store().await;
        let bridge = TemporalAlarmBridge::new(store.clone());

        let fire_at = Timestamp::from_second(9_999_999_999).unwrap();
        let id = bridge.schedule(fire_at).await.unwrap();
        assert!(!id.is_empty());

        // The store should have a next pending fire at our requested time.
        let next = store.next_pending_fire_at().await.unwrap();
        assert_eq!(next, Some(fire_at));
    }

    #[tokio::test]
    async fn cancel_removes_pending_row() {
        let store = make_store().await;
        let bridge = TemporalAlarmBridge::new(store.clone());

        let fire_at = Timestamp::from_second(9_999_999_999).unwrap();
        let id = bridge.schedule(fire_at).await.unwrap();

        // Row exists.
        assert!(store.next_pending_fire_at().await.unwrap().is_some());

        // Cancel.
        bridge.cancel(&id).await.unwrap();

        // Row gone.
        assert!(store.next_pending_fire_at().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn reschedule_sweeps_stale_row_first() {
        let store = make_store().await;
        let bridge = TemporalAlarmBridge::new(store.clone());

        let t1 = Timestamp::from_second(1_000_000_000).unwrap();
        let t2 = Timestamp::from_second(2_000_000_000).unwrap();

        // Two consecutive schedules without cancel in between.
        bridge.schedule(t1).await.unwrap();
        bridge.schedule(t2).await.unwrap();

        // Only one pending row should exist (t2).
        let next = store.next_pending_fire_at().await.unwrap();
        assert_eq!(next, Some(t2));
    }
}
