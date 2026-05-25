use axum::extract::{Path, State};
use axum::Json;
use serde_json::Value;

use std::sync::Arc;

use super::{err, into_api_result, ok, ApiResult, DevState, SseEmitter};
use crate::commands;
use crate::commands::dev_helpers as dev;
use ::app_core::events::AppEventEmitter;

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

    // ── Transport-neutral registry (preferred) ─────────────────────
    // Every `#[klynt_command]` whose params are all JSON-deserializable
    // registers a `json` handler in `KLYNT_COMMANDS`. We run it here — the
    // *same* handler the Tauri IPC adapter uses — so arg decoding cannot drift.
    // `DEV_INLINE` commands need SSE channels / browser-specific behaviour and
    // are handled inline below instead.
    const DEV_INLINE: &[&str] = &[
        "note_insight_review",
        "chat_send",
        "note_insight_tab_chat",
        "open_url",
    ];
    if !DEV_INLINE.contains(&cmd) {
        if let Some(reg) = crate::specta_builder::KLYNT_COMMANDS
            .iter()
            .find(|c| c.name == cmd)
        {
            if let Some(json) = reg.json {
                let emitter: Arc<dyn AppEventEmitter> = Arc::new(SseEmitter {
                    tx: state.global_event_tx.clone(),
                });
                return into_api_result(json(body.clone(), core.clone(), emitter).await);
            }
        }
    }

    // ── note_insight_review (needs SSE emitter, handled inline) ─────
    if cmd == "note_insight_review" {
        let id = dev::get_str(&body, "noteId").unwrap_or_default();
        let scope: Option<desktop_shared::commands::InsightScopeConfigParams> = body
            .get("scopeConfig")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let emitter: Arc<dyn AppEventEmitter> = Arc::new(SseEmitter {
            tx: state.insight_tx.clone(),
        });
        return into_api_result(
            core.note_insight_review(&id, scope.as_ref(), Some(emitter))
                .await
                .map(|v| serde_json::to_value(v).unwrap_or_default()),
        );
    }

    // ── Per-module dispatch (co-located with Tauri commands) ─────────
    if let Some(r) = commands::notes::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::productivity::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::distraction::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::chat::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::launcher::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::shortcuts::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::status_badge::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::focus::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }

    // ── chat_send (needs SSE channels, handled inline) ──────────────
    if cmd == "chat_send" {
        return super::streaming::dispatch_chat_send(core, &body, &state.sse_channels).await;
    }

    // ── note_insight_tab_chat (needs SSE channels, handled inline) ──
    if cmd == "note_insight_tab_chat" {
        return super::streaming::dispatch_insight_tab_chat(core, &body, &state.sse_channels).await;
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
