//! Standalone dev API server — shares the same SQLite DB as the desktop app.
//!
//! Usage: cargo run -p dev-api
//!
//! Starts an HTTP server on port 3456 that serves the same data as Tauri commands.
//! Run `bun run dev` in desktop-ui/ separately, then open localhost:1420 in Chrome.

use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderValue, Method};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use chrono::{Local, Timelike, Utc};
use desktop_shared::cognitive_commands::*;
use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;
use desktop_shared::events as ev;
use feature_notes::link_parser;
use feature_notes::models::{NoteRow, NotebookRow};
use feature_notes::repo::{utc_now_str, NoteRepo};
use feature_productivity::distraction::DistractionInterceptor;
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::{DailyAggregator, FocusManager};
use futures_util::stream::Stream;
use futures_util::StreamExt;
use serde_json::Value;
use storage::{ActionFilter, ActionPatch, ProjectFilter, Repos, StoragePool};
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::info;

/// App state — storage, config, agent, and productivity.
struct DevState {
    repos: Repos,
    note_repo: NoteRepo,
    config: RwLock<config::Config>,
    data_dir: std::path::PathBuf,
    agent: Arc<agent::AgentLoop>,
    /// Active SSE senders keyed by session_key.
    sse_channels: dashmap::DashMap<String, Vec<tokio::sync::mpsc::UnboundedSender<SseEvent>>>,
    /// Active stream cancel tokens.
    active_streams: dashmap::DashMap<String, CancellationToken>,
    /// Pending ask_user interaction oneshot senders keyed by session_key.
    pending_interactions: dashmap::DashMap<String, (String, oneshot::Sender<common::FormResponse>)>,
    productivity_repos: Option<ProductivityRepos>,
    focus_manager: Option<Arc<FocusManager>>,
    aggregator: Option<Arc<DailyAggregator>>,
    distraction_interceptor: Option<Arc<Mutex<DistractionInterceptor>>>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct SseEvent {
    event: String,
    data: serde_json::Value,
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
    let mut config = config::load_with_env_overrides()
        .await
        .expect("failed to load config");
    info!(path = ?config::config_path(), "configuration loaded");

    // 2. Connect storage (read/write to the same DB as the desktop app)
    let data_dir = config.data_dir_path();
    std::fs::create_dir_all(&data_dir).expect("failed to create data dir");
    let pool = StoragePool::connect(&data_dir)
        .await
        .expect("failed to connect storage");
    let repos = Repos::from_pool(&pool);
    info!("storage connected");

    // 2b. Initialize notes feature
    let notes_pool = pool.inner().clone();
    StoragePool::run_feature_migrations(
        &notes_pool,
        &feature_notes::NotesFeature::migrations_static(),
    )
    .await
    .expect("notes migration failed");
    let note_repo = NoteRepo::new(notes_pool);

    // 3. Create LLM provider
    let (provider, resolved_model) =
        providers::create_provider(&config).expect("failed to create provider");
    config.agents.defaults.model = resolved_model;
    info!(provider = %provider.name(), "LLM provider ready");

    // 4. Message bus
    let bus = Arc::new(bus::MessageBus::new(100));

    // 5. Cron service
    let cron_service = scheduling::CronService::new(repos.cron.clone());
    cron_service.start().await.expect("cron start failed");
    let cron_service = Arc::new(cron_service);

    // 6. Build AgentLoop
    let vector_store = storage::VectorStore::connect(&data_dir).await.ok();
    let mut builder = agent::AgentLoop::builder(bus.clone(), provider, config.clone())
        .with_pool(pool.inner().clone())
        .with_cron_service(cron_service.clone());

    if let Some(vs) = vector_store {
        builder = builder.with_vector_store(vs);
    }

    let mut agent_loop_raw = builder.build().await.expect("agent build failed");
    let _inbound_rx = agent_loop_raw
        .take_inbound_rx()
        .expect("inbound rx already taken");
    let agent = Arc::new(agent_loop_raw);
    info!("agent loop initialized");

    // 7. Initialize productivity (optional)
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
        note_repo,
        config: RwLock::new(config),
        data_dir: data_dir.clone(),
        agent,
        sse_channels: dashmap::DashMap::new(),
        active_streams: dashmap::DashMap::new(),
        pending_interactions: dashmap::DashMap::new(),
        productivity_repos: prod_repos,
        focus_manager: focus_mgr,
        aggregator,
        distraction_interceptor: interceptor,
    });

    // 8. Build axum router
    let attachments_dir = data_dir.join("attachments");
    if let Err(e) = std::fs::create_dir_all(&attachments_dir) {
        tracing::error!(path = ?attachments_dir, error = %e, "failed to create attachments directory");
    }

    let app = Router::new()
        .route("/api/events/{session_key}", axum::routing::get(events_sse))
        .route("/api/{cmd}", post(dispatch))
        .nest_service(
            "/attachments",
            tower_http::services::ServeDir::new(&attachments_dir),
        )
        .with_state(state);

    let app = app.layer(
        tower_http::cors::CorsLayer::new()
            .allow_origin("http://localhost:1420".parse::<HeaderValue>().unwrap())
            .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
            .allow_headers(tower_http::cors::Any),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3456")
        .await
        .expect("failed to bind port 3456");
    info!("dev API server listening on http://127.0.0.1:3456");
    info!("open http://localhost:1420 in Chrome to use the UI with real data");
    axum::serve(listener, app).await.unwrap();
}

// ── SSE endpoint ─────────────────────────────────────────────────────────

