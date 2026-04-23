//! Focus deadline watcher — polls every 60s for tasks whose `focus_deadline`
//! has passed while still focused, emits `TaskFocusExpired`, and clears
//! `focused_at` so the event fires only once per expiry.

use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};
use storage::repos::task_repo::TaskRepo;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

/// Spawn the watcher. Returns the handle; dropping it aborts the task.
pub fn spawn(
    bus: Arc<DomainEventBus>,
    task_repo: TaskRepo,
    shutdown: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    debug!("focus_watcher: shutdown");
                    break;
                }
                _ = interval.tick() => {
                    check_expired(&bus, &task_repo).await;
                }
            }
        }
    })
}

async fn check_expired(bus: &DomainEventBus, task_repo: &TaskRepo) {
    let focused = match task_repo.list_focused().await {
        Ok(rows) => rows,
        Err(e) => {
            warn!("focus_watcher: list_focused failed: {e}");
            return;
        }
    };

    let now = jiff::Timestamp::now();

    for row in focused {
        // Skip completed tasks — they're no longer actionable.
        if row.completed {
            continue;
        }

        let deadline = match &row.focus_deadline {
            Some(d) => {
                let ts: jiff::Timestamp = (*d).clone().into();
                ts
            }
            None => continue,
        };

        if deadline <= now {
            // Emit expiry event.
            bus.publish(DomainEvent::TaskFocusExpired {
                task_id: row.id.clone(),
                title: row.title.clone(),
            });

            // Clear focused_at so we don't re-emit on the next tick.
            if let Err(e) = task_repo.unfocus(&row.id).await {
                warn!(task_id = %row.id, "focus_watcher: unfocus failed: {e}");
            } else {
                debug!(task_id = %row.id, "focus_watcher: cleared expired focus");
            }
        }
    }
}
