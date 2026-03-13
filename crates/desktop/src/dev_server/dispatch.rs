use axum::extract::{Path, State};
use axum::Json;
use serde_json::Value;

use super::{err, into_api_result, ok, ApiResult, DevState};
use crate::commands;
use crate::commands::dev_helpers as dev;

// ── Dispatch ────────────────────────────────────────────────────────────
//
// Each command module defines a `dispatch_dev()` function co-located with its
// Tauri commands. The dev server chains them here. This ensures parity: when
// you add a new Tauri command, the dev dispatch is right next to it.
//
// `chat_send` is handled inline because it needs SSE channel state.

pub(super) async fn dispatch(
    State(state): State<DevState>,
    Path(cmd): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult {
    let core = &state.core;
    let cmd = cmd.as_str();

    // ── Per-module dispatch (co-located with Tauri commands) ─────────
    if let Some(r) = commands::tasks::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::projects::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::entity_links::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::project_sources::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::project_memories::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::project_conversations::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::areas::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::objectives::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::key_results::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::status::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::finance::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::notes::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::productivity::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::distraction::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::settings::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::chat::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::groups::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::workflows::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::columns::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::cognitive::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::timeline::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::cron::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::capture::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::work_context::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::workspace::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }

    // ── chat_send (needs SSE channels, handled inline) ──────────────
    if cmd == "chat_send" {
        return super::streaming::dispatch_chat_send(core, &body, &state.sse_channels).await;
    }

    // ── open_url (desktop-like: opens URL in default browser) ───────
    if cmd == "open_url" {
        let url: String = dev::get_str(&body, "url").unwrap_or_default();
        let _ = open::that(&url);
        return ok(serde_json::json!(true));
    }

    err(desktop_shared::errors::ApiError::new(
        "NOT_FOUND",
        format!("command '{cmd}' is not supported in browser dev mode"),
    ))
}
