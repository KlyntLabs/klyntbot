use mcp_bridge::{BridgeClient, BridgeFrame, BridgeServer};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::tempdir;

#[tokio::test]
async fn server_handles_many_frames_across_connections() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("multi.sock");

    let received: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();
    let handler = Box::new(move |frame: BridgeFrame| {
        received_clone.lock().unwrap().push(frame.event);
    });
    let server = BridgeServer::start(path.clone(), handler).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let client = BridgeClient::new(path);
    for i in 0..25 {
        client.send(BridgeFrame {
            event: format!("test:event:{i}"),
            payload: json!({ "i": i }),
        });
    }

    let deadline = std::time::Instant::now() + Duration::from_millis(2000);
    loop {
        if received.lock().unwrap().len() == 25 {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!(
                "only got {} frames in 2s",
                received.lock().unwrap().len()
            );
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let mut events = received.lock().unwrap().clone();
    events.sort();
    let mut expected: Vec<String> = (0..25).map(|i| format!("test:event:{i}")).collect();
    expected.sort();
    assert_eq!(events, expected);

    server.shutdown();
}

#[tokio::test]
async fn server_recovers_from_malformed_frame() {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixStream;

    let dir = tempdir().unwrap();
    let path = dir.path().join("malformed.sock");

    let received: Arc<Mutex<Vec<BridgeFrame>>> = Arc::new(Mutex::new(Vec::new()));
    let received_clone = received.clone();
    let handler = Box::new(move |frame: BridgeFrame| {
        received_clone.lock().unwrap().push(frame);
    });
    let server = BridgeServer::start(path.clone(), handler).await.unwrap();
    tokio::time::sleep(Duration::from_millis(20)).await;

    // Send garbage on one connection.
    {
        let mut s = UnixStream::connect(&path).await.unwrap();
        s.write_all(&(5u32).to_le_bytes()).await.unwrap();
        s.write_all(b"NOTJS").await.unwrap();
        s.shutdown().await.unwrap();
    }

    // Then a valid frame on a fresh connection.
    let client = BridgeClient::new(path);
    client.send(BridgeFrame {
        event: "valid:event".into(),
        payload: serde_json::json!({}),
    });

    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        if !received.lock().unwrap().is_empty() {
            break;
        }
        if std::time::Instant::now() > deadline {
            panic!("server did not recover after malformed frame");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    server.shutdown();
}
