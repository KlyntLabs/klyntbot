//! McpManager: lifecycle management for all MCP server connections.
//!
//! Connects to configured MCP servers at startup, discovers their tools,
//! and wraps each as an `McpTool` for registration in `ToolRegistry`.

use std::collections::HashMap;
use std::sync::Arc;

use rmcp::service::{RunningService, ServiceExt};
use rmcp::transport::TokioChildProcess;
use tokio::process::Command;
use tracing::{info, warn};

use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

use crate::client::handler::KlyntbotClientHandler;
use crate::client::tool_adapter::McpTool;
use config::{McpConfig, McpTransport};

type McpService = RunningService<rmcp::service::RoleClient, KlyntbotClientHandler>;

/// Holds a connected MCP server session and its discovered tools.
struct McpConnection {
    service: McpService,
    tools: Vec<Arc<McpTool>>,
}

/// Manages all MCP server connections and their tools.
pub struct McpManager {
    connections: HashMap<String, McpConnection>,
}

impl McpManager {
    /// Connect to all enabled MCP servers in the config.
    ///
    /// Failures on individual servers are logged as warnings but don't
    /// prevent other servers from connecting.
    pub async fn connect_all(config: &McpConfig) -> Self {
        let mut connections = HashMap::new();

        // Connect to all enabled servers in parallel
        let mut join_set = tokio::task::JoinSet::new();
        for server_def in &config.servers {
            if !server_def.enabled {
                info!(name = %server_def.name, "MCP server disabled, skipping");
                continue;
            }
            let name = server_def.name.clone();
            let transport = server_def.transport.clone();
            join_set.spawn(async move {
                let result = Self::connect_one(&name, &transport).await;
                (name, result)
            });
        }

        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok((name, Ok(conn))) => {
                    let tool_count = conn.tools.len();
                    info!(name = %name, tools = tool_count, "MCP server connected");
                    connections.insert(name, conn);
                }
                Ok((name, Err(e))) => {
                    warn!(name = %name, error = %e, "Failed to connect to MCP server, skipping");
                }
                Err(e) => {
                    warn!(error = %e, "MCP connection task panicked");
                }
            }
        }

        Self { connections }
    }

    /// Connect to a single MCP server and discover its tools.
    async fn connect_one(name: &str, transport: &McpTransport) -> anyhow::Result<McpConnection> {
        let handler = KlyntbotClientHandler::new(name);

        let service: McpService = match transport {
            McpTransport::Stdio { command, args, env } => {
                let mut cmd = Command::new(command);
                cmd.args(args);
                for (k, v) in env {
                    cmd.env(k, v);
                }
                let transport = TokioChildProcess::new(cmd)?;
                handler.serve(transport).await?
            }
            McpTransport::Http { url, headers } => {
                let mut config = StreamableHttpClientTransportConfig::with_uri(url.as_str());
                if !headers.is_empty() {
                    let custom: std::collections::HashMap<
                        http::HeaderName,
                        http::HeaderValue,
                    > = headers
                        .iter()
                        .filter_map(|(k, v)| {
                            let name = http::HeaderName::try_from(k.as_str()).ok()?;
                            let value = http::HeaderValue::try_from(v.as_str()).ok()?;
                            Some((name, value))
                        })
                        .collect();
                    config = config.custom_headers(custom);
                }
                let transport =
                    rmcp::transport::StreamableHttpClientTransport::from_config(config);
                handler.serve(transport).await?
            }
        };

        // Discover tools (handles pagination automatically)
        let tool_defs = service.peer().list_all_tools().await?;
        let peer = Arc::new(service.peer().clone());

        let tools: Vec<Arc<McpTool>> = tool_defs
            .iter()
            .map(|tool_def| Arc::new(McpTool::new(name, tool_def, Arc::clone(&peer))))
            .collect();

        Ok(McpConnection { service, tools })
    }

    /// Get all discovered tools from all connected servers.
    pub fn tools(&self) -> Vec<Arc<McpTool>> {
        self.connections
            .values()
            .flat_map(|conn| conn.tools.clone())
            .collect()
    }

    /// Get the number of connected servers.
    pub fn connected_count(&self) -> usize {
        self.connections.len()
    }

    /// Gracefully disconnect all MCP servers concurrently.
    pub async fn disconnect_all(self) {
        let futures: Vec<_> = self
            .connections
            .into_iter()
            .map(|(name, conn)| async move {
                if let Err(e) = conn.service.cancel().await {
                    warn!(name = %name, error = %e, "Error disconnecting MCP server");
                } else {
                    info!(name = %name, "MCP server disconnected");
                }
            })
            .collect();
        futures_util::future::join_all(futures).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_all_with_no_servers() {
        let config = McpConfig::default();
        let manager = McpManager::connect_all(&config).await;
        assert_eq!(manager.connected_count(), 0);
        assert!(manager.tools().is_empty());
    }

    #[tokio::test]
    async fn test_connect_all_skips_disabled_servers() {
        let config = McpConfig {
            enabled: true,
            servers: vec![config::McpServerDef {
                name: "disabled".to_string(),
                transport: McpTransport::Stdio {
                    command: "nonexistent".to_string(),
                    args: vec![],
                    env: HashMap::new(),
                },
                enabled: false,
            }],
            server: Default::default(),
        };
        let manager = McpManager::connect_all(&config).await;
        assert_eq!(manager.connected_count(), 0);
    }

    #[tokio::test]
    async fn test_connect_all_handles_connection_failure_gracefully() {
        let config = McpConfig {
            enabled: true,
            servers: vec![config::McpServerDef {
                name: "bad-server".to_string(),
                transport: McpTransport::Stdio {
                    command: "nonexistent-binary-that-does-not-exist".to_string(),
                    args: vec![],
                    env: HashMap::new(),
                },
                enabled: true,
            }],
            server: Default::default(),
        };
        // Should not panic — logs warning and skips
        let manager = McpManager::connect_all(&config).await;
        assert_eq!(manager.connected_count(), 0);
    }
}
