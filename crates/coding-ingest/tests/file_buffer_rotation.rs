use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::transport::{
    FileBufferFallback, IngestSocket, BUFFER_HARD_CAP_BYTES, BUFFER_ROTATE_BYTES,
};
use jiff::Timestamp;
use std::path::PathBuf;
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
async fn append_produces_one_line_per_event() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("buf.jsonl");
    let sink = FileBufferFallback::new(path.clone());
    sink.send(&evt(0)).await.unwrap();
    sink.send(&evt(1)).await.unwrap();
    let contents = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(contents.lines().count(), 2);
    for line in contents.lines() {
        let _: AgentEvent = serde_json::from_str(line).unwrap();
    }
}

#[tokio::test]
async fn rotates_when_over_rotate_threshold() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("buf.jsonl");
    // Seed file just over rotate threshold.
    tokio::fs::write(&path, vec![b'x'; (BUFFER_ROTATE_BYTES as usize) + 1])
        .await
        .unwrap();
    let sink = FileBufferFallback::new(path.clone());
    sink.send(&evt(0)).await.unwrap();
    // After rotation, primary file contains only the new event.
    let contents = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(contents.lines().count(), 1);
    // A rotated file exists alongside.
    let siblings: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
        .filter(|n| n.starts_with("buf.jsonl."))
        .collect();
    assert_eq!(siblings.len(), 1);
}

#[tokio::test]
async fn refuses_when_over_hard_cap() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("buf.jsonl");
    // Create many fake rotated siblings totalling > hard cap.
    // We assert hard cap by monkeying the primary.
    tokio::fs::write(&path, vec![b'x'; (BUFFER_HARD_CAP_BYTES as usize) + 1])
        .await
        .unwrap();
    let sink = FileBufferFallback::new(path.clone());
    let r = sink.send(&evt(0)).await;
    assert!(r.is_err(), "expected hard-cap error");
}
