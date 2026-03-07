use desktop_shared::commands::{
    LabelCreateParams, LabelReorderParams, LabelUpdateParams, StatusLabelResponse,
    StatusWorkflowResponse, WorkflowCreateParams,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn workflow_list(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<StatusWorkflowResponse>, ApiError> {
    state.workflow_list().await
}

#[tauri::command]
pub async fn workflow_get(
    id: String,
    state: State<'_, Arc<AppCore>>,
) -> Result<Option<StatusWorkflowResponse>, ApiError> {
    state.workflow_get(id).await
}

#[tauri::command]
pub async fn workflow_get_effective(
    project_id: Option<String>,
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<StatusLabelResponse>, ApiError> {
    state.workflow_get_effective(project_id).await
}

#[tauri::command]
pub async fn workflow_create(
    params: WorkflowCreateParams,
    state: State<'_, Arc<AppCore>>,
) -> Result<StatusWorkflowResponse, ApiError> {
    state.workflow_create(params).await
}

#[tauri::command]
pub async fn workflow_delete(id: String, state: State<'_, Arc<AppCore>>) -> Result<bool, ApiError> {
    state.workflow_delete(id).await
}

#[tauri::command]
pub async fn label_create(
    params: LabelCreateParams,
    state: State<'_, Arc<AppCore>>,
) -> Result<StatusLabelResponse, ApiError> {
    state.label_create(params).await
}

#[tauri::command]
pub async fn label_update(
    params: LabelUpdateParams,
    state: State<'_, Arc<AppCore>>,
) -> Result<StatusLabelResponse, ApiError> {
    state.label_update(params).await
}

#[tauri::command]
pub async fn label_delete(id: String, state: State<'_, Arc<AppCore>>) -> Result<bool, ApiError> {
    state.label_delete(id).await
}

#[tauri::command]
pub async fn label_reorder(
    params: LabelReorderParams,
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    state.label_reorder(params).await
}
