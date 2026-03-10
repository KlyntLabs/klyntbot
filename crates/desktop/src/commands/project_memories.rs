use desktop_shared::cognitive_commands::SemanticFactResponse;
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn project_memories_list(
    state: State<'_, Arc<AppCore>>,
    project_id: String,
) -> Result<Vec<SemanticFactResponse>, ApiError> {
    state.project_memories_list(project_id).await
}

#[tauri::command]
pub async fn project_memories_by_type(
    state: State<'_, Arc<AppCore>>,
    project_id: String,
    memory_type: String,
) -> Result<Vec<SemanticFactResponse>, ApiError> {
    state
        .project_memories_by_type(project_id, memory_type)
        .await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["project_memories_list", "project_memories_by_type"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
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
