//! End-to-end smoke test for KimiPoller against a fixture session dir.

use coding_ingest::adapters::kimi_cli::poller::KimiPoller;
use coding_ingest::event::AgentEvent;
use coding_ingest::store::IngestEventLogRepo;
use std::sync::Arc;
use std::time::Duration;
use storage::StoragePool;
use tokio::sync::mpsc;

#[tokio::test]
async fn poller_ingests_fixture_session() {
    // Migrations: storage core + cognitive + coding_memory provide the
    // ingest_event_log table.
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));

    let dir = tempfile::tempdir().unwrap();
    // Layout: <sessions>/<hash>/<uuid>/wire.jsonl
    let work_dir = "/tmp/kimi-fixture-repo";
    let hash = coding_ingest::adapters::kimi_cli::workdir::hash_for(work_dir, "local");
    let session_id = "11111111-2222-3333-4444-555555555555";
    let session_dir = dir.path().join(format!("sessions/{hash}/{session_id}"));
    tokio::fs::create_dir_all(&session_dir).await.unwrap();

    let kimi_json = dir.path().join("kimi.json");
    tokio::fs::write(
        &kimi_json,
        format!(
            r#"{{"work_dirs":[{{"path":"{work_dir}","kaos":"local","last_session_id":null}}]}}"#
        ),
    )
    .await
    .unwrap();

    // Seed the session file BEFORE spawning so seed_offsets sees size N
    // and we can append AFTER spawn to drive the tail.
    tokio::fs::write(
        session_dir.join("wire.jsonl"),
        b"{\"type\":\"metadata\",\"protocol_version\":\"1.9\"}\n",
    )
    .await
    .unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();
    let poller = KimiPoller::new(
        dir.path().join("sessions"),
        kimi_json.clone(),
        tx,
        repo.clone(),
        Duration::from_millis(50),
    );
    let handle = poller.spawn();

    // Append three records after the poller has started.
    tokio::time::sleep(Duration::from_millis(120)).await;
    let mut f = tokio::fs::OpenOptions::new()
        .append(true)
        .open(session_dir.join("wire.jsonl"))
        .await
        .unwrap();
    use tokio::io::AsyncWriteExt;
    let body = concat!(
        r#"{"timestamp":1777096658.4,"message":{"type":"TurnBegin","payload":{"user_input":[{"type":"text","text":"hi"}]}}}"#,
        "\n",
        r#"{"timestamp":1777096659.0,"message":{"type":"ContentPart","payload":{"type":"text","text":"hello back"}}}"#,
        "\n",
        r#"{"timestamp":1777096660.0,"message":{"type":"ToolCall","payload":{"id":"c1","function":{"name":"Read","arguments":"{\"path\":\"/x\"}"}}}}"#,
        "\n",
        r#"{"timestamp":1777096660.5,"message":{"type":"ToolResult","payload":{"id":"c1","content":"ok"}}}"#,
        "\n",
    );
    f.write_all(body.as_bytes()).await.unwrap();
    f.flush().await.unwrap();
    drop(f);

    // Collect up to 5 events with a 2s timeout.
    let mut received = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while tokio::time::Instant::now() < deadline && received.len() < 4 {
        if let Ok(Some(evt)) = tokio::time::timeout(Duration::from_millis(150), rx.recv()).await {
            received.push(evt);
        }
    }

    handle.abort();

    // Expect: SessionStart + UserPrompt + AssistantMsg + ToolCall = 4 events.
    assert_eq!(received.len(), 4, "got {received:#?}");
    let kinds: Vec<&str> = received
        .iter()
        .map(|e| {
            let AgentEvent::V1(v) = e;
            match &v.kind {
                coding_ingest::event::EventKind::SessionStart { .. } => "sessionStart",
                coding_ingest::event::EventKind::UserPrompt { .. } => "userPrompt",
                coding_ingest::event::EventKind::AssistantMsg { .. } => "assistantMsg",
                coding_ingest::event::EventKind::ToolCall { .. } => "toolCall",
                _ => "other",
            }
        })
        .collect();
    assert_eq!(
        kinds,
        vec!["sessionStart", "userPrompt", "assistantMsg", "toolCall"]
    );

    // Same 4 rows must be in ingest_event_log.
    let count = repo.count_by_session(session_id).await.unwrap();
    assert_eq!(count, 4);
}
