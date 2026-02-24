//! Integration tests for /api/sessions endpoints.
//!
//! Covers AC-9.x: List sessions, get session with messages, delete session.

mod common;

#[tokio::test]
async fn get_sessions_empty_returns_empty_array() {
    // AC-9.1: GET /api/sessions on fresh DB returns []
    // TODO: implement
}

#[tokio::test]
async fn get_sessions_returns_session_list_rows() {
    // AC-9.1: GET /api/sessions returns Vec<SessionListRow> (not full messages)
    // List view is lightweight — no message payloads included.
    // TODO: implement
}

#[tokio::test]
async fn get_session_by_key_includes_messages() {
    // AC-9.2: GET /api/sessions/{key} returns { session: SessionRow, messages: Vec<SessionMessageRow> }
    // This is the full session view used when resuming a conversation.
    // TODO: implement
}

#[tokio::test]
async fn get_session_not_found_returns_404() {
    // AC-9.2: GET /api/sessions/nonexistent-key → 404
    // TODO: implement
}

#[tokio::test]
async fn delete_session_returns_204() {
    // AC-9.3: DELETE /api/sessions/{key} → 204 No Content
    // TODO: implement
}

#[tokio::test]
async fn delete_session_removes_messages() {
    // AC-9.3: After DELETE, GET /api/sessions/{key} returns 404
    // TODO: implement
}

#[tokio::test]
async fn session_list_fields_are_camel_case() {
    // AC-1.5: SessionListRow response fields are camelCase
    // TODO: implement
}
