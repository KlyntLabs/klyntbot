use mcp_bridge::{BridgeClient, BridgeFrame, BridgeServer};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;
use tokio::time::timeout;

#[tokio::test]
async fn client_send_reaches_server_handler() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("bridge.sock");

    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();
    let handler = Box::new(move |frame: BridgeFrame| {
        received_clone.lock().unwrap().push(frame);
    });

    let server = BridgeServer::start(path.clone(), handler).await.unwrap();

    // Yield once so the listener is ready to accept.
    tokio::time::sleep(Duration::from_millis(20)).await;

    let client = BridgeClient::new(path.clone());
    let frame = BridgeFrame {
        event: "entity:updated".into(),
        payload: json!({ "entityKind": "task", "id": "t42" }),
    };
    client.send(frame.clone());

    // Poll until the handler observes the frame.
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        if !received.lock().unwrap().is_empty() {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("frame did not reach handler within 500 ms");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let frames = received.lock().unwrap().clone();
    assert_eq!(frames, vec![frame]);

    server.shutdown();
    // Confirm shutdown removed the socket file.
    let _ = timeout(Duration::from_millis(100), async {
        while path.exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await;
    assert!(!path.exists(), "socket file should be removed on shutdown");
}

#[tokio::test]
async fn emit_entity_updated_through_bridge_arrives_with_camel_case_payload() {
    use app_core::events::AppEventEmitter;
    use desktop_shared::types::EntityKind;
    use mcp_bridge::SocketBridgeEmitter;

    let dir = tempdir().unwrap();
    let path = dir.path().join("emit.sock");
    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();
    let server = BridgeServer::start(
        path.clone(),
        Box::new(move |f| received_clone.lock().unwrap().push(f)),
    )
    .await
    .unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let emitter = SocketBridgeEmitter::new(BridgeClient::new(path));
    emitter.emit_entity_updated(EntityKind::FocusSession, "fs-9");

    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        if !received.lock().unwrap().is_empty() {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("frame did not arrive");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let frames = received.lock().unwrap().clone();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].event, "entity:updated");
    // EntityKind serializes as camelCase per #[serde(rename_all = "camelCase"]
    // on `desktop-shared/src/types.rs:48`.
    assert_eq!(
        frames[0].payload,
        serde_json::json!({ "entityKind": "focusSession", "id": "fs-9" })
    );
    server.shutdown();
}
