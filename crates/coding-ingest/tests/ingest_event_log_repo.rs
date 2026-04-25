use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use jiff::Timestamp;
use std::path::PathBuf;
use storage::StoragePool;
use uuid::Uuid;

fn sample_event() -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        cwd: PathBuf::from("/tmp/repo"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt {
            text: "hello".into(),
            attachments: vec![],
        },
    })
}

#[tokio::test]
async fn repo_inserts_and_lists_unprocessed() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let repo = IngestEventLogRepo::new(pool.inner().clone());
    let evt = sample_event();
    repo.insert(&evt).await.expect("insert");

    let rows = repo.list_unprocessed(100).await.expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].session_id, "s1");
    assert!(!rows[0].processed);
}

#[tokio::test]
async fn repo_count_by_session() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = IngestEventLogRepo::new(pool.inner().clone());
    repo.insert(&sample_event()).await.unwrap();
    repo.insert(&sample_event()).await.unwrap();
    assert_eq!(repo.count_by_session("s1").await.unwrap(), 2);
    assert_eq!(repo.count_by_session("missing").await.unwrap(), 0);
}

#[tokio::test]
async fn repo_mark_processed_advances_cursor() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = IngestEventLogRepo::new(pool.inner().clone());

    let e1 = sample_event();
    let e2 = sample_event();
    let AgentEvent::V1(v1) = &e1;
    let id1 = v1.id.to_string();
    let AgentEvent::V1(v2) = &e2;
    let id2 = v2.id.to_string();
    repo.insert(&e1).await.unwrap();
    repo.insert(&e2).await.unwrap();

    assert_eq!(repo.count_unprocessed().await.unwrap(), 2);
    assert!(repo.last_distilled_at().await.unwrap().is_none());

    let n = repo.mark_processed(&[id1.as_str()]).await.unwrap();
    assert_eq!(n, 1);
    assert_eq!(repo.count_unprocessed().await.unwrap(), 1);
    assert!(repo.last_distilled_at().await.unwrap().is_some());

    // Idempotent: re-marking a processed row reports 0 affected? Actually SQLite
    // counts the matching rows whether or not the value changed; we just assert
    // it doesn't error and unprocessed count stays at 1.
    let _ = repo.mark_processed(&[id1.as_str()]).await.unwrap();
    assert_eq!(repo.count_unprocessed().await.unwrap(), 1);

    // Empty input is a no-op.
    assert_eq!(repo.mark_processed(&[]).await.unwrap(), 0);

    // Mark the second row.
    repo.mark_processed(&[id2.as_str()]).await.unwrap();
    assert_eq!(repo.count_unprocessed().await.unwrap(), 0);
}
