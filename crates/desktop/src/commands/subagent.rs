use desktop_macros::klynt_command;
use desktop_shared::coding::{SubagentActiveSummary, SubagentDetail};

#[klynt_command]
pub async fn subagent_list_active(thread_id: String) -> Vec<SubagentActiveSummary> {
    core.subagent_list_active(&thread_id).await
}

#[klynt_command]
pub async fn subagent_cancel(agent_id: String) -> () {
    core.subagent_cancel(&agent_id).await
}

#[klynt_command]
pub async fn subagent_inspect(agent_id: String) -> SubagentDetail {
    core.subagent_inspect(&agent_id).await
}
