use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

use crate::commands::dev_helpers as dev;

#[klynt_command]
pub async fn coding_mcp_status(workspace_id: Option<String>) -> serde_json::Value {
    state
        .coding_mcp_status(workspace_id.as_deref())
        .await
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .map_err(|e| ApiError::new("MCP_ERROR", e.to_string()))
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "coding_mcp_status" => {
            let workspace_id = body
                .get("workspaceId")
                .and_then(|v| v.as_str())
                .map(String::from);
            match core.coding_mcp_status(workspace_id.as_deref()).await {
                Ok(r) => Ok(serde_json::to_value(r).unwrap_or_default()),
                Err(e) => Err(ApiError::new("MCP_ERROR", e.to_string())),
            }
        }
        _ => return None,
    })
}
