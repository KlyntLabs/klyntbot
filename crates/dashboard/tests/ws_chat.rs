//! Tests for the WebSocket chat handler (/ws/chat endpoint).

use std::sync::Arc;

use agent::AgentEvent;
use bus::MessageBus;
use dashboard::events::{DashboardEvent, DashboardEventBus};
use dashboard::server::{DashboardConfig, DashboardServer};
use futures_util::{SinkExt, StreamExt};
use tokio::time::{timeout, Duration};
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// Spin up a test server and return its address.
async fn start_test_server(
    bus: Arc<MessageBus>,
    event_bus: Arc<DashboardEventBus>,
) -> std::net::SocketAddr {
    let server = DashboardServer::new(DashboardConfig {
        port: 0,
        bus,
        event_bus,
    });
    let (addr, _handle) = server.start_on_random_port().await.unwrap();
    addr
}

/// Parse a JSON WebSocket message from the server.
fn parse_server_msg(msg: &str) -> serde_json::Value {
    serde_json::from_str(msg).unwrap_or_default()
}

#[tokio::test]
async fn test_ws_chat_send_message() {
    let bus = Arc::new(MessageBus::new(32));
    let event_bus = Arc::new(DashboardEventBus::new(64));

    // Take the inbound receiver BEFORE starting the server so we can observe messages.
    let mut inbound_rx = bus.take_inbound_rx().unwrap();

    let addr = start_test_server(bus.clone(), event_bus.clone()).await;

    let url = format!("ws://{}/ws/chat", addr);
    let (mut ws, _) = connect_async(&url).await.expect("WS connect failed");

    // Send a chat message.
    let msg = serde_json::json!({
        "type": "message",
        "content": "Hello agent!",
        "session_id": "web-test-123"
    })
    .to_string();
    ws.send(Message::Text(msg.into())).await.unwrap();

    // The server should publish the message to the message bus.
    let received = timeout(Duration::from_secs(2), inbound_rx.recv())
        .await
        .expect("Timed out waiting for inbound message")
        .expect("Channel closed");

    assert_eq!(received.content, "Hello agent!");
    assert_eq!(received.channel.as_str(), "dashboard");

    ws.close(None).await.ok();
}

#[tokio::test]
async fn test_ws_chat_receive_streaming_chunks() {
    let bus = Arc::new(MessageBus::new(32));
    let event_bus = Arc::new(DashboardEventBus::new(64));
    let addr = start_test_server(bus.clone(), event_bus.clone()).await;

    let url = format!("ws://{}/ws/chat", addr);
    let (mut ws, _) = connect_async(&url).await.expect("WS connect failed");

    // Trigger subscription by sending an initial message.
    let msg = serde_json::json!({
        "type": "message",
        "content": "hi",
        "session_id": "web-stream-123"
    })
    .to_string();
    ws.send(Message::Text(msg.into())).await.unwrap();

    // Give the server time to set up the subscription.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Simulate the agent producing streaming chunks via the event bus.
    event_bus.publish(DashboardEvent::AgentEvent(AgentEvent::ContentChunk(
        "Hello ".to_string(),
    )));
    event_bus.publish(DashboardEvent::AgentEvent(AgentEvent::ContentChunk(
        "world!".to_string(),
    )));

    // Collect the session announcement and chunk messages.
    let mut chunks_received = 0;
    for _ in 0..5 {
        match timeout(Duration::from_millis(500), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let v = parse_server_msg(&text);
                if v.get("type").and_then(|t| t.as_str()) == Some("chunk") {
                    chunks_received += 1;
                }
            }
            Ok(Some(Ok(_))) => {}
            _ => break,
        }
    }

    assert!(
        chunks_received >= 1,
        "Expected at least 1 chunk message, got {}",
        chunks_received
    );

    ws.close(None).await.ok();
}

