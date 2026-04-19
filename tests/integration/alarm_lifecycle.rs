//! Task 4.15 — alarm lifecycle: create → cancel and create → snooze.
//!
//! Exercises `FireStore::cancel_by_prefix` (hard-cancel) and a raw
//! fire_at_ms UPDATE (snooze) against the scheduled_fires table.

use jiff::Timestamp;
use scheduling::temporal::fire_store::{FireSpec, FireStore};
use storage::pool::StoragePool;
use storage::repos::scheduled_fires::ScheduledFiresRepo;
use tools_core::FeatureMigration;

async fn setup() -> (FireStore, ScheduledFiresRepo, StoragePool) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(
        pool.inner(),
        &[FeatureMigration {
            feature_name: "scheduling".into(),
            version: 1,
            description: "scheduled_fires".into(),
            sql: include_str!("../../crates/scheduling/migrations/001_scheduled_fires.sql").into(),
        }],
    )
    .await
    .unwrap();
    let repo = ScheduledFiresRepo::new(pool.inner().clone());
    let store = FireStore::new(repo.clone());
    (store, repo, pool)
}

/// Schedule a fire, assert it appears as pending, then cancel via prefix — row
/// must disappear from pending.
#[tokio::test]
async fn cancel_by_prefix_removes_pending_row() {
    let (store, repo, _pool) = setup().await;

    let fire_at = Timestamp::now()
        .checked_add(jiff::Span::new().seconds(10))
        .unwrap();

    store
        .schedule(FireSpec {
            fire_at,
            kind: "task_alarm".into(),
            ref_id: Some("task:abc".into()),
            payload: serde_json::json!({}),
            dedup_prefix: Some("task:abc:alarm:".into()),
        })
        .await
        .unwrap();

    // Row exists before cancel.
    let pending = repo.list_pending_up_to_ms(i64::MAX).await.unwrap();
    assert_eq!(pending.len(), 1, "one pending fire before cancel");
    assert_eq!(pending[0].ref_id.as_deref(), Some("task:abc"));

    // Cancel by the dedup prefix.
    let deleted = store.cancel_by_prefix("task:abc:alarm:").await.unwrap();
    assert_eq!(deleted, 1, "exactly one row deleted");

    // Row is gone from pending.
    let after = repo.list_pending_up_to_ms(i64::MAX).await.unwrap();
    assert!(
        after.is_empty(),
        "no pending fires remain after cancel_by_prefix"
    );
}

/// Cancelling with a non-matching prefix must not touch unrelated rows.
#[tokio::test]
async fn cancel_by_prefix_is_prefix_scoped() {
    let (store, repo, _pool) = setup().await;

    let fire_at = Timestamp::now()
        .checked_add(jiff::Span::new().seconds(10))
        .unwrap();

    store
        .schedule(FireSpec {
            fire_at,
            kind: "task_alarm".into(),
            ref_id: Some("task:xyz".into()),
            payload: serde_json::json!({}),
            dedup_prefix: Some("task:xyz:alarm:".into()),
        })
        .await
        .unwrap();

    // Cancelling a different prefix should delete nothing.
    let deleted = store.cancel_by_prefix("task:abc:alarm:").await.unwrap();
    assert_eq!(deleted, 0, "unrelated row not deleted");

    let remaining = repo.list_pending_up_to_ms(i64::MAX).await.unwrap();
    assert_eq!(remaining.len(), 1, "original row still pending");
}

/// Snooze: update fire_at_ms to a later time. The row must still exist (not
/// deleted) with the updated timestamp.
#[tokio::test]
async fn snooze_updates_fire_at_and_row_survives() {
    let (store, repo, pool) = setup().await;

    let now_ms = Timestamp::now().as_millisecond();
    let fire_at = Timestamp::from_millisecond(now_ms + 10_000).unwrap(); // +10s

    let id = store
        .schedule(FireSpec {
            fire_at,
            kind: "task_alarm".into(),
            ref_id: Some("task:snooze-me".into()),
            payload: serde_json::json!({}),
            dedup_prefix: Some("task:snooze-me:alarm:".into()),
        })
        .await
        .unwrap();

    // Simulate snooze: advance fire_at_ms by 60 s via a raw SQL UPDATE.
    let snoozed_ms = now_ms + 60_000;
    sqlx::query("UPDATE scheduled_fires SET fire_at_ms = ?1 WHERE id = ?2")
        .bind(snoozed_ms)
        .bind(&id)
        .execute(pool.inner())
        .await
        .unwrap();

    // Row still exists and reports the new fire_at_ms.
    let pending = repo.list_pending_up_to_ms(i64::MAX).await.unwrap();
    assert_eq!(pending.len(), 1, "row still present after snooze");
    assert_eq!(
        pending[0].fire_at_ms, snoozed_ms,
        "fire_at_ms updated to snoozed value"
    );
    assert!(!pending[0].fired, "row still pending (not fired)");
}
