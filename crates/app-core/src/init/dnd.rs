//! DND (Do-Not-Disturb) focus session manager initialization.
//!
//! Builds a [`DndManager`] wired to the real [`TemporalAlarmBridge`] and the
//! platform-appropriate focus bridge. On macOS this is [`MacosFocusBridge`];
//! on other platforms it is a no-op stub so the app still builds and runs.

use std::sync::Arc;

#[cfg(not(target_os = "macos"))]
use async_trait::async_trait;
use bus::DomainEventBus;
use feature_focus::DndManager;
#[cfg(not(target_os = "macos"))]
use feature_focus::FocusMode;
use feature_focus::alarm_bridge::TemporalAlarmBridge;
use feature_focus::manager::FocusBridge;
use scheduling::temporal::fire_store::FireStore;
use storage::StoragePool;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::focus::end_subscriber::spawn_focus_end_subscriber;

// ── Non-macOS stub ────────────────────────────────────────────────────────────

/// Stub bridge used on non-macOS platforms. All operations succeed as no-ops
/// and `is_ready` reports false so callers can show appropriate UI.
#[cfg(not(target_os = "macos"))]
struct NoopFocusBridge;

#[cfg(not(target_os = "macos"))]
#[async_trait]
impl FocusBridge for NoopFocusBridge {
    async fn turn_on(&self, _mode: FocusMode) -> common::Result<()> {
        tracing::debug!("NoopFocusBridge: turn_on (DND only supported on macOS)");
        Ok(())
    }
    async fn turn_off(&self, _mode: FocusMode) -> common::Result<()> {
        tracing::debug!("NoopFocusBridge: turn_off (DND only supported on macOS)");
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

    #[cfg(target_os = "macos")]
    let focus_bridge: Arc<dyn FocusBridge> =
        Arc::new(feature_focus::bridge::macos::MacosFocusBridge);

    #[cfg(not(target_os = "macos"))]
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
