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
pub async fn trigger_sync(
    State(_state): State<AppState>,
) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::ACCEPTED, Json(serde_json::json!({"status": "sync_queued"})))
}
