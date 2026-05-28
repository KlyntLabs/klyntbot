use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use bus::{AlarmEvent, DomainEvent, DomainEventBus};
use config::schema::QuietHoursConfig;
use notifications::{
    channel::{Channel, ChannelRegistry, NotificationPayload},
    held::HeldReleaseService,
    quiet_hours::QuietHoursPolicy,
    retry::RetryPolicy,
    NotificationDispatcher,
};
use scheduling::FireStore;
use storage::repos::held_notifications::HeldNotificationsRepo;
use storage::repos::notification_log::NotificationLogRepo;
use storage::repos::scheduled_fires::ScheduledFiresRepo;
use storage::StoragePool;

struct Counting {
    name: String,
    count: Arc<AtomicUsize>,
}

#[async_trait]
impl Channel for Counting {
    fn name(&self) -> &str {
        &self.name
    }
    async fn deliver(&self, _p: &NotificationPayload) -> notifications::Result<()> {
        self.count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn quiet_hours_holds_then_release_fires_delivery() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(notifications::migration().sql.as_str())
        .execute(pool.inner())
        .await
        .unwrap();
    let scheduling_sql = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../scheduling/migrations/001_scheduled_fires.sql"
    ));
    sqlx::query(scheduling_sql)
        .execute(pool.inner())
        .await
        .unwrap();

    let bus = Arc::new(DomainEventBus::new(64));
    let counter = Arc::new(AtomicUsize::new(0));

    let mut reg = ChannelRegistry::new();
    reg.register(Arc::new(Counting {
        name: "telegram".into(),
        count: counter.clone(),
    }));

    // Near-24/7 quiet window — almost everything gets held.
    let qh = QuietHoursPolicy::new(
        QuietHoursConfig {
            enabled: true,
            start: "00:00".into(),
            end: "23:59".into(),
            override_for_urgent_tasks: false,
        },
        "UTC",
    )
    .unwrap();

    let log = NotificationLogRepo::new(pool.inner().clone());
    let held_repo = HeldNotificationsRepo::new(pool.inner().clone());
    let fires_repo = ScheduledFiresRepo::new(pool.inner().clone());
    let fire_store = FireStore::new(fires_repo);
    let held_rel = HeldReleaseService::new(held_repo.clone(), fire_store);

    let dispatcher = NotificationDispatcher::new(
        bus.clone(),
        reg,
        vec!["telegram".into()],
        Some(qh),
        log,
        held_repo.clone(),
        held_rel,
        RetryPolicy::default(),
    );
    let handle = dispatcher.start();

    // Initial fire — should be held (quiet hours active).
    bus.publish_alarm(AlarmEvent::AlarmFired {
        fire_id: "fire_1".into(),
        kind: "task_alarm".into(),
        ref_id: None,
        payload_json: serde_json::json!({"title": "t", "body": "b"}).to_string(),
        fired_at_ms: jiff::Timestamp::now().as_millisecond(),
    });

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    assert_eq!(counter.load(Ordering::SeqCst), 0, "held, no delivery yet");
    let pending = held_repo.list_pending_before(i64::MAX).await.unwrap();
    assert_eq!(pending.len(), 1, "one held notification row");

    let held_id = pending[0].id.clone();

    // Simulate the scheduler firing the held_release alarm.
    bus.publish_alarm(AlarmEvent::AlarmFired {
        fire_id: format!("release_{held_id}"),
        kind: "held_release".into(),
        ref_id: None,
        payload_json: serde_json::json!({"held_id": held_id}).to_string(),
        fired_at_ms: jiff::Timestamp::now().as_millisecond(),
    });

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    handle.shutdown.cancel();
    let _ = handle.join.await;

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "held delivery released exactly once"
    );
    assert!(
        held_repo
            .list_pending_before(i64::MAX)
            .await
            .unwrap()
            .is_empty(),
        "no pending held notifications remain"
    );
}
