use coding_ingest::daemon::{drain_buffer, spawn, IngestDaemonConfig};
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use coding_ingest::transport::{FileBufferFallback, IngestSocket};
use jiff::Timestamp;
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use tempfile::TempDir;
use uuid::Uuid;

fn evt(i: u32) -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: format!("s-{i}"),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt {
            text: "x".into(),
            attachments: vec![],
        },
    })
}

#[tokio::test]
async fn buffered_events_drain_into_log() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));

    let dir = TempDir::new().unwrap();
    let buffer_path = dir.path().join("buf.jsonl");
    let buf = FileBufferFallback::new(buffer_path.clone());
    buf.send(&evt(0)).await.unwrap();
    buf.send(&evt(1)).await.unwrap();
    buf.send(&evt(2)).await.unwrap();

    let n = drain_buffer(&buffer_path, repo.as_ref()).await.unwrap();
    assert_eq!(n, 3);
    assert_eq!(repo.count_unprocessed().await.unwrap(), 3);
    assert!(!buffer_path.exists());
    // Archive sibling present.
    let siblings: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
        .filter(|n| n.contains(".done."))
        .collect();
    assert_eq!(siblings.len(), 1);
}

#[tokio::test]
async fn daemon_start_drains_pre_existing_buffer() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    let dir = TempDir::new().unwrap();
    let buffer_path = dir.path().join("buf.jsonl");
    FileBufferFallback::new(buffer_path.clone())
        .send(&evt(0))
        .await
        .unwrap();

    let handle = spawn(IngestDaemonConfig {
        socket_path: dir.path().join("s.sock"),
        buffer_path: buffer_path.clone(),
        lock_path: dir.path().join("desktop.lock"),
        repo: repo.clone(),
        event_tx: None,
        op_handler: None,
    })
    .await
    .unwrap();

    // drain is synchronous part of spawn — by the time we have a handle, it's done.
    assert_eq!(repo.count_unprocessed().await.unwrap(), 1);
    assert!(!buffer_path.exists());
    handle.shutdown().await;
}
