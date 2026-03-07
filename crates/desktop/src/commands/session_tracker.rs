use std::sync::Arc;

use desktop_shared::errors::ApiError;
use feature_session_tracker::types::{
    BrainstormConversation, BrainstormMessage, BrainstormMode, PinnedMessage, SessionContext,
    SessionMessage, TrackedSession,
};
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn get_tracked_sessions(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<TrackedSession>, ApiError> {
    state.get_tracked_sessions().await
}

#[tauri::command]
pub async fn get_session_messages(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
    limit: Option<usize>,
) -> Result<Vec<SessionMessage>, ApiError> {
    state.get_session_messages(&session_id, limit).await
}

#[tauri::command]
pub async fn sync_sessions(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<TrackedSession>, ApiError> {
    state.sync_sessions().await
}

#[tauri::command]
pub async fn pin_session_message(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
    message_uuid: String,
    message_content: String,
    message_role: String,
) -> Result<(), ApiError> {
    state
        .pin_session_message(session_id, message_uuid, message_content, message_role)
        .await
}

#[tauri::command]
pub async fn unpin_session_message(
    state: State<'_, Arc<AppCore>>,
    pin_id: i64,
) -> Result<(), ApiError> {
    state.unpin_session_message(pin_id).await
}

#[tauri::command]
pub async fn get_pinned_messages(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
) -> Result<Vec<PinnedMessage>, ApiError> {
    state.get_pinned_messages(&session_id).await
}

#[tauri::command]
pub async fn send_to_claude_code(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
    content: String,
) -> Result<(), ApiError> {
    state.send_to_claude_code(&session_id, &content).await
}

#[tauri::command]
pub async fn create_brainstorm(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
    mode: BrainstormMode,
    model_key: Option<String>,
    agent_profile: Option<String>,
    title: Option<String>,
) -> Result<BrainstormConversation, ApiError> {
    state
        .create_brainstorm(session_id, mode, model_key, agent_profile, title)
        .await
}

#[tauri::command]
pub async fn list_brainstorms(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
) -> Result<Vec<BrainstormConversation>, ApiError> {
    state.list_brainstorms(&session_id).await
}

#[tauri::command]
pub async fn get_brainstorm_messages(
    state: State<'_, Arc<AppCore>>,
    conversation_id: String,
) -> Result<Vec<BrainstormMessage>, ApiError> {
    state.get_brainstorm_messages(&conversation_id).await
}

#[tauri::command]
pub async fn get_session_context(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
) -> Result<SessionContext, ApiError> {
    state.get_session_context(&session_id).await
}
