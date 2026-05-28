//! AlarmFired side-effects — subscribes to `DomainEvent::AlarmFired` and
//! performs the post-fire data mutations:
//!
//! - `kind = "focus_expire"` → call `TaskRepo::unfocus(task_id)`.
//! - `kind = "task_alarm"`   → mark `tasks.last_reminded_at = now()` so the
//!   reminder isn't re-fired if scheduling reconciles.
//!
//! Notification dispatch is handled by `crates/notifications/`.

use std::sync::Arc;

use bus::{AlarmEvent, DomainEvent, DomainEventBus};
use jiff::Timestamp;
use storage::{TaskPatch, TaskRepo};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

/// Spawn the AlarmFired subscriber. Returns the handle so callers can keep
/// it alive (drop = abort).
pub fn spawn(
    bus: Arc<DomainEventBus>,
    task_repo: TaskRepo,
    shutdown: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = bus.subscribe();
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    debug!("alarm_side_effects: shutdown");
                    break;
                }
                msg = rx.recv() => {
                    match msg {
                        Ok(DomainEvent::Alarm(AlarmEvent::AlarmFired { kind, ref_id, payload_json, .. })) => {
                            handle(&task_repo, &kind, ref_id.as_deref(), &payload_json).await;
                        }
                        Ok(_) => {}
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("alarm_side_effects: lagged by {n} events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            debug!("alarm_side_effects: bus closed");
                            break;
                        }
                    }
                }
            }
        }
    })
}

async fn handle(repo: &TaskRepo, kind: &str, ref_id: Option<&str>, payload_json: &str) {
    match kind {
        "focus_expire" => {
            // ref_id is set to task_id by focus_alarms.
            let Some(task_id) = ref_id else {
                warn!("alarm_side_effects: focus_expire with no ref_id, skipping");
                return;
            };
            match repo.unfocus(task_id).await {
                Ok(true) => debug!("alarm_side_effects: unfocused task {task_id} on expire"),
                Ok(false) => {
                    debug!("alarm_side_effects: focus_expire for {task_id} but task not focused")
                }
                Err(e) => warn!("alarm_side_effects: unfocus failed for task {task_id}: {e}"),
            }
        }
        "task_alarm" => {
            // ref_id on task_alarm is the alarm row id; the task_id lives in the payload.
            let task_id = match serde_json::from_str::<serde_json::Value>(payload_json) {
                Ok(v) => v.get("task_id").and_then(|x| x.as_str()).map(String::from),
                Err(e) => {
                    warn!("alarm_side_effects: task_alarm payload not JSON: {e}");
                    None
                }
            };
            let Some(task_id) = task_id else {
                debug!("alarm_side_effects: task_alarm with no task_id in payload, skipping mark");
                return;
            };
            let patch = TaskPatch {
                id: task_id.clone(),
                last_reminded_at: Some(Some(Timestamp::now())),
                ..Default::default()
            };
            if let Err(e) = repo.update(&patch).await {
                warn!("alarm_side_effects: mark task {task_id} reminded failed: {e}");
            }
        }
        _ => {} // not our concern
    }
}
