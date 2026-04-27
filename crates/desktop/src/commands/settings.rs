use desktop_shared::commands::{
    AppInfoResponse, McpAddServerParams, McpConfigResponse, McpRemoveParams, McpToggleParams,
    McpUpdateServerParams,
};
use desktop_shared::specta_helpers::JsonValueWrapper;
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn mcp_get_config() -> McpConfigResponse {
    state.mcp_get_config().await
}

#[klynt_command]
pub async fn mcp_add_server(
    params: McpAddServerParams,
) -> McpConfigResponse {
    state.mcp_add_server(params).await
}

#[klynt_command]
pub async fn mcp_remove_server(
    params: McpRemoveParams,
) -> McpConfigResponse {
    state.mcp_remove_server(params).await
}

#[klynt_command]
pub async fn mcp_toggle_server(
    params: McpToggleParams,
) -> McpConfigResponse {
    state.mcp_toggle_server(params).await
}

#[klynt_command]
pub async fn mcp_update_server(
    params: McpUpdateServerParams,
) -> McpConfigResponse {
    state.mcp_update_server(params).await
}

#[klynt_command]
pub async fn app_info() -> AppInfoResponse {
    state.app_info().await
}

#[klynt_command]
pub async fn config_get_section(
    section: String,
) -> JsonValueWrapper {
    state
        .config_get_section(section)
        .await
        .map(JsonValueWrapper)
}

#[klynt_command]
pub async fn config_update_section(
    section: String,
    patch: JsonValueWrapper,
) -> JsonValueWrapper {
    state
        .config_update_section(section, patch.0)
        .await
        .map(JsonValueWrapper)
}

#[klynt_command]
pub async fn config_mark_setup_completed() -> () {
    state.config_mark_setup_completed().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
