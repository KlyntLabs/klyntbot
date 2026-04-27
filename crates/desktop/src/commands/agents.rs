use app_core::AppCore;
use desktop_shared::commands::{AgentFileContent, AgentFileSummary, AgentProfileSummary};
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn agent_list_profiles() -> Vec<AgentProfileSummary> {
    state.agent_list_profiles().await
}

#[klynt_command]
pub async fn agent_read_file(
    agent_name: String,
    filename: String,
) -> AgentFileContent {
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
pub async fn agent_create_profile(
    name: String,
) -> AgentProfileSummary {
    state.agent_create_profile(&name).await
}

#[klynt_command]
pub async fn agent_create_skill(
    agent_name: String,
    skill_name: String,
) -> AgentFileSummary {
    state.agent_create_skill(&agent_name, &skill_name).await
}

#[klynt_command]
pub async fn agent_delete_file(
    agent_name: String,
    filename: String,
) -> bool {
    state.agent_delete_file(&agent_name, &filename).await
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "agent_list_profiles" => dev::val(core.agent_list_profiles().await),
        "agent_read_file" => {
            let agent_name = try_field!(dev::get_str(body, "agentName"));
            let filename = try_field!(dev::get_str(body, "filename"));
            dev::val(core.agent_read_file(&agent_name, &filename).await)
        }
        "agent_write_file" => {
            let agent_name = try_field!(dev::get_str(body, "agentName"));
            let filename = try_field!(dev::get_str(body, "filename"));
            let content = try_field!(dev::get_str(body, "content"));
            dev::val(
                core.agent_write_file(&agent_name, &filename, &content)
                    .await,
            )
        }
        "agent_create_profile" => {
            let name = try_field!(dev::get_str(body, "name"));
            dev::val(core.agent_create_profile(&name).await)
        }
        "agent_create_skill" => {
            let agent_name = try_field!(dev::get_str(body, "agentName"));
            let skill_name = try_field!(dev::get_str(body, "skillName"));
            dev::val(core.agent_create_skill(&agent_name, &skill_name).await)
        }
        "agent_delete_file" => {
            let agent_name = try_field!(dev::get_str(body, "agentName"));
            let filename = try_field!(dev::get_str(body, "filename"));
            dev::val(core.agent_delete_file(&agent_name, &filename).await)
        }
        _ => return None,
    })
}
