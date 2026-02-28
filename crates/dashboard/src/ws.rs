//! WebSocket endpoint (`GET /ws`) — streaming agent protocol.
//!
//! # Protocol
//!
//! ## Client → Server
//! ```json
//! {"type":"chat.send","sessionKey":"<key>","message":"<text>"}
//! {"type":"chat.cancel"}
//! {"type":"interaction.respond","requestId":"<uuid>","response":<FormResponse>}
//! ```
//!
//! ## Server → Client
//! Any `AgentEvent` variant (already JSON-tagged with camelCase type), plus:
//! ```json
//! {"type":"interaction.request","requestId":"<uuid>","title":"<str>","questions":[...]}
//! ```
//!
//! ## Constraints
//! - One streaming operation per connection at a time (AC-WS.7).
//! - `chat.cancel` or client disconnect cancels in-flight processing (AC-WS.4/8).
//! - Pending `ask_user` senders are dropped on disconnect to unblock the agent.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::state::AppState;

// ── Client → Server messages ─────────────────────────────────────────────────

/// Messages the browser can send over the WebSocket.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Start a new streaming chat turn.
    #[serde(rename = "chat.send")]
    ChatSend {
        #[serde(rename = "sessionKey")]
        session_key: String,
        message: String,
    },

    /// Cancel the current in-flight streaming request.
    #[serde(rename = "chat.cancel")]
    ChatCancel,

    /// Deliver user's answer for a pending `ask_user` request.
    #[serde(rename = "interaction.respond")]
    InteractionRespond {
        #[serde(rename = "requestId")]
        request_id: Uuid,
        response: common::FormResponse,
    },
}

// ── Server → Client extra frames ─────────────────────────────────────────────

/// Sent when the agent calls `ask_user` and needs the user to fill in a form.
#[derive(Debug, Serialize)]
struct InteractionRequestFrame<'a> {
    #[serde(rename = "type")]
    msg_type: &'static str,
    #[serde(rename = "requestId")]
    request_id: Uuid,
    title: &'a str,
    questions: &'a [common::Question],
}

// ── Internal fwd channel ──────────────────────────────────────────────────────

/// Messages forwarded from background streaming tasks into the main socket loop.
enum FwdMsg {
    /// A JSON frame ready to send to the WebSocket client.
    Frame(String),
    /// The active streaming session ended (event_rx channel closed).
    StreamEnded,
}

// ── Handler ───────────────────────────────────────────────────────────────────

/// WebSocket upgrade handler — wired to `GET /ws` in the router.
pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    // Split into independent send/receive halves for concurrent use.
    let (mut ws_tx, mut ws_rx) = socket.split();

    // Channel for background tasks to forward frames to the WS sender.
    let (fwd_tx, mut fwd_rx) = mpsc::channel::<FwdMsg>(64);

    // Tracks the cancel token of the in-flight streaming session.
    let mut cancel_token: Option<CancellationToken> = None;

    // Pending ask_user requests: requestId → oneshot sender for the agent.
    let pending: Arc<Mutex<HashMap<Uuid, oneshot::Sender<common::FormResponse>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    loop {
        tokio::select! {
            // Forward accumulated frames or handle stream-ended signal.
            fwd = fwd_rx.recv() => {
                match fwd {
                    Some(FwdMsg::Frame(json)) => {
                        if ws_tx.send(Message::text(json)).await.is_err() {
                            break;
                        }
                    }
                    Some(FwdMsg::StreamEnded) => {
                        // Streaming session finished — allow next chat.send.
                        cancel_token = None;
                        pending.lock().await.clear();
                    }
                    // fwd_tx is held in scope; None only at shutdown.
                    None => break,
                }
            },

            // Handle messages from the WebSocket client.
            msg_opt = ws_rx.next() => {
                match msg_opt {
                    Some(Ok(Message::Text(text))) => {
                        on_client_text(
                            text.as_str(),
                            &state,
                            &fwd_tx,
                            Arc::clone(&pending),
                            &mut cancel_token,
                        )
                        .await;
                    }
                    // Client closed or connection dropped.
                    Some(Ok(Message::Close(_))) | None => {
                        if let Some(ct) = cancel_token.take() {
                            ct.cancel();
                        }
                        break;
                    }
                    Some(Err(_)) => break,
                    // Ping/Pong/Binary — not used by this protocol.
                    _ => {}
                }
            }
        }
    }

    // Final cleanup: cancel any remaining in-flight task.
    if let Some(ct) = cancel_token {
        ct.cancel();
    }
    // Dropping `pending` drops all oneshot senders → agent unblocks with Err.
}

