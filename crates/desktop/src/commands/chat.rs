//! Chat commands — thin Tauri wrappers delegating to `AppCore` methods.

use std::sync::Arc;

use desktop_shared::commands::{ChatMessageResponse, ChatThreadResponse, SessionContextInput};
use desktop_shared::errors::ApiError;
use tauri::{Emitter, State};

use crate::app_core::AppCore;

/// Bridges `AppEventEmitter` to Tauri's `Emitter` trait.
struct TauriEmitter(tauri::AppHandle);

impl ::app_core::events::AppEventEmitter for TauriEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        let _ = self.0.emit(event_name, payload);
    }
}

#[tauri::command]
pub async fn chat_threads(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ChatThreadResponse>, ApiError> {
    state.chat_threads().await
}

#[tauri::command]
pub async fn chat_messages(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
    limit: Option<i64>,
) -> Result<Vec<ChatMessageResponse>, ApiError> {
    state.chat_messages(session_key, limit).await
}

#[tauri::command]
pub async fn chat_send(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppCore>>,
    content: String,
    session_key: String,
    context: Option<SessionContextInput>,
) -> Result<ChatMessageResponse, ApiError> {
    let (user_msg, stream_info) = state.chat_send(content, session_key, context).await?;

    // Spawn background task to relay streaming events via Tauri emitter
    let emitter: Arc<dyn ::app_core::events::AppEventEmitter> = Arc::new(TauriEmitter(app));

    state.spawn_chat_relay(stream_info, emitter);

    Ok(user_msg)
}

#[tauri::command]
pub async fn chat_pin_thread(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
) -> Result<(), ApiError> {
    state.chat_pin_thread(session_key).await
}

#[tauri::command]
pub async fn chat_rename_thread(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
    title: String,
) -> Result<(), ApiError> {
    state.chat_rename_thread(session_key, title).await
}

#[tauri::command]
pub async fn chat_delete_thread(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
) -> Result<(), ApiError> {
    state.chat_delete_thread(session_key).await
}

#[tauri::command]
pub async fn chat_respond_interaction(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
    request_id: String,
    response: common::FormResponse,
) -> Result<(), ApiError> {
    state
        .chat_respond_interaction(session_key, request_id, response)
        .await
}

#[tauri::command]
pub async fn chat_cancel(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
) -> Result<(), ApiError> {
    state.chat_cancel(session_key).await
}
