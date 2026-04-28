use coding_memory::causal::CausalEdgeRepo;
use coding_memory::recall::causal_walker::CausalWalker;
use coding_memory::scope::{CausalEdge, CausalEdgeKind};
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

fn edge(from: Uuid, to: Uuid, kind: CausalEdgeKind) -> CausalEdge {
    CausalEdge {
        id: Uuid::new_v4(),
        from_id: from,
        to_id: to,
        edge_kind: kind,
        confidence: 0.7,
        inferred_at: Timestamp::now(),
    }
}

#[tokio::test]
async fn walks_descendants_to_depth() {
    let pool = fresh_pool().await;
    let repo = Arc::new(CausalEdgeRepo::new(pool.clone()));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    let c = Uuid::new_v4();
    let d = Uuid::new_v4();
    for (f, t) in [(a, b), (b, c), (c, d)] {
        repo.insert(&edge(f, t, CausalEdgeKind::FixedBy))
            .await
            .unwrap();
    }
    let walker = CausalWalker::new(repo.clone());
    let resp = walker.walk(a, 2).await.unwrap();
    let descendants_to: std::collections::HashSet<_> =
        resp.descendants.iter().map(|e| e.to_id).collect();
    assert!(descendants_to.contains(&b));
    assert!(descendants_to.contains(&c));
    assert!(!descendants_to.contains(&d), "depth 2 should stop at c");
}

#[tokio::test]
async fn walker_handles_cycles() {
    let pool = fresh_pool().await;
    let repo = Arc::new(CausalEdgeRepo::new(pool.clone()));
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();
    repo.insert(&edge(a, b, CausalEdgeKind::FixedBy))
        .await
        .unwrap();
    repo.insert(&edge(b, a, CausalEdgeKind::SharesRootCause))
        .await
        .unwrap();
    let walker = CausalWalker::new(repo.clone());
    let resp = walker.walk(a, 5).await.unwrap();
    assert!(!resp.descendants.is_empty());
    assert!(!resp.ancestors.is_empty());
}
