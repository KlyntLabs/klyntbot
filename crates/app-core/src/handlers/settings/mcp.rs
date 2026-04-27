//! MCP settings handlers.

use std::collections::HashMap;

use desktop_shared::commands::{
    McpAddServerParams, McpConfigResponse, McpRemoveParams, McpServerResponse, McpToggleParams,
    McpUpdateServerParams,
};
use desktop_shared::errors::ApiError;

use crate::errors::map_config_save_err;
use crate::state::AppCore;

// ── Helper functions ──────────────────────────────────────────────────

/// Convert an `McpServerDef` into a response DTO.
pub fn server_to_response(s: &config::McpServerDef) -> McpServerResponse {
    let oauth_provider = s.oauth.as_ref().map(|o| o.provider.clone());
    let oauth_connected = s.oauth.as_ref().is_some_and(|o| !o.access_token.is_empty());

    match &s.transport {
        config::McpTransport::Stdio { command, args, env } => McpServerResponse {
            name: s.name.clone(),
            transport: "stdio".to_string(),
            enabled: s.enabled,
            command: Some(command.clone()),
            args: Some(args.clone()),
            env: Some(env.clone()),
            url: None,
            headers: None,
            oauth_provider,
            oauth_connected,
        },
        config::McpTransport::Http { url, headers } => McpServerResponse {
            name: s.name.clone(),
            transport: "http".to_string(),
            enabled: s.enabled,
            command: None,
            args: None,
            env: None,
            url: Some(url.clone()),
            headers: Some(headers.clone()),
            oauth_provider,
            oauth_connected,
        },
    }
}

/// Build the full MCP config response from config.
pub fn build_mcp_response(cfg: &config::Config) -> McpConfigResponse {
    McpConfigResponse {
        enabled: cfg.mcp.enabled,
        servers: cfg.mcp.servers.iter().map(server_to_response).collect(),
    }
}

/// Find an MCP server by name or return a NOT_FOUND error.
pub fn find_server_mut<'a>(
    servers: &'a mut [config::McpServerDef],
    name: &str,
) -> Result<&'a mut config::McpServerDef, ApiError> {
    servers
        .iter_mut()
        .find(|s| s.name == name)
        .ok_or_else(|| ApiError::new("NOT_FOUND", format!("MCP server '{name}' not found")))
}

/// Build an `McpTransport` from user-provided params.
pub fn build_transport(
    transport_type: &str,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    url: Option<String>,
    headers: Option<HashMap<String, String>>,
) -> Result<config::McpTransport, ApiError> {
    match transport_type {
        "stdio" => {
            let command = command.ok_or_else(|| {
                ApiError::new("VALIDATION", "command is required for stdio transport")
            })?;
            Ok(config::McpTransport::Stdio {
                command,
                args: args.unwrap_or_default(),
                env: env.unwrap_or_default(),
            })
        }
        "http" => {
            let url = url
                .ok_or_else(|| ApiError::new("VALIDATION", "url is required for http transport"))?;
            Ok(config::McpTransport::Http {
                url,
                headers: headers.unwrap_or_default(),
            })
        }
        other => Err(ApiError::new(
            "VALIDATION",
            format!("unknown transport type: {other}"),
        )),
    }
}

// ── AppCore methods ───────────────────────────────────────────────────

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn mcp_get_config(&self) -> Result<McpConfigResponse, ApiError> {
        let cfg = self.config.read().await;
        Ok(build_mcp_response(&cfg))
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn mcp_add_server(
        &self,
        params: McpAddServerParams,
    ) -> Result<McpConfigResponse, ApiError> {
        let server_def = {
            let mut cfg = self.config.write().await;

            if cfg.mcp.servers.iter().any(|s| s.name == params.name) {
                return Err(ApiError::new(
                    "CONFLICT",
                    format!("MCP server '{}' already exists", params.name),
                ));
            }

            let transport = build_transport(
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
            config::save(&cfg).await.map_err(map_config_save_err)?;
            let response = build_mcp_response(&cfg);
            // Drop config lock before calling agent
            drop(cfg);
            (def, response)
        };

        let (def, response) = server_def;
        // Connect the new server in the live agent (no OAuth requirement)
        self.agent.reconnect_mcp_server(&def).await;

        Ok(response)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn mcp_remove_server(
        &self,
        params: McpRemoveParams,
    ) -> Result<McpConfigResponse, ApiError> {
        let response = {
            let mut cfg = self.config.write().await;

            let before = cfg.mcp.servers.len();
            cfg.mcp.servers.retain(|s| s.name != params.name);

            if cfg.mcp.servers.len() == before {
                return Err(ApiError::new(
                    "NOT_FOUND",
                    format!("MCP server '{}' not found", params.name),
                ));
            }

            config::save(&cfg).await.map_err(map_config_save_err)?;
            build_mcp_response(&cfg)
            // Drop config lock before calling agent
        };

        // Disconnect the removed server and unregister its tools
        self.agent.disconnect_mcp_server(&params.name).await;

        Ok(response)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn mcp_toggle_server(
        &self,
        params: McpToggleParams,
    ) -> Result<McpConfigResponse, ApiError> {
        let server_def = {
            let mut cfg = self.config.write().await;

            let server = find_server_mut(&mut cfg.mcp.servers, &params.name)?;

            server.enabled = params.enabled;

            // Clone before releasing the mutable borrow on `server`
            let def = if params.enabled {
                Some(server.clone())
            } else {
                None
            };

            config::save(&cfg).await.map_err(map_config_save_err)?;
            let response = build_mcp_response(&cfg);
            drop(cfg);
            (def, response)
        };

        let (def, response) = server_def;
        if let Some(def) = def {
            // Enabling: reconnect the server
            self.agent.reconnect_mcp_server(&def).await;
        } else {
            // Disabling: disconnect and unregister tools
            self.agent.disconnect_mcp_server(&params.name).await;
        }

        Ok(response)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn mcp_update_server(
        &self,
        params: McpUpdateServerParams,
    ) -> Result<McpConfigResponse, ApiError> {
        let result = {
            let mut cfg = self.config.write().await;

            let server = find_server_mut(&mut cfg.mcp.servers, &params.name)?;

            if let Some(transport_type) = &params.transport {
                server.transport = build_transport(
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

            config::save(&cfg).await.map_err(map_config_save_err)?;
            let response = build_mcp_response(&cfg);
            drop(cfg);
            (def, response)
        };

        let (def, response) = result;
        // Disconnect old tools, then reconnect if enabled
        self.agent.disconnect_mcp_server(&params.name).await;
        if let Some(def) = def {
            self.agent.reconnect_mcp_server(&def).await;
        }

        Ok(response)
    }
}
