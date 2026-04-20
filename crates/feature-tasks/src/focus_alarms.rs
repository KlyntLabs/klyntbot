//! Focus alarms service — subscribes to `TaskFocusChanged` and materializes
//! 6h/3h/1h warning alarms + an expire alarm into `scheduled_fires`.
//!
//! Replaces the in-memory focus warning logic that lived in
//! `app-core/src/init/deadline.rs`. Persistence + crash recovery are inherited
//! for free because everything lives in the canonical `scheduled_fires` table.
//!
//! Dedup convention: `focus:{task_id}:` prefix. Cancel-then-insert on every
//! event, so re-focusing a task reliably replaces its warnings.

use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use scheduling::temporal::fire_store::{FireSpec, FireStore};
use serde_json::json;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

const WARNING_HOURS: &[u32] = &[6, 3, 1];

/// Stable dedup prefix for a task's focus alarms.
fn focus_prefix(task_id: &str) -> String {
    format!("focus:{task_id}:")
}

/// Spawn the subscriber loop. Returns the handle so callers can keep the
/// task alive (drop = abort).
pub fn spawn(
    bus: Arc<DomainEventBus>,
    fire_store: Arc<FireStore>,
    shutdown: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut rx = bus.subscribe();
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    debug!("focus_alarms: shutdown signal received");
                    break;
                }
                msg = rx.recv() => {
                    match msg {
                        Ok(DomainEvent::TaskFocusChanged { task_id, focus_deadline }) => {
                            handle(&fire_store, &task_id, focus_deadline.as_deref()).await;
                        }
                        Ok(_) => {} // ignore other events
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("focus_alarms: lagged by {n} events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            debug!("focus_alarms: bus closed");
                            break;
                        }
                    }
                }
            }
        }
    })
}

async fn handle(fire_store: &FireStore, task_id: &str, focus_deadline: Option<&str>) {
    // Always cancel pending focus alarms first — we're about to either
    // recompute them or clear them entirely.
    if let Err(e) = fire_store.cancel_by_prefix(&focus_prefix(task_id)).await {
        warn!("focus_alarms: cancel for task {task_id} failed: {e}");
        return;
    }

    let Some(dl_str) = focus_deadline else {
        // Unfocus → cancellation was the entire job.
        return;
    };
    let Ok(deadline) = dl_str.parse::<Timestamp>() else {
        warn!("focus_alarms: cannot parse deadline '{dl_str}' for task {task_id}");
        return;
    };
    let now = Timestamp::now();
    let prefix = focus_prefix(task_id);

    // 6h / 3h / 1h warnings.
    for &h in WARNING_HOURS {
        let warn_at = deadline - jiff::SignedDuration::from_secs((h as i64) * 3600);
        if warn_at <= now {
            continue;
        }
        let payload = json!({
            "title": "Focus deadline approaching",
            "body": format!("Focus deadline in {h}h"),
            "channel_mask": 0,           // inherit defaults
            "priority_override": null,
            "task_id": task_id,
            "hours_left": h,
        });
        if let Err(e) = fire_store
            .schedule(FireSpec {
                fire_at: warn_at,
                kind: "focus_warning".into(),
                ref_id: Some(task_id.into()),
                payload,
                dedup_prefix: Some(prefix.clone()),
            })
            .await
        {
            warn!("focus_alarms: schedule warning ({h}h) failed: {e}");
        }
    }

    // Expire alarm — fires at the deadline; the side-effects subscriber
    // calls repo.unfocus on receipt.
    if deadline > now {
        let payload = json!({
            "title": "Focus expired",
            "body": "Your focus session has expired",
            "channel_mask": 0,
            "priority_override": null,
            "task_id": task_id,
        });
        if let Err(e) = fire_store
            .schedule(FireSpec {
                fire_at: deadline,
                kind: "focus_expire".into(),
                ref_id: Some(task_id.into()),
                payload,
                dedup_prefix: Some(prefix),
            })
            .await
        {
            warn!("focus_alarms: schedule expire failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Span;
    use storage::pool::StoragePool;
    use storage::repos::scheduled_fires::ScheduledFiresRepo;

    async fn setup() -> (Arc<FireStore>, ScheduledFiresRepo, StoragePool) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../scheduling/migrations/001_scheduled_fires.sql").into(),
            }],
        )
        .await
        .unwrap();
        let sf_repo = ScheduledFiresRepo::new(pool.inner().clone());
        (Arc::new(FireStore::new(sf_repo.clone())), sf_repo, pool)
    }

    async fn pending_count(repo: &ScheduledFiresRepo) -> usize {
        repo.list_pending_up_to_ms(i64::MAX).await.unwrap().len()
    }

    #[tokio::test]
    async fn focus_set_schedules_warnings_and_expire() {
        let (fs, sf, _pool) = setup().await;
        // Deadline 8h from now → all 3 warnings + expire fire in the future.
        let deadline = Timestamp::now() + Span::new().hours(8);
        handle(&fs, "t1", Some(&deadline.to_string())).await;
        assert_eq!(pending_count(&sf).await, 4); // 6h + 3h + 1h + expire
    }

    #[tokio::test]
    async fn focus_set_with_short_deadline_only_schedules_remaining_warnings() {
        let (fs, sf, _pool) = setup().await;
        // Deadline 2h from now → only the 1h warning + expire are in the future.
        let deadline = Timestamp::now() + Span::new().hours(2);
        handle(&fs, "t2", Some(&deadline.to_string())).await;
        assert_eq!(pending_count(&sf).await, 2);
    }

    #[tokio::test]
    async fn unfocus_cancels_existing_alarms() {
        let (fs, sf, _pool) = setup().await;
        let deadline = Timestamp::now() + Span::new().hours(8);
        handle(&fs, "t3", Some(&deadline.to_string())).await;
        assert_eq!(pending_count(&sf).await, 4);
        handle(&fs, "t3", None).await;
        assert_eq!(pending_count(&sf).await, 0);
    }

    #[tokio::test]
    async fn refocus_replaces_warnings_idempotently() {
        let (fs, sf, _pool) = setup().await;
        let d1 = Timestamp::now() + Span::new().hours(8);
        handle(&fs, "t4", Some(&d1.to_string())).await;
        let d2 = Timestamp::now() + Span::new().hours(4);
        handle(&fs, "t4", Some(&d2.to_string())).await;
        // d2 = 4h → warnings at 3h+1h survive (6h would be in past), plus expire
        assert_eq!(pending_count(&sf).await, 3);
    }

    #[tokio::test]
    async fn other_task_alarms_are_not_affected() {
        let (fs, sf, _pool) = setup().await;
        let d = Timestamp::now() + Span::new().hours(8);
        handle(&fs, "task_a", Some(&d.to_string())).await;
        handle(&fs, "task_b", Some(&d.to_string())).await;
        assert_eq!(pending_count(&sf).await, 8);
        handle(&fs, "task_a", None).await;
        assert_eq!(pending_count(&sf).await, 4); // task_b's still there
    }
}
