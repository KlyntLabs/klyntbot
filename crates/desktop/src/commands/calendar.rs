use std::sync::Arc;
use desktop_shared::commands::CalendarEventResponse;
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn calendar_events(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<CalendarEventResponse>, ApiError> {
    let rows = state
        .repos
        .calendar_event_cache
        .list_upcoming(limit.unwrap_or(10))
        .await
        .map_err(super::map_storage_err)?;

    Ok(rows
        .iter()
        .map(|r| CalendarEventResponse {
            id: r.uid.clone(),
            title: r.summary.clone(),
            start_at: r.start_at,
            end_at: r.end_at,
            color: "#3B82F6".to_string(),
        })
        .collect())
}
