use desktop_shared::commands::AgentStatusResponse;
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn agent_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<AgentStatusResponse, ApiError> {
    state.agent_status().await
}
