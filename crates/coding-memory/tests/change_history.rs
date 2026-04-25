use coding_memory::recall::change_history::ChangeHistoryService;

#[tokio::test]
async fn empty_history_returns_empty_steps() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let svc = ChangeHistoryService::new(std::sync::Arc::new(
        cognitive::SemanticFactRepo::new(pool.inner().clone()),
    ));
    let r = svc.query("nonexistent", "uses", None).await.unwrap();
    assert!(r.steps.is_empty());
}
