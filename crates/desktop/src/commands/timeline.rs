use std::sync::Arc;

use desktop_shared::commands::{TimelineQuery, TimelineResponse, TimelineSource};
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn timeline_query(
    state: State<'_, Arc<AppCore>>,
    start_date: String,
    end_date: String,
    sources: Option<Vec<TimelineSource>>,
    include_point_events: Option<bool>,
) -> Result<TimelineResponse, ApiError> {
    state
        .timeline_query(TimelineQuery {
            start_date,
            end_date,
            sources,
            include_point_events,
        })
        .await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["timeline_query"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "timeline_query" => dev::val(
            core.timeline_query(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
