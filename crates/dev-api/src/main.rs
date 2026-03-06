//! Standalone dev API server — shares the same SQLite DB as the desktop app.
//!
//! Usage: cargo run -p dev-api
//!
//! Starts an HTTP server on port 3456 that serves the same data as Tauri commands.
//! Run `bun run dev` in desktop-ui/ separately, then open localhost:1420 in Chrome.

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
use feature_productivity::distraction::DistractionInterceptor;
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::{DailyAggregator, FocusManager};
use serde_json::Value;
use storage::{ActionFilter, ActionPatch, ProjectFilter, Repos, StoragePool};
use tokio::sync::{Mutex, RwLock};
use tracing::info;

/// Lightweight app state — just storage, config, and productivity.
struct DevState {
    repos: Repos,
    config: RwLock<config::Config>,
    productivity_repos: Option<ProductivityRepos>,
    focus_manager: Option<Arc<FocusManager>>,
    aggregator: Option<Arc<DailyAggregator>>,
    distraction_interceptor: Option<Arc<Mutex<DistractionInterceptor>>>,
}

impl DevState {
    fn productivity_repos(&self) -> Result<&ProductivityRepos, ApiError> {
        self.productivity_repos
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    fn focus_manager(&self) -> Result<&Arc<FocusManager>, ApiError> {
        self.focus_manager
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    fn aggregator(&self) -> Result<&Arc<DailyAggregator>, ApiError> {
        self.aggregator
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    fn distraction_interceptor(&self) -> Result<&Arc<Mutex<DistractionInterceptor>>, ApiError> {
        self.distraction_interceptor
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }
}

type AppState = Arc<DevState>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // 1. Load config
    let config = config::load_with_env_overrides()
        .await
        .expect("failed to load config");
    info!(path = ?config::config_path(), "configuration loaded");

    // 2. Connect storage (read/write to the same DB as the desktop app)
    let data_dir = config.data_dir_path();
    let pool = StoragePool::connect(&data_dir)
        .await
        .expect("failed to connect storage");
    let repos = Repos::from_pool(&pool);
    info!("storage connected");

    // 3. Initialize productivity (optional)
    let (prod_repos, focus_mgr, aggregator, interceptor) = if config.productivity.enabled {
        let inner = pool.inner().clone();
        match StoragePool::run_feature_migrations(
            &inner,
            &feature_productivity::ProductivityFeature::migrations_static(),
        )
        .await
        {
            Ok(()) => {
                let pr = ProductivityRepos::new(inner);
                let fm = Arc::new(FocusManager::new(
                    pr.clone(),
                    config.productivity.focus.clone(),
                ));
                let agg = Arc::new(DailyAggregator::new(pr.clone()));
                let interceptor = Arc::new(Mutex::new(DistractionInterceptor::new(
                    config.productivity.focus.clone(),
                    pr.learned_rules.clone(),
                )));
                (Some(pr), Some(fm), Some(agg), Some(interceptor))
            }
            Err(e) => {
                tracing::warn!("productivity migrations failed: {e}");
                (None, None, None, None)
            }
        }
    } else {
        (None, None, None, None)
    };

    let state = Arc::new(DevState {
        repos,
        config: RwLock::new(config),
        productivity_repos: prod_repos,
        focus_manager: focus_mgr,
        aggregator,
        distraction_interceptor: interceptor,
    });

    // 4. Build axum router
    let app = Router::new()
        .route("/api/{cmd}", post(dispatch))
        .with_state(state);

    let app = app.layer(
        tower_http::cors::CorsLayer::new()
            .allow_origin("http://localhost:1420".parse::<HeaderValue>().unwrap())
            .allow_methods([Method::POST, Method::OPTIONS])
            .allow_headers(tower_http::cors::Any),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3456")
        .await
        .expect("failed to bind port 3456");
    info!("dev API server listening on http://127.0.0.1:3456");
    info!("open http://localhost:1420 in Chrome to use the UI with real data");
    axum::serve(listener, app).await.unwrap();
}

// ── Dispatch ──────────────────────────────────────────────────────────────

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
    match e {
        storage::StorageError::NotFound(msg) => ApiError::new("NOT_FOUND", msg),
        storage::StorageError::Conflict(msg) => ApiError::new("CONFLICT", msg),
        other => ApiError::new("STORAGE_ERROR", other.to_string()),
    }
}

fn prod_err(e: common::KlyntbotError) -> ApiError {
    ApiError::new("PRODUCTIVITY_ERROR", e.to_string())
}

fn get<T: serde::de::DeserializeOwned>(body: &Value, key: &str) -> Option<T> {
    body.get(key)
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn get_str(body: &Value, key: &str) -> Result<String, ApiError> {
    body.get(key)
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| ApiError::new("VALIDATION", format!("missing required field: {key}")))
}

fn parse_date(s: &str) -> Option<chrono::DateTime<Utc>> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
}

fn parse_date_or_err(s: &str) -> Result<chrono::DateTime<Utc>, ApiError> {
    parse_date(s).ok_or_else(|| ApiError::new("VALIDATION", format!("invalid date: {s}")))
}

// ── Row converters ────────────────────────────────────────────────────────

fn priority_label(p: Option<i16>) -> Option<String> {
    p.map(|v| format!("P{v}"))
}

fn action_to_task(
    row: &storage::ActionRow,
    subtask_count: u32,
    subtask_completed_count: u32,
) -> TaskResponse {
    TaskResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        completed: row.status == "done",
        priority: priority_label(row.priority),
        status: row.status.clone(),
        due_date: row.due_date.map(|d| d.format("%Y-%m-%d").to_string()),
        tags: row.tags.clone(),
        project_id: row.project_id.clone(),
        area_id: row.area_id.clone(),
        objective_id: row.key_result_id.clone(),
        description: row.description.clone(),
        parent_id: row.parent_id.clone(),
        subtask_count,
        subtask_completed_count,
    }
}

