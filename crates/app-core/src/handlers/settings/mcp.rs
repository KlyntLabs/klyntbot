//! MCP settings handlers.

use std::collections::HashMap;

use desktop_shared::commands::{
    EmbeddedMcpRejection, EmbeddedMcpStatusResponse, McpAddServerParams, McpConfigResponse,
    McpRemoveParams, McpServerResponse, McpToggleParams, McpUpdateServerParams,
};
use desktop_shared::errors::ApiError;
use mcp::server::exposure::{ExposureValidation, RuntimeState};

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

/// Map stored [`ExposureValidation`] into the embedded-status DTO.
pub fn embedded_status_from_validation(v: &ExposureValidation) -> EmbeddedMcpStatusResponse {
    let mut effective: Vec<String> = v
        .effective_builtins
        .iter()
        .map(|b| b.as_str().to_string())
        .collect();
    effective.extend(v.effective_registry_tools.iter().cloned());

    EmbeddedMcpStatusResponse {
        state: v.runtime_state.as_str().to_string(),
        requested: v.requested.clone(),
        effective,
        rejected: v
            .rejected
            .iter()
            .map(|r| EmbeddedMcpRejection {
                name: r.name.clone(),
                reason: r.reason.as_str().to_string(),
            })
            .collect(),
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
#[tracing::instrument(err)]
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

    /// Embedded MCP status DTO for settings UI (EXPO-5.*).
    #[tracing::instrument(skip(self), err)]
    pub async fn mcp_get_embedded_status(&self) -> Result<EmbeddedMcpStatusResponse, ApiError> {
        let validation = self
            .mcp_exposure()
            .ok_or_else(|| ApiError::new("INTERNAL", "MCP exposure status not available yet"))?;
        Ok(embedded_status_from_validation(validation))
    }

    /// Whether the embedded HTTP MCP server may bind (Ready only).
    pub fn embedded_mcp_bind_allowed(&self) -> bool {
        mcp_bind_allowed(self.mcp_exposure())
    }
}

/// Ready-only gate for embedded HTTP MCP bind (EXPO-3.8 / EXPO-7.2).
pub fn mcp_bind_allowed(validation: Option<&ExposureValidation>) -> bool {
    validation.is_some_and(|v| v.runtime_state == RuntimeState::Ready)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcp::server::exposure::{BuiltinId, RejectedEntry, RejectionReason, RuntimeState};

    fn sample(state: RuntimeState) -> ExposureValidation {
        ExposureValidation {
            runtime_state: state,
            requested: vec!["tasks".into(), "bogus".into()],
            effective_registry_tools: vec!["tasks".into()],
            effective_builtins: vec![BuiltinId::GetStatus, BuiltinId::Agent],
            rejected: if state == RuntimeState::Invalid {
                vec![RejectedEntry {
                    name: "bogus".into(),
                    reason: RejectionReason::Unknown,
                }]
            } else {
                vec![]
            },
        }
    }

    #[test]
    fn dto_maps_ready_effective_and_empty_rejected() {
        let dto = embedded_status_from_validation(&sample(RuntimeState::Ready));
        assert_eq!(dto.state, "ready");
        assert_eq!(dto.effective, vec!["get_status", "agent", "tasks"]);
        assert!(dto.rejected.is_empty());
        assert_eq!(dto.requested, vec!["tasks", "bogus"]);
    }

    #[test]
    fn dto_maps_invalid_rejections() {
        let dto = embedded_status_from_validation(&sample(RuntimeState::Invalid));
        assert_eq!(dto.state, "invalid");
        assert_eq!(dto.rejected.len(), 1);
        assert_eq!(dto.rejected[0].name, "bogus");
        assert_eq!(dto.rejected[0].reason, "unknown");
    }

    #[test]
    fn dto_maps_disabled() {
        let dto = embedded_status_from_validation(&sample(RuntimeState::Disabled));
        assert_eq!(dto.state, "disabled");
    }

    #[test]
    fn bind_allowed_ready_only() {
        let cases = [
            (Some(RuntimeState::Ready), true),
            (Some(RuntimeState::Disabled), false),
            (Some(RuntimeState::Invalid), false),
            (None, false),
        ];
        for (state, expected) in cases {
            let validation = state.map(sample);
            assert_eq!(
                mcp_bind_allowed(validation.as_ref()),
                expected,
                "state={state:?}"
            );
        }
    }
}
