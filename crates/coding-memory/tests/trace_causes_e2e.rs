use coding_memory::causal::CausalEdgeRepo;
use coding_memory::scope::{CausalEdge, CausalEdgeKind};
use coding_memory::{CodingMemoryToolset, CodingRecallService};
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
async fn trace_causes_returns_chain() {
    let pool = fresh_pool().await;
    let edges = Arc::new(CausalEdgeRepo::new(pool.clone()));
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

    let fact_repo = Arc::new(SemanticFactRepo::new(pool.inner().clone()));
    let ep_repo = Arc::new(EpisodicMemoryRepo::new(pool.inner().clone()));
    let ums = Arc::new(cognitive::UnifiedMemoryService::new((*fact_repo).clone()));
    let telemetry = coding_memory::RecallInvocationRepo::new(pool.clone());
    let budgeter = Arc::new(coding_memory::recall::budget::HeuristicBudgeter);
    let svc = CodingRecallService::new(
        Default::default(),
        ums,
        fact_repo,
        ep_repo,
        telemetry,
        budgeter,
    )
    .with_causal_repo(edges.clone());

    let toolset = CodingMemoryToolset::new(Arc::new(svc));
    let resp = toolset
        .dispatch(
            "trace_causes",
            serde_json::json!({"subject": a.to_string(), "depth": 2}),
        )
        .await
        .expect("trace_causes ok");
    let descendants = resp["descendants"].as_array().expect("array");
    assert_eq!(descendants.len(), 1);
}
