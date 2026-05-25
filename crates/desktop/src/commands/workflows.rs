use desktop_macros::klynt_command;
use desktop_shared::commands::{
    LabelCreateParams, LabelReorderParams, LabelUpdateParams, StatusLabelResponse,
    StatusWorkflowResponse, WorkflowCreateParams,
};
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
