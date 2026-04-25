use app_core::coding_memory::reforge::register_session_end_dispatch;
use coding_memory::reforge::SessionEndPass;
use coding_memory::recall::telemetry::RecallInvocationRepo;
use storage::StoragePool;

#[tokio::test]
async fn session_end_event_triggers_pass() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cog");
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .expect("coding");
    let summaries = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let co_act = cognitive::CoActivationRepo::new(pool.inner().clone());
    let utilization = RecallInvocationRepo::new(pool.clone());
    let pass = std::sync::Arc::new(SessionEndPass::new(summaries.clone(), co_act, utilization));

    let bus = std::sync::Arc::new(bus::DomainEventBus::new(64));
    register_session_end_dispatch(bus.clone(), pass.clone()).await;

    bus.publish(bus::DomainEvent::CodingSessionEnded {
        session_id: "s1".into(),
        repo_id: Some("repo:test".into()),
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let row = summaries
        .latest_for_session("s1")
        .await
        .expect("latest")
        .expect("row written");
    assert_eq!(row.session_id, "s1");
}
