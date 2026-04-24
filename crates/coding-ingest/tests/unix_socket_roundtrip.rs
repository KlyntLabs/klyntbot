use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::transport::{IngestSocket, UnixIngestSocket};
use jiff::Timestamp;
use std::path::PathBuf;
use tempfile::TempDir;
use tokio::io::AsyncReadExt;
use tokio::net::UnixListener;
use uuid::Uuid;

#[tokio::test]
async fn send_writes_length_prefix_then_json() {
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("ingest.sock");
    let listener = UnixListener::bind(&sock).unwrap();

    let evt = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::UserPrompt { text: "hi".into(), attachments: vec![] },
    });

    let sink = UnixIngestSocket::new(sock.clone());
    let send_task = tokio::spawn(async move { sink.send(&evt).await });

    let (mut stream, _addr) = listener.accept().await.unwrap();
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.unwrap();
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut body = vec![0u8; len];
    stream.read_exact(&mut body).await.unwrap();
    let decoded: AgentEvent = serde_json::from_slice(&body).unwrap();
    let AgentEvent::V1(v1) = decoded;
    assert_eq!(v1.session_id, "s");
    send_task.await.unwrap().unwrap();
}

#[tokio::test]
async fn send_returns_error_when_socket_missing() {
    let dir = TempDir::new().unwrap();
    let sock = dir.path().join("absent.sock");
    let evt = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: "s".into(),
        turn_id: None,
        cwd: PathBuf::from("/tmp"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::SessionEnd { reason: "x".into() },
    });
    let sink = UnixIngestSocket::new(sock);
    assert!(sink.send(&evt).await.is_err());
}
