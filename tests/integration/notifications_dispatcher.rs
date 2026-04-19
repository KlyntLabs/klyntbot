//! End-to-end: insert a past-due `scheduled_fires` row →
//! TemporalScheduler emits AlarmFired → NotificationDispatcher delivers
//! to a mock channel exactly once (notification_log gate).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use notifications::{
    channel::{Channel, ChannelRegistry, NotificationPayload},
    held::HeldReleaseService,
    retry::RetryPolicy,
    NotificationDispatcher,
};
use scheduling::temporal::fire_store::{FireSpec, FireStore};
use scheduling::temporal::{SchedulerConfig, TemporalScheduler};
use storage::repos::held_notifications::HeldNotificationsRepo;
use storage::repos::notification_log::NotificationLogRepo;
use storage::repos::scheduled_fires::ScheduledFiresRepo;
use storage::StoragePool;
use tools_core::FeatureMigration;

struct Mock {
    name: String,
    hits: Arc<AtomicUsize>,
}

#[async_trait]
impl Channel for Mock {
    fn name(&self) -> &str {
        &self.name
    }

    async fn deliver(&self, _p: &NotificationPayload) -> notifications::Result<()> {
        self.hits.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scheduler_fires_dispatcher_delivers_e2e() {
    let pool = StoragePool::connect_in_memory().await.unwrap();

    // Apply scheduling and notifications migrations on top of the base schema.
    StoragePool::run_feature_migrations(
        pool.inner(),
        &[
            FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../crates/scheduling/migrations/001_scheduled_fires.sql")
                    .into(),
            },
            notifications::migration(),
        ],
    )
    .await
    .unwrap();

    let bus = Arc::new(DomainEventBus::new(64));
    let hits = Arc::new(AtomicUsize::new(0));

    // Build the scheduler.
    let fires_repo = ScheduledFiresRepo::new(pool.inner().clone());
    let fire_store = FireStore::new(fires_repo);
    let scheduler =
        TemporalScheduler::new(fire_store.clone(), bus.clone(), SchedulerConfig::default());

    // Build the dispatcher.
    let mut reg = ChannelRegistry::new();
    reg.register(Arc::new(Mock {
        name: "tray".into(),
        hits: hits.clone(),
    }));
    let log_repo = NotificationLogRepo::new(pool.inner().clone());
    let held_repo = HeldNotificationsRepo::new(pool.inner().clone());
    let held_rel = HeldReleaseService::new(held_repo.clone(), fire_store.clone());

    let disp = NotificationDispatcher::new(
        bus.clone(),
        reg,
        vec!["tray".into()],
        None,
        log_repo,
        held_repo,
        held_rel,
        RetryPolicy::default(),
    );
    let disp_handle = disp.start();

    // Start the scheduler loop.
    let _sched_handle = scheduler.clone().start_background();

    // Insert a past-due fire.
    let past = jiff::Timestamp::now()
        .checked_sub(jiff::Span::new().seconds(1))
        .unwrap();
    let fire_id = fire_store
        .schedule(FireSpec {
            fire_at: past,
            kind: "task_alarm".into(),
            ref_id: Some("task_1".into()),
            payload: serde_json::json!({"title": "T", "body": "B"}),
            dedup_prefix: Some("task:task_1:".into()),
        })
        .await
        .unwrap();

    // Wake the scheduler so it processes immediately.
    scheduler.wake();

    tokio::time::sleep(Duration::from_millis(500)).await;

    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "alarm delivered exactly once"
    );

    // Idempotency gate: publish the same AlarmFired event again with the same
    // fire_id (= alarm_id used by the log repo). The notification_log already
    // has a row for this alarm_id; dispatch_one must suppress the duplicate.
    bus.publish(DomainEvent::AlarmFired {
        fire_id: fire_id.clone(),
        kind: "task_alarm".into(),
        ref_id: Some("task_1".into()),
        payload_json: r#"{"title":"T","body":"B"}"#.into(),
        fired_at_ms: jiff::Timestamp::now().as_millisecond(),
    });

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert_eq!(
        hits.load(Ordering::SeqCst),
        1,
        "duplicate alarm suppressed by notification_log gate"
    );

    // Graceful shutdown.
    disp_handle.shutdown.cancel();
    let _ = disp_handle.join.await;
    scheduler.shutdown();
}
