use desktop_macros::klynt_command;
use desktop_shared::commands::ReviewStatsSummaryResponse;
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn review_stats_summary() -> ReviewStatsSummaryResponse {
    state.review_stats_summary().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

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
