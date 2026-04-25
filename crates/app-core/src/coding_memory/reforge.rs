//! App-core wiring for Phase 5 Reforge integration.
//!
//! - Dispatches `DomainEvent::CodingSessionEnded` into `SessionEndPass`.
//! - Builds `CodingPhaseHandlers` for the cron `run_reforge` call.

use bus::{DomainEvent, DomainEventBus};
use coding_memory::reforge::SessionEndPass;
use std::sync::Arc;
use tracing::warn;

/// Subscribe `SessionEndPass` to `DomainEvent::CodingSessionEnded`.
pub async fn register_session_end_dispatch(
    bus: Arc<DomainEventBus>,
    pass: Arc<SessionEndPass>,
) {
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if let DomainEvent::CodingSessionEnded {
                session_id,
                repo_id,
            } = event
            {
                let pass = pass.clone();
                tokio::spawn(async move {
                    if let Err(e) = pass.run(&session_id, repo_id.as_deref()).await {
                        warn!("SessionEndPass failed for {session_id}: {e}");
                    }
                });
            }
        }
    });
}
