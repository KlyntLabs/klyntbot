use desktop_shared::commands::{ProjectCreateParams, ProjectResponse, ProjectUpdateParams};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn project_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: ProjectCreateParams,
) -> Result<ProjectResponse, ApiError> {
    let (result, updates) = state.project_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn project_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<ProjectResponse, ApiError> {
    state.project_get(id).await
}

#[tauri::command]
pub async fn project_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: ProjectUpdateParams,
) -> Result<ProjectResponse, ApiError> {
    let (result, updates) = state.project_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn project_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.project_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn project_archive(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<ProjectResponse, ApiError> {
    let (result, updates) = state.project_archive(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn project_update_instructions(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    instructions: serde_json::Value,
) -> Result<ProjectResponse, ApiError> {
    let (result, updates) = state.project_update_instructions(id, instructions).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn project_update_role(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    role: String,
) -> Result<ProjectResponse, ApiError> {
    let (result, updates) = state.project_update_role(id, role).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "project_create",
    "project_get",
    "project_update",
    "project_delete",
    "project_archive",
    "project_update_instructions",
    "project_update_role",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "project_create" => dev::val_rh(
            core.project_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "project_get" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.project_get(id).await)
        }
        "project_update" => dev::val_rh(
            core.project_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "project_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.project_delete(id).await)
        }
        "project_archive" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.project_archive(id).await)
        }
        "project_update_instructions" => {
            let id = try_field!(dev::get_str(body, "id"));
            let instructions: serde_json::Value = try_field!(dev::require(body, "instructions"));
            dev::val_rh(core.project_update_instructions(id, instructions).await)
        }
        "project_update_role" => {
            let id = try_field!(dev::get_str(body, "id"));
            let role = try_field!(dev::get_str(body, "role"));
            dev::val_rh(core.project_update_role(id, role).await)
        }
        _ => return None,
    })
}
