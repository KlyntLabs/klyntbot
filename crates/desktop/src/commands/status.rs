use desktop_shared::commands::AgentStatusResponse;
use tauri::State;

use crate::app_core::AppCore;
use super::tasks::action_to_task;

#[tauri::command]
pub async fn agent_status(state: State<'_, AppCore>) -> Result<AgentStatusResponse, String> {
    let focused = state
        .repos
        .actions
        .list_focused()
        .await
        .map_err(|e| e.to_string())?;

    let summary = state
        .repos
        .actions
        .summary()
        .await
        .map_err(|e| e.to_string())?;

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
