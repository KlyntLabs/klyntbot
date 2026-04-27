use desktop_shared::commands::MorningBriefingResponse;
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn morning_briefing_summary() -> MorningBriefingResponse {
    state.morning_briefing().await
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
        "morning_briefing_summary" => dev::val(core.morning_briefing().await),
        _ => return None,
    })
}
