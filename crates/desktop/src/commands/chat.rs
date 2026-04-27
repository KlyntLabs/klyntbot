//! Chat commands — thin Tauri wrappers delegating to `AppCore` methods.

use std::sync::Arc;

use desktop_shared::commands::{
    ChatMessageResponse, ChatSessionResponse, ChatThreadResponse, SessionContextInput,
};
use desktop_shared::{errors::ApiError, CommandResult};
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
#[specta::specta]
pub async fn chat_threads(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<ChatThreadResponse>> {
    state.chat_threads().await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_messages(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
    limit: Option<i64>,
) -> CommandResult<Vec<ChatMessageResponse>> {
    state.chat_messages(session_key, limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_send(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppCore>>,
    content: String,
    session_key: String,
    context: Option<SessionContextInput>,
) -> CommandResult<ChatMessageResponse> {
    let (user_msg, stream_info) = state.chat_send(content, session_key, context).await?;

    // Spawn background task to relay streaming events via Tauri emitter
    let emitter: Arc<dyn ::app_core::events::AppEventEmitter> = Arc::new(TauriEmitter(app));

    state.spawn_chat_relay(stream_info, emitter);

    Ok(user_msg)
}

#[tauri::command]
#[specta::specta]
pub async fn chat_pin_thread(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
) -> CommandResult<()> {
    state.chat_pin_thread(session_key).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_rename_thread(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
    title: String,
) -> CommandResult<()> {
    state.chat_rename_thread(session_key, title).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_delete_thread(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
) -> CommandResult<()> {
    state.chat_delete_thread(session_key).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_respond_interaction(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
    request_id: String,
    response: desktop_shared::specta_helpers::JsonValueWrapper,
) -> CommandResult<()> {
    let response: common::FormResponse = serde_json::from_value(response.0)
        .map_err(|e| ApiError::new("BAD_REQUEST", e.to_string()))?;
    state
        .chat_respond_interaction(session_key, request_id, response)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_get_session(
    state: State<'_, Arc<AppCore>>,
    session_key: String,
) -> CommandResult<ChatSessionResponse> {
    state.chat_get_session(session_key).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_list_sessions_by_project(
    state: State<'_, Arc<AppCore>>,
    project_id: String,
) -> CommandResult<Vec<ChatThreadResponse>> {
    state.chat_list_sessions_by_project(project_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_delete_stale_sessions(
    state: State<'_, Arc<AppCore>>,
    before_days: u32,
) -> CommandResult<u64> {
    state.chat_delete_stale_sessions(before_days).await
}

#[tauri::command]
#[specta::specta]
pub async fn chat_cancel(state: State<'_, Arc<AppCore>>, session_key: String) -> CommandResult<()> {
    state.chat_cancel(session_key).await
}

// ── Dev server dispatch ─────────────────────────────────────────────
// Note: `chat_send` is dispatched directly in `dev_server.rs` because it needs
// SSE channel state. All other chat commands are handled here.

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "chat_threads",
    "chat_messages",
    "chat_get_session",
    "chat_list_sessions_by_project",
    "chat_delete_stale_sessions",
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
) -> Option<CommandResult<serde_json::Value>> {
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
        "chat_get_session" => {
            let session_key = try_field!(dev::get_str(body, "sessionKey"));
            dev::val(core.chat_get_session(session_key).await)
        }
        "chat_list_sessions_by_project" => {
            let project_id = try_field!(dev::get_str(body, "projectId"));
            dev::val(core.chat_list_sessions_by_project(project_id).await)
        }
        "chat_delete_stale_sessions" => {
            let before_days: u32 = try_field!(dev::require(body, "beforeDays"));
            dev::val(core.chat_delete_stale_sessions(before_days).await)
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
