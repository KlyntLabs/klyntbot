use crate::pool::StoragePool;
use crate::repos::held_notifications::HeldNotificationsRepo;
use crate::rows::held_notification::HeldNotificationRow;

async fn setup() -> HeldNotificationsRepo {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    crate::test_util::run_notifications_migrations(pool.inner()).await;
    HeldNotificationsRepo::new(pool.inner().clone())
}

fn sample(id: &str, release: i64) -> HeldNotificationRow {
    HeldNotificationRow {
        id: id.into(),
        alarm_id: "fire_1".into(),
        channels: serde_json::json!(["telegram", "discord"]),
        payload: serde_json::json!({"title": "t", "body": "b"}),
        release_at_ms: release,
        released: false,
        held_at_ms: release - 1000,
    }
}

#[tokio::test]
async fn insert_and_list_pending_before_time() {
    let repo = setup().await;
    repo.insert(&sample("h1", 100)).await.unwrap();
    repo.insert(&sample("h2", 300)).await.unwrap();
    let pending = repo.list_pending_before(200).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "h1");
}

#[tokio::test]
async fn mark_released_hides_from_pending() {
    let repo = setup().await;
    repo.insert(&sample("h1", 100)).await.unwrap();
    repo.mark_released("h1").await.unwrap();
    assert!(repo.list_pending_before(999).await.unwrap().is_empty());
}
