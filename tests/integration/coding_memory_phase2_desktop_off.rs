//! Scenario: desktop off → 3 hook invocations buffer to disk → desktop starts
//! → buffered events drain into ingest_event_log → archive file present.

use assert_cmd::Command;
use coding_ingest::daemon::{spawn, IngestDaemonConfig};
use coding_ingest::store::IngestEventLogRepo;
use std::sync::Arc;
use storage::StoragePool;
use tempfile::TempDir;

#[tokio::test]
async fn desktop_off_buffers_then_drains_on_startup() {
    let home = TempDir::new().unwrap();

    // Phase 1: desktop is OFF. Send 3 hook events — they go to the file buffer.
    for i in 0..3 {
        let body = format!(r#"{{"session_id":"off-{i}","cwd":"/tmp","source":"cli","model":"m"}}"#);
        Command::cargo_bin("klyntbot-hook")
            .unwrap()
            .env("KLYNTBOT_HOME", home.path())
            .args(["claude-code", "SessionStart"])
            .write_stdin(body)
            .assert()
            .success();
    }
    let buffer_path = home.path().join("ingest-buffer.jsonl");
    let contents = std::fs::read_to_string(&buffer_path).unwrap();
    assert_eq!(contents.lines().count(), 3);

    // Phase 2: desktop starts. Daemon drains the buffer on spawn.
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    let repo = Arc::new(IngestEventLogRepo::new(pool.inner().clone()));
    let cfg = IngestDaemonConfig {
        socket_path: home.path().join("ingest.sock"),
        buffer_path: buffer_path.clone(),
        lock_path: home.path().join("desktop.lock"),
        repo: repo.clone(),
        event_tx: None,
        op_handler: None,
        git_invalidation_handler: None,
        opencode_db_path: None,
        opencode_poll_interval: None,
        kimi_wire_socket: None,
        codex_sessions_dir: None,
        codex_poll_interval: None,
    };
    let handle = spawn(cfg).await.expect("spawn");

    let unprocessed: i64 = repo.count_unprocessed().await.unwrap();
    assert_eq!(unprocessed, 3);
    assert!(!buffer_path.exists(), "buffer should be archived");
    let archived: Vec<_> = std::fs::read_dir(home.path())
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().into_string().unwrap()))
        .filter(|n| n.contains(".done."))
        .collect();
    assert_eq!(archived.len(), 1);

    handle.shutdown().await;
}
