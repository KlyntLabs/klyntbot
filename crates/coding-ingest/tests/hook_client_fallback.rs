use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::hook_client::HookClient;
use jiff::Timestamp;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use uuid::Uuid;

fn evt() -> AgentEvent {
    AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::SessionEnd { reason: "x".into() },
    })
}

#[tokio::test]
async fn uses_socket_when_available() {
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("s.sock");
    let listener = UnixListener::bind(&sock).unwrap();
    let client = HookClient::new(
        sock.clone(),
        dir.path().join("buf.jsonl"),
        dir.path().join(".stamp"),
    );
    let task = tokio::spawn(async move { client.send(&evt()).await });
    let (mut s, _) = listener.accept().await.unwrap();
    let mut len = [0u8; 4];
    s.read_exact(&mut len).await.unwrap();
    let mut body = vec![0u8; u32::from_le_bytes(len) as usize];
    s.read_exact(&mut body).await.unwrap();
    task.await.unwrap().unwrap();
    assert!(!dir.path().join("buf.jsonl").exists());
}

#[tokio::test]
async fn falls_back_to_buffer_when_socket_absent() {
    let dir = TempDir::new().unwrap();
    let client = HookClient::new(
        dir.path().join("absent.sock"),
        dir.path().join("buf.jsonl"),
        dir.path().join(".stamp"),
    );
    client.send(&evt()).await.unwrap();
    let contents = tokio::fs::read_to_string(dir.path().join("buf.jsonl"))
        .await
        .unwrap();
    assert_eq!(contents.lines().count(), 1);
    // Stamp was touched → warning was issued once.
    assert!(dir.path().join(".stamp").exists());
}
