use std::sync::Arc;

use bus::{AlarmEvent, DomainEventBus};
use notifications::{
    channel::{os_native::OsNativeChannel, ChannelRegistry},
    held::HeldReleaseService,
    retry::RetryPolicy,
    NotificationDispatcher,
};
use scheduling::FireStore;
use storage::{
    repos::{
        held_notifications::HeldNotificationsRepo, notification_log::NotificationLogRepo,
        scheduled_fires::ScheduledFiresRepo,
    },
    StoragePool,
};

struct CountingSender(Arc<std::sync::atomic::AtomicUsize>);

#[async_trait::async_trait]
impl common::NotificationSender for CountingSender {
    async fn send(&self, _title: &str, _body: &str) -> common::Result<()> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

async fn setup_pool() -> StoragePool {
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
    pool
}

#[tokio::test]
async fn duplicate_alarm_fires_once_per_channel() {
    let pool = setup_pool().await;

    let bus = Arc::new(DomainEventBus::new(64));
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut registry = ChannelRegistry::new();
    registry.register(Arc::new(OsNativeChannel::new(Arc::new(CountingSender(
        counter.clone(),
    )))));

    let log_repo = NotificationLogRepo::new(pool.inner().clone());
    let held_repo = HeldNotificationsRepo::new(pool.inner().clone());
    let fires_repo = ScheduledFiresRepo::new(pool.inner().clone());
    let fire_store = FireStore::new(fires_repo);
    let held_rel = HeldReleaseService::new(held_repo.clone(), fire_store);

    let dispatcher = NotificationDispatcher::new(
        bus.clone(),
        registry,
        vec!["os_native".into()],
        None,
        log_repo,
        held_repo,
        held_rel,
        RetryPolicy::default(),
    );
    let handle = dispatcher.start();

    let payload = serde_json::json!({"title": "t", "body": "b"}).to_string();
    for _ in 0..3 {
        bus.publish_alarm(AlarmEvent::AlarmFired {
            fire_id: "fire_1".into(),
            kind: "task_alarm".into(),
            ref_id: None,
            payload_json: payload.clone(),
            fired_at_ms: 0,
        });
    }

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    handle.shutdown.cancel();
    let _ = handle.join.await;

    assert_eq!(
        counter.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "idempotency gate must suppress duplicates"
    );
}
