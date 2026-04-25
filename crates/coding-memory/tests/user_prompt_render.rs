use coding_memory::recall::renderers::render_user_prompt_block;
use coding_memory::recall::budget::{HeuristicBudgeter, TokenBudgeter};

#[tokio::test]
async fn no_dead_end_no_warn_block() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));
    let ep_repo = std::sync::Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));
    let ums = std::sync::Arc::new(cognitive::UnifiedMemoryService::new((*fact_repo).clone()));
    let svc = std::sync::Arc::new(coding_memory::recall::CodingRecallService::new(
        Default::default(),
        ums, fact_repo, ep_repo,
        coding_memory::RecallInvocationRepo::new(pool.clone()),
        std::sync::Arc::new(HeuristicBudgeter),
    ));
    let md = render_user_prompt_block(&svc, "fix parser bug", Some("repo:demo"))
        .await
        .unwrap();
    assert!(!md.contains("⚠️ Heads-up"));
    assert!(HeuristicBudgeter.count(&md) <= 1500);
}

#[tokio::test]
async fn dead_end_seeded_yields_warn_block() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations()).await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations()).await.unwrap();
    let fact_repo = std::sync::Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));
    let ep_repo = std::sync::Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));
    let ums = std::sync::Arc::new(cognitive::UnifiedMemoryService::new((*fact_repo).clone()));

    let cf = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        domain: "coding".into(),
        subject: "rewrite parser as recursive descent".into(),
        predicate: "outcome".into(),
        object: "abandoned".into(),
        recorded_at: jiff::Timestamp::now().to_string(),
        confidence: 0.95,
        scope_repo_id: Some("repo:demo".into()),
        memory_type: "counterfactual".into(),
        metadata: Some(r#"{"memory_type":"counterfactual","reason":"too slow","attempt_id":"00000000-0000-0000-0000-000000000001","problem_hash":"abc"}"#.into()),
        ..Default::default()
    };
    fact_repo.upsert_with_metadata(&cf, Some("repo:demo"), cf.metadata.as_deref()).await.unwrap();

    let svc = std::sync::Arc::new(coding_memory::recall::CodingRecallService::new(
        Default::default(),
        ums, fact_repo, ep_repo,
        coding_memory::RecallInvocationRepo::new(pool.clone()),
        std::sync::Arc::new(HeuristicBudgeter),
    ));
    let md = render_user_prompt_block(
        &svc,
        "rewrite parser as recursive descent",
        Some("repo:demo"),
    )
    .await
    .unwrap();
    assert!(md.contains("⚠️ Heads-up"), "expected warn block; got:\n{md}");
}
