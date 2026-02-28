//! Cron job CRUD handlers.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::api::deleted_or_not_found;
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCronRequest {
    name: String,
    schedule: serde_json::Value,
    payload: Option<serde_json::Value>,
    delete_after_run: Option<bool>,
}

#[derive(Deserialize)]
pub struct ToggleRequest {
    enabled: bool,
}

/// GET /api/cron — list all cron jobs.
pub async fn list_cron(
    State(state): State<AppState>,
) -> Result<Json<Vec<storage::rows::cron::CronJobRow>>, ApiError> {
    let rows = state.repos.cron.list().await.map_err(ApiError::from)?;
    Ok(Json(rows))
}

/// POST /api/cron — create a new cron job.
pub async fn create_cron(
    State(state): State<AppState>,
    Json(req): Json<CreateCronRequest>,
) -> Result<Json<storage::rows::cron::CronJobRow>, ApiError> {
    if req.schedule.is_null() {
        return Err(ApiError::bad_request("schedule is required"));
    }

    let now_ms = chrono::Utc::now().timestamp_millis();
    let row = storage::rows::cron::CronJobRow {
        id: uuid::Uuid::new_v4().to_string(),
        name: req.name,
        enabled: true,
        schedule: req.schedule,
        payload: req.payload.unwrap_or(serde_json::Value::Null),
        next_run_at_ms: None,
        last_run_at_ms: None,
        last_status: None,
        last_error: None,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
        delete_after_run: req.delete_after_run.unwrap_or(false),
    };

    let created = state
        .repos
        .cron
        .upsert(&row)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(created))
}

/// PATCH /api/cron/:id/toggle — enable or disable a cron job.
pub async fn toggle_cron(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<ToggleRequest>,
) -> Result<Json<storage::rows::cron::CronJobRow>, ApiError> {
    state
        .repos
        .cron
        .set_enabled(&id, req.enabled)
        .await
        .map_err(ApiError::from)?;
    let updated = state.repos.cron.get(&id).await.map_err(ApiError::from)?;
    Ok(Json(updated))
}

/// POST /api/cron/:id/run — manually trigger a cron job immediately.
///
/// Returns 202 Accepted and executes the job in the background so the
/// response isn't blocked by `block_in_place` inside the cron callback.
pub async fn run_cron(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    // Verify the job exists (fast, read-only check via DB).
    let exists = state.repos.cron.get(&id).await.is_ok();
    if !exists {
        return Err(ApiError::not_found(format!("cron job '{id}' not found")));
    }

    // Spawn execution in background so we don't block the HTTP response.
    let svc = state.cron_service.clone();
    tokio::spawn(async move {
        if let Err(e) = svc.run_job(&id, true).await {
            tracing::error!("run_cron background: {e}");
        }
    });

    Ok(StatusCode::ACCEPTED)
}

/// DELETE /api/cron/:id — delete a cron job.
pub async fn delete_cron(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state.repos.cron.delete(&id).await.map_err(ApiError::from)?;
    deleted_or_not_found(deleted, "cron job", &id)
}
