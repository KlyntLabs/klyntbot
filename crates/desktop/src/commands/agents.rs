use desktop_macros::klynt_command;
use desktop_shared::commands::{AgentFileContent, AgentFileSummary, AgentProfileSummary};

#[klynt_command]
pub async fn agent_list_profiles() -> Vec<AgentProfileSummary> {
    state.agent_list_profiles().await
}

#[klynt_command]
pub async fn agent_read_file(agent_name: String, filename: String) -> AgentFileContent {
    state.agent_read_file(&agent_name, &filename).await
}

#[klynt_command]
pub async fn agent_write_file(
    agent_name: String,
    filename: String,
    content: String,
) -> AgentFileContent {
    state
        .agent_write_file(&agent_name, &filename, &content)
        .await
}

#[klynt_command]
pub async fn agent_create_profile(name: String) -> AgentProfileSummary {
    state.agent_create_profile(&name).await
}

#[klynt_command]
pub async fn agent_create_skill(agent_name: String, skill_name: String) -> AgentFileSummary {
    state.agent_create_skill(&agent_name, &skill_name).await
}

#[klynt_command]
pub async fn agent_delete_file(agent_name: String, filename: String) -> bool {
    state.agent_delete_file(&agent_name, &filename).await
}
