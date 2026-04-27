use desktop_shared::entity_link_types::SessionSummaryResponse;
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn project_conversations_list(
    state: State<'_, Arc<AppCore>>,
    project_id: String,
) -> Result<Vec<SessionSummaryResponse>, ApiError> {
    state.project_conversations_list(project_id).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &["project_conversations_list"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
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
