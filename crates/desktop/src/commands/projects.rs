use desktop_macros::klynt_command;
use desktop_shared::commands::{
    ProjectCreateParams, ProjectHealthMetricsResponse, ProjectResponse, ProjectUpdateParams,
};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn project_create(app: tauri::AppHandle, params: ProjectCreateParams) -> ProjectResponse {
    let (result, updates) = state.project_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_get(id: String) -> ProjectResponse {
    state.project_get(id).await
}

#[klynt_command]
pub async fn project_update(app: tauri::AppHandle, params: ProjectUpdateParams) -> ProjectResponse {
    let (result, updates) = state.project_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_delete(app: tauri::AppHandle, id: String) -> bool {
    let (result, updates) = state.project_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_archive(app: tauri::AppHandle, id: String) -> ProjectResponse {
    let (result, updates) = state.project_archive(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_update_instructions(
    app: tauri::AppHandle,
    id: String,
    instructions: desktop_shared::specta_helpers::JsonValueWrapper,
) -> ProjectResponse {
    let (result, updates) = state
        .project_update_instructions(id, instructions.0)
        .await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_update_role(
    app: tauri::AppHandle,
    id: String,
    role: String,
) -> ProjectResponse {
    let (result, updates) = state.project_update_role(id, role).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_health_metrics(project_id: String) -> ProjectHealthMetricsResponse {
    state.project_health_metrics(project_id).await
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
        "project_health_metrics" => {
            let project_id = try_field!(dev::get_str(body, "projectId"));
            dev::val(core.project_health_metrics(project_id).await)
        }
        _ => return None,
    })
}
