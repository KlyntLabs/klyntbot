use desktop_shared::commands::{
    ColumnCreateParams, ColumnReorderParams, ColumnUpdateParams, ColumnValueSetParams,
    CustomColumnResponse, CustomColumnValueResponse,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn custom_column_list(
    state: State<'_, Arc<AppCore>>,
    project_id: String,
) -> Result<Vec<CustomColumnResponse>, ApiError> {
    state.custom_column_list(project_id).await
}

#[tauri::command]
pub async fn custom_column_create(
    state: State<'_, Arc<AppCore>>,
    params: ColumnCreateParams,
) -> Result<CustomColumnResponse, ApiError> {
    state.custom_column_create(params).await
}

#[tauri::command]
pub async fn custom_column_update(
    state: State<'_, Arc<AppCore>>,
    params: ColumnUpdateParams,
) -> Result<CustomColumnResponse, ApiError> {
    state.custom_column_update(params).await
}

#[tauri::command]
pub async fn custom_column_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    state.custom_column_delete(id).await
}

#[tauri::command]
pub async fn custom_column_reorder(
    state: State<'_, Arc<AppCore>>,
    params: ColumnReorderParams,
) -> Result<(), ApiError> {
    state.custom_column_reorder(params).await
}

#[tauri::command]
pub async fn custom_column_values(
    state: State<'_, Arc<AppCore>>,
    task_id: String,
) -> Result<Vec<CustomColumnValueResponse>, ApiError> {
    state.custom_column_values(task_id).await
}

#[tauri::command]
pub async fn custom_column_value_set(
    state: State<'_, Arc<AppCore>>,
    params: ColumnValueSetParams,
) -> Result<(), ApiError> {
    state.custom_column_value_set(params).await
}

#[tauri::command]
pub async fn custom_column_value_delete(
    state: State<'_, Arc<AppCore>>,
    task_id: String,
    column_id: String,
) -> Result<bool, ApiError> {
    state.custom_column_value_delete(task_id, column_id).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "custom_column_list",
    "custom_column_create",
    "custom_column_update",
    "custom_column_delete",
    "custom_column_reorder",
    "custom_column_values",
    "custom_column_value_set",
    "custom_column_value_delete",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
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
