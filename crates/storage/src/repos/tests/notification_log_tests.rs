use crate::pool::StoragePool;
use crate::repos::notification_log::NotificationLogRepo;

async fn setup() -> NotificationLogRepo {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    crate::test_util::run_notifications_migrations(pool.inner()).await;
    NotificationLogRepo::new(pool.inner().clone())
}

#[tokio::test]
async fn insert_or_ignore_gates_duplicate_deliveries() {
    let repo = setup().await;
    let inserted1 = repo.try_insert("fire_1", "os_native", 100).await.unwrap();
    let inserted2 = repo.try_insert("fire_1", "os_native", 200).await.unwrap();
    assert!(inserted1, "first insert must succeed");
    assert!(!inserted2, "duplicate must be ignored");
}

#[tokio::test]
async fn per_channel_rows_independent() {
    let repo = setup().await;
    assert!(repo.try_insert("fire_1", "os_native", 100).await.unwrap());
    assert!(repo.try_insert("fire_1", "tray", 100).await.unwrap());
    assert!(repo.try_insert("fire_1", "telegram", 100).await.unwrap());
}

#[tokio::test]
async fn record_error_updates_existing_row() {
    let repo = setup().await;
    repo.try_insert("fire_1", "telegram", 100).await.unwrap();
    repo.record_error("fire_1", "telegram", "rate limited")
        .await
        .unwrap();
    let row = repo.get("fire_1", "telegram").await.unwrap().unwrap();
    assert_eq!(row.error.as_deref(), Some("rate limited"));
}
