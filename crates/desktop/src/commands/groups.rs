use desktop_shared::commands::{
    TaskGroupCreateParams, TaskGroupReorderParams, TaskGroupResponse, TaskGroupUpdateParams,
};
use desktop_shared::CommandResult;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn group_list(
    state: State<'_, Arc<AppCore>>,
    project_id: Option<String>,
) -> CommandResult<Vec<TaskGroupResponse>> {
    state.group_list(project_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_create(
    state: State<'_, Arc<AppCore>>,
    params: TaskGroupCreateParams,
) -> CommandResult<TaskGroupResponse> {
    state.group_create(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_update(
    state: State<'_, Arc<AppCore>>,
    params: TaskGroupUpdateParams,
) -> CommandResult<TaskGroupResponse> {
    state.group_update(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_delete(state: State<'_, Arc<AppCore>>, id: String) -> CommandResult<bool> {
    state.group_delete(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn group_reorder(
    state: State<'_, Arc<AppCore>>,
    params: TaskGroupReorderParams,
) -> CommandResult<()> {
    state.group_reorder(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "group_list",
    "group_create",
    "group_update",
    "group_delete",
    "group_reorder",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "group_list" => dev::val(core.group_list(dev::get(body, "projectId")).await),
        "group_create" => dev::val(core.group_create(try_field!(dev::parse_params(body))).await),
        "group_update" => dev::val(core.group_update(try_field!(dev::parse_params(body))).await),
        "group_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.group_delete(id).await)
        }
        "group_reorder" => dev::val(
            core.group_reorder(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