async fn row_to_task(repos: &Repos, row: &storage::ActionRow) -> Result<TaskResponse, ApiError> {
    let (total, completed) = repos
        .actions
        .count_children(&row.id)
        .await
        .map_err(storage_err)?;
    Ok(action_to_task(row, total as u32, completed as u32))
}

async fn rows_to_tasks(
    repos: &Repos,
    rows: &[storage::ActionRow],
) -> Result<Vec<TaskResponse>, ApiError> {
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let counts = repos
        .actions
        .count_children_bulk(&ids)
        .await
        .map_err(storage_err)?;
    Ok(rows
        .iter()
        .map(|row| {
            let (total, completed) = counts.get(&row.id).copied().unwrap_or((0, 0));
            action_to_task(row, total as u32, completed as u32)
        })
        .collect())
}

fn kr_to_response(row: &storage::KeyResultRow) -> KeyResultResponse {
    KeyResultResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        progress: row.progress,
        current: row.current_value,
        target: row.target_value.unwrap_or(0.0),
        unit: row.unit.clone().unwrap_or_default(),
    }
}

fn objective_to_response(
    row: &storage::ObjectiveRow,
    key_results: Option<Vec<KeyResultResponse>>,
) -> ObjectiveResponse {
    ObjectiveResponse {
        id: row.id.clone(),
        title: row.title.clone(),
        status: row.status.clone(),
        progress: row.progress,
        project_id: row.project_id.clone(),
        key_results,
    }
}

async fn build_project_response(
    repos: &Repos,
    row: &storage::ProjectRow,
) -> Result<ProjectResponse, ApiError> {
    let (counts, objectives) = tokio::try_join!(
        repos.projects.count_tasks_by_status(&row.id),
        repos.objectives.list(Some(&row.id), None),
    )
    .map_err(storage_err)?;

    let mut task_count: u32 = 0;
    let mut completed_count: u32 = 0;
    for (status, count) in &counts {
        task_count += *count as u32;
        if status == "done" {
            completed_count = *count as u32;
        }
    }
    let objective_ids: Vec<String> = objectives.iter().map(|o| o.id.clone()).collect();

    Ok(ProjectResponse {
        id: row.id.clone(),
        name: row.name.clone(),
        color: row.color.clone(),
        area_id: row.area_id.clone(),
        task_count,
        completed_count,
        objective_ids: if objective_ids.is_empty() {
            None
        } else {
            Some(objective_ids)
        },
    })
}

