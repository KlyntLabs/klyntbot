use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

use crate::commands::dev_helpers as dev;

#[klynt_command]
pub async fn workspace_meta_read(
    workspace_id: String,
    scope: String,
    kind: String,
) -> serde_json::Value {
    state
        .workspace_meta_read(&workspace_id, &scope, &kind)
        .await
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn workspace_meta_write(
    workspace_id: String,
    scope: String,
    kind: String,
    content: String,
) -> () {
    state
        .workspace_meta_write(&workspace_id, &scope, &kind, &content)
        .await
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn workspace_file_read(workspace_id: String, path: String) -> serde_json::Value {
    state
        .workspace_file_read(&workspace_id, &path)
        .await
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn workspace_files_list(
    workspace_id: String,
    query: Option<String>,
    limit: Option<usize>,
) -> Vec<String> {
    state
        .workspace_files_list(&workspace_id, query.as_deref(), limit)
        .await
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn text_file_write(path: String, content: String) -> () {
    state
        .text_file_write(&path, &content)
        .await
        .map_err(|e| ApiError::new("WORKSPACE_ERROR", e.to_string()))
}

#[klynt_command]
pub async fn image_data_url(path: String) -> String {
    state
        .image_data_url(&path)
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
        "workspace_meta_read" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let scope = try_field!(dev::get_str(body, "scope"));
            let kind = try_field!(dev::get_str(body, "kind"));
            match core.workspace_meta_read(&workspace_id, &scope, &kind).await {
                Ok(r) => Ok(serde_json::to_value(r).unwrap_or_default()),
                Err(e) => Err(ApiError::new("WORKSPACE_ERROR", e.to_string())),
            }
        }
        "workspace_meta_write" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let scope = try_field!(dev::get_str(body, "scope"));
            let kind = try_field!(dev::get_str(body, "kind"));
            let content = try_field!(dev::get_str(body, "content"));
            dev::val(
                core.workspace_meta_write(&workspace_id, &scope, &kind, &content)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "workspace_file_read" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let path = try_field!(dev::get_str(body, "path"));
            match core.workspace_file_read(&workspace_id, &path).await {
                Ok(r) => Ok(serde_json::to_value(r).unwrap_or_default()),
                Err(e) => Err(ApiError::new("WORKSPACE_ERROR", e.to_string())),
            }
        }
        "workspace_files_list" => {
            let workspace_id = try_field!(dev::get_str(body, "workspaceId"));
            let query = body.get("query").and_then(|v| v.as_str());
            let limit = body
                .get("limit")
                .and_then(|v| v.as_u64())
                .map(|v| v as usize);
            dev::val(
                core.workspace_files_list(&workspace_id, query, limit)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "text_file_write" => {
            let path = try_field!(dev::get_str(body, "path"));
            let content = try_field!(dev::get_str(body, "content"));
            dev::val(
                core.text_file_write(&path, &content)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "image_data_url" => {
            let path = try_field!(dev::get_str(body, "path"));
            dev::val(core.image_data_url(&path).await.map_err(ApiError::from))
        }
        _ => return None,
    })
}