/// Handle a text frame received from the WebSocket client.
async fn on_client_text(
    text: &str,
    state: &AppState,
    fwd_tx: &mpsc::Sender<FwdMsg>,
    pending: Arc<Mutex<HashMap<Uuid, oneshot::Sender<common::FormResponse>>>>,
    cancel_token: &mut Option<CancellationToken>,
) {
    let msg = match serde_json::from_str::<ClientMessage>(text) {
        Ok(m) => m,
        Err(e) => {
            send_error(fwd_tx, format!("Invalid message: {}", e)).await;
            return;
        }
    };

    match msg {
        ClientMessage::ChatSend {
            session_key,
            message,
        } => {
            // AC-WS.7: one stream at a time — reject if already active.
            if cancel_token.is_some() {
                send_error(fwd_tx, "A chat request is already in progress".into()).await;
                return;
            }

            // Clone session key before it's moved into process_direct_streaming.
            let sk_for_persist = session_key.clone();

            match state
                .agent_loop
                .process_direct_streaming(message, session_key)
                .await
            {
                Ok(streaming_handle) => {
                    *cancel_token = Some(streaming_handle.cancel_token);

                    let mut event_rx = streaming_handle.event_rx;
                    let mut interaction_rx = streaming_handle.interaction_rx;
                    let fwd_events = fwd_tx.clone();
                    let fwd_interactions = fwd_tx.clone();

                    // Clone state for metadata persistence after stream ends.
                    let repos = state.repos.clone();
                    let sk = sk_for_persist;

                    // Task 1: Forward AgentEvents → FwdMsg::Frame; accumulate
                    // tool calls and entity cards, then persist after stream ends.
                    tokio::spawn(async move {
                        // Accumulate tool-call and entity-card data during streaming.
                        let mut pending_args: Vec<(String, serde_json::Value)> = Vec::new();
                        let mut completed_tools: Vec<serde_json::Value> = Vec::new();
                        let mut entity_cards: Vec<serde_json::Value> = Vec::new();

                        while let Some(event) = event_rx.recv().await {
                            // Collect structured metadata from specific event types.
                            if let agent::AgentEvent::ToolStart { name, args } = &event {
                                pending_args.push((name.clone(), args.clone()));
                            }
                            if let agent::AgentEvent::ToolEnd {
                                name,
                                success,
                                duration_ms,
                                result,
                            } = &event
                            {
                                let args = pending_args
                                    .iter()
                                    .rposition(|t| t.0 == *name)
                                    .map(|i| pending_args.remove(i).1);
                                completed_tools.push(serde_json::json!({
                                    "name": name,
                                    "args": args,
                                    "durationMs": duration_ms,
                                    "success": success,
                                    "result": result,
                                }));
                            }
                            if let agent::AgentEvent::EntityCreated(card) = &event {
                                if let Ok(v) = serde_json::to_value(card) {
                                    entity_cards.push(v);
                                }
                            }

                            match serde_json::to_string(&event) {
                                Ok(json) => {
                                    if fwd_events.send(FwdMsg::Frame(json)).await.is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }

                        tracing::debug!(
                            "WS stream ended for {}: {} tools, {} entities",
                            sk,
                            completed_tools.len(),
                            entity_cards.len()
                        );

                        // Persist accumulated metadata to the latest assistant message.
                        if !completed_tools.is_empty() || !entity_cards.is_empty() {
                            let tc = if completed_tools.is_empty() {
                                None
                            } else {
                                Some(serde_json::Value::Array(completed_tools))
                            };
                            let metadata = if entity_cards.is_empty() {
                                None
                            } else {
                                let mut m = serde_json::Map::new();
                                m.insert(
                                    "entityCards".into(),
                                    serde_json::Value::Array(entity_cards),
                                );
                                Some(serde_json::Value::Object(m))
                            };
                            match repos
                                .sessions
                                .update_last_assistant_metadata(
                                    &sk,
                                    tc.as_ref(),
                                    metadata.as_ref(),
                                )
                                .await
                            {
                                Ok(updated) => {
                                    tracing::debug!(
                                        "Metadata persist for {}: updated={}",
                                        sk, updated
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "Metadata persist FAILED for {}: {}",
                                        sk, e
                                    );
                                }
                            }
                        }

                        // Notify main loop that streaming is complete.
                        let _ = fwd_events.send(FwdMsg::StreamEnded).await;
                    });

                    // Task 2: Forward InteractionBundles with generated request IDs.
                    tokio::spawn(async move {
                        while let Some(bundle) = interaction_rx.recv().await {
                            let request_id = Uuid::new_v4();
                            let frame = InteractionRequestFrame {
                                msg_type: "interaction.request",
                                request_id,
                                title: &bundle.request.title,
                                questions: &bundle.request.questions,
                            };
                            match serde_json::to_string(&frame) {
                                Ok(json) => {
                                    pending.lock().await.insert(request_id, bundle.response_tx);
                                    if fwd_interactions.send(FwdMsg::Frame(json)).await.is_err() {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                    });
                }
                Err(e) => {
                    send_error(fwd_tx, e.to_string()).await;
                }
            }
        }

        ClientMessage::ChatCancel => {
            if let Some(ct) = cancel_token.take() {
                ct.cancel();
            }
        }

        ClientMessage::InteractionRespond {
            request_id,
            response,
        } => {
            // Pop and fulfill the waiting oneshot sender.
            // Unknown requestIds are silently ignored (AC-WS.5).
            if let Some(sender) = pending.lock().await.remove(&request_id) {
                let _ = sender.send(response);
            }
        }
    }
}

/// Helper: send a JSON error frame through the forward channel.
async fn send_error(fwd_tx: &mpsc::Sender<FwdMsg>, message: String) {
    let json = serde_json::json!({ "type": "error", "message": message }).to_string();
    let _ = fwd_tx.send(FwdMsg::Frame(json)).await;
}
