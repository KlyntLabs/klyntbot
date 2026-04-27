use desktop_shared::commands::AgentStatusResponse;
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn agent_status() -> AgentStatusResponse {
    state.agent_status().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &crate::app_core::AppCore,
    _body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "agent_status" => dev::val(core.agent_status().await),
        _ => return None,
    })
}
