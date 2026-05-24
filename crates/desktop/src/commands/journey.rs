use app_core::journey::Milestone;
use desktop_macros::klynt_command;
use desktop_shared::{errors::ApiError, CommandResult};

use crate::app_core::AppCore;

#[klynt_command]
pub async fn journey_milestones() -> Vec<String> {
    let tracker = state
        .journey_tracker()
        .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Journey tracker not available"))?;
    Ok(tracker.completed_names().await)
}

#[klynt_command]
pub async fn journey_mark_complete(milestone: String) -> () {
    let tracker = state
        .journey_tracker()
        .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Journey tracker not available"))?;
    let m = Milestone::from_name(&milestone)
        .ok_or_else(|| ApiError::new("VALIDATION", format!("unknown milestone: {milestone}")))?;
    tracker.mark_complete(m).await;
    Ok(())
}

#[klynt_command]
pub async fn journey_item_count() -> i64 {
    if let Some(tracker) = state.journey_tracker() {
        Ok(tracker.total_item_count().await)
    } else {
        Ok(0)
    }
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    use app_core::journey::Milestone;

    let tracker = match core.journey_tracker() {
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
