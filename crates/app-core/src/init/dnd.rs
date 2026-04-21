//! DND (Do-Not-Disturb) focus session manager initialization.
//!
//! Builds a [`DndManager`] wired to the real [`TemporalAlarmBridge`] and a
//! no-op bridge (real macOS bridge comes in PR-3). Spawns the
//! `focus_end_subscriber` so scheduled alarms auto-deactivate sessions.

use std::sync::Arc;

use async_trait::async_trait;
use bus::DomainEventBus;
use feature_focus::alarm_bridge::TemporalAlarmBridge;
use feature_focus::manager::FocusBridge;
use feature_focus::{DndManager, FocusMode};
use scheduling::temporal::fire_store::FireStore;
use storage::StoragePool;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::focus::end_subscriber::spawn_focus_end_subscriber;

// ── No-op bridge (placeholder until PR-3 ships MacosFocusBridge) ─────────────

/// Stub bridge that does nothing. Replaced by `MacosFocusBridge` in PR-3.
struct NoopFocusBridge;

#[async_trait]
impl FocusBridge for NoopFocusBridge {
    async fn turn_on(&self, _mode: FocusMode) -> common::Result<()> {
        tracing::debug!("NoopFocusBridge: turn_on (no-op until PR-3 ships macOS bridge)");
        Ok(())
    }
    async fn turn_off(&self, _mode: FocusMode) -> common::Result<()> {
        tracing::debug!("NoopFocusBridge: turn_off (no-op until PR-3 ships macOS bridge)");
        Ok(())
    }
    async fn is_ready(&self) -> common::Result<bool> {
        Ok(false)
    }
}

// ── Result ────────────────────────────────────────────────────────────────────

pub(super) struct DndResult {
    pub manager: Arc<DndManager>,
    /// Keep alive — drop aborts the subscriber.
    pub _end_subscriber_handle: JoinHandle<()>,
}

// ── Init ──────────────────────────────────────────────────────────────────────

/// Build the `DndManager` + subscriber. Called after the TemporalScheduler and
/// DomainEventBus are both up.
pub(super) fn init_dnd(
    storage_pool: &StoragePool,
    domain_event_bus: &Option<Arc<DomainEventBus>>,
    shutdown: &CancellationToken,
) -> DndResult {
    let fire_store = FireStore::new(storage::repos::ScheduledFiresRepo::new(
        storage_pool.inner().clone(),
    ));
    let alarm_bridge = Arc::new(TemporalAlarmBridge::new(fire_store));
    let focus_bridge: Arc<dyn FocusBridge> = Arc::new(NoopFocusBridge);
    let repo = feature_focus::FocusSessionRepo::new(storage_pool.inner().clone());

    let manager = Arc::new(DndManager::new(repo, alarm_bridge, focus_bridge));

    let handle = if let Some(bus) = domain_event_bus {
        spawn_focus_end_subscriber(Arc::clone(&manager), Arc::clone(bus), shutdown.clone())
    } else {
        // No bus — spawn a no-op task so the handle type is consistent.
        tokio::spawn(async {})
    };

    info!("DndManager initialized (end subscriber wired)");

    DndResult {
        manager,
        _end_subscriber_handle: handle,
    }
}
