use desktop_shared::commands::{
    LabelCreateParams, LabelReorderParams, LabelUpdateParams, StatusLabelResponse,
    StatusWorkflowResponse, WorkflowCreateParams,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use storage::rows::status::{StatusLabelRow, StatusWorkflowRow};
use tauri::State;

use crate::app_core::AppCore;

// ── Row → Response converters ───────────────────────────────────────────

fn workflow_to_response(
    wf: StatusWorkflowRow,
    labels: Vec<StatusLabelRow>,
) -> StatusWorkflowResponse {
    StatusWorkflowResponse {
        id: wf.id,
        name: wf.name,
        is_template: wf.is_template,
        is_global_default: wf.is_global_default,
        labels: labels.into_iter().map(label_to_response).collect(),
    }
}

fn label_to_response(l: StatusLabelRow) -> StatusLabelResponse {
    StatusLabelResponse {
        id: l.id,
        workflow_id: l.workflow_id,
        name: l.name,
        color: l.color,
        status_group: l.status_group,
        position: l.position,
    }
}

// ── Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn workflow_list(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<StatusWorkflowResponse>, ApiError> {
    let workflows = state
        .repos
        .status_workflows
        .list_all()
        .await
        .map_err(super::map_storage_err)?;

    let mut results = Vec::with_capacity(workflows.len());
    for wf in workflows {
        let labels = state
            .repos
            .status_workflows
            .get_labels(&wf.id)
            .await
            .map_err(super::map_storage_err)?;
        results.push(workflow_to_response(wf, labels));
    }
    Ok(results)
}

#[tauri::command]
pub async fn workflow_get(
    id: String,
    state: State<'_, Arc<AppCore>>,
) -> Result<Option<StatusWorkflowResponse>, ApiError> {
    let wf = state
        .repos
        .status_workflows
        .get(&id)
        .await
        .map_err(super::map_storage_err)?;

    match wf {
        Some(wf) => {
            let labels = state
                .repos
                .status_workflows
                .get_labels(&wf.id)
                .await
                .map_err(super::map_storage_err)?;
            Ok(Some(workflow_to_response(wf, labels)))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub async fn workflow_get_effective(
    project_id: Option<String>,
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<StatusLabelResponse>, ApiError> {
    // ProjectRow does not have workflow_id yet, so we always pass None
    // (returns the global default workflow labels).
    let _project_id = project_id;
    let labels = state
        .repos
        .status_workflows
        .get_effective_labels(None)
        .await
        .map_err(super::map_storage_err)?;

    Ok(labels.into_iter().map(label_to_response).collect())
}

#[tauri::command]
pub async fn workflow_create(
    params: WorkflowCreateParams,
    state: State<'_, Arc<AppCore>>,
) -> Result<StatusWorkflowResponse, ApiError> {
    let wf = match params.source_workflow_id {
        Some(source_id) => {
            state
                .repos
                .status_workflows
                .duplicate(&source_id, &params.name)
                .await
                .map_err(super::map_storage_err)?
        }
        None => {
            state
                .repos
                .status_workflows
                .create(&params.name, params.is_template.unwrap_or(false))
                .await
                .map_err(super::map_storage_err)?
        }
    };

    let labels = state
        .repos
        .status_workflows
        .get_labels(&wf.id)
        .await
        .map_err(super::map_storage_err)?;

    Ok(workflow_to_response(wf, labels))
}

#[tauri::command]
pub async fn workflow_delete(
    id: String,
    state: State<'_, Arc<AppCore>>,
) -> Result<bool, ApiError> {
    state
        .repos
        .status_workflows
        .delete(&id)
        .await
        .map_err(super::map_storage_err)
}

#[tauri::command]
pub async fn label_create(
    params: LabelCreateParams,
    state: State<'_, Arc<AppCore>>,
) -> Result<StatusLabelResponse, ApiError> {
    let position = params.position.unwrap_or_else(|| {
        // Default position will be set; caller should provide it ideally.
        0
    });

    let label = state
        .repos
        .status_workflows
        .add_label(
            &params.workflow_id,
            &params.name,
            &params.color,
            &params.status_group,
            position,
        )
        .await
        .map_err(super::map_storage_err)?;

    Ok(label_to_response(label))
}

#[tauri::command]
pub async fn label_update(
    params: LabelUpdateParams,
    state: State<'_, Arc<AppCore>>,
) -> Result<StatusLabelResponse, ApiError> {
    let label = state
        .repos
        .status_workflows
        .update_label(
            &params.id,
            params.name.as_deref(),
            params.color.as_deref(),
            params.status_group.as_deref(),
            params.position,
        )
        .await
        .map_err(super::map_storage_err)?;

    Ok(label_to_response(label))
}

#[tauri::command]
pub async fn label_delete(
    id: String,
    state: State<'_, Arc<AppCore>>,
) -> Result<bool, ApiError> {
    state
        .repos
        .status_workflows
        .delete_label(&id)
        .await
        .map_err(super::map_storage_err)
}

#[tauri::command]
pub async fn label_reorder(
    params: LabelReorderParams,
    state: State<'_, Arc<AppCore>>,
) -> Result<(), ApiError> {
    state
        .repos
        .status_workflows
        .reorder_labels(&params.workflow_id, &params.label_ids)
        .await
        .map_err(super::map_storage_err)
}
