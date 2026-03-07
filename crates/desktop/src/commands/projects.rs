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
