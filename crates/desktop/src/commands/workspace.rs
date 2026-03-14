use std::sync::Arc;

use app_core::AppCore;
use desktop_shared::commands::{WorkspaceFile, WorkspaceFileContent};
use desktop_shared::errors::ApiError;
use tauri::State;

pub const DEV_COMMANDS: &[&str] = &[
    "workspace_list_files",
    "workspace_read_file",
    "workspace_write_file",
];

#[tauri::command]
pub async fn workspace_list_files(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<WorkspaceFile>, ApiError> {
    state.workspace_list_files().await
}

#[tauri::command]
pub async fn workspace_read_file(
    state: State<'_, Arc<AppCore>>,
    filename: String,
) -> Result<WorkspaceFileContent, ApiError> {
    state.workspace_read_file(&filename).await
}

#[tauri::command]
pub async fn workspace_write_file(
    state: State<'_, Arc<AppCore>>,
    filename: String,
    content: String,
) -> Result<WorkspaceFileContent, ApiError> {
    state.workspace_write_file(&filename, &content).await
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
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
