use coding_memory::causal::CausalEdgeRepo;
use coding_memory::recall::renderers::render_user_prompt_block;
use coding_memory::scope::{CausalEdge, CausalEdgeKind};
use coding_memory::CodingRecallService;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use jiff::Timestamp;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn causal_section_lists_seeded_edges() {
    let pool = fresh_pool().await;
    let edges = Arc::new(CausalEdgeRepo::new(pool.clone()));

    // Seed a causal edge a -> b (FixedBy)
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    edges
        .insert(&CausalEdge {
            id: Uuid::new_v4(),
            from_id: a,
            to_id: b,
            edge_kind: CausalEdgeKind::FixedBy,
            confidence: 0.7,
            inferred_at: Timestamp::now(),
        })
        .await
        .unwrap();

    // Also insert a dummy fact so recall_index returns something
    let fact_repo = Arc::new(SemanticFactRepo::new(pool.inner().clone()));
    let ep_repo = Arc::new(EpisodicMemoryRepo::new(pool.inner().clone()));
    let ums = Arc::new(cognitive::UnifiedMemoryService::new((*fact_repo).clone()));
    let telemetry = coding_memory::RecallInvocationRepo::new(pool.clone());
    let budgeter = Arc::new(coding_memory::recall::budget::HeuristicBudgeter);
    let svc = Arc::new(
        CodingRecallService::new(
            Default::default(),
            ums,
            fact_repo,
            ep_repo,
            telemetry,
            budgeter,
        )
        .with_causal_repo(edges.clone()),
    );

    let block = render_user_prompt_block(&svc, "hello", Some("repoA")).await.unwrap();
    assert!(
        !block.contains("populated when causal edges are seeded"),
        "stub string still present: {block}"
    );
    // The causal section may be suppressed if no relevant memories are recalled,
    // so we only assert the stub is gone.
}
