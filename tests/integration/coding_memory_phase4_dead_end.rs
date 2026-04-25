use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig};
use coding_memory::recall::budget::HeuristicBudgeter;
use std::sync::Arc;

#[tokio::test]
async fn repeat_attempt_yields_warning() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let fact_repo = Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));
    let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));

    // Seed a counterfactual.
    let cf = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "work".into(),
        subject: "rewrite parser as recursive descent".into(),
        predicate: "outcome".into(),
        object: "abandoned — too slow".into(),
        recorded_at: jiff::Timestamp::now().to_string(),
        confidence: 0.95,
        source: "distiller".into(),
        valid_from: jiff::Timestamp::now().to_string(),
        valid_until: None,
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 1.0,
        project_id: None,
        memory_type: "counterfactual".into(),
        scope_type: "repo".into(),
        scope_id: None,
        scope_repo_id: Some("repo:demo".into()),
        metadata: Some(r#"{"memory_type":"counterfactual","reason":"too slow","attempt_id":"00000000-0000-0000-0000-000000000001","problem_hash":"abc"}"#.into()),
    };
    fact_repo.upsert_with_metadata(&cf, cf.scope_repo_id.as_deref(), cf.metadata.as_deref()).await.unwrap();

    let ums = Arc::new(cognitive::UnifiedMemoryService::new((*fact_repo).clone()));
    let telem = coding_memory::RecallInvocationRepo::new(pool.clone());
    let svc = Arc::new(CodingRecallService::new(
        CodingRecallServiceConfig::default(),
        ums, fact_repo, ep_repo, telem,
        Arc::new(HeuristicBudgeter),
    ));
    let md = coding_memory::recall::renderers::render_user_prompt_block(
        &svc, "rewrite parser as recursive descent", Some("repo:demo")
    ).await.unwrap();
    assert!(md.contains("Heads-up") || md.contains("dead end") || md.contains("counterfactual"), "got:\n{md}");
}
