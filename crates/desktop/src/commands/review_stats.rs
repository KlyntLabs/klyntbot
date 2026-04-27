use desktop_shared::commands::ReviewStatsSummaryResponse;
use desktop_shared::CommandResult;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn review_stats_summary(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<ReviewStatsSummaryResponse> {
    state.review_stats_summary().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["review_stats_summary"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    _body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "review_stats_summary" => dev::val(core.review_stats_summary().await),
        _ => return None,
    })
}
