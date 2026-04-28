use desktop_macros::klynt_command;
use desktop_shared::entity_link_types::*;
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn project_source_create(
    app: tauri::AppHandle,
    params: ProjectSourceCreateParams,
) -> ProjectSourceResponse {
    let (result, updates) = state.project_source_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_source_delete(app: tauri::AppHandle, id: String) -> bool {
    let (result, updates) = state.project_source_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn project_source_list(project_id: String) -> Vec<ProjectSourceResponse> {
    state.project_source_list(project_id).await
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
