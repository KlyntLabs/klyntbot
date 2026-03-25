use desktop_shared::commands::MorningBriefingResponse;
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn morning_briefing_summary(
    state: State<'_, Arc<AppCore>>,
) -> Result<MorningBriefingResponse, ApiError> {
    state.morning_briefing().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["morning_briefing_summary"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    _body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "morning_briefing_summary" => dev::val(core.morning_briefing().await),
        _ => return None,
    })
}
