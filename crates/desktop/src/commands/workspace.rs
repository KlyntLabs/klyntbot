use app_core::AppCore;
use desktop_shared::commands::{WorkspaceFile, WorkspaceFileContent};
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn workspace_list_files() -> Vec<WorkspaceFile> {
    state.workspace_list_files().await
}

#[klynt_command]
pub async fn workspace_read_file(
    filename: String,
) -> WorkspaceFileContent {
    state.workspace_read_file(&filename).await
}

#[klynt_command]
pub async fn workspace_write_file(
    filename: String,
    content: String,
) -> WorkspaceFileContent {
    state.workspace_write_file(&filename, &content).await
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "workspace_list_files" => dev::val(core.workspace_list_files().await),
        "workspace_read_file" => {
            let filename = try_field!(dev::get_str(body, "filename"));
            dev::val(core.workspace_read_file(&filename).await)
        }
        "workspace_write_file" => {
            let filename = try_field!(dev::get_str(body, "filename"));
            let content = try_field!(dev::get_str(body, "content"));
            dev::val(core.workspace_write_file(&filename, &content).await)
        }
        _ => return None,
    })
}
