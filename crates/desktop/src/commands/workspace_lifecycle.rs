//! Workspace lifecycle commands (Cursor/Codex-style "open folder").
//!
//! These are the missing backend for `desktop-ui/src/api/endpoints/workspace.ts`.
//! Returns are typed as `JsonValueWrapper` so the existing TypeScript
//! `WorkspaceInfo` shape is preserved without re-typing the surface in specta.

use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;
use desktop_shared::specta_helpers::JsonValueWrapper;

#[klynt_command]
pub async fn list_workspaces() -> JsonValueWrapper {
    state
        .list_workspaces()
        .await
        .map(JsonValueWrapper)
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn add_workspace(path: String) -> JsonValueWrapper {
    state
        .add_workspace(path)
        .await
        .map(JsonValueWrapper)
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn is_workspace_path_dir(path: String) -> bool {
    state
        .is_workspace_path_dir(path)
        .await
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn remove_workspace(id: String) -> () {
    state
        .remove_workspace(id)
        .await
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn connect_workspace(id: String) -> () {
    state
        .connect_workspace(id)
        .await
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "list_workspaces" => dev::val(core.list_workspaces().await.map_err(ApiError::from)),
        "add_workspace" => {
            let path = try_field!(dev::get_str(body, "path"));
            dev::val(core.add_workspace(path).await.map_err(ApiError::from))
        }
        "is_workspace_path_dir" => {
            let path = try_field!(dev::get_str(body, "path"));
            dev::val(
                core.is_workspace_path_dir(path)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "remove_workspace" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.remove_workspace(id).await.map_err(ApiError::from))
        }
        "connect_workspace" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.connect_workspace(id).await.map_err(ApiError::from))
        }
        _ => return None,
    })
}
