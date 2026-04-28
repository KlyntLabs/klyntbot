use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::pending_invalidations::PendingInvalidationsRepo;
use jiff::Timestamp;
use std::path::PathBuf;
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
async fn append_and_drain_returns_unprocessed() {
    let pool = fresh_pool().await;
    let repo = PendingInvalidationsRepo::new(pool.clone());
    let event = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "git:abc".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::GitCommit {
            commit_hash: "abc".into(),
            parent_hash: None,
            repo_root: PathBuf::from("/tmp"),
            changed_files: vec![PathBuf::from("a.rs")],
        },
    });
    repo.append(&event).await.unwrap();
    let drained = repo.drain_unprocessed().await.unwrap();
    assert_eq!(drained.len(), 1);

    repo.mark_processed(&drained[0].0).await.unwrap();
    let again = repo.drain_unprocessed().await.unwrap();
    assert!(again.is_empty());
}