fn summary_to_response(
    s: feature_productivity::types::DailySummary,
) -> ProductivitySummaryResponse {
    ProductivitySummaryResponse {
        date: s.date,
        total_active_secs: s.total_active_secs,
        total_focus_secs: s.total_focus_secs,
        total_break_secs: s.total_break_secs,
        total_idle_secs: s.total_idle_secs,
        productive_secs: s.productive_secs,
        neutral_secs: s.neutral_secs,
        distracting_secs: s.distracting_secs,
        focus_sessions_count: s.focus_sessions_count,
        avg_session_quality: s.avg_session_quality,
        interruptions_count: s.interruptions_count,
        context_switches: s.context_switches,
        top_apps: s
            .top_apps
            .into_iter()
            .map(|a| AppUsageResponse {
                app_name: a.app_name,
                duration_secs: a.duration_secs,
                category: a.category,
            })
            .collect(),
        top_categories: s
            .top_categories
            .into_iter()
            .map(|c| CategoryUsageResponse {
                category: c.category,
                duration_secs: c.duration_secs,
            })
            .collect(),
        top_projects: s
            .top_projects
            .into_iter()
            .map(|p| ProjectUsageResponse {
                project_id: p.project_id,
                display_name: p.display_name,
                duration_secs: p.duration_secs,
                color: p.color,
            })
            .collect(),
        ai_summary: s.ai_summary,
        productivity_score: s.productivity_score,
    }
}

fn session_to_response(s: feature_productivity::types::FocusSession) -> FocusSessionResponse {
    FocusSessionResponse {
        id: s.id,
        action_id: s.action_id,
        project_id: s.project_id,
        session_type: s.session_type.to_string(),
        target_mins: s.target_mins,
        started_at: s.started_at,
        ended_at: s.ended_at,
        actual_mins: s.actual_mins,
        interruptions: s.interruptions,
        quality_score: s.quality_score,
        completed: s.completed,
        notes: s.notes,
    }
}

