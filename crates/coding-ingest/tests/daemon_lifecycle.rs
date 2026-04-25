use coding_ingest::daemon::{spawn, IngestDaemonConfig};
use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::store::IngestEventLogRepo;
use coding_ingest::transport::{IngestSocket, UnixIngestSocket};
use jiff::Timestamp;
use std::path::PathBuf;
use std::sync::Arc;
use storage::StoragePool;
use tempfile::TempDir;
use uuid::Uuid;

#[tokio::test]
async fn daemon_accepts_event_and_writes_row() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));

    let dir = TempDir::new().unwrap();
    let cfg = IngestDaemonConfig {
        socket_path: dir.path().join("s.sock"),
        buffer_path: dir.path().join("buf.jsonl"),
        lock_path: dir.path().join("desktop.lock"),
        repo: repo.clone(),
        event_tx: None,
        op_handler: None,
    };
    let handle = spawn(cfg.clone()).await.expect("spawn");

    let sink = UnixIngestSocket::new(cfg.socket_path.clone());
    let evt = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s1".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt {
            text: "hi".into(),
            attachments: vec![],
        },
    });
    sink.send(&evt).await.unwrap();

    // Poll briefly — daemon handles inserts async.
    for _ in 0..50 {
        if repo.count_by_session("s1").await.unwrap() > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(repo.count_by_session("s1").await.unwrap(), 1);

    handle.shutdown().await;
}
