use desktop_shared::commands::{AreaCreateParams, AreaResponse, AreaUpdateParams};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn area_list(state: State<'_, Arc<AppCore>>) -> Result<Vec<AreaResponse>, ApiError> {
    state.area_list().await
}

#[tauri::command]
pub async fn area_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: AreaCreateParams,
) -> Result<AreaResponse, ApiError> {
    let (result, updates) = state.area_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn area_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: AreaUpdateParams,
) -> Result<AreaResponse, ApiError> {
    let (result, updates) = state.area_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn area_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.area_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn area_reorder(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    position: i32,
) -> Result<AreaResponse, ApiError> {
    let (result, updates) = state.area_reorder(id, position).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}
