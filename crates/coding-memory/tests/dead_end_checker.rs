use coding_memory::recall::dead_end::{DeadEndChecker, DeadEndConfig};

#[tokio::test]
async fn no_facts_returns_empty() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));
    let checker = DeadEndChecker::new(fact_repo, DeadEndConfig::default());
    let resp = checker
        .check("rewrite the parser as a recursive descent", Some("repo:foo"))
        .await
        .expect("ok");
    assert!(resp.matches.is_empty());
    assert_eq!(resp.aggregate_confidence, 0.0);
}
