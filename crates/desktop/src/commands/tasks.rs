use desktop_shared::commands::{
    ObjectiveResponse, ProjectResponse, TaskCreateParams, TaskResponse, TaskUpdateParams,
    TodayTaskResponse,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn task_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<Option<TaskResponse>, ApiError> {
    state.task_get(id).await
}

#[tauri::command]
pub async fn task_list(
    state: State<'_, Arc<AppCore>>,
    area_id: Option<String>,
    project_id: Option<String>,
    status: Option<String>,
) -> Result<Vec<TaskResponse>, ApiError> {
    state.task_list(area_id, project_id, status).await
}

#[tauri::command]
pub async fn task_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: TaskCreateParams,
) -> Result<TaskResponse, ApiError> {
    let (result, updates) = state.task_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn task_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: TaskUpdateParams,
) -> Result<TaskResponse, ApiError> {
    let (result, updates) = state.task_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn task_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.task_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn task_toggle_complete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<TaskResponse, ApiError> {
    let (result, updates) = state.task_toggle_complete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn task_list_children(
    state: State<'_, Arc<AppCore>>,
    parent_id: String,
) -> Result<Vec<TaskResponse>, ApiError> {
    state.task_list_children(parent_id).await
}

#[tauri::command]
pub async fn today_tasks(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<TodayTaskResponse>, ApiError> {
    state.today_tasks().await
}

#[tauri::command]
pub async fn project_list(
    state: State<'_, Arc<AppCore>>,
    area_id: Option<String>,
) -> Result<Vec<ProjectResponse>, ApiError> {
    state.project_list_for_tasks(area_id).await
}

#[tauri::command]
pub async fn objective_list(
    state: State<'_, Arc<AppCore>>,
    project_id: Option<String>,
) -> Result<Vec<ObjectiveResponse>, ApiError> {
    state.objective_list_for_tasks(project_id).await
}
