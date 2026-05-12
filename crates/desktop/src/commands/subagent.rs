use desktop_macros::klynt_command;
use desktop_shared::coding::{SubagentActiveSummary, SubagentDetail, SubagentInstanceSummary};
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

#[klynt_command]
pub async fn subagent_list_for_session(session_id: String) -> Vec<SubagentInstanceSummary> {
    state
        .subagent_list_for_session(session_id)
        .await
        .map_err(|e| ApiError::new("SUBAGENT_ERROR", e.to_string()))
        .map(|rows| {
            rows.into_iter()
                .map(|r| SubagentInstanceSummary {
                    agent_id: r.agent_id,
                    session_id: r.session_id,
                    parent_agent_id: r.parent_agent_id,
                    description: r.description,
                    status: r.status,
                    turns_used_total: r.turns_used_total,
                    last_cap_hit_at: r.last_cap_hit_at,
                    updated_at: r.updated_at,
                })
                .collect()
        })
}
