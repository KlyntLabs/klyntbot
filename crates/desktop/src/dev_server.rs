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

use ::app_core::events::AppEventEmitter;
use crate::app_core::AppCore;

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

    let state = DevState {
        core,
        sse_channels,
    };

    let app = Router::new()
        .route("/api/events/{sessionKey}", axum::routing::get(sse_handler))
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

/// Convert a plain `Result<T, ApiError>` into `ApiResult`.
fn r<T: serde::Serialize>(result: Result<T, ApiError>) -> ApiResult {
    match result {
        Ok(v) => ok(v),
        Err(e) => err(e),
    }
}

/// Convert a `HandlerResult<T>` (value + entity updates) into `ApiResult`,
/// discarding the entity updates (no Tauri event bus in dev mode).
fn rh<T: serde::Serialize, U>(result: Result<(T, U), ApiError>) -> ApiResult {
    match result {
        Ok((v, _)) => ok(v),
        Err(e) => err(e),
    }
}

// ── JSON extraction helpers ─────────────────────────────────────────────

/// Extract a typed field from the JSON body (returns `None` if missing or wrong type).
fn get<T: serde::de::DeserializeOwned>(body: &Value, key: &str) -> Option<T> {
    body.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// Extract a required string field.
fn get_str(body: &Value, key: &str) -> Result<String, ApiError> {
    body.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| ApiError::new("VALIDATION", format!("missing required field: {key}")))
}

/// Deserialize structured params from the body (tries `body.params` first, then `body` itself).
fn parse_params<T: serde::de::DeserializeOwned>(body: &Value) -> Result<T, ApiError> {
    serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone()))
        .map_err(|e| ApiError::new("VALIDATION", e.to_string()))
}

/// Require a typed field (returns validation error if missing).
fn require<T: serde::de::DeserializeOwned>(body: &Value, key: &str) -> Result<T, ApiError> {
    get(body, key)
        .ok_or_else(|| ApiError::new("VALIDATION", format!("missing required field: {key}")))
}

// ── Dispatch ────────────────────────────────────────────────────────────

