//! E2E: schedule a fire via the facade-exposed TemporalScheduler, observe
//! AlarmFired on the bus. Exercises the full jiff-native FireStore API and
//! the scheduler's wall-clock-anchored loop.

use std::sync::Arc;
use std::time::Duration;

use bus::{DomainEvent, DomainEventBus};
use scheduling::temporal::fire_store::{FireSpec, FireStore};
use scheduling::temporal::{SchedulerConfig, TemporalScheduler};
use storage::pool::StoragePool;
use storage::repos::scheduled_fires::ScheduledFiresRepo;
use tools_core::FeatureMigration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn end_to_end_fires_and_emits_event() {
    // Bootstrap storage + scheduling migration (the initial storage migration
    // runs automatically via connect_in_memory; add the scheduling feature
    // migration on top).
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

    let store = FireStore::new(ScheduledFiresRepo::new(pool.inner().clone()));
    let bus = Arc::new(DomainEventBus::new(32));
    let mut rx = bus.subscribe();

    let scheduler = TemporalScheduler::new(store.clone(), bus.clone(), SchedulerConfig::default());
    let _loop_handle = scheduler.clone().start_background();

    // Schedule a fire ~100ms in the future and wake the loop.
    let fire_at = jiff::Timestamp::now()
        .checked_add(jiff::Span::new().milliseconds(100))
        .unwrap();
    store
        .schedule(FireSpec {
            fire_at,
            kind: "test".into(),
            ref_id: Some("x".into()),
            payload: serde_json::json!({ "m": "hi" }),
            dedup_prefix: None,
        })
        .await
        .unwrap();
    scheduler.wake();

    // Receive the AlarmFired event within a generous timeout.
    let event = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("no AlarmFired event received in time")
        .unwrap();

    match event {
        DomainEvent::Alarm(bus::AlarmEvent::AlarmFired { kind, ref_id, .. }) => {
            assert_eq!(kind, "test");
            assert_eq!(ref_id.as_deref(), Some("x"));
        }
        other => panic!("expected AlarmFired, got {other:?}"),
    }
}
