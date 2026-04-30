//! Chat commands — thin Tauri wrappers delegating to `AppCore` methods.

use std::sync::Arc;

use tauri::Manager;

use desktop_shared::commands::{
    ChatMessageResponse, ChatSessionResponse, ChatThreadResponse, SessionContextInput,
};
use desktop_shared::{errors::ApiError, CommandResult};
use tauri::Emitter;

use desktop_macros::klynt_command;

use crate::app_core::AppCore;

/// Bridges `AppEventEmitter` to Tauri's `Emitter` trait.
struct TauriEmitter(tauri::AppHandle);

impl ::app_core::events::AppEventEmitter for TauriEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        let _ = self.0.emit(event_name, payload);
    }
}

#[klynt_command]
pub async fn chat_threads() -> Vec<ChatThreadResponse> {
    state.chat_threads().await
}

#[klynt_command]
pub async fn chat_messages(session_key: String, limit: Option<i64>) -> Vec<ChatMessageResponse> {
    state.chat_messages(session_key, limit).await
}

#[klynt_command]
pub async fn chat_send(
    app: tauri::AppHandle,
    content: String,
    session_key: String,
    context: Option<SessionContextInput>,
    mode: Option<String>,
) -> ChatMessageResponse {
    let (user_msg, stream_info) = state.chat_send(content, session_key, context, mode).await?;

    // Spawn background task to relay streaming events via Tauri emitter
    let emitter: Arc<dyn ::app_core::events::AppEventEmitter> = Arc::new(TauriEmitter(app));

    state.spawn_chat_relay(stream_info, emitter);

    Ok(user_msg)
}

#[klynt_command]
pub async fn chat_pin_thread(session_key: String) -> () {
    state.chat_pin_thread(session_key).await
}

#[klynt_command]
pub async fn chat_rename_thread(session_key: String, title: String) -> () {
    state.chat_rename_thread(session_key, title).await
}

#[klynt_command]
pub async fn chat_delete_thread(session_key: String) -> () {
    state.chat_delete_thread(session_key).await
}

#[klynt_command]
pub async fn chat_respond_interaction(
    session_key: String,
    request_id: String,
    response: desktop_shared::specta_helpers::JsonValueWrapper,
) -> () {
    let response: common::FormResponse = serde_json::from_value(response.0)
        .map_err(|e| ApiError::new("BAD_REQUEST", e.to_string()))?;
    state
        .chat_respond_interaction(session_key, request_id, response)
        .await
}

#[klynt_command]
pub async fn chat_get_session(session_key: String) -> ChatSessionResponse {
    state.chat_get_session(session_key).await
}

#[klynt_command]
pub async fn chat_list_sessions_by_project(project_id: String) -> Vec<ChatThreadResponse> {
    state.chat_list_sessions_by_project(project_id).await
}

#[klynt_command]
pub async fn chat_delete_stale_sessions(before_days: u32) -> u64 {
    state.chat_delete_stale_sessions(before_days).await
}

#[klynt_command]
pub async fn chat_cancel(session_key: String) -> () {
    state.chat_cancel(session_key).await
}

#[klynt_command]
pub async fn chat_respond_approval(
    app: tauri::AppHandle,
    session_key: String,
    request_id: String,
    decision: app_core::coding::approval_handler::AppApprovalDecision,
) -> () {
    let core = app.state::<std::sync::Arc<app_core::AppCore>>();
    app_core::coding::approval_handler::respond_approval(
        &core.pending_approvals,
        &request_id,
        decision,
    )
    .await
    .map_err(
        |e: app_core::coding::approval_handler::ApprovalHandlerError| {
            desktop_shared::errors::ApiError::new("NOT_FOUND", e.to_string())
        },
    )?;
    let _ = session_key; // session_key unused today; Plan 4 persists per-thread rules.
    Ok(())
}

#[klynt_command]
pub async fn chat_set_mode(
    app: tauri::AppHandle,
    session_key: String,
    mode: app_core::coding::mode_handler::ChatMode,
) -> storage::SessionRow {
    let core = app.state::<std::sync::Arc<app_core::AppCore>>();
    let row = app_core::coding::mode_handler::set_mode(&core.repos, &session_key, mode)
        .await
        .map_err(|e| {
            let code = match &e {
                app_core::coding::mode_handler::ModeError::NotFound(_) => "NOT_FOUND",
                app_core::coding::mode_handler::ModeError::Storage(_) => "STORAGE_ERROR",
            };
            desktop_shared::errors::ApiError::new(code, e.to_string())
        })?;
    Ok(row)
}

// ── Dev server dispatch ─────────────────────────────────────────────
// Note: `chat_send` is dispatched directly in `dev_server.rs` because it needs
// SSE channel state. All other chat commands are handled here.

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
