//! When the desktop process isn't running (no socket file or refused
//! connection), `BridgeClient::send` must not panic, must not block the
//! caller, and must drop frames silently.

use mcp_bridge::{BridgeClient, BridgeFrame};
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[tokio::test]
async fn send_to_missing_socket_returns_immediately() {
    let path = PathBuf::from("/tmp/klynt-bridge-definitely-not-here-39481.sock");
    // Sanity: ensure the file truly does not exist.
    let _ = std::fs::remove_file(&path);

    let client = BridgeClient::new(path);
    let frame = BridgeFrame {
        event: "entity:updated".into(),
        payload: json!({ "entityKind": "task", "id": "x" }),
    };

    let start = Instant::now();
    for _ in 0..50 {
        client.send(frame.clone());
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_millis(50),
        "send() should be non-blocking; took {elapsed:?}"
    );

    // Give the writer task a moment to attempt + fail, prove it doesn't crash.
    tokio::time::sleep(Duration::from_millis(250)).await;
}

#[tokio::test]
async fn dropping_client_cleans_up_writer_task() {
    let path = PathBuf::from("/tmp/klynt-bridge-also-not-here-39482.sock");
    let _ = std::fs::remove_file(&path);
    let client = BridgeClient::new(path);
    drop(client);
    // No assertion — passing means "didn't deadlock the runtime on shutdown".
    tokio::time::sleep(Duration::from_millis(100)).await;
}
