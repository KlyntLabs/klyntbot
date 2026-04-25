use coding_memory::recall::renderers::render_session_start_block;
use coding_memory::recall::budget::{HeuristicBudgeter, TokenBudgeter};

#[tokio::test]
async fn within_budget_and_well_formed() {
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

    let md = render_session_start_block(&svc, Some("repo:demo"))
        .await
        .unwrap();
    let bud = HeuristicBudgeter;
    assert!(bud.count(&md) <= 800, "got {} tokens", bud.count(&md));
    assert!(md.contains("Project memory"));
}
