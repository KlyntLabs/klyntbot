use desktop_shared::cognitive_commands::SemanticFactResponse;
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn project_memories_list(
    project_id: String,
) -> Vec<SemanticFactResponse> {
    state.project_memories_list(project_id).await
}

#[klynt_command]
pub async fn project_memories_by_type(
    project_id: String,
    memory_type: String,
) -> Vec<SemanticFactResponse> {
    state
        .project_memories_by_type(project_id, memory_type)
        .await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "project_memories_list" => {
            let project_id = dev::get_str(body, "projectId").ok()?;
            dev::val(core.project_memories_list(project_id).await)
        }
        "project_memories_by_type" => {
            let project_id = dev::get_str(body, "projectId").ok()?;
            let memory_type = dev::get_str(body, "memoryType").ok()?;
            dev::val(core.project_memories_by_type(project_id, memory_type).await)
        }
        _ => return None,
    })
}
