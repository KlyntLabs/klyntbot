use desktop_shared::entity_link_types::SessionSummaryResponse;
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn project_conversations_list(
    project_id: String,
) -> Vec<SessionSummaryResponse> {
    state.project_conversations_list(project_id).await
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
        "project_conversations_list" => {
            let project_id = try_field!(
                dev::get_str(body, "projectId").or_else(|_| dev::get_str(body, "project_id"))
            );
            dev::val(core.project_conversations_list(project_id).await)
        }
        _ => return None,
    })
}
