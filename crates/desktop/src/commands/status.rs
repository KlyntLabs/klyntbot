use desktop_macros::klynt_command;
use desktop_shared::commands::AgentStatusResponse;
#[klynt_command]
pub async fn agent_status() -> AgentStatusResponse {
    state.agent_status().await
}