// ── Main dispatch ─────────────────────────────────────────────────────────

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
                due_date: params.due_date.and_then(|d| parse_date(&d)),
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
                due_date: params.due_date.map(|opt| opt.and_then(|d| parse_date(&d))),
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
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.repos.actions.delete(&id).await {
                Ok(d) => ok(d),
                Err(e) => err(storage_err(e)),
            }
        }
        "task_toggle_complete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
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
                            Ok(t) => ok(t),
                            Err(e) => err(e),
                        },
                        Err(e) => err(storage_err(e)),
                    }
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "task_list_children" => {
            let pid = match get_str(&body, "parentId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.repos.actions.get_children(&pid).await {
                Ok(rows) => match rows_to_tasks(&core.repos, &rows).await {
                    Ok(t) => ok(t),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "today_tasks" => {
            let now = Utc::now();
            let sot = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
            let sot2 = sot + chrono::Duration::days(1);
            let doing_filter = ActionFilter {
                status: Some("doing".to_string()),
                ..Default::default()
            };
            let due_filter = ActionFilter {
                due_after: Some(sot),
                due_before: Some(sot2),
                ..Default::default()
            };
            match tokio::try_join!(
                core.repos.actions.list(&doing_filter),
                core.repos.actions.list(&due_filter),
                core.repos.actions.overdue(),
            ) {
                Ok((doing, due, overdue)) => {
                    let mut seen = std::collections::HashSet::new();
                    let mut all: Vec<storage::ActionRow> = Vec::new();
                    for row in overdue.into_iter().chain(doing).chain(due) {
                        if row.status != "done"
                            && row.status != "archived"
                            && seen.insert(row.id.clone())
                        {
                            all.push(row);
                        }
                    }
                    all.sort_by(|a, b| {
                        let ao = a.due_date.is_some_and(|d| d < now) as u8;
                        let bo = b.due_date.is_some_and(|d| d < now) as u8;
                        bo.cmp(&ao)
                            .then(a.priority.unwrap_or(99).cmp(&b.priority.unwrap_or(99)))
                            .then(a.due_date.cmp(&b.due_date))
                    });
                    let tasks: Vec<TodayTaskResponse> = all
                        .iter()
                        .map(|r| {
                            let is_overdue =
                                r.due_date.is_some_and(|d| d < now) && r.status != "done";
                            let is_due_today = !is_overdue
                                && r.due_date
                                    .is_some_and(|d| d.date_naive() == now.date_naive());
                            TodayTaskResponse {
                                id: r.id.clone(),
                                title: r.title.clone(),
                                priority: priority_label(r.priority),
                                status: r.status.clone(),
                                completed: r.status == "done",
                                is_overdue,
                                is_due_today,
                                due_display: r.due_date.map(|d| d.format("%b %-d").to_string()),
                            }
                        })
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
                        .map(|p| build_project_response(&core.repos, p));
                    match futures_util::future::try_join_all(futs).await {
                        Ok(r) => ok(r),
                        Err(e) => err(e),
                    }
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "project_get" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.repos.projects.get_or_err(&id).await {
                Ok(row) => match build_project_response(&core.repos, &row).await {
                    Ok(r) => ok(r),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "project_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.repos.projects.delete(&id).await {
                Ok(d) => ok(d),
                Err(e) => err(storage_err(e)),
            }
        }
        "project_archive" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.repos.projects.archive(&id).await {
                Ok(row) => match build_project_response(&core.repos, &row).await {
                    Ok(r) => ok(r),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Areas ─────────────────────────────────────────────
        "area_list" => match core.repos.areas.list(Some("active")).await {
            Ok(areas) => {
                let futs = areas.iter().map(|a| {
                    let repos = &core.repos;
                    async move {
                        let (pc, tc) = tokio::try_join!(
                            repos.areas.count_projects(&a.id),
                            repos.areas.count_actions(&a.id)
                        )?;
                        Ok::<_, storage::StorageError>(AreaResponse {
                            id: a.id.clone(),
                            name: a.name.clone(),
                            color: a.color.clone(),
                            icon: a.icon.clone(),
                            project_count: pc,
                            task_count: tc,
                        })
                    }
                });
                match futures_util::future::try_join_all(futs).await {
                    Ok(results) => ok(results),
                    Err(e) => err(storage_err(e)),
                }
            }
            Err(e) => err(storage_err(e)),
        },

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
                    let futs = objectives
                        .iter()
                        .map(|o| core.repos.key_results.list(Some(&o.id)));
                    match futures_util::future::try_join_all(futs).await {
                        Ok(all_krs) => {
                            let r: Vec<_> = objectives
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
                            ok(r)
                        }
                        Err(e) => err(storage_err(e)),
                    }
                }
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Status ────────────────────────────────────────────
        "agent_status" => {
            match tokio::try_join!(
                core.repos.actions.list_focused(),
                core.repos.actions.summary()
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
            Ok(r) => ok(r),
            Err(e) => err(storage_err(e)),
        },
        "finance_transactions" => {
            let filter = storage::rows::finance::FinanceTransactionFilter {
                limit: get(&body, "limit"),
                ..Default::default()
            };
            match core.repos.finance.transactions.list(&filter).await {
                Ok(r) => ok(r),
                Err(e) => err(storage_err(e)),
            }
        }
        "finance_budget_usage" => match core.repos.finance.budgets.all_budget_usage().await {
            Ok(r) => ok(r),
            Err(e) => err(storage_err(e)),
        },
        "finance_portfolios" => match core.repos.finance.investments.list_portfolios().await {
            Ok(portfolios) => {
                let futs = portfolios
                    .iter()
                    .map(|p| core.repos.finance.investments.portfolio_summary(&p.id));
                match futures_util::future::try_join_all(futs).await {
                    Ok(summaries) => ok(portfolios
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
                        .collect::<Vec<_>>()),
                    Err(e) => err(storage_err(e)),
                }
            }
            Err(e) => err(storage_err(e)),
        },
        "finance_investments" => match core
            .repos
            .finance
            .investments
            .list_investments(&Default::default())
            .await
        {
            Ok(r) => ok(r),
            Err(e) => err(storage_err(e)),
        },
        "finance_goals" => match core.repos.finance.goals.list_active().await {
            Ok(r) => ok(r),
            Err(e) => err(storage_err(e)),
        },
        "finance_liabilities" => match core.repos.finance.liabilities.list_all().await {
            Ok(r) => ok(r),
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
                    ok(FinanceNetWorthResponse {
                        totals_by_currency: by_currency
                            .into_values()
                            .map(|mut c| {
                                c.net = c.accounts + c.investments - c.liabilities;
                                c
                            })
                            .collect(),
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
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let start = match parse_date_or_err(&date) {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let end = start + chrono::Duration::days(1);
            let cap: i64 = get(&body, "limit").unwrap_or(10_000);
            match repos
                .events
                .list_range_offset(&start, &end, Some(cap.min(10_000)), get(&body, "offset"))
                .await
            {
                Ok(events) => ok(events
                    .into_iter()
                    .map(|e| ActivityTimelineResponse {
                        app_name: e.app_name,
                        window_title: e.window_title,
                        site_name: e.site_name,
                        category_id: e.category_id,
                        started_at: e.started_at,
                        duration_secs: e.duration_secs,
                        is_idle: e.is_idle,
                        project_id: e.project_id,
                    })
                    .collect::<Vec<_>>()),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_focus_status" => match core.focus_manager() {
            Ok(mgr) => match mgr.get_active().await {
                Ok(s) => ok(s.map(session_to_response)),
                Err(e) => err(prod_err(e)),
            },
            Err(e) => err(e),
        },
        "productivity_focus_start" => match core.focus_manager() {
            Ok(mgr) => match mgr
                .start_session(
                    get(&body, "action_id"),
                    get(&body, "project_id"),
                    get(&body, "target_mins"),
                )
                .await
            {
                Ok(s) => ok(session_to_response(s)),
                Err(e) => err(prod_err(e)),
            },
            Err(e) => err(e),
        },
        "productivity_focus_end" => match core.focus_manager() {
            Ok(mgr) => match mgr.end_session(get(&body, "notes")).await {
                Ok(s) => ok(s.map(session_to_response)),
                Err(e) => err(prod_err(e)),
            },
            Err(e) => err(e),
        },
        "productivity_sessions" => {
            let date = match get_str(&body, "date") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let start = match parse_date_or_err(&date) {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let end = start + chrono::Duration::days(1);
            match repos.sessions.list_range(&start, &end, None).await {
                Ok(s) => ok(s.into_iter().map(session_to_response).collect::<Vec<_>>()),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_weekly" => {
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let today = Utc::now().date_naive();
            let ws = today - chrono::Duration::days(6);
            match repos
                .summaries
                .list_range(
                    &ws.format("%Y-%m-%d").to_string(),
                    &today.format("%Y-%m-%d").to_string(),
                )
                .await
            {
                Ok(s) => ok(s.into_iter().map(summary_to_response).collect::<Vec<_>>()),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_categories" => {
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.categories.list_all().await {
                Ok(c) => ok(c
                    .into_iter()
                    .map(|c| ActivityCategoryResponse {
                        id: c.id,
                        name: c.name,
                        category_type: c.category_type.to_string(),
                        color: c.color,
                        icon: c.icon,
                        is_system: c.is_system,
                    })
                    .collect::<Vec<_>>()),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_summary_range" => {
            let sd = match get_str(&body, "start_date") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let ed = match get_str(&body, "end_date") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.summaries.list_range(&sd, &ed).await {
                Ok(mut summaries) => {
                    let today = Utc::now().format("%Y-%m-%d").to_string();
                    if today >= sd && today <= ed {
                        if let Ok(agg) = core.aggregator() {
                            if let Ok(live) = agg.compute_today().await {
                                summaries.retain(|s| s.date != today);
                                summaries.push(live);
                                summaries.sort_by(|a, b| a.date.cmp(&b.date));
                            }
                        }
                    }
                    ok(summaries
                        .into_iter()
                        .map(summary_to_response)
                        .collect::<Vec<_>>())
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
                Ok(events) => ok(events
                    .into_iter()
                    .map(|e| ActivityTimelineResponse {
                        app_name: e.app_name,
                        window_title: e.window_title,
                        site_name: e.site_name,
                        category_id: e.category_id,
                        started_at: e.started_at,
                        duration_secs: e.duration_secs,
                        is_idle: e.is_idle,
                        project_id: e.project_id,
                    })
                    .collect::<Vec<_>>()),
                Err(e) => err(prod_err(e)),
            }
        }
        "productivity_goals" => match core.aggregator() {
            Ok(agg) => {
                let today = Utc::now().format("%Y-%m-%d").to_string();
                match agg.check_goals(&today).await {
                    Ok(r) => ok(r
                        .into_iter()
                        .map(|(g, cur, met)| GoalProgressResponse {
                            id: g.id.unwrap_or(0),
                            goal_type: g.goal_type.to_string(),
                            metric: g.metric.to_string(),
                            target_value: g.target_value,
                            current_value: cur,
                            met,
                            project_id: g.project_id.clone(),
                        })
                        .collect::<Vec<_>>()),
                    Err(e) => err(prod_err(e)),
                }
            }
            Err(e) => err(e),
        },
        "productivity_pomodoro_start" => match core.focus_manager() {
            Ok(mgr) => match mgr
                .start_pomodoro(
                    None,
                    None,
                    get(&body, "work_mins"),
                    get(&body, "break_mins"),
                )
                .await
            {
                Ok(s) => ok(session_to_response(s)),
                Err(e) => err(prod_err(e)),
            },
            Err(e) => err(e),
        },
        "productivity_time_entries" => {
            let date = match get_str(&body, "date") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let start = match parse_date_or_err(&date) {
                Ok(d) => d,
                Err(e) => return err(e),
            };
            let end = start + chrono::Duration::days(1);
            match repos.time_entries.list_range(&start, &end).await {
                Ok(entries) => ok(entries
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
                    .collect::<Vec<_>>()),
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
                Err(_) => return err(ApiError::new("VALIDATION", "Invalid metric")),
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
                    project_id: goal.project_id,
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
            let duration_mins: f64 = match get(&body, "duration_mins") {
                Some(v) => v,
                None => {
                    return err(ApiError::new(
                        "VALIDATION",
                        "missing required field: duration_mins",
                    ))
                }
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let entry = feature_productivity::types::TimeEntry {
                id: None,
                description,
                category_id: get(&body, "category_id"),
                project_id: get(&body, "project_id"),
                started_at: Utc::now(),
                duration_secs: (duration_mins * 60.0) as i64,
                source: "manual".to_string(),
                created_at: Utc::now(),
            };
            match repos.time_entries.insert(&entry).await {
                Ok(id) => ok(TimeEntryResponse {
                    id,
                    description: entry.description,
                    category_id: entry.category_id,
                    project_id: entry.project_id,
                    started_at: entry.started_at,
                    duration_secs: entry.duration_secs,
                    source: entry.source,
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
            let category_type = match get_str(&body, "category_type") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let ct: feature_productivity::types::CategoryType = match category_type.parse() {
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
                color: get(&body, "color"),
                icon: get(&body, "icon"),
                rules: None,
                is_system: false,
            };
            match repos.categories.upsert(&cat).await {
                Ok(_) => ok(ActivityCategoryResponse {
                    id,
                    name,
                    category_type,
                    color: cat.color,
                    icon: cat.icon,
                    is_system: false,
                }),
                Err(e) => err(prod_err(e)),
            }
        }

        // ── Settings (read-only) ──────────────────────────────
        "mcp_get_config" => {
            let cfg = core.config.read().await;
            let servers: Vec<_> = cfg.mcp.servers.iter().map(|s| {
                let (transport, command, args, env, url, headers) = match &s.transport {
                    config::McpTransport::Stdio { command, args, env } => ("stdio", Some(command.clone()), Some(args.clone()), Some(env.clone()), None, None),
                    config::McpTransport::Http { url, headers } => ("http", None, None, None, Some(url.clone()), Some(headers.clone())),
                };
                let oauth_provider = s.oauth.as_ref().map(|o| o.provider.clone());
                let oauth_connected = s.oauth.as_ref().is_some_and(|o| !o.access_token.is_empty());
                serde_json::json!({
                    "name": s.name, "transport": transport, "enabled": s.enabled,
                    "command": command, "args": args, "env": env, "url": url, "headers": headers,
                    "oauthProvider": oauth_provider, "oauthConnected": oauth_connected,
                })
            }).collect();
            ok(serde_json::json!({ "enabled": cfg.mcp.enabled, "servers": servers }))
        }

        // ── Chat (read-only) ──────────────────────────────────
        "chat_threads" => {
            let default_filter = ProjectFilter::default();
            let (sessions, contexts, areas, projects) = tokio::join!(
                core.repos.sessions.list_sessions(),
                core.repos.session_context.list_visible(),
                core.repos.areas.list(None),
                core.repos.projects.list(&default_filter),
            );
            match (sessions, contexts, areas, projects) {
                (Ok(sessions), Ok(contexts), Ok(areas), Ok(projects)) => {
                    let ctx_map: HashMap<&str, _> = contexts
                        .iter()
                        .map(|c| (c.session_key.as_str(), c))
                        .collect();
                    let area_names: HashMap<&str, &str> = areas
                        .iter()
                        .map(|a| (a.id.as_str(), a.name.as_str()))
                        .collect();
                    let proj_names: HashMap<&str, &str> = projects
                        .iter()
                        .map(|p| (p.id.as_str(), p.name.as_str()))
                        .collect();
                    ok(sessions
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
                                        .and_then(|id| proj_names.get(id).map(|s| s.to_string()))
                                }),
                            }
                        })
                        .collect::<Vec<_>>())
                }
                _ => err(ApiError::new("STORAGE_ERROR", "failed to load chat data")),
            }
        }
        "chat_messages" => {
            let sk = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let limit: Option<i64> = get(&body, "limit");
            let rows = if let Some(lim) = limit {
                core.repos.sessions.get_recent_messages(&sk, lim).await
            } else {
                core.repos.sessions.get_messages(&sk).await
            };
            match rows {
                Ok(msgs) => ok(msgs
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
                    .collect::<Vec<_>>()),
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Distraction ──────────────────────────────────────
        "distraction_dismiss" => {
            let app_name = match get_str(&body, "appName") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.focus_manager() {
                Ok(mgr) => match mgr.record_distraction(&app_name).await {
                    Ok(()) => ok(()),
                    Err(e) => err(prod_err(e)),
                },
                Err(e) => err(e),
            }
        }
        "distraction_allow_temp" => {
            let pattern = match get_str(&body, "pattern") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.distraction_interceptor() {
                Ok(interceptor) => {
                    let mut guard = interceptor.lock().await;
                    guard.grant_temp_pass(&pattern);
                    ok(())
                }
                Err(e) => err(e),
            }
        }
        "distraction_allow_session" => {
            let pattern = match get_str(&body, "pattern") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let classification = match get_str(&body, "classification") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let pattern_type: Option<String> = get(&body, "patternType");
            let pt = pattern_type.as_deref().unwrap_or("title_keyword");
            match core.distraction_interceptor() {
                Ok(interceptor) => {
                    let mut guard = interceptor.lock().await;
                    guard.whitelist_for_session(&pattern);
                    drop(guard);

                    let repos = match core.productivity_repos() {
                        Ok(r) => r,
                        Err(e) => return err(e),
                    };
                    match repos
                        .learned_rules
                        .upsert_or_hit(&pattern.to_lowercase(), pt, &classification)
                        .await
                    {
                        Ok(()) => ok(()),
                        Err(e) => err(prod_err(e)),
                    }
                }
                Err(e) => err(e),
            }
        }
        "distraction_learned_rules" => {
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.learned_rules.list_all().await {
                Ok(rules) => ok(rules
                    .into_iter()
                    .map(|r| LearnedRuleResponse {
                        id: r.id.unwrap_or(0),
                        pattern: r.pattern,
                        pattern_type: r.pattern_type,
                        classification: r.classification,
                        confidence: r.confidence,
                        hit_count: r.hit_count,
                        last_used_at: r.last_used_at.to_rfc3339(),
                        created_at: r.created_at.to_rfc3339(),
                    })
                    .collect::<Vec<_>>()),
                Err(e) => err(prod_err(e)),
            }
        }
        "distraction_delete_rule" => {
            let id: i64 = match get(&body, "id") {
                Some(v) => v,
                None => return err(ApiError::new("VALIDATION", "missing required field: id")),
            };
            let repos = match core.productivity_repos() {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            match repos.learned_rules.delete(id).await {
                Ok(()) => ok(()),
                Err(e) => err(prod_err(e)),
            }
        }
        "distraction_evaluate" => {
            let app_name = match get_str(&body, "appName") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let window_title: Option<String> = get(&body, "windowTitle");
            match core.distraction_interceptor() {
                Ok(interceptor) => {
                    let mut guard = interceptor.lock().await;
                    let decision = guard.evaluate(&app_name, window_title.as_deref()).await;
                    ok(serde_json::json!({
                        "decision": format!("{:?}", decision),
                    }))
                }
                Err(e) => err(e),
            }
        }

        // ── Permissions ──────────────────────────────────────
        "permissions_check_accessibility" => ok(desktop_shared::permissions::check_accessibility()),
        "permissions_open_accessibility" => {
            desktop_shared::permissions::open_accessibility_settings();
            ok(())
        }

        _ => err(ApiError::new(
            "NOT_FOUND",
            format!("command '{cmd}' not supported in browser dev mode"),
        )),
    }
}
