use desktop_shared::commands::{
    AppInfoResponse, McpAddServerParams, McpConfigResponse, McpRemoveParams, McpToggleParams,
    McpUpdateServerParams,
};
use desktop_shared::errors::ApiError;
use desktop_shared::specta_helpers::JsonValueWrapper;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn mcp_get_config(state: State<'_, Arc<AppCore>>) -> Result<McpConfigResponse, ApiError> {
    state.mcp_get_config().await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_add_server(
    state: State<'_, Arc<AppCore>>,
    params: McpAddServerParams,
) -> Result<McpConfigResponse, ApiError> {
    state.mcp_add_server(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_remove_server(
    state: State<'_, Arc<AppCore>>,
    params: McpRemoveParams,
) -> Result<McpConfigResponse, ApiError> {
    state.mcp_remove_server(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_toggle_server(
    state: State<'_, Arc<AppCore>>,
    params: McpToggleParams,
) -> Result<McpConfigResponse, ApiError> {
    state.mcp_toggle_server(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn mcp_update_server(
    state: State<'_, Arc<AppCore>>,
    params: McpUpdateServerParams,
) -> Result<McpConfigResponse, ApiError> {
    state.mcp_update_server(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn app_info(state: State<'_, Arc<AppCore>>) -> Result<AppInfoResponse, ApiError> {
    state.app_info().await
}

#[tauri::command]
#[specta::specta]
pub async fn config_get_section(
    state: State<'_, Arc<AppCore>>,
    section: String,
) -> Result<JsonValueWrapper, ApiError> {
    state.config_get_section(section).await.map(JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn config_update_section(
    state: State<'_, Arc<AppCore>>,
    section: String,
    patch: JsonValueWrapper,
) -> Result<JsonValueWrapper, ApiError> {
    state.config_update_section(section, patch.0).await.map(JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn config_mark_setup_completed(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    state.config_mark_setup_completed().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "mcp_get_config",
    "mcp_add_server",
    "mcp_remove_server",
    "mcp_toggle_server",
    "mcp_update_server",
    "app_info",
    "config_get_section",
    "config_update_section",
    "config_mark_setup_completed",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "mcp_get_config" => dev::val(core.mcp_get_config().await),
        "mcp_add_server" => dev::val(
            core.mcp_add_server(try_field!(dev::parse_params(body)))
                .await,
        ),
        "mcp_remove_server" => dev::val(
            core.mcp_remove_server(try_field!(dev::parse_params(body)))
                .await,
        ),
        "mcp_toggle_server" => dev::val(
            core.mcp_toggle_server(try_field!(dev::parse_params(body)))
                .await,
        ),
        "mcp_update_server" => dev::val(
            core.mcp_update_server(try_field!(dev::parse_params(body)))
                .await,
        ),
        "app_info" => dev::val(core.app_info().await),
        "config_get_section" => {
            let section = try_field!(dev::get_str(body, "section"));
            dev::val(core.config_get_section(section).await)
        }
        "config_update_section" => {
            let section = try_field!(dev::get_str(body, "section"));
            let patch = body
                .get("patch")
                .cloned()
                .unwrap_or(serde_json::Value::Object(Default::default()));
            dev::val(core.config_update_section(section, patch).await)
        }
        "config_mark_setup_completed" => dev::val(core.config_mark_setup_completed().await),
        _ => return None,
    })
}
