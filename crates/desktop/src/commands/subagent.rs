use desktop_macros::klynt_command;
use desktop_shared::coding::{SubagentActiveSummary, SubagentDetail};
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn subagent_list_active(thread_id: String) -> Vec<SubagentActiveSummary> {
    state
        .subagent_list_active(&thread_id)
        .await
        .map_err(|e| ApiError::new("SUBAGENT_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn subagent_cancel(agent_id: String) -> () {
    state
        .subagent_cancel(&agent_id)
        .await
        .map_err(|e| ApiError::new("SUBAGENT_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn subagent_inspect(agent_id: String) -> SubagentDetail {
    state
        .subagent_inspect(&agent_id)
        .await
        .map_err(|e| ApiError::new("SUBAGENT_ERROR", e.to_string()))
}
