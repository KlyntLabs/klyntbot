//! Handlers for `/api/tasks` — full CRUD, subtasks, attachments, time entries, and focus slots.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use storage::{TodoFilter, TodoPatch, TodoRow};

use crate::error::ApiError;
use crate::state::AppState;

// ── Request / Response types ─────────────────────────────────────────────────

/// Query params accepted by `GET /api/tasks`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct TaskQueryParams {
    pub status: Option<String>,
    pub project_id: Option<String>,
    pub priority_min: Option<i16>,
    pub limit: Option<i64>,
    pub tags: Option<String>, // comma-separated
    pub templates_only: Option<bool>,
}

/// Body for `POST /api/tasks`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub parent_id: Option<String>,
    pub project_id: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub is_template: Option<bool>,
    pub recurrence_rule: Option<String>,
}

/// Body for `PATCH /api/tasks/:id`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchTaskRequest {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Option<i16>>,
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub estimated_minutes: Option<Option<i32>>,
    pub recurrence_rule: Option<Option<String>>,
}

/// Body for `POST /api/tasks/:id/time-entries`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddTimeEntryRequest {
    pub source: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub note: Option<String>,
}

/// Query params for `POST /api/tasks/:id/focus`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FocusParams {
    pub max_slots: Option<i64>,
    pub deadline: Option<DateTime<Utc>>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn storage_err(e: storage::StorageError) -> ApiError {
    ApiError::from(common::KlyntbotError::from(e))
}

fn validate_create(req: &CreateTaskRequest) -> Result<(), ApiError> {
    if req.title.trim().is_empty() {
        return Err(ApiError::unprocessable("title must not be empty"));
    }
    if let Some(p) = req.priority {
        if !(1..=5).contains(&p) {
            return Err(ApiError::unprocessable("priority must be between 1 and 5"));
        }
    }
    Ok(())
}

fn validate_patch(req: &PatchTaskRequest) -> Result<(), ApiError> {
    if let Some(ref title) = req.title {
        if title.trim().is_empty() {
            return Err(ApiError::unprocessable("title must not be empty"));
        }
    }
    if let Some(Some(p)) = req.priority {
        if !(1..=5).contains(&p) {
            return Err(ApiError::unprocessable("priority must be between 1 and 5"));
        }
    }
    Ok(())
}

// ── GET /api/tasks ────────────────────────────────────────────────────────────

pub async fn list_tasks(
    State(state): State<AppState>,
    Query(params): Query<TaskQueryParams>,
) -> Result<Json<Vec<TodoRow>>, ApiError> {
    let tags = params.tags.as_deref().map(|s| {
        s.split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect::<Vec<_>>()
    });

    let filter = TodoFilter {
        status: params.status,
        tags,
        project_id: params.project_id,
        priority_min: params.priority_min,
        limit: params.limit,
        templates_only: params.templates_only.unwrap_or(false),
    };

    let rows = state.repos.todos.list(&filter).await.map_err(storage_err)?;
    Ok(Json(rows))
}

// ── POST /api/tasks ───────────────────────────────────────────────────────────

pub async fn create_task(
    State(state): State<AppState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TodoRow>), ApiError> {
    validate_create(&req)?;

    let now = Utc::now();
    let row = TodoRow {
        id: uuid::Uuid::new_v4().to_string(),
        title: req.title.trim().to_string(),
        description: req.description,
        priority: req.priority,
        due_date: req.due_date,
        tags: req.tags.unwrap_or_default(),
        status: req.status.unwrap_or_else(|| "todo".to_string()),
        focused_at: None,
        focus_deadline: None,
        focus_expired_count: 0,
        created_at: now,
        updated_at: now,
        completed_at: None,
        parent_id: req.parent_id,
        project_id: req.project_id,
        total_tracked_secs: 0,
        estimated_minutes: req.estimated_minutes,
        calendar_event_uid: None,
        last_reminded_at: None,
        recurrence_rule: req.recurrence_rule.filter(|s| !s.is_empty()),
        recurrence_parent_id: None,
        is_template: req.is_template.unwrap_or(false),
        next_instance_date: None,
    };

    let inserted = state.repos.todos.add(&row).await.map_err(storage_err)?;
    Ok((StatusCode::CREATED, Json(inserted)))
}

// ── GET /api/tasks/summary ────────────────────────────────────────────────────

pub async fn get_summary(
    State(state): State<AppState>,
) -> Result<Json<storage::TodoSummary>, ApiError> {
    let summary = state.repos.todos.summary().await.map_err(storage_err)?;
    Ok(Json(summary))
}

// ── GET /api/tasks/:id ────────────────────────────────────────────────────────

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TodoRow>, ApiError> {
    let row = state
        .repos
        .todos
        .get_or_err(&id)
        .await
        .map_err(storage_err)?;
    Ok(Json(row))
}

// ── PATCH /api/tasks/:id ──────────────────────────────────────────────────────

pub async fn patch_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchTaskRequest>,
) -> Result<Json<TodoRow>, ApiError> {
    validate_patch(&req)?;

    let patch = TodoPatch {
        id: id.clone(),
        title: req.title,
        description: req.description,
        priority: req.priority,
        due_date: req.due_date,
        tags: req.tags,
        status: req.status,
        calendar_event_uid: None,
        next_instance_date: None,
        last_reminded_at: None,
        estimated_minutes: req.estimated_minutes,
        // Normalise empty string → None so the FE can clear the rule with "".
        recurrence_rule: req.recurrence_rule.map(|v| v.filter(|s| !s.is_empty())),
    };

    // Confirm the task exists first so we get a clean 404.
    state
        .repos
        .todos
        .get_or_err(&id)
        .await
        .map_err(storage_err)?;

    let updated = state
        .repos
        .todos
        .update(&patch)
        .await
        .map_err(storage_err)?;
    Ok(Json(updated))
}