async fn dispatch(
    State(state): State<DevState>,
    Path(cmd): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult {
    let core = &state.core;
    match cmd.as_str() {
        // ── Tasks ──────────────────────────────────────────────
        "task_list" => r(core
            .task_list(
                get(&body, "area_id"),
                get(&body, "project_id"),
                get(&body, "status"),
            )
            .await),
        "task_get" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.task_get(id).await)
        }
        "task_create" => rh(core
            .task_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "task_update" => rh(core
            .task_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "task_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.task_delete(id).await)
        }
        "task_toggle_complete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.task_toggle_complete(id).await)
        }
        "task_list_children" => {
            let parent_id = match get_str(&body, "parentId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.task_list_children(parent_id).await)
        }
        "today_tasks" => r(core.today_tasks().await),

        // ── Projects ──────────────────────────────────────────
        "project_list" => r(core.project_list_for_tasks(get(&body, "area_id")).await),
        "project_create" => rh(core
            .project_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "project_get" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.project_get(id).await)
        }
        "project_update" => rh(core
            .project_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "project_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.project_delete(id).await)
        }
        "project_archive" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.project_archive(id).await)
        }

        // ── Areas ─────────────────────────────────────────────
        "area_list" => r(core.area_list().await),
        "area_create" => rh(core
            .area_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "area_update" => rh(core
            .area_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "area_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.area_delete(id).await)
        }
        "area_reorder" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core
                .area_reorder(id, get(&body, "position").unwrap_or(0))
                .await)
        }

        // ── Objectives ────────────────────────────────────────
        "objective_list" => r(core
            .objective_list_for_tasks(get(&body, "project_id"))
            .await),
        "objective_create" => rh(core
            .objective_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "objective_get" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.objective_get(id).await)
        }
        "objective_update" => rh(core
            .objective_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "objective_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.objective_delete(id).await)
        }

        // ── Key Results ───────────────────────────────────────
        "key_result_create" => rh(core
            .key_result_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "key_result_update" => rh(core
            .key_result_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "key_result_update_metric" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core
                .key_result_update_metric(id, get(&body, "currentValue").unwrap_or(0.0))
                .await)
        }
        "key_result_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.key_result_delete(id).await)
        }

        // ── Status ────────────────────────────────────────────
        "agent_status" => r(core.agent_status().await),

        // ── Finance — queries ─────────────────────────────────
        "finance_accounts" => r(core.finance_accounts().await),
        "finance_transactions" => r(core.finance_transactions(get(&body, "limit")).await),
        "finance_transactions_filtered" => r(core
            .finance_transactions_filtered(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_budget_usage" => r(core.finance_budget_usage().await),
        "finance_portfolios" => r(core.finance_portfolios().await),
        "finance_investments" => r(core.finance_investments().await),
        "finance_investments_filtered" => r(core
            .finance_investments_filtered(get(&body, "portfolio_id"))
            .await),
        "finance_goals" => r(core.finance_goals().await),
        "finance_liabilities" => r(core.finance_liabilities().await),
        "finance_net_worth" => r(core.finance_net_worth().await),
        "finance_exchange_rates" => r(core.finance_exchange_rates().await),
        // ── Finance — mutations ──────────────────────────────
        "finance_account_create" => rh(core
            .finance_account_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_account_update" => rh(core
            .finance_account_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_account_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.finance_account_delete(id).await)
        }
        "finance_transaction_create" => rh(core
            .finance_transaction_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_transaction_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.finance_transaction_delete(id).await)
        }
        "finance_budget_create" => rh(core
            .finance_budget_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_budget_update" => rh(core
            .finance_budget_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_budget_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.finance_budget_delete(id).await)
        }
        "finance_goal_create" => rh(core
            .finance_goal_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_goal_update" => rh(core
            .finance_goal_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_goal_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.finance_goal_delete(id).await)
        }
        "finance_liability_create" => rh(core
            .finance_liability_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_liability_update" => rh(core
            .finance_liability_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_liability_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.finance_liability_delete(id).await)
        }
        "finance_portfolio_create" => rh(core
            .finance_portfolio_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_investment_create" => rh(core
            .finance_investment_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "finance_investment_update" => rh(core
            .finance_investment_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        // ── Finance — reports ────────────────────────────────
        "finance_report_spending" => r(core
            .finance_report_spending(get(&body, "date_from"), get(&body, "date_to"))
            .await),
        "finance_report_income" => r(core
            .finance_report_income(get(&body, "date_from"), get(&body, "date_to"))
            .await),
        "finance_report_trends" => {
            let metric = match get_str(&body, "metric") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core
                .finance_report_trends(metric, get(&body, "periods"))
                .await)
        }

        // ── Notes ─────────────────────────────────────────────
        "note_list" => r(core.note_list(get(&body, "notebook_id")).await),
        "note_get" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.note_get(id).await)
        }
        "note_create" => rh(core
            .note_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "note_update" => rh(core
            .note_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "note_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.note_delete(id).await)
        }
        "note_search" => {
            let query = match get_str(&body, "query") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.note_search(query).await)
        }
        "note_links_all" => r(core.note_links_all().await),
        "note_list_by_entity" => {
            let entity_type = match get_str(&body, "entity_type") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let entity_id = match get_str(&body, "entity_id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.note_list_by_entity(entity_type, entity_id).await)
        }
        "note_version_list" => {
            let note_id = match get_str(&body, "note_id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.note_version_list(note_id).await)
        }
        "note_version_create" => {
            let note_id = match get_str(&body, "note_id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.note_version_create(note_id).await)
        }
        "note_version_restore" => {
            let version_id = match get_str(&body, "version_id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let note_id = match get_str(&body, "note_id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.note_version_restore(version_id, note_id).await)
        }
        "note_save_attachment" => {
            let data = match get_str(&body, "data") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let filename = match get_str(&body, "filename") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.note_save_attachment(data, filename).await)
        }
        "notebook_list" => r(core.notebook_list().await),
        "notebook_create" => rh(core
            .notebook_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "notebook_update" => rh(core
            .notebook_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "notebook_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            rh(core.notebook_delete(id).await)
        }

        // ── Productivity ──────────────────────────────────────
        "productivity_today" => r(core.productivity_today().await),
        "productivity_timeline" => {
            let date = match get_str(&body, "date") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core
                .productivity_timeline(date, get(&body, "limit"), get(&body, "offset"))
                .await)
        }
        "productivity_focus_start" => r(core
            .productivity_focus_start(
                get(&body, "action_id"),
                get(&body, "project_id"),
                get(&body, "target_mins"),
            )
            .await),
        "productivity_focus_end" => r(core.productivity_focus_end(get(&body, "notes")).await),
        "productivity_focus_status" => r(core.productivity_focus_status().await),
        "productivity_sessions" => {
            let date = match get_str(&body, "date") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.productivity_sessions(date).await)
        }
        "productivity_weekly" => r(core.productivity_weekly().await),
        "productivity_categories" => r(core.productivity_categories().await),
        "productivity_summary_range" => {
            let start_date = match get_str(&body, "startDate") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let end_date = match get_str(&body, "endDate") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.productivity_summary_range(start_date, end_date).await)
        }
        "productivity_activity_feed" => {
            r(core.productivity_activity_feed(get(&body, "limit")).await)
        }
        "productivity_goals" => r(core.productivity_goals().await),
        "productivity_pomodoro_start" => r(core
            .productivity_pomodoro_start(get(&body, "work_mins"), get(&body, "break_mins"))
            .await),
        "productivity_time_entries" => {
            let date = match get_str(&body, "date") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.productivity_time_entries(date).await)
        }
        "productivity_goal_create" => {
            let goal_type = match get_str(&body, "goal_type") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let metric = match get_str(&body, "metric") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let target_value: f64 = match require(&body, "target_value") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core
                .productivity_goal_create(goal_type, metric, target_value)
                .await)
        }
        "productivity_goal_delete" => {
            let id: i64 = match require(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.productivity_goal_delete(id).await)
        }
        "productivity_goal_toggle" => {
            let id: i64 = match require(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let enabled: bool = match require(&body, "enabled") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.productivity_goal_toggle(id, enabled).await)
        }
        "productivity_time_entry_create" => {
            let description = match get_str(&body, "description") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let duration_mins: i64 = match require(&body, "duration_mins") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core
                .productivity_time_entry_create(
                    description,
                    duration_mins,
                    get(&body, "category_id"),
                    get(&body, "project_id"),
                )
                .await)
        }
        "productivity_time_entry_delete" => {
            let id: i64 = match require(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.productivity_time_entry_delete(id).await)
        }
        "productivity_category_upsert" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let name = match get_str(&body, "name") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let category_type = match get_str(&body, "category_type") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core
                .productivity_category_upsert(
                    id,
                    name,
                    category_type,
                    get(&body, "color"),
                    get(&body, "icon"),
                )
                .await)
        }
        "productivity_insights" => r(core.productivity_insights(get(&body, "date")).await),
        "productivity_insight_dismiss" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.productivity_insight_dismiss(id).await)
        }
        "productivity_auto_focus_confirm" => r(core
            .productivity_auto_focus_confirm(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "productivity_projects_list" => r(core.productivity_projects_list().await),
        "productivity_project_upsert" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let display_name = match get_str(&body, "display_name") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let path = match get_str(&body, "path") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core
                .productivity_project_upsert(
                    id,
                    display_name,
                    path,
                    get(&body, "url_patterns"),
                    get(&body, "color"),
                )
                .await)
        }
        "productivity_project_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.productivity_project_delete(id).await)
        }

        // ── Distraction ───────────────────────────────────────
        "distraction_dismiss" => {
            let app_name = match get_str(&body, "app_name") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.distraction_dismiss(app_name).await)
        }
        "distraction_allow_temp" => {
            let pattern = match get_str(&body, "pattern") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.distraction_allow_temp(pattern).await)
        }
        "distraction_allow_session" => {
            let app_name = match get_str(&body, "app_name") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let classification = match get_str(&body, "classification") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core
                .distraction_allow_session(app_name, get(&body, "window_title"), classification)
                .await)
        }
        "distraction_learned_rules" => r(core.distraction_learned_rules().await),
        "distraction_delete_rule" => {
            let id: i64 = match require(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.distraction_delete_rule(id).await)
        }

        // ── Settings (MCP) ────────────────────────────────────
        "mcp_get_config" => r(core.mcp_get_config().await),
        "mcp_add_server" => r(core
            .mcp_add_server(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "mcp_remove_server" => r(core
            .mcp_remove_server(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "mcp_toggle_server" => r(core
            .mcp_toggle_server(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "mcp_update_server" => r(core
            .mcp_update_server(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),

        // ── Settings (generic config) ────────────────────────
        "app_info" => r(core.app_info().await),
        "config_get_section" => {
            let section = match get_str(&body, "section") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.config_get_section(section).await)
        }
        "config_update_section" => {
            let section = match get_str(&body, "section") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let patch = body
                .get("patch")
                .cloned()
                .unwrap_or(serde_json::Value::Object(Default::default()));
            r(core.config_update_section(section, patch).await)
        }
        "config_mark_setup_completed" => r(core.config_mark_setup_completed().await),

        // ── Chat ──────────────────────────────────────────────
        "chat_threads" => r(core.chat_threads().await),
        "chat_messages" => {
            let session_key = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.chat_messages(session_key, get(&body, "limit")).await)
        }
        "chat_pin_thread" => {
            let session_key = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.chat_pin_thread(session_key).await)
        }
        "chat_rename_thread" => {
            let session_key = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let title = match get_str(&body, "title") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.chat_rename_thread(session_key, title).await)
        }
        "chat_delete_thread" => {
            let session_key = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.chat_delete_thread(session_key).await)
        }
        "chat_cancel" => {
            let session_key = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.chat_cancel(session_key).await)
        }
        "chat_send" => {
            let content = match get_str(&body, "content") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let session_key = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let context: Option<desktop_shared::commands::SessionContextInput> =
                get(&body, "context");

            match core.chat_send(content, session_key.clone(), context).await {
                Ok((user_msg, stream_info)) => {
                    let (tx, _) = broadcast::channel(256);
                    state.sse_channels.insert(session_key, tx.clone());
                    let emitter: Arc<dyn ::app_core::events::AppEventEmitter> =
                        Arc::new(SseEmitter { tx });
                    core.spawn_chat_relay(stream_info, emitter);
                    ok(user_msg)
                }
                Err(e) => err(e),
            }
        }
        "chat_respond_interaction" => {
            let session_key = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let request_id = match get_str(&body, "requestId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let response: common::FormResponse = match require(&body, "response") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core
                .chat_respond_interaction(session_key, request_id, response)
                .await)
        }

        // ── Task Groups ──────────────────────────────────────
        "group_list" => r(core.group_list(get(&body, "projectId")).await),
        "group_create" => r(core
            .group_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "group_update" => r(core
            .group_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "group_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.group_delete(id).await)
        }
        "group_reorder" => r(core
            .group_reorder(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),

        // ── Workflows ────────────────────────────────────────
        "workflow_list" => r(core.workflow_list().await),
        "workflow_get" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.workflow_get(id).await)
        }
        "workflow_get_effective" => r(core.workflow_get_effective(get(&body, "projectId")).await),
        "workflow_create" => r(core
            .workflow_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "workflow_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.workflow_delete(id).await)
        }
        "label_create" => r(core
            .label_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "label_update" => r(core
            .label_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "label_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.label_delete(id).await)
        }
        "label_reorder" => r(core
            .label_reorder(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),

        // ── Custom Columns ───────────────────────────────────
        "custom_column_list" => {
            let project_id = match get_str(&body, "projectId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.custom_column_list(project_id).await)
        }
        "custom_column_create" => r(core
            .custom_column_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "custom_column_update" => r(core
            .custom_column_update(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "custom_column_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.custom_column_delete(id).await)
        }
        "custom_column_reorder" => r(core
            .custom_column_reorder(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "custom_column_values" => {
            let task_id = match get_str(&body, "taskId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.custom_column_values(task_id).await)
        }
        "custom_column_value_set" => r(core
            .custom_column_value_set(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "custom_column_value_delete" => {
            let task_id = match get_str(&body, "taskId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let column_id = match get_str(&body, "columnId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.custom_column_value_delete(task_id, column_id).await)
        }

        // ── Cognitive ─────────────────────────────────────────
        "cognitive_user_model" => r(core.cognitive_user_model().await),
        "cognitive_facts_list" => r(core.cognitive_facts_list(get(&body, "domain")).await),
        "cognitive_episodic_list" => r(core
            .cognitive_episodic_list(get(&body, "domain"), get(&body, "limit"))
            .await),
        "cognitive_rules_list" => r(core.cognitive_rules_list(get(&body, "domain")).await),
        "cognitive_memory_stats" => r(core.cognitive_memory_stats().await),
        "cognitive_system_status" => r(core.cognitive_system_status().await),
        "cognitive_fact_create" => r(core
            .cognitive_fact_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "cognitive_fact_update" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core
                .cognitive_fact_update(
                    id,
                    match parse_params(&body) {
                        Ok(p) => p,
                        Err(e) => return err(e),
                    },
                )
                .await)
        }
        "cognitive_fact_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.cognitive_fact_delete(id).await)
        }
        "cognitive_rule_create" => r(core
            .cognitive_rule_create(match parse_params(&body) {
                Ok(p) => p,
                Err(e) => return err(e),
            })
            .await),
        "cognitive_rule_deactivate" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            r(core.cognitive_rule_deactivate(id).await)
        }
        "cognitive_run_compaction" => r(core.cognitive_run_compaction().await),
        "cognitive_inject_event" => {
            let event_type = match get_str(&body, "event_type") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let payload = body.get("payload").cloned().unwrap_or_default();
            r(core.cognitive_inject_event(event_type, payload).await)
        }

        // ── Coaching ──────────────────────────────────────────
        "coaching_situation" => r(core.coaching_situation().await),
        "coaching_signals" => r(core.coaching_signals().await),
        "coaching_patterns" => r(core.coaching_patterns().await),
        "coaching_feedback_stats" => r(core.coaching_feedback_stats().await),
        "coaching_router_status" => r(core.coaching_router_status().await),
        "coaching_reset_dismissals" => r(core
            .coaching_reset_dismissals(get(&body, "trigger_name"))
            .await),
        "coaching_clear_signals" => r(core.coaching_clear_signals().await),

        // ── Unsupported ───────────────────────────────────────
        _ => err(ApiError::new(
            "NOT_FOUND",
            format!("command '{cmd}' is not supported in browser dev mode"),
        )),
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
    let rx = state
        .sse_channels
        .get(&session_key)
        .map(|entry| entry.value().subscribe())
        .unwrap_or_else(|| {
            let (tx, rx) = broadcast::channel(256);
            state.sse_channels.insert(session_key.clone(), tx);
            rx
        });

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
