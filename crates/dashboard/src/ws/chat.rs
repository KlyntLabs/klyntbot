//! Chat WebSocket handler for /ws/chat.
//!
//! Protocol (JSON messages):
//!
//! Client → Server:
//!   { "type": "message", "content": "Hello", "session_id": "web-123" }
//!
//! Server → Client (streamed):
//!   { "type": "chunk", "content": "Hello! " }
//!   { "type": "tool_start", "name": "todo", "args": {} }
//!   { "type": "tool_end", "name": "todo", "success": true, "duration_ms": 42 }
//!   { "type": "chunk", "content": "I've added..." }
//!   { "type": "done", "content": "Full response here" }
//!   { "type": "error", "message": "..." }
//!   { "type": "session", "session_id": "web-<uuid>" }   ← on first connection

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tracing::{debug, warn};
use uuid::Uuid;

use agent::AgentEvent;
use bus::InboundMessage;
use bus::MessageBus;

use crate::events::{DashboardEvent, DashboardEventBus};

/// Shared state passed to the WebSocket handler via axum's `State` extractor.
#[derive(Clone)]
pub struct ChatState {
    pub bus: Arc<MessageBus>,
    pub event_bus: Arc<DashboardEventBus>,
}

/// Inbound message from the browser client.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Message {
        content: String,
        session_id: Option<String>,
    },
}

/// Outbound message to the browser client.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMessage {
    Chunk {
        content: String,
    },
    ToolStart {
        name: String,
        args: serde_json::Value,
    },
    ToolEnd {
        name: String,
        success: bool,
        duration_ms: u64,
    },
    Done {
        content: String,
    },
    Error {
        message: String,
    },
    Session {
        session_id: String,
    },
}

/// axum handler: upgrades HTTP connection to WebSocket for the chat protocol.
pub async fn ws_chat_handler(
    ws: WebSocketUpgrade,
    State(state): State<ChatState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: ChatState) {
    // Each connection gets its own session ID if the client doesn't provide one.
    let mut session_id: Option<String> = None;
    let mut event_rx: Option<broadcast::Receiver<DashboardEvent>> = None;

    loop {
        tokio::select! {
            // Messages from the browser client.
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        match serde_json::from_str::<ClientMessage>(&text) {
                            Ok(ClientMessage::Message { content, session_id: client_sid }) => {
                                // Assign or adopt session ID.
                                let sid = client_sid.unwrap_or_else(|| {
                                    format!("web-{}", Uuid::new_v4())
                                });

                                // On first message: announce session ID to client.
                                let is_new = session_id.is_none();
                                session_id = Some(sid.clone());

                                if is_new {
                                    // Subscribe to dashboard events for this session.
                                    event_rx = Some(state.event_bus.subscribe());

                                    let msg = ServerMessage::Session { session_id: sid.clone() };
                                    if let Ok(json) = serde_json::to_string(&msg) {
                                        let _ = socket.send(Message::Text(json.into())).await;
                                    }
                                }

                                // Publish inbound message to the agent loop.
                                let inbound = InboundMessage::new(
                                    "dashboard",
                                    "web-user",
                                    sid.as_str(),
                                    content,
                                );
                                if let Err(e) = state.bus.publish_inbound(inbound).await {
                                    let err = ServerMessage::Error {
                                        message: format!("Failed to route message: {}", e),
                                    };
                                    if let Ok(json) = serde_json::to_string(&err) {
                                        let _ = socket.send(Message::Text(json.into())).await;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("WS chat: invalid message format: {}", e);
                                let err = ServerMessage::Error {
                                    message: format!("Invalid message format: {}", e),
                                };
                                if let Ok(json) = serde_json::to_string(&err) {
                                    let _ = socket.send(Message::Text(json.into())).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        debug!("WS chat: connection closed");
                        break;
                    }
                    Some(Ok(_)) => {} // ignore binary/ping/pong
                    Some(Err(e)) => {
                        warn!("WS chat: receive error: {}", e);
                        break;
                    }
                }
            }

            // Events from the dashboard event bus (agent streaming output).
            event = async {
                match event_rx.as_mut() {
                    Some(rx) => rx.recv().await,
                    None => std::future::pending().await,
                }
            } => {
                match event {
                    Ok(DashboardEvent::AgentEvent(agent_event)) => {
                        let msg = agent_event_to_server_message(agent_event);
                        if let Some(msg) = msg {
                            if let Ok(json) = serde_json::to_string(&msg) {
                                if socket.send(Message::Text(json.into())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(_) => {} // ignore non-agent events
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("WS chat: lagged {} events", n);
                    }
                }
            }
        }
    }
}

fn agent_event_to_server_message(event: AgentEvent) -> Option<ServerMessage> {
    match event {
        AgentEvent::ContentChunk(content) => Some(ServerMessage::Chunk { content }),
        AgentEvent::ToolStart { name, args } => Some(ServerMessage::ToolStart { name, args }),
        AgentEvent::ToolEnd {
            name,
            success,
            duration_ms,
        } => Some(ServerMessage::ToolEnd {
            name,
            success,
            duration_ms,
        }),
        AgentEvent::Done(content) => Some(ServerMessage::Done { content }),
        AgentEvent::Error(message) => Some(ServerMessage::Error { message }),
        _ => None, // iteration progress, confidence, plan events — not forwarded to chat UI
    }
}
