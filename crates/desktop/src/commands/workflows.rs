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

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "workflow_list",
    "workflow_get",
    "workflow_get_effective",
    "workflow_create",
    "workflow_delete",
    "label_create",
    "label_update",
    "label_delete",
    "label_reorder",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "workflow_list" => dev::val(core.workflow_list().await),
        "workflow_get" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.workflow_get(id).await)
        }
        "workflow_get_effective" => dev::val(
            core.workflow_get_effective(dev::get(body, "projectId"))
                .await,
        ),
        "workflow_create" => dev::val(
            core.workflow_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "workflow_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.workflow_delete(id).await)
        }
        "label_create" => dev::val(core.label_create(try_field!(dev::parse_params(body))).await),
        "label_update" => dev::val(core.label_update(try_field!(dev::parse_params(body))).await),
        "label_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.label_delete(id).await)
        }
        "label_reorder" => dev::val(
            core.label_reorder(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
