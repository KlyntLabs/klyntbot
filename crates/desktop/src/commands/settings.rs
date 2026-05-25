use desktop_macros::klynt_command;
use desktop_shared::commands::{
    AppInfoResponse, McpAddServerParams, McpConfigResponse, McpRemoveParams, McpToggleParams,
    McpUpdateServerParams,
};
use desktop_shared::specta_helpers::JsonValueWrapper;
#[klynt_command]
pub async fn mcp_get_config() -> McpConfigResponse {
    state.mcp_get_config().await
}

#[klynt_command]
pub async fn mcp_add_server(params: McpAddServerParams) -> McpConfigResponse {
    state.mcp_add_server(params).await
}

#[klynt_command]
pub async fn mcp_remove_server(params: McpRemoveParams) -> McpConfigResponse {
    state.mcp_remove_server(params).await
}

#[klynt_command]
pub async fn mcp_toggle_server(params: McpToggleParams) -> McpConfigResponse {
    state.mcp_toggle_server(params).await
}

#[klynt_command]
pub async fn mcp_update_server(params: McpUpdateServerParams) -> McpConfigResponse {
    state.mcp_update_server(params).await
}

#[klynt_command]
pub async fn app_info() -> AppInfoResponse {
    state.app_info().await
}

#[klynt_command]
pub async fn config_get_section(section: String) -> JsonValueWrapper {
    state
        .config_get_section(section)
        .await
        .map(JsonValueWrapper)
}

#[klynt_command]
pub async fn config_update_section(section: String, patch: JsonValueWrapper) -> JsonValueWrapper {
    state
        .config_update_section(section, patch.0)
        .await
        .map(JsonValueWrapper)
}

#[klynt_command]
pub async fn config_mark_setup_completed() -> () {
    state.config_mark_setup_completed().await
}