async fn events_sse(
    Path(session_key): Path<String>,
    State(core): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<SseEvent>();

    // Register this SSE sender
    core.sse_channels.entry(session_key).or_default().push(tx);

    let stream = UnboundedReceiverStream::new(rx).map(|sse_event| {
        Ok(Event::default()
            .event(sse_event.event)
            .json_data(sse_event.data)
            .unwrap_or_else(|_| Event::default()))
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
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
    match serde_json::to_value(v) {
        Ok(val) => ApiResult::Ok(val),
        Err(e) => {
            tracing::error!(error = %e, "response serialization failed");
            ApiResult::Err(ApiError::new("INTERNAL", format!("serialization error: {e}")))
        }
    }
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
    body.get(key).and_then(|v| {
        match serde_json::from_value(v.clone()) {
            Ok(val) => Some(val),
            Err(e) => {
                tracing::warn!(field = key, error = %e, "ignoring malformed optional field");
                None
            }
        }
    })
}

fn note_row_to_resp(row: &NoteRow, tags: Vec<String>) -> NoteResponse {
    NoteResponse {
        id: row.id.clone(),
        notebook_id: row.notebook_id.clone(),
        title: row.title.clone(),
        body: row.body.clone(),
        body_html: row.body_html.clone(),
        pinned: row.pinned != 0,
        archived: row.archived != 0,
        tags,
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
    }
}

fn notebook_row_to_resp(row: &NotebookRow, note_count: i64) -> NotebookResponse {
    NotebookResponse {
        id: row.id.clone(),
        parent_id: row.parent_id.clone(),
        title: row.title.clone(),
        icon: row.icon.clone(),
        sort_order: row.sort_order,
        note_count,
    }
}

/// Convert a list of NoteRows to NoteResponses with batch-fetched tags (single query).
async fn notes_with_tags_batch(
    repo: &NoteRepo,
    rows: &[NoteRow],
) -> Result<Vec<NoteResponse>, ApiError> {
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let mut tag_map = repo
        .get_tags_batch(&ids)
        .await
        .map_err(storage_err)?;
    Ok(rows
        .iter()
        .map(|row| {
            let tags = tag_map.remove(&row.id).unwrap_or_default();
            note_row_to_resp(row, tags)
        })
        .collect())
}

fn version_row_to_resp(row: &feature_notes::models::NoteVersionRow) -> NoteVersionResponse {
    NoteVersionResponse {
        id: row.id.clone(),
        note_id: row.note_id.clone(),
        body: row.body.clone(),
        created_at: row.created_at.clone(),
    }
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
        "task_get" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.repos.actions.get(&id).await {
                Ok(Some(row)) => match row_to_task(&core.repos, &row).await {
                    Ok(task) => ok(task),
                    Err(e) => err(e),
                },
                Ok(None) => ok(serde_json::Value::Null),
                Err(e) => err(storage_err(e)),
            }
        }
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

        // ── Chat ──────────────────────────────────────────────
        "chat_send" => {
            let content = match get_str(&body, "content") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let session_key = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };

            // Ensure session exists
            let title: String = content
                .chars()
                .take(60)
                .collect::<String>()
                .trim()
                .to_string();
            let metadata = serde_json::json!({ "title": title });
            if let Err(e) = core
                .repos
                .sessions
                .upsert_session(&session_key, &metadata)
                .await
            {
                return err(storage_err(e));
            }

            // Start agent streaming
            let msg_id = uuid::Uuid::new_v4();
            let now = chrono::Utc::now();
            let streaming_handle = match core
                .agent
                .process_direct_streaming(content.clone(), session_key.clone())
                .await
            {
                Ok(h) => h,
                Err(e) => return err(ApiError::new("AGENT_ERROR", e.to_string())),
            };

            // Track cancel token
            core.active_streams
                .insert(session_key.clone(), streaming_handle.cancel_token);

            // Spawn background task to relay AgentEvents -> SSE
            let sk = session_key.clone();
            let state = Arc::clone(&core);
            let mut event_rx = streaming_handle.event_rx;
            let mut interaction_rx = streaming_handle.interaction_rx;

            tokio::spawn(async move {
                // Cleanup guard: removes active_streams AND sse_channels on drop (incl. panic)
                struct StreamGuard {
                    key: String,
                    state: Arc<DevState>,
                }
                impl Drop for StreamGuard {
                    fn drop(&mut self) {
                        self.state.active_streams.remove(&self.key);
                        self.state.sse_channels.remove(&self.key);
                        // Cancel any pending interaction to avoid leaking the oneshot
                        if let Some((_, (_, tx))) =
                            self.state.pending_interactions.remove(&self.key)
                        {
                            let _ = tx.send(common::FormResponse::Cancelled);
                        }
                    }
                }
                let _guard = StreamGuard {
                    key: sk.clone(),
                    state: Arc::clone(&state),
                };

                // Segment + transparency accumulation
                let mut segments: Vec<desktop_shared::events::MessageSegment> = Vec::new();
                let mut transparency = desktop_shared::events::TransparencyData::default();
                let mut current_text = String::new();
                let mut pending_actions: HashMap<String, VecDeque<String>> = HashMap::new();
                let mut tool_token_sum: u32 = 0;

                let flush_text =
                    |text: &mut String, segs: &mut Vec<desktop_shared::events::MessageSegment>| {
                        if !text.is_empty() {
                            segs.push(desktop_shared::events::MessageSegment::Text {
                                content: std::mem::take(text),
                            });
                        }
                    };

                // Helper to send SSE event to all connected clients for this session.
                // Retains only live senders.
                let send_sse = |event_name: &str, data: serde_json::Value| {
                    if let Some(mut senders) = state.sse_channels.get_mut(&sk) {
                        let sse_event = SseEvent {
                            event: event_name.to_string(),
                            data,
                        };
                        senders.retain(|tx| tx.send(sse_event.clone()).is_ok());
                    }
                };

                loop {
                    let event = tokio::select! {
                        biased;
                        Some(bundle) = interaction_rx.recv() => {
                            let request_id = uuid::Uuid::new_v4().to_string();
                            send_sse(ev::AGENT_INTERACTION_REQUEST, serde_json::json!({
                                "sessionKey": sk,
                                "requestId": request_id,
                                "request": bundle.request,
                            }));
                            state.pending_interactions.insert(sk.clone(), (request_id, bundle.response_tx));
                            continue;
                        }
                        Some(event) = event_rx.recv() => event,
                        else => break,
                    };
                    match event {
                        agent::AgentEvent::ContentChunk { data } => {
                            current_text.push_str(&data);
                            send_sse(
                                ev::AGENT_CONTENT_CHUNK,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "data": data,
                                }),
                            );
                        }
                        agent::AgentEvent::ToolStart {
                            name,
                            args,
                            agent: agent_name,
                        } => {
                            flush_text(&mut current_text, &mut segments);
                            let action = args
                                .get("action")
                                .and_then(|v| v.as_str())
                                .map(String::from);
                            if let Some(ref a) = action {
                                pending_actions
                                    .entry(name.clone())
                                    .or_default()
                                    .push_back(a.clone());
                            }
                            send_sse(
                                ev::AGENT_TOOL_START,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "name": name,
                                    "action": action,
                                    "agent": agent_name,
                                }),
                            );
                        }
                        agent::AgentEvent::ToolEnd {
                            name,
                            success,
                            duration_ms,
                            result,
                            agent: agent_name,
                        } => {
                            let action = match pending_actions.entry(name.clone()) {
                                std::collections::hash_map::Entry::Occupied(mut e) => {
                                    let a = e.get_mut().pop_front();
                                    if e.get().is_empty() {
                                        e.remove();
                                    }
                                    a
                                }
                                std::collections::hash_map::Entry::Vacant(_) => None,
                            };
                            let estimated_tokens = result
                                .as_ref()
                                .map(|r| (r.len() as u32).saturating_add(3) / 4);
                            if let Some(t) = estimated_tokens {
                                tool_token_sum += t;
                            }
                            segments.push(desktop_shared::events::MessageSegment::Tool {
                                name: name.clone(),
                                action: action.clone(),
                                success,
                                duration_ms,
                                result: result.clone(),
                                estimated_tokens,
                                agent: agent_name.clone(),
                            });
                            transparency
                                .tools
                                .push(desktop_shared::events::TransparencyTool {
                                    name: name.clone(),
                                    action: action.clone(),
                                    success,
                                    duration_ms,
                                    estimated_tokens,
                                    agent: agent_name.clone(),
                                });
                            send_sse(
                                ev::AGENT_TOOL_END,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "name": name,
                                    "action": action,
                                    "success": success,
                                    "durationMs": duration_ms,
                                    "result": result,
                                    "estimatedTokens": estimated_tokens,
                                    "agent": agent_name,
                                }),
                            );
                        }
                        agent::AgentEvent::Done { content } => {
                            flush_text(&mut current_text, &mut segments);
                            if tool_token_sum > 0 {
                                transparency.tool_tokens_total = Some(tool_token_sum);
                            }
                            // Persist segments + transparency metadata
                            let mut meta = serde_json::Map::new();
                            if !segments.is_empty() {
                                meta.insert(
                                    "segments".into(),
                                    serde_json::to_value(&segments).unwrap_or_default(),
                                );
                            }
                            meta.insert(
                                "transparency".into(),
                                serde_json::to_value(&transparency).unwrap_or_default(),
                            );
                            let meta_value = serde_json::Value::Object(meta);
                            let _ = state
                                .repos
                                .sessions
                                .update_last_assistant_metadata(&sk, None, Some(&meta_value))
                                .await;

                            send_sse(
                                ev::AGENT_DONE,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "content": content,
                                }),
                            );
                            break;
                        }
                        agent::AgentEvent::Error { message } => {
                            send_sse(
                                ev::AGENT_ERROR,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "message": message,
                                }),
                            );
                            break;
                        }
                        agent::AgentEvent::ClassificationComplete {
                            strategy,
                            confidence,
                            source,
                            ..
                        } => {
                            send_sse(
                                ev::AGENT_CLASSIFICATION_COMPLETE,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "strategy": strategy,
                                    "confidence": confidence,
                                    "source": source,
                                }),
                            );
                        }
                        agent::AgentEvent::ExecutionStarted {
                            engine,
                            max_iterations,
                        } => {
                            send_sse(
                                ev::AGENT_EXECUTION_STARTED,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "engine": engine,
                                    "maxIterations": max_iterations,
                                }),
                            );
                        }
                        agent::AgentEvent::IterationStart { iteration, max } => {
                            send_sse(
                                ev::AGENT_ITERATION_START,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "iteration": iteration,
                                    "maxIterations": max,
                                }),
                            );
                        }
                        agent::AgentEvent::UsageReport {
                            prompt_tokens,
                            completion_tokens,
                            cache_read_tokens,
                            cache_write_tokens,
                            estimated_cost_usd,
                            model,
                            response_time_ms,
                        } => {
                            send_sse(
                                ev::AGENT_USAGE_REPORT,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "promptTokens": prompt_tokens,
                                    "completionTokens": completion_tokens,
                                    "cacheReadTokens": cache_read_tokens,
                                    "cacheWriteTokens": cache_write_tokens,
                                    "estimatedCostUsd": estimated_cost_usd,
                                    "model": model,
                                    "responseTimeMs": response_time_ms,
                                }),
                            );
                        }
                        agent::AgentEvent::AgentSelected { name, description } => {
                            send_sse(
                                ev::AGENT_SELECTED,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "name": name,
                                    "description": description,
                                }),
                            );
                        }
                        agent::AgentEvent::SkillLoaded {
                            name,
                            trigger,
                            agent: agent_name,
                        } => {
                            send_sse(
                                ev::AGENT_SKILL_LOADED,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "name": name,
                                    "trigger": trigger,
                                    "agent": agent_name,
                                }),
                            );
                        }
                        agent::AgentEvent::MemoryAccess {
                            action,
                            query,
                            results_count,
                        } => {
                            send_sse(
                                ev::AGENT_MEMORY_ACCESS,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "action": action,
                                    "query": query,
                                    "resultsCount": results_count,
                                }),
                            );
                        }
                        agent::AgentEvent::SubagentSpawned { label, profile } => {
                            send_sse(
                                ev::AGENT_SUBAGENT_SPAWNED,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "label": label,
                                    "profile": profile,
                                }),
                            );
                        }
                        agent::AgentEvent::DelegationStarted {
                            from_agent,
                            to_agent,
                            query,
                            depth,
                        } => {
                            send_sse(
                                ev::AGENT_DELEGATION_STARTED,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "fromAgent": from_agent,
                                    "toAgent": to_agent,
                                    "query": query,
                                    "depth": depth,
                                }),
                            );
                        }
                        agent::AgentEvent::DelegationCompleted {
                            from_agent,
                            to_agent,
                            success,
                            duration_ms,
                        } => {
                            send_sse(
                                ev::AGENT_DELEGATION_COMPLETED,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "fromAgent": from_agent,
                                    "toAgent": to_agent,
                                    "success": success,
                                    "durationMs": duration_ms,
                                }),
                            );
                        }
                        agent::AgentEvent::LearningEvent { event_type, detail } => {
                            send_sse(
                                ev::AGENT_LEARNING_EVENT,
                                serde_json::json!({
                                    "sessionKey": sk,
                                    "eventType": event_type,
                                    "detail": detail,
                                }),
                            );
                        }
                        // Skip events that have no frontend consumer
                        _ => {}
                    }
                }

                // StreamGuard handles sse_channels + active_streams cleanup on drop
            });

            // Return user message immediately
            ok(ChatMessageResponse {
                id: msg_id.to_string(),
                role: common::MessageRole::User.to_string(),
                content,
                timestamp: now,
                segments: None,
                transparency: None,
            })
        }
        "chat_cancel" => {
            let sk = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            if let Some((_, token)) = core.active_streams.remove(&sk) {
                token.cancel();
            }
            if let Some((_, (_, tx))) = core.pending_interactions.remove(&sk) {
                let _ = tx.send(common::FormResponse::Cancelled);
            }
            ok(())
        }
        "chat_respond_interaction" => {
            let sk = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let request_id = match get_str(&body, "requestId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let response: common::FormResponse = match body
                .get("response")
                .cloned()
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: response"))
                .and_then(|v| {
                    serde_json::from_value(v)
                        .map_err(|e| ApiError::new("VALIDATION", e.to_string()))
                }) {
                Ok(r) => r,
                Err(e) => return err(e),
            };
            let (stored_id, sender) = match core.pending_interactions.remove(&sk) {
                Some((_, pair)) => pair,
                None => {
                    return err(ApiError::new(
                        "NOT_FOUND",
                        "no pending interaction for this session",
                    ))
                }
            };
            if stored_id != request_id {
                return err(ApiError::new(
                    "INVALID_PARAMS",
                    format!("request_id mismatch: expected {stored_id}, got {request_id}"),
                ));
            }
            let _ = sender.send(response);
            ok(())
        }
        "chat_delete_thread" => {
            let sk = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            if let Some((_, token)) = core.active_streams.remove(&sk) {
                token.cancel();
            }
            if let Some((_, (_, tx))) = core.pending_interactions.remove(&sk) {
                let _ = tx.send(common::FormResponse::Cancelled);
            }
            match core.repos.sessions.delete_session(&sk).await {
                Ok(_) => ok(()),
                Err(e) => err(storage_err(e)),
            }
        }
        "chat_rename_thread" => {
            let sk = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let title = match get_str(&body, "title") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.repos.sessions.rename_session(&sk, &title).await {
                Ok(_) => ok(()),
                Err(e) => err(storage_err(e)),
            }
        }
        "chat_pin_thread" => {
            let sk = match get_str(&body, "sessionKey") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.repos.session_context.pin(&sk).await {
                Ok(_) => ok(()),
                Err(e) => err(storage_err(e)),
            }
        }
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

        // ── Notes ──────────────────────────────────────────────
        "note_list" => {
            let notebook_id: Option<String> = get(&body, "notebookId");
            match core.note_repo.list_notes(notebook_id.as_deref()).await {
                Ok(rows) => match notes_with_tags_batch(&core.note_repo, &rows).await {
                    Ok(results) => ok(results),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "note_get" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.note_repo.get_note(&id).await {
                Ok(Some(row)) => {
                    let tags = match core.note_repo.get_tags(&id).await {
                        Ok(t) => t,
                        Err(e) => return err(storage_err(e)),
                    };
                    ok(note_row_to_resp(&row, tags))
                }
                Ok(None) => err(ApiError::new("NOT_FOUND", format!("note '{id}' not found"))),
                Err(e) => err(storage_err(e)),
            }
        }
        "note_create" => {
            let params: NoteCreateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            if params.title.trim().is_empty() {
                return err(ApiError::new("VALIDATION", "title must not be empty"));
            }
            let id = uuid::Uuid::new_v4().to_string();
            let now = utc_now_str();
            let row = NoteRow {
                id: id.clone(),
                notebook_id: params.notebook_id,
                title: params.title,
                body: params.body.unwrap_or_default(),
                body_html: None,
                pinned: 0,
                archived: 0,
                created_at: now.clone(),
                updated_at: now,
            };
            match core.note_repo.create_note(&row).await {
                Ok(created) => {
                    if let Some(tags) = params.tags {
                        if let Err(e) = core.note_repo.set_tags(&id, &tags).await {
                            return err(storage_err(e));
                        }
                    }
                    let tags = match core.note_repo.get_tags(&id).await {
                        Ok(t) => t,
                        Err(e) => return err(storage_err(e)),
                    };
                    ok(note_row_to_resp(&created, tags))
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "note_update" => {
            let params: NoteUpdateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            match core
                .note_repo
                .update_note(
                    &params.id,
                    params.title.as_deref(),
                    params.body.as_deref(),
                    params.body_html.as_deref(),
                    params.pinned,
                    params.notebook_id.as_ref().map(|o| o.as_deref()),
                )
                .await
            {
                Ok(updated) => {
                    if let Some(tags) = params.tags {
                        if let Err(e) = core.note_repo.set_tags(&params.id, &tags).await {
                            return err(storage_err(e));
                        }
                    }

                    // Extract wiki-links and entity mentions from content
                    let mut target_ids: Vec<String> = Vec::new();
                    if let Some(html) = &updated.body_html {
                        target_ids.extend(link_parser::extract_wiki_link_ids(html));
                    }
                    let titles = link_parser::extract_wiki_link_titles(&updated.body);
                    if !titles.is_empty() {
                        match core.note_repo.resolve_titles_to_ids(&titles).await {
                            Ok(resolved) => {
                                for (_title, id) in resolved {
                                    if !target_ids.contains(&id) {
                                        target_ids.push(id);
                                    }
                                }
                            }
                            Err(e) => return err(storage_err(e)),
                        }
                    }
                    target_ids.retain(|id| id != &updated.id);
                    let mentions = link_parser::extract_entity_mentions(&updated.body);

                    if let Err(e) = core.note_repo.set_links(&updated.id, &target_ids).await {
                        return err(storage_err(e));
                    }
                    let mention_tuples: Vec<(String, String)> = mentions
                        .into_iter()
                        .map(|m| (m.entity_type, m.entity_id))
                        .collect();
                    if let Err(e) = core.note_repo.set_entity_mentions(&updated.id, &mention_tuples).await {
                        return err(storage_err(e));
                    }

                    let tags = match core.note_repo.get_tags(&params.id).await {
                        Ok(t) => t,
                        Err(e) => return err(storage_err(e)),
                    };
                    ok(note_row_to_resp(&updated, tags))
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "note_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.note_repo.delete_note(&id).await {
                Ok(d) => ok(d),
                Err(e) => err(storage_err(e)),
            }
        }
        "note_search" => {
            let query = match get_str(&body, "query") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.note_repo.search_notes(&query).await {
                Ok(rows) => match notes_with_tags_batch(&core.note_repo, &rows).await {
                    Ok(results) => ok(results),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "note_links_all" => match core.note_repo.get_all_links().await {
            Ok(rows) => {
                let results: Vec<NoteLinkResponse> = rows
                    .into_iter()
                    .map(|r| NoteLinkResponse {
                        source_id: r.source_id,
                        target_id: r.target_id,
                    })
                    .collect();
                ok(results)
            }
            Err(e) => err(storage_err(e)),
        },
        "note_version_list" => {
            let note_id = match get_str(&body, "noteId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.note_repo.list_versions(&note_id).await {
                Ok(rows) => {
                    let results: Vec<NoteVersionResponse> = rows
                        .iter()
                        .map(version_row_to_resp)
                        .collect();
                    ok(results)
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "note_version_create" => {
            let note_id = match get_str(&body, "noteId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.note_repo.get_note(&note_id).await {
                Ok(Some(note)) => {
                    let row = feature_notes::models::NoteVersionRow {
                        id: uuid::Uuid::new_v4().to_string(),
                        note_id: note_id.clone(),
                        body: note.body,
                        created_at: utc_now_str(),
                    };
                    match core.note_repo.create_version(&row).await {
                        Ok(created) => {
                            if let Err(e) = core.note_repo.prune_versions(&note_id, 50).await {
                                tracing::warn!(note_id = %note_id, error = %e, "failed to prune old versions");
                            }
                            ok(version_row_to_resp(&created))
                        }
                        Err(e) => err(storage_err(e)),
                    }
                }
                Ok(None) => err(ApiError::new("NOT_FOUND", format!("note '{note_id}' not found"))),
                Err(e) => err(storage_err(e)),
            }
        }
        "note_version_restore" => {
            let version_id = match get_str(&body, "versionId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let note_id = match get_str(&body, "noteId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.note_repo.get_version(&version_id).await {
                Ok(Some(version)) => {
                    // Snapshot current before restoring
                    if let Ok(Some(current)) = core.note_repo.get_note(&note_id).await {
                        let snap = feature_notes::models::NoteVersionRow {
                            id: uuid::Uuid::new_v4().to_string(),
                            note_id: note_id.clone(),
                            body: current.body,
                            created_at: utc_now_str(),
                        };
                        if let Err(e) = core.note_repo.create_version(&snap).await {
                            return err(storage_err(e));
                        }
                    }
                    match core.note_repo.update_note(&note_id, None, Some(&version.body), None, None, None).await {
                        Ok(updated) => {
                            let tags = match core.note_repo.get_tags(&note_id).await {
                                Ok(t) => t,
                                Err(e) => return err(storage_err(e)),
                            };
                            ok(note_row_to_resp(&updated, tags))
                        }
                        Err(e) => err(storage_err(e)),
                    }
                }
                Ok(None) => err(ApiError::new("NOT_FOUND", format!("version '{version_id}' not found"))),
                Err(e) => err(storage_err(e)),
            }
        }
        "note_list_by_entity" => {
            let entity_type = match get_str(&body, "entityType") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let entity_id = match get_str(&body, "entityId") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.note_repo.list_notes_by_entity(&entity_type, &entity_id).await {
                Ok(rows) => match notes_with_tags_batch(&core.note_repo, &rows).await {
                    Ok(results) => ok(results),
                    Err(e) => err(e),
                },
                Err(e) => err(storage_err(e)),
            }
        }
        "note_save_attachment" => {
            let data_str = match get_str(&body, "data") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let filename = match get_str(&body, "filename") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            let attachments_dir = core.data_dir.join("attachments");
            if let Err(e) = tokio::fs::create_dir_all(&attachments_dir).await {
                return err(ApiError::new("IO_ERROR", format!("failed to create dir: {e}")));
            }
            const ALLOWED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];
            let ext = std::path::Path::new(&filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("png");
            let ext = if ALLOWED_EXTENSIONS.contains(&ext) { ext } else { "png" };
            let id = uuid::Uuid::new_v4();
            let file_name = format!("{id}.{ext}");
            let file_path = attachments_dir.join(&file_name);

            use base64::Engine;
            let bytes = match base64::engine::general_purpose::STANDARD.decode(&data_str) {
                Ok(b) => b,
                Err(e) => return err(ApiError::new("DECODE_ERROR", format!("invalid base64: {e}"))),
            };
            if let Err(e) = tokio::fs::write(&file_path, &bytes).await {
                return err(ApiError::new("IO_ERROR", format!("failed to write: {e}")));
            }
            // In dev mode, serve via the /attachments static route
            ok(format!("/attachments/{file_name}"))
        }
        "notebook_list" => match core.note_repo.list_notebooks().await {
            Ok(rows) => {
                let counts = match core.note_repo.count_notes_by_notebook().await {
                    Ok(c) => c,
                    Err(e) => return err(storage_err(e)),
                };
                let results: Vec<_> = rows
                    .iter()
                    .map(|row| notebook_row_to_resp(row, counts.get(&row.id).copied().unwrap_or(0)))
                    .collect();
                ok(results)
            }
            Err(e) => err(storage_err(e)),
        },
        "notebook_create" => {
            let params: NotebookCreateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            if params.title.trim().is_empty() {
                return err(ApiError::new("VALIDATION", "notebook title must not be empty"));
            }
            let id = uuid::Uuid::new_v4().to_string();
            let now = utc_now_str();
            let row = NotebookRow {
                id: id.clone(),
                parent_id: params.parent_id,
                title: params.title,
                icon: params.icon,
                sort_order: 0,
                created_at: now.clone(),
                updated_at: now,
            };
            match core.note_repo.create_notebook(&row).await {
                Ok(created) => ok(notebook_row_to_resp(&created, 0)),
                Err(e) => err(storage_err(e)),
            }
        }
        "notebook_update" => {
            let params: NotebookUpdateParams =
                match serde_json::from_value(body.get("params").cloned().unwrap_or(body.clone())) {
                    Ok(p) => p,
                    Err(e) => return err(ApiError::new("VALIDATION", e.to_string())),
                };
            // Check for cycles when changing parent
            if let Some(Some(new_parent_id)) = &params.parent_id {
                match core.note_repo.would_create_cycle(&params.id, new_parent_id).await {
                    Ok(true) => {
                        return err(ApiError::new(
                            "VALIDATION",
                            "cannot set parent: would create a cycle",
                        ));
                    }
                    Ok(false) => {}
                    Err(e) => return err(storage_err(e)),
                }
            }
            let id = &params.id;
            let parent_id_ref = params.parent_id.as_ref().map(|o| o.as_deref());
            match core
                .note_repo
                .update_notebook(id, params.title.as_deref(), params.icon.as_deref(), parent_id_ref)
                .await
            {
                Ok(updated) => {
                    let count = core
                        .note_repo
                        .count_notes_in_notebook(&id)
                        .await
                        .unwrap_or(0);
                    ok(notebook_row_to_resp(&updated, count))
                }
                Err(e) => err(storage_err(e)),
            }
        }
        "notebook_delete" => {
            let id = match get_str(&body, "id") {
                Ok(v) => v,
                Err(e) => return err(e),
            };
            match core.note_repo.delete_notebook(&id).await {
                Ok(d) => ok(d),
                Err(e) => err(storage_err(e)),
            }
        }

        // ── Cognitive Debug ─────────────────────────────────────
        "cognitive_user_model" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let model = cognitive::repos::load_user_model(&fact_repo).await;
            let preview = |f: &cognitive::types::SemanticFact| format!("{} = {}", f.predicate, f.object);
            let resp = UserModelSummaryResponse {
                identity_count: model.identity.len(),
                energy_count: model.energy.len(),
                work_count: model.work.len(),
                finance_count: model.finance.len(),
                learning_count: model.learning.len(),
                preferences_count: model.preferences.len(),
                identity_preview: model.identity.iter().take(3).map(preview).collect(),
                energy_preview: model.energy.iter().take(3).map(preview).collect(),
                work_preview: model.work.iter().take(3).map(preview).collect(),
                finance_preview: model.finance.iter().take(3).map(preview).collect(),
                learning_preview: model.learning.iter().take(3).map(preview).collect(),
                preferences_preview: model.preferences.iter().take(3).map(preview).collect(),
            };
            ok(resp)
        }
        "cognitive_facts_list" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let domain: Option<String> = get(&body, "domain");
            let domains: Vec<&str> = match domain.as_deref() {
                Some(d) => vec![d],
                None => cognitive::repos::USER_MODEL_DOMAINS.to_vec(),
            };
            let mut all = Vec::new();
            for d in &domains {
                match fact_repo.list_active(d).await {
                    Ok(facts) => all.extend(facts),
                    Err(e) => return err(ApiError::new("STORAGE_ERROR", e.to_string())),
                }
            }
            let now = chrono::Utc::now();
            let resp: Vec<SemanticFactResponse> = all.into_iter().map(|f| {
                let last_ts = f.last_accessed.as_deref()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(|| {
                        chrono::DateTime::parse_from_rfc3339(&f.recorded_at)
                            .map(|dt| dt.with_timezone(&chrono::Utc))
                            .unwrap_or(now)
                    });
                let elapsed_days = now.signed_duration_since(last_ts).num_seconds() as f64 / 86400.0;
                let retrievability = cognitive::decay::retrievability(elapsed_days, f.stability);
                let status = if f.superseded_at.is_some() { "superseded".into() } else { "active".into() };
                SemanticFactResponse {
                    id: f.id, domain: f.domain, subject: f.subject,
                    predicate: f.predicate, object: f.object,
                    confidence: f.confidence, source: f.source,
                    valid_from: f.valid_from, valid_until: f.valid_until,
                    stability: f.stability, retrievability,
                    last_accessed: f.last_accessed, access_count: f.access_count,
                    status,
                }
            }).collect();
            ok(resp)
        }
        "cognitive_episodic_list" => {
            let pool = core.repos.pool();
            let repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
            let domain: Option<String> = get(&body, "domain");
            let limit: i64 = get(&body, "limit").unwrap_or(50);
            let memories = match domain.as_deref() {
                Some(d) => repo.list_by_domain(d, limit).await
                    .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string())),
                None => {
                    let mut all = Vec::new();
                    for d in cognitive::repos::USER_MODEL_DOMAINS {
                        match repo.list_by_domain(d, limit).await {
                            Ok(mems) => all.extend(mems),
                            Err(e) => return err(ApiError::new("STORAGE_ERROR", e.to_string())),
                        }
                    }
                    Ok(all)
                }
            };
            match memories {
                Ok(mems) => {
                    let resp: Vec<EpisodicMemoryResponse> = mems.into_iter().map(|m| EpisodicMemoryResponse {
                        id: m.id,
                        domain: m.domain,
                        content: m.content,
                        summary: m.summary,
                        importance: m.importance,
                        occurred_at: m.occurred_at,
                        recorded_at: m.recorded_at,
                        stability: m.stability,
                        access_count: m.access_count,
                    }).collect();
                    ok(resp)
                }
                Err(e) => err(e),
            }
        }
        "cognitive_rules_list" => {
            let pool = core.repos.pool();
            let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());
            let domain: Option<String> = get(&body, "domain");
            let domains: Vec<&str> = match domain.as_deref() {
                Some(d) => vec![d],
                None => cognitive::repos::RULE_DOMAINS.to_vec(),
            };
            let mut all = Vec::new();
            for d in &domains {
                match repo.list_active(d).await {
                    Ok(rules) => all.extend(rules),
                    Err(e) => return err(ApiError::new("STORAGE_ERROR", e.to_string())),
                }
            }
            let resp: Vec<ProceduralRuleResponse> = all.into_iter().map(|r| ProceduralRuleResponse {
                id: r.id, domain: r.domain,
                rule_text: r.rule_text, confidence: r.confidence,
                source: r.source, signal_count: r.signal_count,
                active: r.active, created_at: r.created_at,
                updated_at: r.updated_at,
            }).collect();
            ok(resp)
        }
        "cognitive_memory_stats" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
            let rule_repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());
            let model = cognitive::repos::load_user_model(&fact_repo).await;
            let active_facts = model.active_fact_count();
            // TODO: add a count_archived() query to SemanticFactRepo —
            // archive_superseded() is a destructive mutation and must NOT be used for reads.
            let archived = 0u64;
            let mut episodic_count = 0;
            for d in cognitive::repos::USER_MODEL_DOMAINS {
                episodic_count += episodic_repo.list_by_domain(d, 10000).await.map(|v| v.len()).unwrap_or(0);
            }
            let mut rules_count = 0;
            for d in cognitive::repos::RULE_DOMAINS {
                rules_count += rule_repo.list_active(d).await.map(|v| v.len()).unwrap_or(0);
            }
            ok(MemoryStatsResponse {
                active_facts,
                archived_facts: archived,
                episodic_count,
                rules_count,
                last_compaction: None,
            })
        }
        "cognitive_system_status" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let model = cognitive::repos::load_user_model(&fact_repo).await;
            let active_facts = model.active_fact_count();

            let components = vec![
                ComponentStatusResponse { name: "DomainEvent Bus".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "tokio::broadcast, 256 capacity".into() },
                ComponentStatusResponse { name: "Salience Filter".into(), status: "wired".into(), handler_type: "heuristic".into(), notes: "Rule-based classification".into() },
                ComponentStatusResponse { name: "Extraction".into(), status: "wired".into(), handler_type: "heuristic".into(), notes: "HeuristicExtractionHandler in agent crate".into() },
                ComponentStatusResponse { name: "Consolidation".into(), status: "wired".into(), handler_type: "heuristic".into(), notes: "HeuristicConsolidationHandler in agent crate".into() },
                ComponentStatusResponse { name: "FSRS Decay".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "R = exp(ln(0.9) * elapsed / stability)".into() },
                ComponentStatusResponse { name: "Compaction".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "Archive superseded, prune episodic, size budget".into() },
                ComponentStatusResponse { name: "Reflection".into(), status: "built".into(), handler_type: "heuristic".into(), notes: "ReflectionHandler trait defined, needs scheduling".into() },
                ComponentStatusResponse { name: "Context Source".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "CognitiveContextSource at priority 60, 60s cache".into() },
                ComponentStatusResponse { name: "UserSituation".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "compute_situation() from SituationInputs".into() },
                ComponentStatusResponse { name: "Signal Accumulator".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "30min rolling window, 7 trigger conditions".into() },
                ComponentStatusResponse { name: "Pattern Detector".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "5 pattern types detected".into() },
                ComponentStatusResponse { name: "Intervention Router".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "Rate limits: 3/hr, 10/day, exponential backoff".into() },
                ComponentStatusResponse { name: "Feedback Tracker".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "3 channels: explicit, behavioral, outcome".into() },
                ComponentStatusResponse { name: "LLM Extraction".into(), status: "stub".into(), handler_type: "llm".into(), notes: "Trait defined, not yet implemented".into() },
                ComponentStatusResponse { name: "LLM Consolidation".into(), status: "stub".into(), handler_type: "llm".into(), notes: "Trait defined, not yet implemented".into() },
                ComponentStatusResponse { name: "LLM Reflection".into(), status: "stub".into(), handler_type: "llm".into(), notes: "ReflectionHandler trait, not yet implemented".into() },
                ComponentStatusResponse { name: "LLM Coaching Reasoner".into(), status: "stub".into(), handler_type: "llm".into(), notes: "CoachingReasonerHandler trait, heuristic fallback exists".into() },
                ComponentStatusResponse { name: "Background Consolidation".into(), status: "wired".into(), handler_type: "n/a".into(), notes: "DomainEventBus subscriber, accumulation buffers".into() },
            ];

            ok(SystemStatusResponse {
                domain_bus_subscribers: 0,
                domain_bus_published: 0,
                background_service_running: false,
                background_events_processed: 0,
                active_facts,
                episodic_count: 0,
                rules_count: 0,
                components,
            })
        }
        "coaching_situation" => {
            ok(UserSituationResponse {
                energy_level: 0.0, focus_state: 0.0,
                deadline_pressure: 0.0, distraction_risk: 0.0,
                coaching_receptivity: 0.0, task_avoidance_detected: false,
                hours_active_today: 0.0, mins_since_break: 0.0,
                hour_of_day: Local::now().hour(),
                recent_context_switches: 0,
            })
        }
        "coaching_signals" => {
            ok(SignalWindowResponse { window_size: 0, signals: vec![], triggers: vec![] })
        }
        "coaching_patterns" => { ok(Vec::<DetectedPatternResponse>::new()) }
        "coaching_feedback_stats" => { ok(Vec::<StrategyFeedbackResponse>::new()) }
        "coaching_router_status" => {
            ok(RouterStatusResponse {
                hourly_count: 0, hourly_limit: 3,
                daily_count: 0, daily_limit: 10,
            })
        }
        "cognitive_fact_create" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let params: FactCreateParams = match serde_json::from_value(body.clone()) {
                Ok(p) => p,
                Err(e) => return err(ApiError::new("INVALID_PARAMS", e.to_string())),
            };
            let now = Utc::now().to_rfc3339();
            let fact = cognitive::types::SemanticFact {
                id: uuid::Uuid::new_v4().to_string(),
                domain: params.domain,
                subject: params.subject,
                predicate: params.predicate,
                object: params.object,
                confidence: params.confidence,
                source: "debug_dashboard".to_string(),
                valid_from: now.clone(),
                valid_until: None,
                recorded_at: now,
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
            };
            match fact_repo.upsert(&fact).await {
                Ok(()) => {
                    let elapsed_days = 0.0_f64;
                    let r = cognitive::decay::retrievability(elapsed_days, fact.stability);
                    ok(SemanticFactResponse {
                        id: fact.id, domain: fact.domain, subject: fact.subject,
                        predicate: fact.predicate, object: fact.object,
                        confidence: fact.confidence, source: fact.source,
                        valid_from: fact.valid_from, valid_until: None,
                        stability: fact.stability, retrievability: r,
                        last_accessed: None, access_count: 0, status: "active".into(),
                    })
                }
                Err(e) => err(ApiError::new("STORAGE_ERROR", e.to_string())),
            }
        }
        "cognitive_fact_update" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let id = match get_str(&body, "id") { Ok(v) => v, Err(e) => return err(e) };
            let params: FactUpdateParams = match serde_json::from_value(body.clone()) {
                Ok(p) => p,
                Err(e) => return err(ApiError::new("INVALID_PARAMS", e.to_string())),
            };
            match fact_repo.get(&id).await {
                Ok(Some(mut fact)) => {
                    if let Some(obj) = params.object { fact.object = obj; }
                    if let Some(conf) = params.confidence { fact.confidence = conf; }
                    match fact_repo.upsert(&fact).await {
                        Ok(()) => ok(serde_json::json!({ "ok": true })),
                        Err(e) => err(ApiError::new("STORAGE_ERROR", e.to_string())),
                    }
                }
                Ok(None) => err(ApiError::new("NOT_FOUND", format!("fact {id} not found"))),
                Err(e) => err(ApiError::new("STORAGE_ERROR", e.to_string())),
            }
        }
        "cognitive_fact_delete" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let id = match get_str(&body, "id") { Ok(v) => v, Err(e) => return err(e) };
            match fact_repo.supersede(&id, "deleted_by_debug_dashboard").await {
                Ok(()) => ok(true),
                Err(e) => err(ApiError::new("STORAGE_ERROR", e.to_string())),
            }
        }
        "cognitive_rule_create" => {
            let pool = core.repos.pool();
            let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());
            let params: RuleCreateParams = match serde_json::from_value(body.clone()) {
                Ok(p) => p,
                Err(e) => return err(ApiError::new("INVALID_PARAMS", e.to_string())),
            };
            let now = Utc::now().to_rfc3339();
            let rule = cognitive::types::ProceduralRule {
                id: uuid::Uuid::new_v4().to_string(),
                domain: params.domain,
                rule_text: params.rule_text,
                confidence: params.confidence,
                source: "debug_dashboard".to_string(),
                signal_count: 0,
                created_at: now.clone(),
                updated_at: now,
                active: true,
            };
            match repo.upsert(&rule).await {
                Ok(()) => ok(ProceduralRuleResponse {
                    id: rule.id, domain: rule.domain, rule_text: rule.rule_text,
                    confidence: rule.confidence, source: rule.source,
                    signal_count: rule.signal_count, active: true,
                    created_at: rule.created_at, updated_at: rule.updated_at,
                }),
                Err(e) => err(ApiError::new("STORAGE_ERROR", e.to_string())),
            }
        }
        "cognitive_rule_deactivate" => {
            let pool = core.repos.pool();
            let repo = cognitive::repos::ProceduralRuleRepo::new(pool.clone());
            let id = match get_str(&body, "id") { Ok(v) => v, Err(e) => return err(e) };
            match repo.deactivate(&id).await {
                Ok(()) => ok(true),
                Err(e) => err(ApiError::new("STORAGE_ERROR", e.to_string())),
            }
        }
        "cognitive_run_compaction" => {
            let pool = core.repos.pool();
            let fact_repo = cognitive::repos::SemanticFactRepo::new(pool.clone());
            let episodic_repo = cognitive::repos::EpisodicMemoryRepo::new(pool.clone());
            let (archived, deleted_episodic) = tokio::join!(
                async { fact_repo.archive_superseded(90).await.unwrap_or(0) },
                async { episodic_repo.delete_old(90, 2).await.unwrap_or(0) },
            );
            ok(CompactionResultResponse { archived_count: archived, deleted_episodic })
        }
        "coaching_reset_dismissals" | "coaching_clear_signals" => {
            ok(serde_json::json!({ "ok": true }))
        }

        _ => err(ApiError::new(
            "NOT_FOUND",
            format!("command '{cmd}' not supported in browser dev mode"),
        )),
    }
}
