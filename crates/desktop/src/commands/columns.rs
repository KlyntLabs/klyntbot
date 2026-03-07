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
