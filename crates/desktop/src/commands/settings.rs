use desktop_macros::klynt_command;
use desktop_shared::commands::{
    AppInfoResponse, EmbeddedMcpStatusResponse, McpAddServerParams, McpConfigResponse,
    McpRemoveParams, McpToggleParams, McpUpdateServerParams,
};
use desktop_shared::specta_helpers::JsonValueWrapper;
#[klynt_command]
pub async fn mcp_get_config() -> McpConfigResponse {
    state.mcp_get_config().await
}

#[klynt_command]
pub async fn mcp_get_embedded_status() -> EmbeddedMcpStatusResponse {
    state.mcp_get_embedded_status().await
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

#[klynt_command]
pub async fn get_app_settings() -> serde_json::Value {
    state.get_app_settings().await
}

#[klynt_command]
pub async fn update_app_settings(settings: serde_json::Value) -> serde_json::Value {
    state.update_app_settings(settings).await
}

#[klynt_command]
pub async fn app_build_type() -> String {
    Ok(if cfg!(debug_assertions) {
        "debug".to_string()
    } else {
        "release".to_string()
    })
}

#[klynt_command]
pub async fn is_mobile_runtime() -> bool {
    Ok(false)
}
