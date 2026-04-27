use desktop_shared::commands::{TimelineQuery, TimelineResponse, TimelineSource};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

use desktop_macros::klynt_command;

#[klynt_command]
pub async fn timeline_query(
    start_date: String,
    end_date: String,
    sources: Option<Vec<TimelineSource>>,
    include_point_events: Option<bool>,
    tz_offset_mins: Option<i32>,
) -> TimelineResponse {
    state
        .timeline_query(TimelineQuery {
            start_date,
            end_date,
            sources,
            include_point_events,
            tz_offset_mins,
        })
        .await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "timeline_query" => dev::val(
            core.timeline_query(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
