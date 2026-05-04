use crate::AppCore;
use common::Result;
use desktop_shared::coding::{SubagentActiveSummary, SubagentDetail};

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn subagent_list_active(
        &self,
        thread_id: &str,
    ) -> Result<Vec<SubagentActiveSummary>> {
        let active = self.agent.subagent_manager().list_active(thread_id).await;
        Ok(active.into_iter().map(Into::into).collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn subagent_cancel(&self, agent_id: &str) -> Result<()> {
        self.agent
            .subagent_manager()
            .cancel_subagent(agent_id)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn subagent_inspect(&self, agent_id: &str) -> Result<SubagentDetail> {
        let row = self.repos.agent_tasks.get(agent_id).await?;
        Ok(SubagentDetail {
            agent_id: agent_id.to_string(),
            messages: row
                .messages_json
                .as_deref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_default(),
            tokens_used: row.tokens_used.unwrap_or(0),
            duration_ms: row.duration_ms.unwrap_or(0),
        })
    }
}
