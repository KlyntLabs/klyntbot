use desktop_macros::klynt_command;
use desktop_shared::commands::{
    ColumnCreateParams, ColumnReorderParams, ColumnUpdateParams, ColumnValueSetParams,
    CustomColumnResponse, CustomColumnValueResponse,
};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn custom_column_list(project_id: String) -> Vec<CustomColumnResponse> {
    state.custom_column_list(project_id).await
}

#[klynt_command]
pub async fn custom_column_create(params: ColumnCreateParams) -> CustomColumnResponse {
    state.custom_column_create(params).await
}

#[klynt_command]
pub async fn custom_column_update(params: ColumnUpdateParams) -> CustomColumnResponse {
    state.custom_column_update(params).await
}

#[klynt_command]
pub async fn custom_column_delete(id: String) -> bool {
    state.custom_column_delete(id).await
}

#[klynt_command]
pub async fn custom_column_reorder(params: ColumnReorderParams) -> () {
    state.custom_column_reorder(params).await
}

#[klynt_command]
pub async fn custom_column_values(task_id: String) -> Vec<CustomColumnValueResponse> {
    state.custom_column_values(task_id).await
}

#[klynt_command]
pub async fn custom_column_value_set(params: ColumnValueSetParams) -> () {
    state.custom_column_value_set(params).await
}

#[klynt_command]
pub async fn custom_column_value_delete(task_id: String, column_id: String) -> bool {
    state.custom_column_value_delete(task_id, column_id).await
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
        "custom_column_list" => {
            let project_id = try_field!(dev::get_str(body, "projectId"));
            dev::val(core.custom_column_list(project_id).await)
        }
        "custom_column_create" => dev::val(
            core.custom_column_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "custom_column_update" => dev::val(
            core.custom_column_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "custom_column_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.custom_column_delete(id).await)
        }
        "custom_column_reorder" => dev::val(
            core.custom_column_reorder(try_field!(dev::parse_params(body)))
                .await,
        ),
        "custom_column_values" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            dev::val(core.custom_column_values(task_id).await)
        }
        "custom_column_value_set" => dev::val(
            core.custom_column_value_set(try_field!(dev::parse_params(body)))
                .await,
        ),
        "custom_column_value_delete" => {
            let task_id = try_field!(dev::get_str(body, "taskId"));
            let column_id = try_field!(dev::get_str(body, "columnId"));
            dev::val(core.custom_column_value_delete(task_id, column_id).await)
        }
        _ => return None,
    })
}