#[tokio::test]
async fn test_ws_chat_receive_tool_events() {
    let bus = Arc::new(MessageBus::new(32));
    let event_bus = Arc::new(DashboardEventBus::new(64));
    let addr = start_test_server(bus.clone(), event_bus.clone()).await;

    let url = format!("ws://{}/ws/chat", addr);
    let (mut ws, _) = connect_async(&url).await.expect("WS connect failed");

    // Subscribe by sending a message.
    let msg = serde_json::json!({
        "type": "message",
        "content": "use tool",
        "session_id": "web-tool-123"
    })
    .to_string();
    ws.send(Message::Text(msg.into())).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Simulate tool start/end events.
    event_bus.publish(DashboardEvent::AgentEvent(AgentEvent::ToolStart {
        name: "todo".to_string(),
        args: serde_json::json!({}),
    }));
    event_bus.publish(DashboardEvent::AgentEvent(AgentEvent::ToolEnd {
        name: "todo".to_string(),
        success: true,
        duration_ms: 42,
    }));

    let mut tool_start_seen = false;
    let mut tool_end_seen = false;

    for _ in 0..6 {
        match timeout(Duration::from_millis(500), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let v = parse_server_msg(&text);
                match v.get("type").and_then(|t| t.as_str()) {
                    Some("tool_start") => tool_start_seen = true,
                    Some("tool_end") => tool_end_seen = true,
                    _ => {}
                }
            }
            Ok(Some(Ok(_))) => {}
            _ => break,
        }
        if tool_start_seen && tool_end_seen {
            break;
        }
    }

    assert!(tool_start_seen, "Should receive tool_start message");
    assert!(tool_end_seen, "Should receive tool_end message");

    ws.close(None).await.ok();
}

#[tokio::test]
async fn test_ws_chat_receive_done_message() {
    let bus = Arc::new(MessageBus::new(32));
    let event_bus = Arc::new(DashboardEventBus::new(64));
    let addr = start_test_server(bus.clone(), event_bus.clone()).await;

    let url = format!("ws://{}/ws/chat", addr);
    let (mut ws, _) = connect_async(&url).await.expect("WS connect failed");

    let msg = serde_json::json!({
        "type": "message",
        "content": "complete",
        "session_id": "web-done-123"
    })
    .to_string();
    ws.send(Message::Text(msg.into())).await.unwrap();

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Emit done event.
    event_bus.publish(DashboardEvent::AgentEvent(AgentEvent::Done(
        "The answer is 42.".to_string(),
    )));

    let mut done_seen = false;
    for _ in 0..5 {
        match timeout(Duration::from_millis(500), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let v = parse_server_msg(&text);
                if v.get("type").and_then(|t| t.as_str()) == Some("done") {
                    let content = v.get("content").and_then(|c| c.as_str()).unwrap_or("");
                    assert_eq!(content, "The answer is 42.");
                    done_seen = true;
                    break;
                }
            }
            Ok(Some(Ok(_))) => {}
            _ => break,
        }
    }

    assert!(done_seen, "Should receive done message");
    ws.close(None).await.ok();
}

#[tokio::test]
async fn test_ws_chat_session_id_routing() {
    // Two WebSocket connections with different session IDs.
    // Agent events should go to both (broadcast), but the architecture
    // allows per-session filtering in the future. For now, both receive events
    // since the event bus is shared.
    let bus = Arc::new(MessageBus::new(32));
    let event_bus = Arc::new(DashboardEventBus::new(64));

    // Take the inbound receiver before starting the server.
    let mut inbound_rx = bus.take_inbound_rx().unwrap();

    let addr = start_test_server(bus.clone(), event_bus.clone()).await;

    let url = format!("ws://{}/ws/chat", addr);
    let (mut ws1, _) = connect_async(&url).await.expect("WS1 connect failed");
    let (mut ws2, _) = connect_async(&url).await.expect("WS2 connect failed");

    // Connect each to a different session.
    for (ws, sid) in [(&mut ws1, "web-session-A"), (&mut ws2, "web-session-B")] {
        let msg = serde_json::json!({
            "type": "message",
            "content": "ping",
            "session_id": sid
        })
        .to_string();
        ws.send(Message::Text(msg.into())).await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(50)).await;

    // Drain the "ping" messages from the inbound receiver.
    // We need to receive both ping messages before sending "hello from A".
    let _ = timeout(Duration::from_millis(200), inbound_rx.recv()).await;
    let _ = timeout(Duration::from_millis(200), inbound_rx.recv()).await;

    // Both connections should be alive (not closed due to routing errors).
    // Verify by sending a message on ws1 and checking the bus receives it.
    ws1.send(Message::Text(
        serde_json::json!({
            "type": "message",
            "content": "hello from A",
            "session_id": "web-session-A"
        })
        .to_string()
        .into(),
    ))
    .await
    .unwrap();

    let inbound = timeout(Duration::from_secs(1), inbound_rx.recv())
        .await
        .expect("Timed out")
        .expect("Channel closed");
    assert_eq!(inbound.content, "hello from A");

    ws1.close(None).await.ok();
    ws2.close(None).await.ok();
}

