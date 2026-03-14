//! Dev-only HTTP server that exposes Tauri commands as REST endpoints.
//!
//! When running `cargo tauri dev`, this server starts on port 3456 alongside
//! the Tauri app, allowing the Vite dev server (localhost:1420) to be opened
//! in Chrome with real API data via `fetch()` instead of Tauri's `invoke()`.
//!
//! Only compiled in debug builds. Chat streaming is supported via SSE
//! at `/api/events/{sessionKey}`.

mod dispatch;
mod ingest;
mod streaming;

use std::sync::Arc;

use axum::http::{HeaderValue, Method};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use dashmap::DashMap;
use desktop_shared::errors::ApiError;
use serde_json::Value;
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::app_core::AppCore;
use ::app_core::events::AppEventEmitter;

pub(super) type SseChannels = Arc<DashMap<String, broadcast::Sender<(String, Value)>>>;

#[derive(Clone)]
pub(super) struct DevState {
    pub(super) core: Arc<AppCore>,
    pub(super) sse_channels: SseChannels,
}

/// Bridges `AppEventEmitter` to a tokio broadcast channel for SSE streaming.
pub(super) struct SseEmitter {
    pub(super) tx: broadcast::Sender<(String, Value)>,
}

impl AppEventEmitter for SseEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        let _ = self.tx.send((event_name.to_string(), payload));
    }
}

/// Start the dev HTTP server on port 3456.
pub async fn start(core: Arc<AppCore>) {
    let sse_channels: SseChannels = Arc::new(DashMap::new());

    let state = DevState { core, sse_channels };

    let app = Router::new()
        .route(
            "/api/events/{sessionKey}",
            axum::routing::get(streaming::sse_handler),
        )
        .route(
            "/api/cognitive/stream",
            axum::routing::get(streaming::cognitive_sse_handler),
        )
        .route("/api/v1/ingest", post(ingest::ingest_handler))
        .route("/api/v1/ingest/batch", post(ingest::ingest_batch_handler))
        .route("/api/{cmd}", post(dispatch::dispatch))
        .with_state(state);

    // Add CORS headers for the Vite dev server
    let app = app.layer(
        tower_http::cors::CorsLayer::new()
            .allow_origin("http://localhost:1420".parse::<HeaderValue>().unwrap())
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(tower_http::cors::Any),
    );

    let listener = match tokio::net::TcpListener::bind("127.0.0.1:3456").await {
        Ok(l) => l,
        Err(e) => {
            error!("dev server failed to bind port 3456: {e}");
            return;
        }
    };
    info!("dev server listening on http://127.0.0.1:3456");
    if let Err(e) = axum::serve(listener, app).await {
        error!("dev server error: {e}");
    }
}

// ── Response helpers ────────────────────────────────────────────────────

pub(super) enum ApiResult {
    Ok(Value),
    Err(ApiError),
}

impl IntoResponse for ApiResult {
    fn into_response(self) -> axum::response::Response {
        match self {
            ApiResult::Ok(v) => Json(v).into_response(),
            ApiResult::Err(e) => {
                let status = match e.code.as_str() {
                    "NOT_FOUND" => axum::http::StatusCode::NOT_FOUND,
                    "CONFLICT" => axum::http::StatusCode::CONFLICT,
                    "VALIDATION" | "INVALID_PARAMS" => axum::http::StatusCode::BAD_REQUEST,
                    "FEATURE_DISABLED" => axum::http::StatusCode::SERVICE_UNAVAILABLE,
                    _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, Json(e)).into_response()
            }
        }
    }
}

pub(super) fn ok(v: impl serde::Serialize) -> ApiResult {
    ApiResult::Ok(serde_json::to_value(v).unwrap_or(Value::Null))
}

pub(super) fn err(e: ApiError) -> ApiResult {
    ApiResult::Err(e)
}

/// Convert a module dispatch result into an `ApiResult`.
pub(super) fn into_api_result(r: Result<Value, ApiError>) -> ApiResult {
    match r {
        Ok(v) => ApiResult::Ok(v),
        Err(e) => ApiResult::Err(e),
    }
}

