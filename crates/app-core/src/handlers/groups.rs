use desktop_shared::commands::{
    TaskGroupCreateParams, TaskGroupReorderParams, TaskGroupResponse, TaskGroupUpdateParams,
};
use desktop_shared::errors::ApiError;

use crate::errors::map_storage_err;
use crate::state::AppCore;

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

impl AppCore {
    pub async fn group_list(
        &self,
        project_id: Option<String>,
    ) -> Result<Vec<TaskGroupResponse>, ApiError> {
        let rows = self
            .repos
            .task_groups
            .list(project_id.as_deref())
            .await
            .map_err(map_storage_err)?;

        let mut results = Vec::with_capacity(rows.len());
        for row in &rows {
            let count = self
                .repos
                .task_groups
                .count_tasks(&row.id)
                .await
                .map_err(map_storage_err)?;
            results.push(group_row_to_response(row, count));
        }
        Ok(results)
    }

    pub async fn group_create(
        &self,
        params: TaskGroupCreateParams,
    ) -> Result<TaskGroupResponse, ApiError> {
        let existing = self
            .repos
            .task_groups
            .list(params.project_id.as_deref())
            .await
            .map_err(map_storage_err)?;
        let position = existing.len() as i32;

        let row = self
            .repos
            .task_groups
            .create(
                params.project_id.as_deref(),
                &params.name,
                params.color.as_deref(),
                position,
            )
            .await
            .map_err(map_storage_err)?;

        Ok(group_row_to_response(&row, 0))
    }

    pub async fn group_update(
        &self,
        params: TaskGroupUpdateParams,
    ) -> Result<TaskGroupResponse, ApiError> {
        let row = self
            .repos
            .task_groups
            .update(
                &params.id,
                params.name.as_deref(),
                params.color.as_deref(),
                params.position,
            )
            .await
            .map_err(map_storage_err)?;

        let count = self
            .repos
            .task_groups
            .count_tasks(&row.id)
            .await
            .map_err(map_storage_err)?;

        Ok(group_row_to_response(&row, count))
    }

    pub async fn group_delete(&self, id: String) -> Result<bool, ApiError> {
        self.repos
            .task_groups
            .delete(&id)
            .await
            .map_err(map_storage_err)
    }

    pub async fn group_reorder(&self, params: TaskGroupReorderParams) -> Result<(), ApiError> {
        self.repos
            .task_groups
            .reorder(params.project_id.as_deref(), &params.group_ids)
            .await
            .map_err(map_storage_err)
    }
}
