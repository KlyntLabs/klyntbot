use desktop_macros::klynt_command;
use desktop_shared::commands::{
    LabelCreateParams, LabelReorderParams, LabelUpdateParams, StatusLabelResponse,
    StatusWorkflowResponse, WorkflowCreateParams,
};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn workflow_list() -> Vec<StatusWorkflowResponse> {
    state.workflow_list().await
}

#[klynt_command]
pub async fn workflow_get(id: String) -> Option<StatusWorkflowResponse> {
    state.workflow_get(id).await
}

#[klynt_command]
pub async fn workflow_get_effective(project_id: Option<String>) -> Vec<StatusLabelResponse> {
    state.workflow_get_effective(project_id).await
}

#[klynt_command]
pub async fn workflow_create(params: WorkflowCreateParams) -> StatusWorkflowResponse {
    state.workflow_create(params).await
}

#[klynt_command]
pub async fn workflow_delete(id: String) -> bool {
    state.workflow_delete(id).await
}

#[klynt_command]
pub async fn label_create(params: LabelCreateParams) -> StatusLabelResponse {
    state.label_create(params).await
}

#[klynt_command]
pub async fn label_update(params: LabelUpdateParams) -> StatusLabelResponse {
    state.label_update(params).await
}

#[klynt_command]
pub async fn label_delete(id: String) -> bool {
    state.label_delete(id).await
}

#[klynt_command]
pub async fn label_reorder(params: LabelReorderParams) -> () {
    state.label_reorder(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
