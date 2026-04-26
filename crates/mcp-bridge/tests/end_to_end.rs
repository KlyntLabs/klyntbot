//! End-to-end: every default `AppEventEmitter` helper bridges as expected.

use app_core::events::AppEventEmitter;
use mcp_bridge::{BridgeClient, BridgeFrame, BridgeServer, SocketBridgeEmitter};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;

async fn collect_one(received: Arc<Mutex<Vec<BridgeFrame>>>) -> BridgeFrame {
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        {
            let g = received.lock().unwrap();
            if !g.is_empty() {
                return g[0].clone();
            }
        }
        if std::time::Instant::now() > deadline {
            panic!("no frame received within 500 ms");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn chat_thread_helper_bridges_as_chat_thread_updated() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("e2e1.sock");
    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let r = received.clone();
    let server = BridgeServer::start(path.clone(), Box::new(move |f| r.lock().unwrap().push(f)))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let emitter = SocketBridgeEmitter::new(BridgeClient::new(path));
    // is_new = false → CHAT_THREAD_UPDATED
    emitter.emit_chat_thread(false, "session-abc");

    let frame = collect_one(received).await;
    assert_eq!(frame.event, "chat:thread_updated");
    assert_eq!(frame.payload, json!({ "sessionKey": "session-abc" }));
    server.shutdown();
}

#[tokio::test]
async fn arbitrary_emit_event_bridges_unchanged() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("e2e2.sock");
    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let r = received.clone();
    let server = BridgeServer::start(path.clone(), Box::new(move |f| r.lock().unwrap().push(f)))
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let emitter = SocketBridgeEmitter::new(BridgeClient::new(path));
    emitter.emit_event(
        "provider:degraded",
        json!({ "provider": "anthropic", "reason": "rate_limit" }),
    );

    let frame = collect_one(received).await;
    assert_eq!(frame.event, "provider:degraded");
    assert_eq!(
        frame.payload,
        json!({ "provider": "anthropic", "reason": "rate_limit" })
    );
    server.shutdown();
}