#[tokio::test]
async fn test_ws_chat_new_session_auto_creation() {
    let bus = Arc::new(MessageBus::new(32));
    let event_bus = Arc::new(DashboardEventBus::new(64));
    let addr = start_test_server(bus.clone(), event_bus.clone()).await;

    let url = format!("ws://{}/ws/chat", addr);
    let (mut ws, _) = connect_async(&url).await.expect("WS connect failed");

    // Send a message WITHOUT a session_id.
    let msg = serde_json::json!({
        "type": "message",
        "content": "hello without session"
    })
    .to_string();
    ws.send(Message::Text(msg.into())).await.unwrap();

    // Server should respond with a session announcement.
    let mut session_id_received = false;
    for _ in 0..5 {
        match timeout(Duration::from_millis(500), ws.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let v = parse_server_msg(&text);
                if v.get("type").and_then(|t| t.as_str()) == Some("session") {
                    let sid = v.get("session_id").and_then(|s| s.as_str()).unwrap_or("");
                    assert!(
                        sid.starts_with("web-"),
                        "Session ID should start with 'web-', got: {}",
                        sid
                    );
                    session_id_received = true;
                    break;
                }
            }
            Ok(Some(Ok(_))) => {}
            _ => break,
        }
    }

    assert!(
        session_id_received,
        "Server should auto-create and announce session ID"
    );
    ws.close(None).await.ok();
}

#[tokio::test]
async fn test_ws_chat_reconnection_resumes_session() {
    // Reconnecting with the same session_id should not cause errors.
    // The agent session history is in the MessageBus/AgentLoop, not in the
    // dashboard WS handler — so we verify the protocol stays valid across reconnect.
    let bus = Arc::new(MessageBus::new(32));
    let event_bus = Arc::new(DashboardEventBus::new(64));

    // Take the receiver before starting the server.
    let mut inbound_rx = bus.take_inbound_rx().unwrap();

    let addr = start_test_server(bus.clone(), event_bus.clone()).await;

    let session_id = "web-resume-abc";
    let url = format!("ws://{}/ws/chat", addr);

    // First connection.
    {
        let (mut ws, _) = connect_async(&url).await.expect("WS connect failed");
        let msg = serde_json::json!({
            "type": "message",
            "content": "first connection",
            "session_id": session_id
        })
        .to_string();
        ws.send(Message::Text(msg.into())).await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        // Drain the first message from the receiver.
        let _ = timeout(Duration::from_millis(200), inbound_rx.recv()).await;
        ws.close(None).await.ok();
    }

    // Second connection with the same session_id.
    {
        let (mut ws, _) = connect_async(&url).await.expect("WS reconnect failed");
        let msg = serde_json::json!({
            "type": "message",
            "content": "reconnected",
            "session_id": session_id
        })
        .to_string();
        ws.send(Message::Text(msg.into())).await.unwrap();

        // Should still work — message published to bus.
        let inbound = timeout(Duration::from_secs(1), inbound_rx.recv())
            .await
            .expect("Timed out");
        assert!(
            inbound.is_some(),
            "Reconnected session should publish messages"
        );
        ws.close(None).await.ok();
    }
}

#[tokio::test]
async fn test_ws_chat_invalid_message_format() {
    let bus = Arc::new(MessageBus::new(32));
    let event_bus = Arc::new(DashboardEventBus::new(64));
    let addr = start_test_server(bus.clone(), event_bus.clone()).await;

    let url = format!("ws://{}/ws/chat", addr);
    let (mut ws, _) = connect_async(&url).await.expect("WS connect failed");

    // Send malformed JSON.
    ws.send(Message::Text("not valid json at all !!!".into()))
        .await
        .unwrap();

    // Server should respond with an error message (not close the connection).
    let response = timeout(Duration::from_secs(1), ws.next()).await;
    match response {
        Ok(Some(Ok(Message::Text(text)))) => {
            let v = parse_server_msg(&text);
            assert_eq!(
                v.get("type").and_then(|t| t.as_str()),
                Some("error"),
                "Expected error response for malformed message, got: {}",
                text
            );
        }
        Ok(Some(Ok(Message::Close(_)))) => {
            // Acceptable: server closed the connection after invalid input.
        }
        other => {
            panic!("Unexpected WS response to malformed message: {:?}", other);
        }
    }

    ws.close(None).await.ok();
}
