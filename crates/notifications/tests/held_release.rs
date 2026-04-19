use jiff::Timestamp;
use notifications::{
    channel::{NotificationPayload, Priority},
    held::HeldReleaseService,
};
use scheduling::FireStore;
use storage::repos::held_notifications::HeldNotificationsRepo;
use storage::repos::scheduled_fires::ScheduledFiresRepo;
use storage::StoragePool;

async fn setup_stores() -> (HeldNotificationsRepo, FireStore, StoragePool) {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(notifications::migration().sql.as_str())
        .execute(pool.inner())
        .await
        .unwrap();
    sqlx::query(include_str!(
        "../../scheduling/migrations/001_scheduled_fires.sql"
    ))
    .execute(pool.inner())
    .await
    .unwrap();

    let held_repo = HeldNotificationsRepo::new(pool.inner().clone());
    let fire_store = FireStore::new(ScheduledFiresRepo::new(pool.inner().clone()));
    (held_repo, fire_store, pool)
}

#[tokio::test]
async fn hold_inserts_row_and_schedules_release_fire() {
    let (held_repo, fire_store, _pool) = setup_stores().await;

    let svc = HeldReleaseService::new(held_repo.clone(), fire_store.clone());

    let payload = NotificationPayload {
        alarm_id: "fire_1".into(),
        title: "t".into(),
        body: "b".into(),
        priority: Priority::Normal,
    };
    let release_at = Timestamp::now()
        .checked_add(std::time::Duration::from_secs(600))
        .unwrap();

    svc.hold("fire_1", &["telegram".to_string()], &payload, release_at)
        .await
        .unwrap();

    let pending = held_repo.list_pending_before(i64::MAX).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].alarm_id, "fire_1");

    let due = fire_store
        .pending_with_kind_before(i64::MAX, "held_release")
        .await
        .unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].fire_at_ms, release_at.as_millisecond());
}
