use desktop_shared::commands::{
    McpAddServerParams, McpConfigResponse, McpRemoveParams, McpToggleParams,
    McpUpdateServerParams,
};
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn mcp_get_config(state: State<'_, AppCore>) -> Result<McpConfigResponse, ApiError> {
    let cfg = state.config.read().await;
    Ok(super::build_mcp_response(&cfg))
}

#[tauri::command]
pub async fn mcp_add_server(
    state: State<'_, AppCore>,
    params: McpAddServerParams,
) -> Result<McpConfigResponse, ApiError> {
    let server_def = {
        let mut cfg = state.config.write().await;

        if cfg.mcp.servers.iter().any(|s| s.name == params.name) {
            return Err(ApiError::new(
                "CONFLICT",
                format!("MCP server '{}' already exists", params.name),
            ));
        }

        let transport = super::build_transport(
            &params.transport,
            params.command,
            params.args,
            params.env,
            params.url,
            params.headers,
        )?;

        let def = config::McpServerDef {
            name: params.name,
            transport,
            enabled: true,
            oauth: None,
            startup_timeout_sec: config::DEFAULT_STARTUP_TIMEOUT_SEC,
            tool_timeout_sec: config::DEFAULT_TOOL_TIMEOUT_SEC,
            enabled_tools: None,
            disabled_tools: None,
        };

        cfg.mcp.servers.push(def.clone());
        config::save(&cfg).await.map_err(super::map_config_save_err)?;
        let response = super::build_mcp_response(&cfg);
        // Drop config lock before calling agent
        drop(cfg);
        (def, response)
    };

    let (def, response) = server_def;
    // Connect the new server in the live agent (no OAuth requirement)
    state.agent.reconnect_mcp_server(&def).await;

    Ok(response)
}

#[tauri::command]
pub async fn mcp_remove_server(
    state: State<'_, AppCore>,
    params: McpRemoveParams,
) -> Result<McpConfigResponse, ApiError> {
    let response = {
        let mut cfg = state.config.write().await;

        let before = cfg.mcp.servers.len();
        cfg.mcp.servers.retain(|s| s.name != params.name);

        if cfg.mcp.servers.len() == before {
            return Err(ApiError::new(
                "NOT_FOUND",
                format!("MCP server '{}' not found", params.name),
            ));
        }

        config::save(&cfg).await.map_err(super::map_config_save_err)?;
        super::build_mcp_response(&cfg)
        // Drop config lock before calling agent
    };

    // Disconnect the removed server and unregister its tools
    state.agent.disconnect_mcp_server(&params.name).await;

    Ok(response)
}

#[tauri::command]
pub async fn mcp_toggle_server(
    state: State<'_, AppCore>,
    params: McpToggleParams,
) -> Result<McpConfigResponse, ApiError> {
    let server_def = {
        let mut cfg = state.config.write().await;

        let server = super::find_server_mut(&mut cfg.mcp.servers, &params.name)?;

        server.enabled = params.enabled;

        // Clone before releasing the mutable borrow on `server`
        let def = if params.enabled {
            Some(server.clone())
        } else {
            None
        };

        config::save(&cfg).await.map_err(super::map_config_save_err)?;
        let response = super::build_mcp_response(&cfg);
        drop(cfg);
        (def, response)
    };

    let (def, response) = server_def;
    if let Some(def) = def {
        // Enabling: reconnect the server
        state.agent.reconnect_mcp_server(&def).await;
    } else {
        // Disabling: disconnect and unregister tools
        state.agent.disconnect_mcp_server(&params.name).await;
    }

    Ok(response)
}

#[tauri::command]
pub async fn mcp_update_server(
    state: State<'_, AppCore>,
    params: McpUpdateServerParams,
) -> Result<McpConfigResponse, ApiError> {
    let result = {
        let mut cfg = state.config.write().await;

        let server = super::find_server_mut(&mut cfg.mcp.servers, &params.name)?;

        if let Some(transport_type) = &params.transport {
            server.transport = super::build_transport(
                transport_type,
                params.command,
                params.args,
                params.env,
                params.url,
                params.headers,
            )?;
        } else {
            // Update fields within existing transport
            match &mut server.transport {
                config::McpTransport::Stdio { command, args, env } => {
                    if let Some(c) = params.command {
                        *command = c;
                    }
                    if let Some(a) = params.args {
                        *args = a;
                    }
                    if let Some(e) = params.env {
                        *env = e;
                    }
                }
                config::McpTransport::Http { url, headers } => {
                    if let Some(u) = params.url {
                        *url = u;
                    }
                    if let Some(h) = params.headers {
                        *headers = h;
                    }
                }
            }
        }

        // Clone the server_def before releasing the mutable borrow
        let def = if server.enabled {
            Some(server.clone())
        } else {
            None
        };

        config::save(&cfg).await.map_err(super::map_config_save_err)?;
        let response = super::build_mcp_response(&cfg);
        drop(cfg);
        (def, response)
    };

    let (def, response) = result;
    // Disconnect old tools, then reconnect if enabled
    state.agent.disconnect_mcp_server(&params.name).await;
    if let Some(def) = def {
        state.agent.reconnect_mcp_server(&def).await;
    }

    Ok(response)
}
