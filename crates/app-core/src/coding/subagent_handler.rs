use crate::AppCore;
use common::Result;
use desktop_shared::coding::{SubagentActiveSummary, SubagentDetail};

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn subagent_list_active(
        &self,
        thread_id: &str,
    ) -> Result<Vec<SubagentActiveSummary>> {
        let mgr = self.agent.subagent_manager().ok_or_else(|| {
            common::KlyntbotError::Storage("subagent manager not available".into())
        })?;
        let active = mgr.list_active(thread_id).await;
        Ok(active
            .into_iter()
            .map(|s| SubagentActiveSummary {
                agent_id: s.agent_id,
                label: s.label,
                profile: s.profile,
                iteration: s.iteration,
                status: s.status,
                started_at: s.started_at,
                last_tool: s.last_tool,
                duration_ms: s.duration_ms,
            })
            .collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn subagent_cancel(&self, agent_id: &str) -> Result<()> {
        let mgr = self.agent.subagent_manager().ok_or_else(|| {
            common::KlyntbotError::Storage("subagent manager not available".into())
        })?;
        mgr.cancel_subagent(agent_id).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn subagent_inspect(&self, agent_id: &str) -> Result<SubagentDetail> {
        let _row = self.repos.agent_tasks.get(agent_id).await?;
        // AgentTaskRow does not currently track messages_json, tokens_used, or duration_ms.
        Ok(SubagentDetail {
            agent_id: agent_id.to_string(),
            messages: vec![],
            tokens_used: 0,
            duration_ms: 0,
        })
    }
}
