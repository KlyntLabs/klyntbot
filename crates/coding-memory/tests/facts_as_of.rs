use coding_memory::recall::facts_as_of::FactsAsOfService;
use jiff::Timestamp;

#[tokio::test]
async fn returns_row_valid_at_timestamp() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));

    let t0 = Timestamp::now();
    let fact = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        subject: "auth".into(),
        predicate: "uses".into(),
        object: "JWT".into(),
        recorded_at: t0.to_string(),
        confidence: 0.8,
        scope_repo_id: Some("repo:x".into()),
        memory_type: "repo_context".into(),
        valid_from: t0.to_string(),
        valid_until: None,
        ..Default::default()
    };
    fact_repo.upsert_with_metadata(&fact, Some("repo:x"), None).await.unwrap();

    let svc = FactsAsOfService::new(fact_repo);
    let resp = svc
        .query("auth", "uses", t0.saturating_add(jiff::SignedDuration::from_secs(60)).unwrap())
        .await
        .unwrap();
    assert_eq!(resp.rows.len(), 1);
    assert_eq!(resp.rows[0].object, "JWT");
}
