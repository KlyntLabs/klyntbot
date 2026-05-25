use app_core::journey::Milestone;
use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

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
