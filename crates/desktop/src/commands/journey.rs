use std::sync::Arc;

use app_core::journey::Milestone;
use desktop_shared::{errors::ApiError, CommandResult};
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn journey_milestones(state: State<'_, Arc<AppCore>>) -> CommandResult<Vec<String>> {
    let tracker = state
        .journey_tracker
        .as_ref()
        .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Journey tracker not available"))?;
    Ok(tracker.completed_names().await)
}

#[tauri::command]
#[specta::specta]
pub async fn journey_mark_complete(
    state: State<'_, Arc<AppCore>>,
    milestone: String,
) -> CommandResult<()> {
    let tracker = state
        .journey_tracker
        .as_ref()
        .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Journey tracker not available"))?;
    let m = Milestone::from_name(&milestone)
        .ok_or_else(|| ApiError::new("VALIDATION", format!("unknown milestone: {milestone}")))?;
    tracker.mark_complete(m).await;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn journey_item_count(state: State<'_, Arc<AppCore>>) -> CommandResult<i64> {
    if let Some(ref tracker) = state.journey_tracker {
        Ok(tracker.total_item_count().await)
    } else {
        Ok(0)
    }
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "journey_milestones",
    "journey_mark_complete",
    "journey_item_count",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    use app_core::journey::Milestone;

    let tracker = match core.journey_tracker.as_ref() {
        Some(t) => t,
        None => {
            return Some(Err(ApiError::new(
                "NOT_AVAILABLE",
                "Journey tracker not available",
            )))
        }
    };

    Some(match cmd {
        "journey_milestones" => dev::val(Ok(tracker.completed_names().await)),
        "journey_mark_complete" => {
            let name: String = try_field!(dev::get_str(body, "milestone"));
            let m = try_field!(Milestone::from_name(&name)
                .ok_or_else(|| ApiError::new("VALIDATION", format!("unknown milestone: {name}"))));
            tracker.mark_complete(m).await;
            dev::val(Ok(()))
        }
        "journey_item_count" => dev::val(Ok(tracker.total_item_count().await)),
        _ => return None,
    })
}
