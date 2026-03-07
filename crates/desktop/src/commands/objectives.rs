use desktop_shared::commands::{ObjectiveCreateParams, ObjectiveResponse, ObjectiveUpdateParams};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn objective_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: ObjectiveCreateParams,
) -> Result<ObjectiveResponse, ApiError> {
    let (result, updates) = state.objective_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn objective_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<ObjectiveResponse, ApiError> {
    state.objective_get(id).await
}

#[tauri::command]
pub async fn objective_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: ObjectiveUpdateParams,
) -> Result<ObjectiveResponse, ApiError> {
    let (result, updates) = state.objective_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn objective_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.objective_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}
