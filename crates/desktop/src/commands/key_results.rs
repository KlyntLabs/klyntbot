use desktop_shared::commands::{KeyResultCreateParams, KeyResultResponse, KeyResultUpdateParams};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn key_result_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: KeyResultCreateParams,
) -> Result<KeyResultResponse, ApiError> {
    let (result, updates) = state.key_result_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn key_result_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: KeyResultUpdateParams,
) -> Result<KeyResultResponse, ApiError> {
    let (result, updates) = state.key_result_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn key_result_update_metric(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    current_value: f64,
) -> Result<KeyResultResponse, ApiError> {
    let (result, updates) = state.key_result_update_metric(id, current_value).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn key_result_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.key_result_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}
