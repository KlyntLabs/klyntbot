//! Dev-only HTTP server that exposes Tauri commands as REST endpoints.
//!
//! When running `cargo tauri dev`, this server starts on port 3456 alongside
//! the Tauri app, allowing the Vite dev server (localhost:1420) to be opened
//! in Chrome with real API data via `fetch()` instead of Tauri's `invoke()`.
//!
//! Only compiled in debug builds. Chat streaming is not supported (use Tauri for that).

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderValue, Method};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use chrono::Utc;
use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;
use serde_json::Value;
use storage::{ActionFilter, ActionPatch, ProjectFilter};
use tracing::{error, info};

use crate::app_core::AppCore;
use crate::commands::areas::build_area_response;
use crate::commands::productivity::{
    insight_to_response, project_to_response, session_to_response, summary_to_response,
};
use crate::commands::tasks::{
    action_to_task, action_to_today_task, kr_to_response, objective_to_response, row_to_task,
    rows_to_tasks,
};

type AppState = Arc<AppCore>;

/// Start the dev HTTP server on port 3456.
pub async fn start(core: Arc<AppCore>) {
    let app = Router::new()
        .route("/api/{cmd}", post(dispatch))
        .with_state(core);

    // Add CORS headers for the Vite dev server
    let app = app.layer(
        tower_http::cors::CorsLayer::new()
            .allow_origin("http://localhost:1420".parse::<HeaderValue>().unwrap())
            .allow_methods([Method::POST, Method::OPTIONS])
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

/// Generic response type — either success JSON or error JSON with status code.
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

fn storage_err(e: storage::StorageError) -> ApiError {
    crate::commands::map_storage_err(e)
}

fn prod_err(e: common::KlyntbotError) -> ApiError {
    ApiError::new("PRODUCTIVITY_ERROR", e.to_string())
}

/// Extract a typed field from the JSON body.
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

async fn dispatch(
    State(core): State<AppState>,
    Path(cmd): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult {
    match cmd.as_str() {
        // ── Tasks ──────────────────────────────────────────────
        "task_list" => {
            let filter = ActionFilter {
                area_id: get(&body, "area_id"),
                project_id: get(&body, "project_id"),
                status: get(&body, "status"),
                root_only: true,
                ..Default::default()
            };
            match core.repos.actions.list(&filter).await {
                Ok(rows) => match rows_to_tasks(&core.repos, &rows).await {
                    Ok(tasks) => ok(tasks),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "task_create" => {
            let params: TaskCreateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            let id = uuid::Uuid::new_v4().to_string();
            let now = Utc::now();
            let area_id = match (&params.area_id, &params.parent_id) {
                (Some(aid), _) => aid.clone(),
                (None, Some(pid)) => match core.repos.actions.get_or_err(pid).await {
                    Ok(parent) => parent.area_id,
                    Err(e) => return err(storage_err(e)),
                },
                (None, None) => "default".to_string(),
            };
            let row = storage::ActionRow {
                id: id.clone(),
                title: params.title,
                description: None,
                area_id,
                project_id: params.project_id,
                key_result_id: None,
                parent_id: params.parent_id,
                priority: params.priority,
                due_date: params
                    .due_date
                    .and_then(|d| crate::commands::parse_date(&d)),
                tags: params.tags.unwrap_or_default(),
                status: "todo".to_string(),
                focused_at: None,
                focus_deadline: None,
                focus_expired_count: 0,
                created_at: now,
                updated_at: now,
                completed_at: None,
                total_tracked_secs: 0,
                estimated_minutes: None,
                calendar_event_uid: None,
                last_reminded_at: None,
                recurrence_rule: None,
                recurrence_parent_id: None,
                is_template: false,
                next_instance_date: None,
            };
            match core.repos.actions.add(&row).await {
                Ok(created) => ok(action_to_task(&created, 0, 0)),
                Err(e) => err(storage_err(e)),
            }
        }
        "task_update" => {
            let params: TaskUpdateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            let patch = ActionPatch {
                id: params.id.clone(),
                title: params.title,
                description: params.description,
                priority: params.priority,
                status: params.status,
                due_date: params
                    .due_date
                    .map(|opt| opt.and_then(|d| crate::commands::parse_date(&d))),
                tags: params.tags,
                area_id: params.area_id,
                project_id: params.project_id,
                key_result_id: params.key_result_id,
                ..Default::default()
            };
            match core.repos.actions.update(&patch).await {
                Ok(updated) => match row_to_task(&core.repos, &updated).await {
                    Ok(task) => ok(task),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "task_delete" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.actions.delete(&id).await {
                Ok(deleted) => ok(deleted),
                Err(e) => err(storage_err(e)),
            }
        }
        "task_toggle_complete" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.actions.get_or_err(&id).await {
                Ok(row) => {
                    let new_status = if row.status == "done" { "todo" } else { "done" };
                    let patch = ActionPatch {
                        id: id.clone(),
                        status: Some(new_status.to_string()),
                        ..Default::default()
                    };
                    match core.repos.actions.update(&patch).await {
                        Ok(updated) => match row_to_task(&core.repos, &updated).await {
                            Ok(task) => ok(task),
                            Err(e) => err(e),
                        },
                        Err(e) => err(storage_err(e)),
                    }
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "task_list_children" => {
            let parent_id = match get_str(&body, "parentId") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.actions.get_children(&parent_id).await {
                Ok(rows) => match rows_to_tasks(&core.repos, &rows).await {
                    Ok(tasks) => ok(tasks),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "today_tasks" => {
            let now = Utc::now();
            let start_of_today = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let start_of_tomorrow = start_of_today + chrono::Duration::days(1);
            let doing_filter = ActionFilter {
                status: Some("doing".to_string()),
                ..Default::default()
            };
            let due_today_filter = ActionFilter {
                due_after: Some(start_of_today),
                due_before: Some(start_of_tomorrow),
                ..Default::default()
            };
            match tokio::try_join!(
                core.repos.actions.list(&doing_filter),
                core.repos.actions.list(&due_today_filter),
                core.repos.actions.overdue(),
            ) {
                Ok((doing, due_today, overdue)) => {
                    let mut seen = std::collections::HashSet::new();
                    let mut all: Vec<storage::ActionRow> = Vec::new();
                    for row in overdue.into_iter().chain(doing).chain(due_today) {
                        if row.status != "done"
                            && row.status != "archived"
                            && seen.insert(row.id.clone())
                        {
                            all.push(row);
                        }
                    }
                    all.sort_by(|a, b| {
                        let a_od = a.due_date.is_some_and(|d| d < now) as u8;
                        let b_od = b.due_date.is_some_and(|d| d < now) as u8;
                        b_od.cmp(&a_od)
                            .then(a.priority.unwrap_or(99).cmp(&b.priority.unwrap_or(99)))
                            .then(a.due_date.cmp(&b.due_date))
                    });
                    let tasks: Vec<TodayTaskResponse> = all
                        .iter()
                        .map(|row| action_to_today_task(row, now))
                        .collect();
                    ok(tasks)
                }
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Projects ──────────────────────────────────────────
        "project_list" => {
            let filter = ProjectFilter {
                area_id: get(&body, "area_id"),
                status: Some("active".to_string()),
                ..Default::default()
            };
            match core.repos.projects.list(&filter).await {
                Ok(projects) => {
                    let futs = projects
                        .iter()
                        .map(|p| crate::commands::projects::build_project_response(&core, p));
                    match futures_util::future::try_join_all(futs).await {
                        Ok(results) => ok(results),
                        Err(e) => err(e),
                    }
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "project_create" => {
            let params: ProjectCreateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            let id = uuid::Uuid::new_v4().to_string();
            let now = Utc::now();
            let row = storage::ProjectRow {
                id: id.clone(),
                area_id: params.area_id,
                name: params.name,
                description: params.description,
                color: params.color.unwrap_or_else(|| "blue".to_string()),
                tags: params.tags.unwrap_or_default(),
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            };
            match core.repos.projects.create(&row).await {
                Ok(created) => ok(crate::commands::projects::project_to_response(
                    &created,
                    0,
                    0,
                    vec![],
                )),
                Err(e) => err(storage_err(e)),
            }
        }
        "project_get" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.projects.get_or_err(&id).await {
                Ok(row) => {
                    match crate::commands::projects::build_project_response(&core, &row).await {
                        Ok(resp) => ok(resp),
                        Err(e) => err(e),
                    }
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "project_update" => {
            let params: ProjectUpdateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            let patch = storage::ProjectPatch {
                id: params.id.clone(),
                name: params.name,
                area_id: params.area_id,
                color: params.color,
                description: params.description,
                tags: params.tags,
                status: params.status,
            };
            match core.repos.projects.update(&patch).await {
                Ok(updated) => {
                    match crate::commands::projects::build_project_response(&core, &updated).await {
                        Ok(resp) => ok(resp),
                        Err(e) => err(e),
                    }
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "project_delete" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.projects.delete(&id).await {
                Ok(deleted) => ok(deleted),
                Err(e) => err(storage_err(e)),
            }
        }
        "project_archive" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.projects.archive(&id).await {
                Ok(archived) => {
                    match crate::commands::projects::build_project_response(&core, &archived).await
                    {
                        Ok(resp) => ok(resp),
                        Err(e) => err(e),
                    }
                }
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Areas ─────────────────────────────────────────────
        "area_list" => match core.repos.areas.list(Some("active")).await {
            Ok(areas) => {
                let futs = areas.iter().map(|a| build_area_response(&core, a));
                match futures_util::future::try_join_all(futs).await {
                    Ok(results) => ok(results),
                    Err(e) => err(e),
                }
            }
            Err(e) => err(storage_err(e)),
        },
        "area_create" => {
            let params: AreaCreateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            let id = uuid::Uuid::new_v4().to_string();
            let now = Utc::now();
            let row = storage::AreaRow {
                id: id.clone(),
                name: params.name,
                description: None,
                color: params.color.unwrap_or_else(|| "blue".to_string()),
                icon: params.icon,
                position: 0,
                status: "active".to_string(),
                created_at: now,
                updated_at: now,
            };
            match core.repos.areas.create(&row).await {
                Ok(_) => ok(AreaResponse {
                    id: row.id,
                    name: row.name,
                    color: row.color,
                    icon: row.icon,
                    project_count: 0,
                    task_count: 0,
                }),
                Err(e) => err(storage_err(e)),
            }
        }
        "area_update" => {
            let params: AreaUpdateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            match core
                .repos
                .areas
                .update(
                    &params.id,
                    params.name.as_deref(),
                    None,
                    params.color.as_deref(),
                    params.icon.as_ref().map(|o| o.as_deref()),
                    None,
                )
                .await
            {
                Ok(updated) => match build_area_response(&core, &updated).await {
                    Ok(resp) => ok(resp),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "area_delete" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.areas.delete(&id).await {
                Ok(deleted) => ok(deleted),
                Err(e) => err(storage_err(e)),
            }
        }
        "area_reorder" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            let position: i32 = get(&body, "position").unwrap_or(0);
            match core.repos.areas.reorder(&id, position).await {
                Ok(updated) => match build_area_response(&core, &updated).await {
                    Ok(resp) => ok(resp),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Objectives ────────────────────────────────────────
        "objective_list" => {
            let project_id: Option<String> = get(&body, "project_id");
            match core
                .repos
                .objectives
                .list(project_id.as_deref(), None)
                .await
            {
                Ok(objectives) => {
                    let kr_futs = objectives
                        .iter()
                        .map(|o| core.repos.key_results.list(Some(&o.id)));
                    match futures_util::future::try_join_all(kr_futs).await {
                        Ok(all_krs) => {
                            let results: Vec<ObjectiveResponse> = objectives
                                .iter()
                                .zip(all_krs)
                                .map(|(o, krs)| {
                                    let kr_resps = if krs.is_empty() {
                                        None
                                    } else {
                                        Some(krs.iter().map(kr_to_response).collect())
                                    };
                                    objective_to_response(o, kr_resps)
                                })
                                .collect();
                            ok(results)
                        }
                        Err(e) => err(storage_err(e)),
                    }
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "objective_create" => {
            let params: ObjectiveCreateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            let id = uuid::Uuid::new_v4().to_string();
            let now = Utc::now();
            let due_date = params
                .due_date
                .and_then(|d| crate::commands::parse_date(&d));
            let row = storage::ObjectiveRow {
                id: id.clone(),
                project_id: params.project_id,
                title: params.title,
                description: params.description,
                status: "active".to_string(),
                priority: params.priority,
                due_date,
                progress: 0.0,
                created_at: now,
                updated_at: now,
                completed_at: None,
            };
            match core.repos.objectives.create(&row).await {
                Ok(created) => ok(objective_to_response(&created, None)),
                Err(e) => err(storage_err(e)),
            }
        }
        "objective_get" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.objectives.get_or_err(&id).await {
                Ok(row) => match core.repos.key_results.list(Some(&row.id)).await {
                    Ok(krs) => {
                        let kr_resps = if krs.is_empty() {
                            None
                        } else {
                            Some(krs.iter().map(kr_to_response).collect())
                        };
                        ok(objective_to_response(&row, kr_resps))
                    }
                    Err(e) => err(storage_err(e)),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "objective_update" => {
            let params: ObjectiveUpdateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            let due_date = params
                .due_date
                .map(|opt| opt.and_then(|d| crate::commands::parse_date(&d)));
            match core
                .repos
                .objectives
                .update(
                    &params.id,
                    params.title.as_deref(),
                    params.description.as_ref().map(|o| o.as_deref()),
                    params.status.as_deref(),
                    params.priority.as_ref().copied(),
                    due_date,
                )
                .await
            {
                Ok(updated) => match core.repos.key_results.list(Some(&updated.id)).await {
                    Ok(krs) => {
                        let kr_resps = if krs.is_empty() {
                            None
                        } else {
                            Some(krs.iter().map(kr_to_response).collect())
                        };
                        ok(objective_to_response(&updated, kr_resps))
                    }
                    Err(e) => err(storage_err(e)),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "objective_delete" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.objectives.delete(&id).await {
                Ok(deleted) => ok(deleted),
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Key Results ───────────────────────────────────────
        "key_result_create" => {
            let params: KeyResultCreateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            let id = uuid::Uuid::new_v4().to_string();
            let now = Utc::now();
            let row = storage::KeyResultRow {
                id: id.clone(),
                objective_id: params.objective_id.clone(),
                title: params.title,
                description: None,
                status: "active".to_string(),
                tracking_mode: params.tracking_mode.unwrap_or_else(|| "metric".to_string()),
                target_value: params.target_value,
                current_value: 0.0,
                unit: params.unit,
                progress: 0.0,
                due_date: None,
                created_at: now,
                updated_at: now,
                completed_at: None,
            };
            match core.repos.key_results.create(&row).await {
                Ok(created) => {
                    let _ = core
                        .repos
                        .objectives
                        .recalculate_progress(&params.objective_id)
                        .await;
                    ok(kr_to_response(&created))
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "key_result_update" => {
            let params: KeyResultUpdateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            let due_date = params
                .due_date
                .map(|opt| opt.and_then(|d| crate::commands::parse_date(&d)));
            match core
                .repos
                .key_results
                .update(
                    &params.id,
                    params.title.as_deref(),
                    params.description.as_ref().map(|o| o.as_deref()),
                    params.status.as_deref(),
                    due_date,
                )
                .await
            {
                Ok(updated) => {
                    let _ = core
                        .repos
                        .objectives
                        .recalculate_progress(&updated.objective_id)
                        .await;
                    ok(kr_to_response(&updated))
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "key_result_update_metric" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            let current_value: f64 = get(&body, "currentValue").unwrap_or(0.0);
            match core
                .repos
                .key_results
                .update_metric(&id, current_value)
                .await
            {
                Ok(updated) => {
                    let _ = core
                        .repos
                        .objectives
                        .recalculate_progress(&updated.objective_id)
                        .await;
                    ok(kr_to_response(&updated))
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "key_result_delete" => {
            let id = match get_str(&body, "id") {
                Ok(id) => id,
                Err(e) => return err(e),
            };
            match core.repos.key_results.get_or_err(&id).await {
                Ok(kr) => match core.repos.key_results.delete(&id).await {
                    Ok(deleted) => {
                        if deleted {
                            let _ = core
                                .repos
                                .objectives
                                .recalculate_progress(&kr.objective_id)
                                .await;
                        }
                        ok(deleted)
                    }
                    Err(e) => err(storage_err(e)),
                },
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Calendar ──────────────────────────────────────────
        "calendar_events" => {
            let limit: i64 = get(&body, "limit").unwrap_or(10);
            match core.repos.calendar_event_cache.list_upcoming(limit).await {
                Ok(rows) => {
                    let events: Vec<CalendarEventResponse> = rows
                        .iter()
                        .map(|r| CalendarEventResponse {
                            id: r.uid.clone(),
                            title: r.summary.clone(),
                            start_at: r.start_at,
                            end_at: r.end_at,
                            color: "#3B82F6".to_string(),
                        })
                        .collect();
                    ok(events)
                }
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Status ────────────────────────────────────────────
        "agent_status" => {
            match tokio::try_join!(
                core.repos.actions.list_focused(),
                core.repos.actions.summary(),
            ) {
                Ok((focused, summary)) => {
                    let focus_task = if let Some(row) = focused.first() {
                        row_to_task(&core.repos, row).await.ok()
                    } else {
                        None
                    };
                    ok(AgentStatusResponse {
                        status: if focused.is_empty() {
                            "idle".to_string()
                        } else {
                            "active".to_string()
                        },
                        active_task_count: summary.doing,
                        focus_task,
                    })
                }
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Finance ───────────────────────────────────────────
        "finance_accounts" => match core.repos.finance.accounts.list(false).await {
            Ok(rows) => ok(rows),
            Err(e) => err(storage_err(e)),
        },
        "finance_transactions" => {
            let filter = storage::rows::finance::FinanceTransactionFilter {
                limit: get(&body, "limit"),
                ..Default::default()
            };
            match core.repos.finance.transactions.list(&filter).await {
                Ok(rows) => ok(rows),
                Err(e) => err(storage_err(e)),
            }
        }
        "finance_budget_usage" => match core.repos.finance.budgets.all_budget_usage().await {
            Ok(rows) => ok(rows),
            Err(e) => err(storage_err(e)),
        },
        "finance_portfolios" => match core.repos.finance.investments.list_portfolios().await {
            Ok(portfolios) => {
                let futs = portfolios
                    .iter()
                    .map(|p| core.repos.finance.investments.portfolio_summary(&p.id));
                match futures_util::future::try_join_all(futs).await {
                    Ok(summaries) => {
                        let results: Vec<FinancePortfolioResponse> = portfolios
                            .iter()
                            .zip(summaries)
                            .map(|(p, s)| FinancePortfolioResponse {
                                id: p.id.clone(),
                                name: p.name.clone(),
                                description: p.description.clone(),
                                currency: p.currency.clone(),
                                total_value: s.total_current_value,
                                total_cost_basis: s.total_cost_basis,
                                holding_count: s.holding_count,
                            })
                            .collect();
                        ok(results)
                    }
                    Err(e) => err(storage_err(e)),
                }
            }
            Err(e) => err(storage_err(e)),
        },
        "finance_investments" => {
            match core
                .repos
                .finance
                .investments
                .list_investments(&Default::default())
                .await
            {
                Ok(rows) => ok(rows),
                Err(e) => err(storage_err(e)),
            }
        }
        "finance_goals" => match core.repos.finance.goals.list_active().await {
            Ok(rows) => ok(rows),
            Err(e) => err(storage_err(e)),
        },
        "finance_liabilities" => match core.repos.finance.liabilities.list_all().await {
            Ok(rows) => ok(rows),
            Err(e) => err(storage_err(e)),
        },
        "finance_net_worth" => {
            match tokio::try_join!(
                core.repos.finance.accounts.total_balance_by_currency(),
                core.repos.finance.investments.total_value_by_currency(),
                core.repos.finance.liabilities.total_remaining_by_currency(),
            ) {
                Ok((accts, invests, liabs)) => {
                    let mut by_currency: HashMap<&str, CurrencyNetWorth> = HashMap::new();
                    for (c, t) in &accts {
                        by_currency
                            .entry(c)
                            .or_insert_with(|| CurrencyNetWorth::zero(c.clone()))
                            .accounts = *t;
                    }
                    for (c, t) in &invests {
                        by_currency
                            .entry(c)
                            .or_insert_with(|| CurrencyNetWorth::zero(c.clone()))
                            .investments = *t;
                    }
                    for (c, t) in &liabs {
                        by_currency
                            .entry(c)
                            .or_insert_with(|| CurrencyNetWorth::zero(c.clone()))
                            .liabilities = *t;
                    }
                    let totals: Vec<CurrencyNetWorth> = by_currency
                        .into_values()
                        .map(|mut c| {
                            c.net = c.accounts + c.investments - c.liabilities;
                            c
                        })
                        .collect();
                    ok(FinanceNetWorthResponse {
                        totals_by_currency: totals,
                    })
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "finance_exchange_rates" => ok(HashMap::<String, f64>::new()),

        // ── Productivity ──────────────────────────────────────
        "productivity_today" => match core.aggregator() {
            Ok(agg) => match agg.compute_today().await {
                Ok(s) => ok(Some(summary_to_response(s))),
                Err(e) => err(prod_err(e)),
            },
            Err(e) => err(e),
        },
        "productivity_timeline" => {
            let date = match get_str(&body, "date") {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let start = match crate::commands::parse_date_or_err(&date) {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let end = start + chrono::Duration::days(1);
            let cap: i64 = get(&body, "limit").unwrap_or(500);
            let offset: Option<i64> = get(&body, "offset");
            match repos
                .events
                .list_range_offset(&start, &end, Some(cap.min(10_000)), offset)
                .await
            {
                Ok(events) => {
                    let resp: Vec<ActivityTimelineResponse> = events
                        .into_iter()
                        .map(|e| ActivityTimelineResponse {
                            app_name: e.app_name,
                            window_title: e.window_title,
                            site_name: e.site_name,
                            category_id: e.category_id,
                            started_at: e.started_at,
                            duration_secs: e.duration_secs,
                            is_idle: e.is_idle,
                        })
                        .collect();
                    ok(resp)
                }
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_focus_start" => match core.focus_manager() {
            Ok(mgr) => {
                let action_id: Option<String> = get(&body, "action_id");
                let project_id: Option<String> = get(&body, "project_id");
                let target_mins: Option<i64> = get(&body, "target_mins");
                match mgr.start_session(action_id, project_id, target_mins).await {
                    Ok(s) => ok(session_to_response(s)),
                    Err(e) => err(prod_err(e)),
                }
            }
            Err(e) => err(e),
        },
        "productivity_focus_end" => match core.focus_manager() {
            Ok(mgr) => {
                let notes: Option<String> = get(&body, "notes");
                match mgr.end_session(notes).await {
                    Ok(s) => ok(s.map(session_to_response)),
                    Err(e) => err(prod_err(e)),
                }
            }
            Err(e) => err(e),
        },
        "productivity_focus_status" => match core.focus_manager() {
            Ok(mgr) => match mgr.get_active().await {
                Ok(s) => ok(s.map(session_to_response)),
                Err(e) => err(prod_err(e)),
            },
            Err(e) => err(e),
        },
        "productivity_sessions" => {
            let date = match get_str(&body, "date") {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let start = match crate::commands::parse_date_or_err(&date) {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let end = start + chrono::Duration::days(1);
            match repos.sessions.list_range(&start, &end, None).await {
                Ok(sessions) => {
                    let resp: Vec<FocusSessionResponse> =
                        sessions.into_iter().map(session_to_response).collect();
                    ok(resp)
                }
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_weekly" => {
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let today = Utc::now().date_naive();
            let week_start = today - chrono::Duration::days(6);
            match repos
                .summaries
                .list_range(
                    &week_start.format("%Y-%m-%d").to_string(),
                    &today.format("%Y-%m-%d").to_string(),
                )
                .await
            {
                Ok(summaries) => {
                    let resp: Vec<ProductivitySummaryResponse> = summaries
                        .into_iter()
                        .map(summary_to_response)
                        .collect();
                    ok(resp)
                }
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_categories" => {
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.categories.list_all().await {
                Ok(categories) => {
                    let resp: Vec<ActivityCategoryResponse> = categories
                        .into_iter()
                        .map(|c| ActivityCategoryResponse {
                            id: c.id,
                            name: c.name,
                            category_type: c.category_type.to_string(),
                            color: c.color,
                            icon: c.icon,
                            is_system: c.is_system,
                        })
                        .collect();
                    ok(resp)
                }
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_summary_range" => {
            let start_date = match get_str(&body, "startDate") {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let end_date = match get_str(&body, "endDate") {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.summaries.list_range(&start_date, &end_date).await {
                Ok(mut summaries) => {
                    let today = Utc::now().format("%Y-%m-%d").to_string();
                    if today >= start_date && today <= end_date {
                        if let Ok(agg) = core.aggregator() {
                            if let Ok(live) = agg.compute_today().await {
                                summaries.retain(|s| s.date != today);
                                summaries.push(live);
                                summaries.sort_by(|a, b| a.date.cmp(&b.date));
                            }
                        }
                    }
                    let resp: Vec<ProductivitySummaryResponse> = summaries
                        .into_iter()
                        .map(summary_to_response)
                        .collect();
                    ok(resp)
                }
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_activity_feed" => {
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let cap: i64 = get(&body, "limit").unwrap_or(50);
            match repos.events.list_recent(cap.min(200)).await {
                Ok(events) => {
                    let resp: Vec<ActivityTimelineResponse> = events
                        .into_iter()
                        .map(|e| ActivityTimelineResponse {
                            app_name: e.app_name,
                            window_title: e.window_title,
                            site_name: e.site_name,
                            category_id: e.category_id,
                            started_at: e.started_at,
                            duration_secs: e.duration_secs,
                            is_idle: e.is_idle,
                        })
                        .collect();
                    ok(resp)
                }
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_goals" => match core.aggregator() {
            Ok(agg) => {
                let today = Utc::now().format("%Y-%m-%d").to_string();
                match agg.check_goals(&today).await {
                    Ok(results) => {
                        let resp: Vec<GoalProgressResponse> = results
                            .into_iter()
                            .map(|(goal, current, met)| GoalProgressResponse {
                                id: goal.id.unwrap_or(0),
                                goal_type: goal.goal_type.to_string(),
                                metric: goal.metric.to_string(),
                                target_value: goal.target_value,
                                current_value: current,
                                met,
                            })
                            .collect();
                        ok(resp)
                    }
                    Err(e) => err(prod_err(e)),
                }
            }
            Err(e) => err(e),
        },
        "productivity_pomodoro_start" => match core.focus_manager() {
            Ok(mgr) => {
                let work_mins: Option<i64> = get(&body, "work_mins");
                let break_mins: Option<i64> = get(&body, "break_mins");
                match mgr.start_pomodoro(None, None, work_mins, break_mins).await {
                    Ok(s) => ok(session_to_response(s)),
                    Err(e) => err(prod_err(e)),
                }
            }
            Err(e) => err(e),
        },
        "productivity_time_entries" => {
            let date = match get_str(&body, "date") {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let start = match crate::commands::parse_date_or_err(&date) {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let end = start + chrono::Duration::days(1);
            match repos.time_entries.list_range(&start, &end).await {
                Ok(entries) => {
                    let resp: Vec<TimeEntryResponse> = entries
                        .into_iter()
                        .map(|e| TimeEntryResponse {
                            id: e.id.unwrap_or(0),
                            description: e.description,
                            category_id: e.category_id,
                            project_id: e.project_id,
                            started_at: e.started_at,
                            duration_secs: e.duration_secs,
                            source: e.source,
                        })
                        .collect();
                    ok(resp)
                }
                Err(e) => err(prod_err(e)),
            }
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
            let target_value: f64 = match get(&body, "target_value") {
                Some(v) => v,
                None => {
                    return err(ApiError::new(
                        "VALIDATION",
                        "missing required field: target_value",
                    ))
                }
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let gt: feature_productivity::types::GoalType = match goal_type.parse() {
                Ok(v) => v,
                Err(_) => {
                    return err(ApiError::new(
                        "VALIDATION",
                        "Invalid goal_type. Use: daily, weekly",
                    ))
                }
            };
            let gm: feature_productivity::types::GoalMetric = match metric.parse() {
                Ok(v) => v,
                Err(_) => return err(ApiError::new("VALIDATION", "Invalid metric. Use: productive_hours, focus_sessions, productivity_score, max_distracting_mins")),
            };
            let goal = feature_productivity::types::ProductivityGoal {
                id: None,
                goal_type: gt,
                metric: gm,
                target_value,
                enabled: true,
                project_id: None,
                created_at: Utc::now(),
            };
            match repos.goals.insert(&goal).await {
                Ok(id) => ok(GoalProgressResponse {
                    id,
                    goal_type: goal.goal_type.to_string(),
                    metric: goal.metric.to_string(),
                    target_value: goal.target_value,
                    current_value: 0.0,
                    met: false,
                }),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_goal_delete" => {
            let id: i64 = match get(&body, "id") {
                Some(v) => v,
                None => return err(ApiError::new("VALIDATION", "missing required field: id")),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.goals.delete(id).await {
                Ok(_) => ok(()),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_goal_toggle" => {
            let id: i64 = match get(&body, "id") {
                Some(v) => v,
                None => return err(ApiError::new("VALIDATION", "missing required field: id")),
            };
            let enabled: bool = match get(&body, "enabled") {
                Some(v) => v,
                None => {
                    return err(ApiError::new(
                        "VALIDATION",
                        "missing required field: enabled",
                    ))
                }
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.goals.set_enabled(id, enabled).await {
                Ok(_) => ok(()),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_time_entry_create" => {
            let description = match get_str(&body, "description") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let duration_mins: i64 = match get(&body, "duration_mins") {
                Some(v) => v,
                None => {
                    return err(ApiError::new(
                        "VALIDATION",
                        "missing required field: duration_mins",
                    ))
                }
            };
            let category_id: Option<String> = get(&body, "category_id");
            let project_id: Option<String> = get(&body, "project_id");
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let now = Utc::now();
            let started_at = now - chrono::Duration::minutes(duration_mins);
            let entry = feature_productivity::types::TimeEntry {
                id: None,
                description: description.clone(),
                category_id: category_id.clone(),
                project_id: project_id.clone(),
                started_at,
                duration_secs: duration_mins * 60,
                source: "manual".to_string(),
                created_at: now,
            };
            match repos.time_entries.insert(&entry).await {
                Ok(id) => ok(TimeEntryResponse {
                    id,
                    description,
                    category_id,
                    project_id,
                    started_at,
                    duration_secs: duration_mins * 60,
                    source: "manual".to_string(),
                }),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_time_entry_delete" => {
            let id: i64 = match get(&body, "id") {
                Some(v) => v,
                None => return err(ApiError::new("VALIDATION", "missing required field: id")),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.time_entries.delete(id).await {
                Ok(_) => ok(()),
                Err(e) => err(prod_err(e)),
            }
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
            let category_type_str = match get_str(&body, "category_type") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let color: Option<String> = get(&body, "color");
            let icon: Option<String> = get(&body, "icon");
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let ct: feature_productivity::types::CategoryType = match category_type_str.parse() {
                Ok(v) => v,
                Err(_) => {
                    return err(ApiError::new(
                        "VALIDATION",
                        "Invalid category_type. Use: productive, neutral, distracting",
                    ))
                }
            };
            let cat = feature_productivity::types::ActivityCategory {
                id: id.clone(),
                name: name.clone(),
                category_type: ct,
                color: color.clone(),
                icon: icon.clone(),
                rules: None,
                is_system: false,
            };
            match repos.categories.upsert(&cat).await {
                Ok(_) => ok(ActivityCategoryResponse {
                    id,
                    name,
                    category_type: cat.category_type.to_string(),
                    color,
                    icon,
                    is_system: false,
                }),
                Err(e) => err(prod_err(e)),
            }
        }

        // ── Insights & Auto-Focus ─────────────────────────────
        "productivity_insights" => {
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let date: String =
                get(&body, "date").unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
            let engine = feature_productivity::insights::InsightEngine::new(repos.clone());
            match engine.generate_for_date(&date).await {
                Ok(cards) => {
                    let resp: Vec<_> = cards.into_iter().map(insight_to_response).collect();
                    ok(resp)
                }
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_insight_dismiss" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.insights.dismiss(&id).await {
                Ok(_) => ok(()),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_auto_focus_confirm" => {
            // In dev mode, just acknowledge — no auto-focus detection running
            ok(serde_json::json!(null))
        }
        "productivity_projects_list" => {
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.projects.list_all().await {
                Ok(projects) => {
                    let resp: Vec<ProductivityProjectResponse> =
                        projects.into_iter().map(project_to_response).collect();
                    ok(resp)
                }
                Err(e) => err(prod_err(e)),
            }
        }
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
            let url_patterns: Vec<String> = body
                .get("url_patterns")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let color = body.get("color").and_then(|v| v.as_str()).map(String::from);
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let project = feature_productivity::types::ProductivityProject {
                id,
                display_name,
                path,
                url_patterns,
                color,
                is_auto_detected: false,
                created_at: Utc::now(),
            };
            match repos.projects.upsert(&project).await {
                Ok(_) => ok(project_to_response(project)),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_project_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.projects.delete(&id).await {
                Ok(_) => ok(()),
                Err(e) => err(prod_err(e)),
            }
        }
        // ── Settings (MCP) ────────────────────────────────────
        "mcp_get_config" => {
            let cfg = core.config.read().await;
            ok(crate::commands::build_mcp_response(&cfg))
        }

        // ── Chat (read-only in browser) ───────────────────────
        "chat_threads" => {
            let default_filter = ProjectFilter::default();
            let (sessions, visible_contexts, all_areas, all_projects) = tokio::join!(
                core.repos.sessions.list_sessions(),
                core.repos.session_context.list_visible(),
                core.repos.areas.list(None),
                core.repos.projects.list(&default_filter),
            );
            match (sessions, visible_contexts, all_areas, all_projects) {
                (Ok(sessions), Ok(contexts), Ok(areas), Ok(projects)) => {
                    let ctx_map: HashMap<&str, _> = contexts
                        .iter()
                        .map(|c| (c.session_key.as_str(), c))
                        .collect();
                    let area_names: HashMap<&str, &str> = areas
                        .iter()
                        .map(|a| (a.id.as_str(), a.name.as_str()))
                        .collect();
                    let project_names: HashMap<&str, &str> = projects
                        .iter()
                        .map(|p| (p.id.as_str(), p.name.as_str()))
                        .collect();
                    let threads: Vec<ChatThreadResponse> = sessions
                        .iter()
                        .map(|s| {
                            let title = s
                                .metadata
                                .get("title")
                                .and_then(|v| v.as_str())
                                .unwrap_or(&s.key)
                                .to_string();
                            let ctx = ctx_map.get(s.key.as_str());
                            ChatThreadResponse {
                                session_key: s.key.clone(),
                                title,
                                message_count: s.message_count,
                                updated_at: s.updated_at,
                                context_type: ctx.map(|c| c.context_type.clone()),
                                entity_kind: ctx.and_then(|c| c.entity_kind.clone()),
                                entity_id: ctx.and_then(|c| c.entity_id.clone()),
                                area_id: ctx.and_then(|c| c.area_id.clone()),
                                area_name: ctx.and_then(|c| {
                                    c.area_id
                                        .as_deref()
                                        .and_then(|id| area_names.get(id).map(|s| s.to_string()))
                                }),
                                project_id: ctx.and_then(|c| c.project_id.clone()).or_else(|| {
                                    s.metadata
                                        .get("projectId")
                                        .and_then(|v| v.as_str())
                                        .map(String::from)
                                }),
                                project_name: ctx.and_then(|c| {
                                    c.project_id
                                        .as_deref()
                                        .and_then(|id| project_names.get(id).map(|s| s.to_string()))
                                }),
                            }
                        })
                        .collect();
                    ok(threads)
                }
                _ => err(ApiError::new("STORAGE_ERROR", "failed to load chat data")),
            }
        }
        "chat_messages" => {
            let session_key = match get_str(&body, "sessionKey") {
                Ok(k) => k,
                Err(e) => return err(e),
            };
            let limit: Option<i64> = get(&body, "limit");
            let rows = if let Some(lim) = limit {
                core.repos
                    .sessions
                    .get_recent_messages(&session_key, lim)
                    .await
            } else {
                core.repos.sessions.get_messages(&session_key).await
            };
            match rows {
                Ok(msgs) => {
                    let resp: Vec<ChatMessageResponse> = msgs
                        .iter()
                        .filter(|m| {
                            m.role == "user" || m.role == "assistant" || m.role == "interaction"
                        })
                        .map(|m| {
                            let segments = m
                                .metadata
                                .as_ref()
                                .and_then(|meta| meta.get("segments"))
                                .and_then(|v| serde_json::from_value(v.clone()).ok());
                            let transparency = m
                                .metadata
                                .as_ref()
                                .and_then(|meta| meta.get("transparency"))
                                .and_then(|v| serde_json::from_value(v.clone()).ok());
                            ChatMessageResponse {
                                id: m.id.to_string(),
                                role: m.role.clone(),
                                content: m.content.clone(),
                                timestamp: m.timestamp,
                                segments,
                                transparency,
                            }
                        })
                        .collect();
                    ok(resp)
                }
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Unsupported commands ──────────────────────────────
        _ => err(ApiError::new(
            "NOT_FOUND",
            format!("command '{cmd}' is not supported in browser dev mode"),
        )),
    }
}

// ── Productivity response helpers ─────────────────────────────────────────

// summary_to_response, session_to_response, project_to_response
// imported from crate::commands::productivity
