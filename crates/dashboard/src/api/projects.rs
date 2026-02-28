//! Handlers for `/api/projects` — CRUD with optional stats aggregation.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use storage::{ProjectFilter, ProjectPatch, ProjectRow, ProjectWithStats};

use crate::api::{deleted_or_not_found, parse_comma_tags};
use crate::error::ApiError;
use crate::state::AppState;

// ── Request / Response types ─────────────────────────────────────────────────

/// Query params accepted by `GET /api/projects`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectQueryParams {
    pub status: Option<String>,
    pub tags: Option<String>, // comma-separated
    pub limit: Option<i64>,
}

/// Body for `POST /api/projects`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
    pub color: Option<String>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
}

/// Query params for `GET /api/projects/:id`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectGetParams {
    pub with_stats: Option<bool>,
}

/// Body for `PATCH /api/projects/:id`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchProjectRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub color: Option<String>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
}

/// Response envelope for `GET /api/projects/:id` — either stats or plain row.
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ProjectGetResponse {
    WithStats(ProjectWithStats),
    Plain(ProjectRow),
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn validate_create(req: &CreateProjectRequest) -> Result<(), ApiError> {
    if req.name.trim().is_empty() {
        return Err(ApiError::unprocessable("name must not be empty"));
    }
    Ok(())
}

fn validate_patch(req: &PatchProjectRequest) -> Result<(), ApiError> {
    if let Some(ref name) = req.name {
        if name.trim().is_empty() {
            return Err(ApiError::unprocessable("name must not be empty"));
        }
    }
    Ok(())
}

// ── GET /api/projects ─────────────────────────────────────────────────────────

pub async fn list_projects(
    State(state): State<AppState>,
    Query(params): Query<ProjectQueryParams>,
) -> Result<Json<Vec<ProjectRow>>, ApiError> {
    let tags = params.tags.as_deref().map(parse_comma_tags);

    let filter = ProjectFilter {
        status: params.status,
        tags,
        limit: params.limit,
    };

    let rows = state
        .repos
        .projects
        .list(&filter)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

// ── POST /api/projects ────────────────────────────────────────────────────────

pub async fn create_project(
    State(state): State<AppState>,
    Json(req): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectRow>), ApiError> {
    validate_create(&req)?;

    let now = Utc::now();
    let row = ProjectRow {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name.trim().to_string(),
        description: req.description,
        color: req.color.unwrap_or_else(|| "#4f46e5".to_string()),
        tags: req.tags.unwrap_or_default(),
        status: req.status.unwrap_or_else(|| "active".to_string()),
        created_at: now,
        updated_at: now,
    };

    let inserted = state
        .repos
        .projects
        .create(&row)
        .await
        .map_err(ApiError::from)?;
    Ok((StatusCode::CREATED, Json(inserted)))
}

// ── GET /api/projects/:id ─────────────────────────────────────────────────────

pub async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ProjectGetParams>,
) -> Result<Json<ProjectGetResponse>, ApiError> {
    if params.with_stats.unwrap_or(false) {
        let result = state
            .repos
            .projects
            .get_with_stats(&id)
            .await
            .map_err(ApiError::from)?;

        match result {
            Some(stats) => Ok(Json(ProjectGetResponse::WithStats(stats))),
            None => Err(ApiError::not_found(format!("project '{id}' not found"))),
        }
    } else {
        let row = state
            .repos
            .projects
            .get_or_err(&id)
            .await
            .map_err(ApiError::from)?;
        Ok(Json(ProjectGetResponse::Plain(row)))
    }
}

// ── PATCH /api/projects/:id ───────────────────────────────────────────────────

pub async fn patch_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PatchProjectRequest>,
) -> Result<Json<ProjectRow>, ApiError> {
    validate_patch(&req)?;

    // Verify it exists before patching.
    state
        .repos
        .projects
        .get_or_err(&id)
        .await
        .map_err(ApiError::from)?;

    let patch = ProjectPatch {
        id: id.clone(),
        name: req.name,
        description: req.description,
        color: req.color,
        tags: req.tags,
        status: req.status,
    };

    let updated = state
        .repos
        .projects
        .update(&patch)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(updated))
}

// ── DELETE /api/projects/:id ──────────────────────────────────────────────────

pub async fn delete_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .repos
        .projects
        .delete(&id)
        .await
        .map_err(ApiError::from)?;
    deleted_or_not_found(deleted, "project", &id)
}
