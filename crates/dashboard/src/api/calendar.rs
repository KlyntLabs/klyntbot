//! Calendar API handlers.

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventParams {
    pub provider_id: Option<String>,
    pub limit: Option<i64>,
}

/// GET /api/calendar/events — list calendar events.
pub async fn list_events(
    State(state): State<AppState>,
    Query(params): Query<CalendarEventParams>,
) -> Result<Json<Vec<storage::rows::calendar::CalendarEventCacheRow>>, ApiError> {
    let rows = if let Some(provider_id) = params.provider_id {
        state
            .repos
            .calendar_event_cache
            .list_by_provider(&provider_id)
            .await
            .map_err(ApiError::from)?
    } else {
        let limit = params.limit.unwrap_or(50);
        state
            .repos
            .calendar_event_cache
            .list_upcoming(limit)
            .await
            .map_err(ApiError::from)?
    };
    Ok(Json(rows))
}

/// GET /api/calendar/sync-status — get sync state for all providers.
pub async fn get_sync_status(
    State(state): State<AppState>,
) -> Result<Json<Vec<storage::rows::calendar::CalendarSyncStateRow>>, ApiError> {
    let rows = state
        .repos
        .calendar_sync
        .list()
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

/// POST /api/calendar/sync — trigger a calendar sync (queued).
pub async fn trigger_sync(State(_state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({"status": "sync_queued"})),
    )
}

/// Body for `POST /api/calendar/events`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEventRequest {
    pub summary: String,
    pub description: Option<String>,
    pub start: String,
    pub end: String,
}

/// POST /api/calendar/events — create a calendar event in the local cache.
pub async fn create_event(
    State(state): State<AppState>,
    Json(req): Json<CreateEventRequest>,
) -> Result<
    (
        StatusCode,
        Json<storage::rows::calendar::CalendarEventCacheRow>,
    ),
    ApiError,
> {
    if req.summary.trim().is_empty() {
        return Err(ApiError::unprocessable("summary must not be empty"));
    }

    let uid = format!("dashboard-{}", uuid::Uuid::new_v4());
    let start_at: chrono::DateTime<chrono::Utc> = req
        .start
        .parse()
        .map_err(|_| ApiError::unprocessable("invalid start datetime (expected RFC3339)"))?;
    let end_at: chrono::DateTime<chrono::Utc> = req
        .end
        .parse()
        .map_err(|_| ApiError::unprocessable("invalid end datetime (expected RFC3339)"))?;

    let row = state
        .repos
        .calendar_event_cache
        .upsert(
            &uid,
            "local",
            req.summary.trim(),
            req.description.as_deref(),
            start_at,
            end_at,
            "dashboard",
            None,
            Some("CONFIRMED"),
        )
        .await
        .map_err(ApiError::from)?;

    Ok((StatusCode::CREATED, Json(row)))
}
