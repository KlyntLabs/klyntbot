//! Handlers for `/api/plans` — CRUD and status lifecycle transitions.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use storage::{PlanRow, PlanStepRow};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

// ── Request / Response types ─────────────────────────────────────────────────

/// Query params accepted by `GET /api/plans`.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct PlanQueryParams {
    pub status: Option<String>,
    pub session_key: Option<String>,
    pub goal_id: Option<Uuid>,
}

/// Body for `POST /api/plans`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatePlanRequest {
    pub title: String,
    pub description: Option<String>,
    pub session_key: Option<String>,
    pub goal_id: Option<Uuid>,
    pub iteration_limit: Option<i32>,
}

/// Body for `PATCH /api/plans/:id`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchPlanRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub iteration_limit: Option<i32>,
}

/// Body for `PATCH /api/plans/:id/status`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatusRequest {
    pub status: String,
}

/// Response for `GET /api/plans/:id` — plan row with its steps.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanWithSteps {
    pub plan: PlanRow,
    pub steps: Vec<PlanStepRow>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn storage_err(e: storage::StorageError) -> ApiError {
    ApiError::from(common::KlyntbotError::from(e))
}

fn validate_create(req: &CreatePlanRequest) -> Result<(), ApiError> {
    if req.title.trim().is_empty() {
        return Err(ApiError::unprocessable("title must not be empty"));
    }
    Ok(())
}

// ── GET /api/plans ────────────────────────────────────────────────────────────

pub async fn list_plans(
    State(state): State<AppState>,
    Query(params): Query<PlanQueryParams>,
) -> Result<Json<Vec<PlanRow>>, ApiError> {
    let rows = state
        .repos
        .plans
        .list(
            params.status.as_deref(),
            params.session_key.as_deref(),
            params.goal_id,
        )
        .await
        .map_err(storage_err)?;
    Ok(Json(rows))
}

// ── POST /api/plans ───────────────────────────────────────────────────────────

pub async fn create_plan(
    State(state): State<AppState>,
    Json(req): Json<CreatePlanRequest>,
) -> Result<(StatusCode, Json<PlanRow>), ApiError> {
    validate_create(&req)?;

    let now = Utc::now();
    let row = PlanRow {
        id: Uuid::new_v4(),
        session_key: req.session_key.unwrap_or_default(),
        goal_id: req.goal_id,
        title: req.title.trim().to_string(),
        description: req.description.unwrap_or_default(),
        status: "draft".to_string(),
        current_step_index: 0,
        iteration_limit: req.iteration_limit.unwrap_or(20),
        backtrack_history: serde_json::json!([]),
        created_at: now,
        updated_at: now,
        completed_at: None,
    };

    let inserted = state.repos.plans.create(&row).await.map_err(storage_err)?;
    Ok((StatusCode::CREATED, Json(inserted)))
}

// ── GET /api/plans/:id ────────────────────────────────────────────────────────

pub async fn get_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PlanWithSteps>, ApiError> {
    let plan = state.repos.plans.get(id).await.map_err(storage_err)?;
    let steps = state
        .repos
        .plans
        .get_steps(id)
        .await
        .map_err(storage_err)?;
    Ok(Json(PlanWithSteps { plan, steps }))
}

// ── PATCH /api/plans/:id ──────────────────────────────────────────────────────

pub async fn patch_plan(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<PatchPlanRequest>,
) -> Result<Json<PlanRow>, ApiError> {
    if let Some(ref title) = req.title {
        if title.trim().is_empty() {
            return Err(ApiError::unprocessable("title must not be empty"));
        }
    }

    let mut row = state.repos.plans.get(id).await.map_err(storage_err)?;

    if let Some(title) = req.title {
        row.title = title.trim().to_string();
    }
    if let Some(description) = req.description {
        row.description = description;
    }
    if let Some(limit) = req.iteration_limit {
        row.iteration_limit = limit;
    }
    row.updated_at = Utc::now();

    let updated = state.repos.plans.update(&row).await.map_err(storage_err)?;
    Ok(Json(updated))
}

// ── GET /api/plans/:id/steps ──────────────────────────────────────────────────

pub async fn get_plan_steps(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<PlanStepRow>>, ApiError> {
    // Confirm the plan exists — get() returns NotFound if absent.
    state.repos.plans.get(id).await.map_err(storage_err)?;

    let steps = state
        .repos
        .plans
        .get_steps(id)
        .await
        .map_err(storage_err)?;
    Ok(Json(steps))
}

// ── PATCH /api/plans/:id/status ───────────────────────────────────────────────

pub async fn update_plan_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateStatusRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let plan = state.repos.plans.get(id).await.map_err(storage_err)?;

    let from = plan::conversions::str_to_plan_status(&plan.status);
    let to = plan::conversions::str_to_plan_status(&req.status);

    plan::PlanStatus::validate_transition(&from, &to).map_err(|_| ApiError {
        status: 409,
        message: format!(
            "Invalid status transition: '{}' → '{}'",
            plan.status, req.status
        ),
    })?;

    state
        .repos
        .plans
        .update_status(id, &req.status)
        .await
        .map_err(storage_err)?;

    Ok(Json(serde_json::json!({ "status": req.status })))
}
