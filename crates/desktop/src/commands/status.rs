use desktop_shared::commands::AgentStatusResponse;
use desktop_shared::errors::ApiError;
use tauri::State;

use super::tasks::action_to_task;
use crate::app_core::AppCore;

#[tauri::command]
pub async fn agent_status(state: State<'_, AppCore>) -> Result<AgentStatusResponse, ApiError> {
    let focused = state
        .repos
        .actions
        .list_focused()
        .await
        .map_err(super::map_storage_err)?;

    let summary = state
        .repos
        .actions
        .summary()
        .await
        .map_err(super::map_storage_err)?;

    let focus_task = focused.first().map(action_to_task);

    Ok(AgentStatusResponse {
        status: if focused.is_empty() {
            "idle".to_string()
        } else {
            "active".to_string()
        },
        active_task_count: summary.doing,
        focus_task,
    })
}
