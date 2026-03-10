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

// ── Dev server dispatch ─────────────────────────────────────────────
// Note: `chat_send` is dispatched directly in `dev_server.rs` because it needs
// SSE channel state. All other chat commands are handled here.

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "chat_threads",
    "chat_messages",
    "chat_pin_thread",
    "chat_rename_thread",
    "chat_delete_thread",
    "chat_cancel",
    "chat_respond_interaction",
    // chat_send handled in dev_server.rs (needs SSE channels)
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "chat_threads" => dev::val(core.chat_threads().await),
        "chat_messages" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            dev::val(
                core.chat_messages(session_key, dev::get(body, "limit"))
                    .await,
            )
        }
        "chat_pin_thread" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            dev::val(core.chat_pin_thread(session_key).await)
        }
        "chat_rename_thread" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            let title = try_field!(dev::get_str(body, "title"));
            dev::val(core.chat_rename_thread(session_key, title).await)
        }
        "chat_delete_thread" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            dev::val(core.chat_delete_thread(session_key).await)
        }
        "chat_cancel" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            dev::val(core.chat_cancel(session_key).await)
        }
        "chat_respond_interaction" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            let request_id = try_field!(dev::get_str(body, "requestId"));
            let response: common::FormResponse = try_field!(dev::require(body, "response"));
            dev::val(
                core.chat_respond_interaction(session_key, request_id, response)
                    .await,
            )
        }
        _ => return None,
    })
}
