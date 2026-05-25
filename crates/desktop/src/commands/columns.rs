use desktop_macros::klynt_command;
use desktop_shared::commands::{
    ColumnCreateParams, ColumnReorderParams, ColumnUpdateParams, ColumnValueSetParams,
    CustomColumnResponse, CustomColumnValueResponse,
};

#[klynt_command]
pub async fn custom_column_list(project_id: String) -> Vec<CustomColumnResponse> {
    state.custom_column_list(project_id).await
}

#[klynt_command]
pub async fn custom_column_create(params: ColumnCreateParams) -> CustomColumnResponse {
    state.custom_column_create(params).await
}

#[klynt_command]
pub async fn custom_column_update(params: ColumnUpdateParams) -> CustomColumnResponse {
    state.custom_column_update(params).await
}

#[klynt_command]
pub async fn custom_column_delete(id: String) -> bool {
    state.custom_column_delete(id).await
}

#[klynt_command]
pub async fn custom_column_reorder(params: ColumnReorderParams) -> () {
    state.custom_column_reorder(params).await
}

#[klynt_command]
pub async fn custom_column_values(task_id: String) -> Vec<CustomColumnValueResponse> {
    state.custom_column_values(task_id).await
}

#[klynt_command]
pub async fn custom_column_value_set(params: ColumnValueSetParams) -> () {
    state.custom_column_value_set(params).await
}

#[klynt_command]
pub async fn custom_column_value_delete(task_id: String, column_id: String) -> bool {
    state.custom_column_value_delete(task_id, column_id).await
}
