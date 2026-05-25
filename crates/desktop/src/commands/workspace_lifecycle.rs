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
