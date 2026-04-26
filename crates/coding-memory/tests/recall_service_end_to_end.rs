use coding_memory::recall::budget::HeuristicBudgeter;
use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig};
use coding_memory::RecallInvocationRepo;
use std::sync::Arc;
use storage::StoragePool;

#[tokio::test]
async fn recall_index_returns_entries_and_writes_telemetry() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let fact_repo = Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));
    let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));
    let ums = Arc::new(cognitive::UnifiedMemoryService::new((*fact_repo).clone()));
    let telem = RecallInvocationRepo::new(pool.clone());

    // Seed
    let fact = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "work".into(),
        subject: "auth_module".into(),
        predicate: "uses".into(),
        object: "JWT HS256".into(),
        recorded_at: jiff::Timestamp::now().to_string(),
        confidence: 0.9,
        scope_repo_id: Some("repo:demo".into()),
        memory_type: "repo_context".into(),
        ..Default::default()
    };
    fact_repo
        .upsert_with_metadata(&fact, Some("repo:demo"), None)
        .await
        .unwrap();

    let svc = CodingRecallService::new(
        CodingRecallServiceConfig::default(),
        ums,
        fact_repo,
        ep_repo,
        telem,
        Arc::new(HeuristicBudgeter),
    );
    let resp = svc
        .recall_index("JWT auth", Some("repo:demo"), None, None, 10)
        .await
        .unwrap();
    assert!(!resp.results.is_empty());
    let log = svc
        .telemetry_repo()
        .list_recent(10, 0, Some("index"))
        .await
        .unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].layer, "index");
}
