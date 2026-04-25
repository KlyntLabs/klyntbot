use coding_memory::recall::fetch_builder::FetchBuilder;
use jiff::Timestamp;

#[tokio::test]
async fn fact_fetch_round_trip() {
    use storage::StoragePool;
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));
    let ep_repo = std::sync::Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));

    let fact = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        subject: "module".into(),
        predicate: "uses".into(),
        object: "lib".into(),
        recorded_at: Timestamp::now().to_string(),
        confidence: 0.9,
        scope_repo_id: Some("repo:x".into()),
        memory_type: "repo_context".into(),
        metadata: Some(r#"{"provenance": {"source_events": ["evt1"]}}"#.into()),
        ..Default::default()
    };
    fact_repo
        .upsert_with_metadata(&fact, Some("repo:x"), fact.metadata.as_deref())
        .await
        .unwrap();

    let builder = FetchBuilder::new(fact_repo.clone(), ep_repo.clone());
    let out = builder
        .fetch(std::slice::from_ref(&fact.id), true, false)
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, "repo_context");
    assert!(out[0].metadata.get("provenance").is_some());
}
