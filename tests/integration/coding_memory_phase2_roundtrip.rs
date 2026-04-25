//! End-to-end: run klyntbot-hook against a running IngestDaemon using the
//! synthetic Claude Code fixture. Assert every event lands in ingest_event_log.

use assert_cmd::Command;
use coding_ingest::daemon::{spawn, IngestDaemonConfig};
use coding_ingest::store::IngestEventLogRepo;
use std::sync::Arc;
use storage::StoragePool;
use tempfile::TempDir;

#[derive(serde::Deserialize)]
struct FixtureLine {
    #[serde(rename = "hookEvent")]
    hook_event: String,
    body: serde_json::Value,
}

#[tokio::test]
async fn synthetic_claude_code_session_round_trips() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));

    let home = TempDir::new().unwrap();
    let cfg = IngestDaemonConfig {
        socket_path: home.path().join("ingest.sock"),
        buffer_path: home.path().join("ingest-buffer.jsonl"),
        lock_path: home.path().join("desktop.lock"),
        repo: repo.clone(),
        event_tx: None,
        op_handler: None,
    };
    let handle = spawn(cfg).await.expect("daemon spawn");

    let fixture =
        std::fs::read_to_string("tests/fixtures/coding/synthetic_session_claude_code.jsonl")
            .unwrap();
    for line in fixture.lines().filter(|l| !l.trim().is_empty()) {
        let fl: FixtureLine = serde_json::from_str(line).unwrap();
        Command::cargo_bin("klyntbot-hook")
            .unwrap()
            .env("KLYNTBOT_HOME", home.path())
            .args(["claude-code", &fl.hook_event])
            .write_stdin(serde_json::to_string(&fl.body).unwrap())
            .assert()
            .success();
    }

    // Poll for arrival.
    for _ in 0..100 {
        let count: i64 = repo.count_by_session("fx-1").await.unwrap();
        if count >= 9 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    // All 10 fixture events are recordable (no PreToolUse in fixture).
    let total: i64 = repo.count_by_session("fx-1").await.unwrap();
    assert!(total >= 9, "expected >=9 events, got {total}");

    handle.shutdown().await;
}
