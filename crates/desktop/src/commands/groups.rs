use desktop_shared::commands::{
    TaskGroupCreateParams, TaskGroupReorderParams, TaskGroupResponse, TaskGroupUpdateParams,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn group_list(
    state: State<'_, Arc<AppCore>>,
    project_id: Option<String>,
) -> Result<Vec<TaskGroupResponse>, ApiError> {
    state.group_list(project_id).await
}

#[tauri::command]
pub async fn group_create(
    state: State<'_, Arc<AppCore>>,
    params: TaskGroupCreateParams,
) -> Result<TaskGroupResponse, ApiError> {
    state.group_create(params).await
}

#[tauri::command]
pub async fn group_update(
    state: State<'_, Arc<AppCore>>,
    params: TaskGroupUpdateParams,
) -> Result<TaskGroupResponse, ApiError> {
    state.group_update(params).await
}

#[tauri::command]
pub async fn group_delete(state: State<'_, Arc<AppCore>>, id: String) -> Result<bool, ApiError> {
    state.group_delete(id).await
}

#[tauri::command]
pub async fn group_reorder(
    state: State<'_, Arc<AppCore>>,
    params: TaskGroupReorderParams,
) -> Result<(), ApiError> {
    state.group_reorder(params).await
}
