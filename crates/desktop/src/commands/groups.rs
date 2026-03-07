use desktop_shared::commands::{
    TaskGroupCreateParams, TaskGroupReorderParams, TaskGroupResponse, TaskGroupUpdateParams,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

fn group_row_to_response(row: &storage::TaskGroupRow, task_count: u32) -> TaskGroupResponse {
    TaskGroupResponse {
        id: row.id.clone(),
        project_id: row.project_id.clone(),
        name: row.name.clone(),
        color: row.color.clone(),
        position: row.position,
        task_count,
    }
}

#[tauri::command]
pub async fn group_list(
    state: State<'_, Arc<AppCore>>,
    project_id: Option<String>,
) -> Result<Vec<TaskGroupResponse>, ApiError> {
    let rows = state
        .repos
        .task_groups
        .list(project_id.as_deref())
        .await
        .map_err(super::map_storage_err)?;

    let mut results = Vec::with_capacity(rows.len());
    for row in &rows {
        let count = state
            .repos
            .task_groups
            .count_tasks(&row.id)
            .await
            .map_err(super::map_storage_err)?;
        results.push(group_row_to_response(row, count));
    }
    Ok(results)
}

#[tauri::command]
pub async fn group_create(
    state: State<'_, Arc<AppCore>>,
    params: TaskGroupCreateParams,
) -> Result<TaskGroupResponse, ApiError> {
    // Determine position: append after existing groups
    let existing = state
        .repos
        .task_groups
        .list(params.project_id.as_deref())
        .await
        .map_err(super::map_storage_err)?;
    let position = existing.len() as i32;

    let row = state
        .repos
        .task_groups
        .create(
            params.project_id.as_deref(),
            &params.name,
            params.color.as_deref(),
            position,
        )
        .await
        .map_err(super::map_storage_err)?;

    Ok(group_row_to_response(&row, 0))
}

#[tauri::command]
pub async fn group_update(
    state: State<'_, Arc<AppCore>>,
    params: TaskGroupUpdateParams,
) -> Result<TaskGroupResponse, ApiError> {
    let row = state
        .repos
        .task_groups
        .update(
            &params.id,
            params.name.as_deref(),
            params.color.as_deref(),
            params.position,
        )
        .await
        .map_err(super::map_storage_err)?;

    let count = state
        .repos
        .task_groups
        .count_tasks(&row.id)
        .await
        .map_err(super::map_storage_err)?;

    Ok(group_row_to_response(&row, count))
}

#[tauri::command]
pub async fn group_delete(state: State<'_, Arc<AppCore>>, id: String) -> Result<bool, ApiError> {
    state
        .repos
        .task_groups
        .delete(&id)
        .await
        .map_err(super::map_storage_err)
}

#[tauri::command]
pub async fn group_reorder(
    state: State<'_, Arc<AppCore>>,
    params: TaskGroupReorderParams,
) -> Result<(), ApiError> {
    state
        .repos
        .task_groups
        .reorder(params.project_id.as_deref(), &params.group_ids)
        .await
        .map_err(super::map_storage_err)
}
