use desktop_macros::klynt_command;
use desktop_shared::commands::{
    TaskGroupCreateParams, TaskGroupReorderParams, TaskGroupResponse, TaskGroupUpdateParams,
};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn group_list(
    project_id: Option<String>,
) -> Vec<TaskGroupResponse> {
    state.group_list(project_id).await
}

#[klynt_command]
pub async fn group_create(
    params: TaskGroupCreateParams,
) -> TaskGroupResponse {
    state.group_create(params).await
}

#[klynt_command]
pub async fn group_update(
    params: TaskGroupUpdateParams,
) -> TaskGroupResponse {
    state.group_update(params).await
}

#[klynt_command]
pub async fn group_delete(id: String) -> bool {
    state.group_delete(id).await
}

#[klynt_command]
pub async fn group_reorder(
    params: TaskGroupReorderParams,
) -> () {
    state.group_reorder(params).await
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
