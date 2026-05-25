use desktop_macros::klynt_command;
use desktop_shared::commands::{
    TaskGroupCreateParams, TaskGroupReorderParams, TaskGroupResponse, TaskGroupUpdateParams,
};

#[klynt_command]
pub async fn group_list(project_id: Option<String>) -> Vec<TaskGroupResponse> {
    state.group_list(project_id).await
}

#[klynt_command]
pub async fn group_create(params: TaskGroupCreateParams) -> TaskGroupResponse {
    state.group_create(params).await
}

#[klynt_command]
pub async fn group_update(params: TaskGroupUpdateParams) -> TaskGroupResponse {
    state.group_update(params).await
}

#[klynt_command]
pub async fn group_delete(id: String) -> bool {
    state.group_delete(id).await
}

#[klynt_command]
pub async fn group_reorder(params: TaskGroupReorderParams) -> () {
    state.group_reorder(params).await
}