// ── Parity test ─────────────────────────────────────────────────────────
//
// Verifies that every Tauri command registered in `main.rs` has a matching
// entry in some module's `dispatch_dev` (via `DEV_COMMANDS`). If someone adds
// a new Tauri command but forgets the dev dispatch, this test fails.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    /// Commands that only exist in Tauri IPC (desktop-only, no HTTP equivalent).
    const TAURI_ONLY: &[&str] = &[
        "permissions_check_accessibility",
        "permissions_open_accessibility",
        "resize_window",
        "open_url",
        "quit_app",
        "show_dashboard",
        "focus_timer_start",
        "focus_timer_stop",
        "focus_timer_status",
        "focus_break_start",
        "focus_timer_extend",
        "focus_timer_pause",
        "focus_timer_resume",
        "mcp_oauth_start",
        "mcp_oauth_disconnect",
    ];

    /// Parse Tauri command function names from `main.rs` source text.
    fn tauri_command_names() -> BTreeSet<String> {
        let src = include_str!("../main.rs");
        src.lines()
            .map(str::trim)
            .filter(|l| l.starts_with("commands::") || l.starts_with("oauth::commands::"))
            .filter_map(|l| {
                l.rsplit("::")
                    .next()
                    .map(|s| s.trim_end_matches(',').to_string())
            })
            .collect()
    }

    /// Collect all dev command names from module `DEV_COMMANDS` arrays.
    fn dev_command_names() -> BTreeSet<String> {
        use crate::commands;
        let modules: &[&[&str]] = &[
            commands::tasks::DEV_COMMANDS,
            commands::projects::DEV_COMMANDS,
            commands::areas::DEV_COMMANDS,
            commands::objectives::DEV_COMMANDS,
            commands::key_results::DEV_COMMANDS,
            commands::status::DEV_COMMANDS,
            commands::finance::DEV_COMMANDS,
            commands::notes::DEV_COMMANDS,
            commands::productivity::DEV_COMMANDS,
            commands::distraction::DEV_COMMANDS,
            commands::settings::DEV_COMMANDS,
            commands::chat::DEV_COMMANDS,
            commands::groups::DEV_COMMANDS,
            commands::workflows::DEV_COMMANDS,
            commands::columns::DEV_COMMANDS,
            commands::cognitive::DEV_COMMANDS,
            commands::timeline::DEV_COMMANDS,
            commands::cron::DEV_COMMANDS,
            commands::capture::DEV_COMMANDS,
            commands::work_context::DEV_COMMANDS,
            commands::entity_links::DEV_COMMANDS,
            commands::project_sources::DEV_COMMANDS,
            commands::project_memories::DEV_COMMANDS,
            commands::project_conversations::DEV_COMMANDS,
            commands::agents::DEV_COMMANDS,
            commands::workspace::DEV_COMMANDS,
            commands::integrations::DEV_COMMANDS,
        ];
        // chat_send is handled inline in dev_server.rs
        let mut set: BTreeSet<String> = modules
            .iter()
            .flat_map(|m| m.iter().map(|s| s.to_string()))
            .collect();
        set.insert("chat_send".to_string());
        set
    }

    #[test]
    fn dev_server_covers_all_tauri_commands() {
        let tauri = tauri_command_names();
        let dev = dev_command_names();
        let tauri_only: BTreeSet<String> = TAURI_ONLY.iter().map(|s| s.to_string()).collect();

        let expected: BTreeSet<String> = tauri.difference(&tauri_only).cloned().collect();
        let missing: Vec<&String> = expected.difference(&dev).collect();

        assert!(
            missing.is_empty(),
            "Tauri commands missing from dev server dispatch: {missing:?}\n\
             Add dispatch_dev entries in the corresponding commands/*.rs module."
        );
    }

    #[test]
    fn dev_server_has_no_orphan_commands() {
        let tauri = tauri_command_names();
        let dev = dev_command_names();

        let orphans: Vec<&String> = dev.difference(&tauri).collect();

        assert!(
            orphans.is_empty(),
            "Dev server dispatches commands not registered in Tauri: {orphans:?}\n\
             Remove orphan entries from the corresponding DEV_COMMANDS array."
        );
    }
}
