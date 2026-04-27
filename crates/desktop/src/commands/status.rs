use desktop_shared::commands::AgentStatusResponse;
use desktop_shared::CommandResult;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn agent_status(state: State<'_, Arc<AppCore>>) -> CommandResult<AgentStatusResponse> {
    state.agent_status().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["agent_status"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    _body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "agent_status" => dev::val(core.agent_status().await),
        _ => return None,
    })
}
