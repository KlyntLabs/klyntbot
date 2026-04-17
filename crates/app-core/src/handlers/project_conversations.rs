//! Project conversation handlers — list and create sessions scoped to a project.

use desktop_shared::entity_link_types::SessionSummaryResponse;
use desktop_shared::errors::ApiError;

use crate::errors::map_storage_err;
use crate::state::AppCore;

impl AppCore {
    /// List all conversations (sessions) associated with a project.
    pub async fn project_conversations_list(
        &self,
        project_id: String,
    ) -> Result<Vec<SessionSummaryResponse>, ApiError> {
        let rows = self
            .repos
            .sessions
            .list_by_project(&project_id)
            .await
            .map_err(map_storage_err)?;

        Ok(rows
            .into_iter()
            .map(|r| SessionSummaryResponse {
                key: r.key,
                title: r
                    .metadata
                    .as_object()
                    .and_then(|m| m.get("title"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                conversation_type: r.conversation_type,
                updated_at: common::time::bridge::jiff_to_chrono(*r.updated_at).to_rfc3339(),
            })
            .collect())
    }
}
