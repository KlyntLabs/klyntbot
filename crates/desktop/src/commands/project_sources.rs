use desktop_shared::entity_link_types::*;
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn project_source_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: ProjectSourceCreateParams,
) -> Result<ProjectSourceResponse, ApiError> {
    let (result, updates) = state.project_source_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn project_source_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.project_source_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn project_source_list(
    state: State<'_, Arc<AppCore>>,
    project_id: String,
) -> Result<Vec<ProjectSourceResponse>, ApiError> {
    state.project_source_list(project_id).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "project_source_create",
    "project_source_delete",
    "project_source_list",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "project_source_create" => dev::val_rh(
            core.project_source_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "project_source_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.project_source_delete(id).await)
        }
        "project_source_list" => {
            let project_id = try_field!(dev::get_str(body, "projectId"));
            dev::val(core.project_source_list(project_id).await)
        }
        _ => return None,
    })
}
