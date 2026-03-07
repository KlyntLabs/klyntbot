use desktop_shared::commands::{
    AppInfoResponse, McpAddServerParams, McpConfigResponse, McpRemoveParams, McpToggleParams,
    McpUpdateServerParams,
};
use desktop_shared::errors::ApiError;
use serde_json::Value;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn mcp_get_config(state: State<'_, Arc<AppCore>>) -> Result<McpConfigResponse, ApiError> {
    state.mcp_get_config().await
}

#[tauri::command]
pub async fn mcp_add_server(
    state: State<'_, Arc<AppCore>>,
    params: McpAddServerParams,
) -> Result<McpConfigResponse, ApiError> {
    state.mcp_add_server(params).await
}

#[tauri::command]
pub async fn mcp_remove_server(
    state: State<'_, Arc<AppCore>>,
    params: McpRemoveParams,
) -> Result<McpConfigResponse, ApiError> {
    state.mcp_remove_server(params).await
}

#[tauri::command]
pub async fn mcp_toggle_server(
    state: State<'_, Arc<AppCore>>,
    params: McpToggleParams,
) -> Result<McpConfigResponse, ApiError> {
    state.mcp_toggle_server(params).await
}

#[tauri::command]
pub async fn mcp_update_server(
    state: State<'_, Arc<AppCore>>,
    params: McpUpdateServerParams,
) -> Result<McpConfigResponse, ApiError> {
    state.mcp_update_server(params).await
}

#[tauri::command]
pub async fn app_info(state: State<'_, Arc<AppCore>>) -> Result<AppInfoResponse, ApiError> {
    state.app_info().await
}

#[tauri::command]
pub async fn config_get_section(
    state: State<'_, Arc<AppCore>>,
    section: String,
) -> Result<Value, ApiError> {
    state.config_get_section(section).await
}

#[tauri::command]
pub async fn config_update_section(
    state: State<'_, Arc<AppCore>>,
    section: String,
    patch: Value,
) -> Result<Value, ApiError> {
    state.config_update_section(section, patch).await
}

#[tauri::command]
pub async fn config_mark_setup_completed(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    state.config_mark_setup_completed().await
}
