use desktop_macros::klynt_command;
use desktop_shared::commands::{WorkspaceFile, WorkspaceFileContent};

#[klynt_command]
pub async fn workspace_list_files() -> Vec<WorkspaceFile> {
    state.workspace_list_files().await
}

#[klynt_command]
pub async fn workspace_read_file(filename: String) -> WorkspaceFileContent {
    state.workspace_read_file(&filename).await
}

#[klynt_command]
pub async fn workspace_write_file(filename: String, content: String) -> WorkspaceFileContent {
    state.workspace_write_file(&filename, &content).await
}
