use coding_memory::recall::decision_points::DecisionPointsService;

#[tokio::test]
async fn empty_returns_empty_rows() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let svc = DecisionPointsService::new(std::sync::Arc::new(
        cognitive::EpisodicMemoryRepo::new(pool.inner().clone()),
    ));
    let r = svc.list(Some("repo:x"), 50).await.unwrap();
    assert!(r.rows.is_empty());
}
