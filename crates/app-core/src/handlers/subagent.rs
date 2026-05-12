//! AppCore handlers for subagent persistence/resume operations exposed to the
//! desktop UI. These are thin wrappers around `SubagentRuntime` that the
//! Tauri command shells delegate to.

use crate::AppCore;
use common::Result;
use storage::rows::SubagentInstanceRow;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn subagent_list_for_session(
        &self,
        session_id: String,
    ) -> Result<Vec<SubagentInstanceRow>> {
        let direct = self.repos.subagent_instances.get_by_session(&session_id).await?;
        if let Some(row) = direct {
            return Ok(vec![row]);
        }
        // No instance for this session itself — caller is the parent session;
        // return all immediate children.
        // Children are rows whose underlying session has parent_session_id == session_id.
        let rows = sqlx::query_as::<_, SubagentInstanceRow>(
            r#"
            SELECT si.*
            FROM subagent_instances si
            INNER JOIN sessions s ON s.key = si.session_id
            WHERE s.parent_session_id = ?1
            ORDER BY si.created_at DESC
            "#,
        )
        .bind(&session_id)
        .fetch_all(self.repos.pool())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent_list_for_session: {e}")))?;
        Ok(rows)
    }
}
