use std::sync::Arc;

use app_core::AppCore;
use desktop_shared::commands::{AgentFileContent, AgentFileSummary, AgentProfileSummary};
use desktop_shared::CommandResult;

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "agent_list_profiles",
    "agent_read_file",
    "agent_write_file",
    "agent_create_profile",
    "agent_create_skill",
    "agent_delete_file",
];
use tauri::State;

#[tauri::command]
#[specta::specta]
pub async fn agent_list_profiles(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<AgentProfileSummary>> {
    state.agent_list_profiles().await
}

#[tauri::command]
#[specta::specta]
pub async fn agent_read_file(
    state: State<'_, Arc<AppCore>>,
    agent_name: String,
    filename: String,
) -> CommandResult<AgentFileContent> {
    state.agent_read_file(&agent_name, &filename).await
}

#[tauri::command]
#[specta::specta]
pub async fn agent_write_file(
    state: State<'_, Arc<AppCore>>,
    agent_name: String,
    filename: String,
    content: String,
) -> CommandResult<AgentFileContent> {
    state
        .agent_write_file(&agent_name, &filename, &content)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn agent_create_profile(
    state: State<'_, Arc<AppCore>>,
    name: String,
) -> CommandResult<AgentProfileSummary> {
    state.agent_create_profile(&name).await
}

#[tauri::command]
#[specta::specta]
pub async fn agent_create_skill(
    state: State<'_, Arc<AppCore>>,
    agent_name: String,
    skill_name: String,
) -> CommandResult<AgentFileSummary> {
    state.agent_create_skill(&agent_name, &skill_name).await
}

#[tauri::command]
#[specta::specta]
pub async fn agent_delete_file(
    state: State<'_, Arc<AppCore>>,
    agent_name: String,
    filename: String,
) -> CommandResult<bool> {
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
