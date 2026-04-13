//! Agent status handler.

use desktop_shared::commands::AgentStatusResponse;
use desktop_shared::errors::ApiError;

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    pub async fn agent_status(&self) -> Result<AgentStatusResponse, ApiError> {
        let focused = self
            .repos
            .tasks
            .list_focused()
            .await
            .map_err(map_storage_err)?;

        let summary = self.repos.tasks.summary().await.map_err(map_storage_err)?;

        let focus_task = match focused.first() {
            Some(row) => Some(super::task_converters::row_to_task(&self.repos, row.clone()).await?),
            None => None,
        };

        Ok(AgentStatusResponse {
            status: if focused.is_empty() {
                "idle".to_string()
            } else {
                "active".to_string()
            },
            active_task_count: summary.doing,
            focus_task,
        })
    }
}
