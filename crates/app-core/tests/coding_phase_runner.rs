use app_core::coding_memory::reforge::CodingPhaseRunnerImpl;
use cognitive::services::reforge::CodingPhaseRunner;
use storage::StoragePool;

#[tokio::test]
async fn empty_pool_runs_clean_in_all_4_phases() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let runner = CodingPhaseRunnerImpl::new_for_test(pool.clone());

    runner.run_synthesis().await.unwrap();
    runner.run_rule_artifacts().await.unwrap();
    runner.run_selective_delete().await.unwrap();
    runner.run_cross_session_dedup().await.unwrap();
}
