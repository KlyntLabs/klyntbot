//! Integration tests for the WebSocket endpoint (`GET /ws`).
//!
//! Covers AC-WS.x:
//! - WebSocket upgrade handshake
//! - chat.send → streaming AgentEvent frames
//! - done event signals end of stream
//! - chat.cancel stops streaming
//! - interaction.respond delivers FormResponse to waiting oneshot
//! - Error propagation from agent to client
//! - One-stream-at-a-time constraint
//! - Disconnect cancels in-flight processing
//!
//! Test strategy: Bind to a random localhost port with tokio::net::TcpListener,
//! serve the dashboard router, connect via tokio-tungstenite.

mod common;

// ── Upgrade ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn ws_upgrade_returns_101_switching_protocols() {
    // AC-WS.1: GET /ws with Upgrade: websocket header → 101 Switching Protocols
    // TODO: implement
}

#[tokio::test]
async fn ws_invalid_upgrade_returns_400() {
    // AC-WS.1: GET /ws without WebSocket headers → 400
    // TODO: implement
}

// ── chat.send ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn chat_send_produces_agent_event_frames() {
    // AC-WS.2: Sending {"type":"chat.send","sessionKey":"test","message":"hi"}
    // should trigger the agent and produce at least one server frame.
    // TODO: implement
}

#[tokio::test]
async fn chat_send_produces_content_chunk_frames() {
    // AC-WS.2: contentChunk frames are sent during streaming
    // Each frame: {"type":"contentChunk","data":"..."}
    // TODO: implement
}

#[tokio::test]
async fn chat_send_eventually_produces_done_frame() {
    // AC-WS.3: After contentChunk frames, a "done" frame signals completion
    // {"type":"done","content":"final accumulated text"}
    // TODO: implement
}

#[tokio::test]
async fn chat_send_uses_session_key_for_session_routing() {
    // AC-WS.2: sessionKey in chat.send maps to the correct session in storage
    // TODO: implement
}

// ── chat.cancel ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn chat_cancel_stops_streaming() {
    // AC-WS.4: During streaming, sending {"type":"chat.cancel"} triggers cancellation.
    // After cancel, no more contentChunk or done frames arrive.
    // Input bar re-enables (observable client-side — tested in frontend tests).
    // TODO: implement
}

#[tokio::test]
async fn chat_cancel_partial_content_preserved_in_session() {
    // AC-WS.4, UX §1.1: After cancel, partial response is preserved in session history
    // (not truncated). Agent stores whatever content accumulated before cancellation.
    // TODO: implement
}

// ── interaction.respond ───────────────────────────────────────────────────────

#[tokio::test]
async fn interaction_request_is_sent_when_agent_needs_input() {
    // AC-WS.5: When the agent calls ask_user, an interaction.request frame is sent:
    // {"type":"interaction.request","requestId":"uuid","title":"...","questions":[...]}
    // TODO: implement (requires MockProvider that emits InteractionBundle)
}

#[tokio::test]
async fn interaction_respond_delivers_response_to_agent() {
    // AC-WS.5: Client sends interaction.respond with matching requestId.
    // The pending oneshot::Sender is popped and fulfilled.
    // Agent continues processing after receiving the response.
    // TODO: implement
}

#[tokio::test]
async fn interaction_respond_wrong_request_id_is_ignored() {
    // AC-WS.5: Responding with a non-existent requestId does not crash the server
    // TODO: implement
}

#[tokio::test]
async fn interaction_respond_cancelled_response() {
    // AC-WS.5, UX §1.1: User cancels interaction form → sends { "Cancelled": null }
    // Agent receives the cancellation and handles gracefully
    // TODO: implement
}

// ── Error propagation ─────────────────────────────────────────────────────────

#[tokio::test]
async fn agent_error_propagates_as_error_frame() {
    // AC-WS.6: When the agent errors during processing, an error frame is sent:
    // {"type":"error","message":"what went wrong"}
    // TODO: implement (requires MockProvider or ErrorProvider that returns an error)
}

#[tokio::test]
async fn malformed_client_message_produces_error_frame() {
    // AC-WS.6: Sending invalid JSON or unknown type → error frame (not a crash)
    // TODO: implement
}

// ── One-stream-at-a-time constraint ──────────────────────────────────────────

#[tokio::test]
async fn second_chat_send_while_streaming_is_rejected() {
    // AC-WS.7, UX §1.4: If a stream is already active and client sends another
    // chat.send, the server MUST reject or cancel it.
    // Constraint: one streaming operation per connection at a time.
    // TODO: implement
}

// ── Disconnect ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn client_disconnect_cancels_inflight_processing() {
    // AC-WS.8: When the WebSocket client disconnects mid-stream,
    // the cancel_token is triggered and the agent processing task is cleaned up.
    // TODO: implement
}

#[tokio::test]
async fn server_cleans_up_pending_interactions_on_disconnect() {
    // AC-WS.8: Any pending oneshot senders in the interaction HashMap are dropped
    // when the connection closes, preventing memory leaks.
    // TODO: implement
}
