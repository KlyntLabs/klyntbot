//! Subscribes to `DomainEvent::AlarmFired { kind: "focus_end" }` and calls
//! `DndManager::deactivate(FocusMode::Dnd)` so sessions auto-end when the
//! TemporalScheduler fires their alarm.

use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};
use feature_focus::{DndManager, FocusMode};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

/// Spawn the focus-end subscriber. Returns the handle — drop = abort.
pub fn spawn_focus_end_subscriber(
    manager: Arc<DndManager>,
    bus: Arc<DomainEventBus>,
    shutdown: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = bus.subscribe();
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    debug!("focus_end_subscriber: shutdown");
                    break;
                }
                msg = rx.recv() => {
                    match msg {
                        Ok(DomainEvent::Alarm(bus::AlarmEvent::AlarmFired { kind, .. })) if kind == "focus_end" => {
                            debug!("focus_end_subscriber: focus_end alarm fired — deactivating DND");
                            if let Err(e) = manager.deactivate(FocusMode::Dnd).await {
                                warn!("focus_end_subscriber: deactivate failed: {e}");
                            }
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("focus_end_subscriber: lagged by {n} events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            debug!("focus_end_subscriber: bus closed");
                            break;
                        }
                    }
                }
            }
        }
    })
}