// ── DELETE /api/tasks/:id ─────────────────────────────────────────────────────

pub async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .repos
        .todos
        .delete(&id)
        .await
        .map_err(storage_err)?;

    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!("task '{id}' not found")))
    }
}

// ── GET /api/tasks/:id/subtasks ───────────────────────────────────────────────

pub async fn get_subtasks(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<TodoRow>>, ApiError> {
    // Verify parent exists.
    state
        .repos
        .todos
        .get_or_err(&id)
        .await
        .map_err(storage_err)?;

    let children = state
        .repos
        .todos
        .get_children(&id)
        .await
        .map_err(storage_err)?;
    Ok(Json(children))
}

// ── GET /api/tasks/:id/attachments ───────────────────────────────────────────

pub async fn get_attachments(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<storage::TodoAttachmentRow>>, ApiError> {
    state
        .repos
        .todos
        .get_or_err(&id)
        .await
        .map_err(storage_err)?;

    let attachments = state
        .repos
        .todos
        .list_attachments(&id)
        .await
        .map_err(storage_err)?;
    Ok(Json(attachments))
}

// ── GET /api/tasks/:id/time-entries ──────────────────────────────────────────

pub async fn get_time_entries(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<storage::TodoTimeEntryRow>>, ApiError> {
    state
        .repos
        .todos
        .get_or_err(&id)
        .await
        .map_err(storage_err)?;

    let entries = state
        .repos
        .todos
        .list_time_entries(&id)
        .await
        .map_err(storage_err)?;
    Ok(Json(entries))
}

// ── POST /api/tasks/:id/time-entries ─────────────────────────────────────────

pub async fn add_time_entry(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AddTimeEntryRequest>,
) -> Result<(StatusCode, Json<storage::TodoTimeEntryRow>), ApiError> {
    state
        .repos
        .todos
        .get_or_err(&id)
        .await
        .map_err(storage_err)?;

    let source = req.source.as_deref().unwrap_or("manual");
    let started_at = req.started_at.unwrap_or_else(Utc::now);

    let entry = state
        .repos
        .todos
        .add_time_entry(&id, source, started_at, req.duration_secs, req.note.as_deref())
        .await
        .map_err(storage_err)?;

    Ok((StatusCode::CREATED, Json(entry)))
}

// ── POST /api/tasks/:id/focus ─────────────────────────────────────────────────

pub async fn set_focus(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<FocusParams>,
) -> Result<Json<serde_json::Value>, ApiError> {
    state
        .repos
        .todos
        .get_or_err(&id)
        .await
        .map_err(storage_err)?;

    let max_slots = params.max_slots.unwrap_or(3);
    let focused = state
        .repos
        .todos
        .focus(&id, max_slots, params.deadline)
        .await
        .map_err(storage_err)?;

    Ok(Json(serde_json::json!({ "focused": focused })))
}

// ── DELETE /api/tasks/:id/focus ───────────────────────────────────────────────

pub async fn delete_focus(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .repos
        .todos
        .get_or_err(&id)
        .await
        .map_err(storage_err)?;

    state
        .repos
        .todos
        .unfocus(&id)
        .await
        .map_err(storage_err)?;

    Ok(StatusCode::NO_CONTENT)
}

// ── Dependency types ────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddDependencyRequest {
    pub blocker_id: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependenciesResponse {
    pub blocked_by: Vec<TodoRow>,
    pub blocks: Vec<TodoRow>,
}

// ── GET /api/tasks/:id/dependencies ─────────────────────────────────────────

pub async fn get_task_dependencies(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DependenciesResponse>, ApiError> {
    state.repos.todos.get_or_err(&id).await.map_err(storage_err)?;

    let blocked_by = state.repos.todos.get_blockers(&id).await.map_err(storage_err)?;
    let blocks = state.repos.todos.get_blocking(&id).await.map_err(storage_err)?;

    Ok(Json(DependenciesResponse { blocked_by, blocks }))
}

// ── POST /api/tasks/:id/dependencies ────────────────────────────────────────

pub async fn add_task_dependency(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<AddDependencyRequest>,
) -> Result<StatusCode, ApiError> {
    state.repos.todos.get_or_err(&id).await.map_err(storage_err)?;
    state.repos.todos.get_or_err(&req.blocker_id).await.map_err(storage_err)?;

    state
        .repos
        .todos
        .add_dependency(&id, &req.blocker_id)
        .await
        .map_err(storage_err)?;

    Ok(StatusCode::CREATED)
}

// ── DELETE /api/tasks/:id/dependencies/:blocker_id ──────────────────────────

pub async fn remove_task_dependency(
    State(state): State<AppState>,
    Path((id, blocker_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    state.repos.todos.get_or_err(&id).await.map_err(storage_err)?;

    let removed = state
        .repos
        .todos
        .remove_dependency(&id, &blocker_id)
        .await
        .map_err(storage_err)?;

    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::not_found(format!(
            "dependency {id} -> {blocker_id} not found"
        )))
    }
}
