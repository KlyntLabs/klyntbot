//! Dev-only HTTP server that exposes Tauri commands as REST endpoints.
//!
//! When running `cargo tauri dev`, this server starts on port 3456 alongside
//! the Tauri app, allowing the Vite dev server (localhost:1420) to be opened
//! in Chrome with real API data via `fetch()` instead of Tauri's `invoke()`.
//!
//! Only compiled in debug builds. Chat streaming is supported via SSE
//! at `/api/events/{sessionKey}`.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderValue, Method};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use dashmap::DashMap;
use desktop_shared::errors::ApiError;
use futures_util::stream::Stream;
use serde_json::Value;
use tokio::sync::broadcast;
use tracing::{error, info};

use crate::app_core::AppCore;
use ::app_core::events::AppEventEmitter;

type SseChannels = Arc<DashMap<String, broadcast::Sender<(String, Value)>>>;

#[derive(Clone)]
struct DevState {
    core: Arc<AppCore>,
    sse_channels: SseChannels,
}

/// Bridges `AppEventEmitter` to a tokio broadcast channel for SSE streaming.
struct SseEmitter {
    tx: broadcast::Sender<(String, Value)>,
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
        .route("/api/events/{sessionKey}", axum::routing::get(sse_handler))
        .route(
            "/api/cognitive/stream",
            axum::routing::get(cognitive_sse_handler),
        )
        .route("/api/{cmd}", post(dispatch))
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

enum ApiResult {
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

fn ok(v: impl serde::Serialize) -> ApiResult {
    ApiResult::Ok(serde_json::to_value(v).unwrap_or(Value::Null))
}

fn err(e: ApiError) -> ApiResult {
    ApiResult::Err(e)
}

// ── Dispatch ────────────────────────────────────────────────────────────
//
// Each command module defines a `dispatch_dev()` function co-located with its
// Tauri commands. The dev server chains them here. This ensures parity: when
// you add a new Tauri command, the dev dispatch is right next to it.
//
// `chat_send` is handled inline because it needs SSE channel state.

use crate::commands::dev_helpers as dev;

/// Convert a module dispatch result into an `ApiResult`.
fn into_api_result(r: Result<Value, ApiError>) -> ApiResult {
    match r {
        Ok(v) => ApiResult::Ok(v),
        Err(e) => ApiResult::Err(e),
    }
}

async fn dispatch(
    State(state): State<DevState>,
    Path(cmd): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult {
    let core = &state.core;
    let cmd = cmd.as_str();

    use crate::commands;

    // ── Per-module dispatch (co-located with Tauri commands) ─────────
    if let Some(r) = commands::tasks::dispatch_dev(cmd, core, &body).await {
        return into_api_result(r);
    }
    if let Some(r) = commands::projects::dispatch_dev(cmd, core, &body).await {
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

    // ── chat_send (needs SSE channels, handled inline) ──────────────
    if cmd == "chat_send" {
        return dispatch_chat_send(core, &body, &state.sse_channels).await;
    }

    // ── open_url (desktop-like: opens URL in default browser) ───────
    if cmd == "open_url" {
        let url: String = dev::get_str(&body, "url").unwrap_or_default();
        let _ = open::that(&url);
        return ok(serde_json::json!(true));
    }

    err(ApiError::new(
        "NOT_FOUND",
        format!("command '{cmd}' is not supported in browser dev mode"),
    ))
}

/// Handle `chat_send` separately because it needs SSE channel state to relay
/// streaming agent events back to the browser via Server-Sent Events.
async fn dispatch_chat_send(core: &AppCore, body: &Value, sse_channels: &SseChannels) -> ApiResult {
    let content = match dev::get_str(body, "content") {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let session_key = match dev::get_str(body, "sessionKey") {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let context: Option<desktop_shared::commands::SessionContextInput> = dev::get(body, "context");

    match core.chat_send(content, session_key.clone(), context).await {
        Ok((user_msg, stream_info)) => {
            let tx = sse_channels
                .entry(session_key)
                .or_insert_with(|| broadcast::channel(256).0)
                .clone();
            let emitter: Arc<dyn AppEventEmitter> = Arc::new(SseEmitter { tx });
            core.spawn_chat_relay(stream_info, emitter);
            ok(user_msg)
        }
        Err(e) => err(e),
    }
}

/// SSE endpoint — streams agent events for a chat session.
///
/// The frontend (`useAgentStream.ts`) connects here in browser dev mode
/// via `new EventSource("/api/events/{sessionKey}")`.
async fn sse_handler(
    State(state): State<DevState>,
    Path(session_key): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Use atomic entry API to avoid TOCTOU race with chat_send:
    // whichever handler runs first creates the channel, the other reuses it.
    let rx = state
        .sse_channels
        .entry(session_key.clone())
        .or_insert_with(|| broadcast::channel(256).0)
        .value()
        .subscribe();

    let sse_channels = Arc::clone(&state.sse_channels);
    let sk = session_key.clone();

    let stream = futures_util::stream::unfold(
        (rx, sse_channels, sk),
        |(mut rx, channels, sk)| async move {
            loop {
                match rx.recv().await {
                    Ok((event_name, payload)) => {
                        let data = serde_json::to_string(&payload).unwrap_or_default();
                        let event = Event::default().event(&event_name).data(data);
                        if event_name == "agent:done" || event_name == "agent:error" {
                            channels.remove(&sk);
                        }
                        return Some((Ok(event), (rx, channels, sk)));
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("SSE stream for {sk} lagged by {n} events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        channels.remove(&sk);
                        return None;
                    }
                }
            }
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// SSE endpoint — streams cognitive debug events (domain events + pipeline).
///
/// The frontend (`DebugDashboard.tsx`) connects here in browser dev mode
/// via `new EventSource("/api/cognitive/stream")`.
async fn cognitive_sse_handler(
    State(state): State<DevState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Subscribe to domain events
    let domain_rx = state
        .core
        .domain_event_bus
        .as_ref()
        .map(|bus| bus.subscribe());

    // Subscribe to pipeline events
    let pipeline_rx = state
        .core
        .pipeline_broadcast
        .as_ref()
        .map(|tx| tx.subscribe());

    let stream = futures_util::stream::unfold(
        (domain_rx, pipeline_rx),
        |(mut domain_rx, mut pipeline_rx)| async move {
            loop {
                tokio::select! {
                    // Domain events
                    result = async {
                        match domain_rx.as_mut() {
                            Some(rx) => rx.recv().await.map_err(|e| matches!(e, broadcast::error::RecvError::Closed)),
                            None => std::future::pending::<Result<bus::DomainEvent, bool>>().await,
                        }
                    } => {
                        match result {
                            Ok(event) => {
                                let salience = cognitive::salience::evaluate_salience(&event);
                                let domain = domain_for_event(&event);
                                let salience_str = match salience {
                                    cognitive::types::SalienceVerdict::Extract => "extract",
                                    cognitive::types::SalienceVerdict::Accumulate => "accumulate",
                                    cognitive::types::SalienceVerdict::Discard => "discard",
                                };
                                let payload = serde_json::json!({
                                    "eventType": format!("{:?}", event).split('{').next().unwrap_or("Unknown").trim(),
                                    "salience": salience_str,
                                    "domain": domain,
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                    "payload": serde_json::to_value(&event).unwrap_or_default(),
                                });
                                let data = serde_json::to_string(&payload).unwrap_or_default();
                                let event = Event::default().event("cognitive:domain_event").data(data);
                                return Some((Ok(event), (domain_rx, pipeline_rx)));
                            }
                            Err(true) => return None, // closed
                            Err(false) => continue, // lagged
                        }
                    }

                    // Pipeline events
                    result = async {
                        match pipeline_rx.as_mut() {
                            Some(rx) => rx.recv().await.map_err(|e| matches!(e, broadcast::error::RecvError::Closed)),
                            None => std::future::pending::<Result<cognitive::PipelineEvent, bool>>().await,
                        }
                    } => {
                        match result {
                            Ok(pe) => {
                                let event_name = match &pe {
                                    cognitive::PipelineEvent::Extraction { .. } => "cognitive:extraction",
                                    cognitive::PipelineEvent::Consolidation { .. } => "cognitive:consolidation",
                                };
                                let data = serde_json::to_string(&pe).unwrap_or_default();
                                let event = Event::default().event(event_name).data(data);
                                return Some((Ok(event), (domain_rx, pipeline_rx)));
                            }
                            Err(true) => return None,
                            Err(false) => continue,
                        }
                    }
                }
            }
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Map a DomainEvent to its domain string.
fn domain_for_event(event: &bus::DomainEvent) -> &'static str {
    match event {
        bus::DomainEvent::TaskCreated { .. }
        | bus::DomainEvent::TaskCompleted { .. }
        | bus::DomainEvent::TaskDeferred { .. }
        | bus::DomainEvent::GoalProgress { .. } => "work",
        bus::DomainEvent::ActivitySessionCompleted { .. }
        | bus::DomainEvent::FocusSessionStarted { .. }
        | bus::DomainEvent::FocusSessionEnded { .. }
        | bus::DomainEvent::DistractionDetected { .. }
        | bus::DomainEvent::ProductivityScoreComputed { .. } => "energy",
        bus::DomainEvent::TransactionRecorded { .. } | bus::DomainEvent::BudgetAlert { .. } => {
            "finance"
        }
        bus::DomainEvent::UserStatedFact { .. } => "general",
        bus::DomainEvent::UserCorrectedAI { .. } => "learning",
        bus::DomainEvent::CoachingFeedback { .. } => "coaching",
        bus::DomainEvent::ChatTurnCompleted { .. } => "general",
        bus::DomainEvent::NoteCreated { .. } | bus::DomainEvent::NoteUpdated { .. } => "notes",
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
        let src = include_str!("main.rs");
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
