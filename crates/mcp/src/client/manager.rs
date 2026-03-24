//! McpManager: lifecycle management for all MCP server connections.
//!
//! Connects to configured MCP servers at startup, discovers their tools,
//! and wraps each as an `McpTool` for registration in `ToolRegistry`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rmcp::service::{RunningService, ServiceExt};
use rmcp::transport::TokioChildProcess;
use tokio::process::Command;
use tracing::{info, warn};

use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

use crate::client::circuit_breaker::McpCircuitBreaker;
use crate::client::events::McpStartupEvent;
use crate::client::handler::KlyntbotClientHandler;
use crate::client::tool_adapter::McpTool;
use config::{McpConfig, McpTransport};

type McpService = RunningService<rmcp::service::RoleClient, KlyntbotClientHandler>;

/// Holds a connected MCP server session and its discovered tools.
///
/// Process cleanup: rmcp's `TokioChildProcess` uses `process_wrap` which
/// sets up process groups and kills the entire tree when the service is
/// cancelled or dropped. No manual SIGTERM/SIGKILL needed.
struct McpConnection {
    service: McpService,
    tools: Vec<Arc<McpTool>>,
}

/// Manages all MCP server connections and their tools.
pub struct McpManager {
    connections: HashMap<String, McpConnection>,
    circuit_breaker: Arc<McpCircuitBreaker>,
    #[allow(dead_code)]
    config: McpConfig,
    /// Sender for tool-list-changed notifications from MCP servers.
    tool_list_changed_tx: tokio::sync::mpsc::UnboundedSender<String>,
    /// Receiver for tool-list-changed notifications (taken by `start_health_check`).
    tool_list_changed_rx: Option<tokio::sync::mpsc::UnboundedReceiver<String>>,
}

impl McpManager {
    /// Connect to all enabled MCP servers in the config.
    ///
    /// Failures on individual servers are logged as warnings but don't
    /// prevent other servers from connecting.
    ///
    /// If `event_tx` is provided, startup progress events are emitted
    /// for observability (UI spinners, logging).
    pub async fn connect_all(
        config: &McpConfig,
        event_tx: Option<tokio::sync::mpsc::Sender<McpStartupEvent>>,
    ) -> Self {
        let circuit_breaker = Arc::new(McpCircuitBreaker::new(3, 60));
        let (tool_list_changed_tx, tool_list_changed_rx) =
            tokio::sync::mpsc::unbounded_channel::<String>();
        let mut connections = HashMap::new();
        let mut ready_count = 0usize;
        let mut failed_count = 0usize;
        let mut skipped_count = 0usize;

        // Connect to all enabled servers in parallel
        let mut join_set = tokio::task::JoinSet::new();
        for server_def in &config.servers {
            if !server_def.enabled {
                info!(name = %server_def.name, "MCP server disabled, skipping");
                skipped_count += 1;
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(McpStartupEvent::Skipped {
                            server_name: server_def.name.clone(),
                        })
                        .await;
                }
                continue;
            }
            let def = server_def.clone();
            let tx = event_tx.clone();
            let cb = Arc::clone(&circuit_breaker);
            let tlc_tx = tool_list_changed_tx.clone();
            join_set.spawn(async move {
                let name = def.name.clone();
                if let Some(ref tx) = tx {
                    let _ = tx
                        .send(McpStartupEvent::Starting {
                            server_name: name.clone(),
                        })
                        .await;
                }
                let result = Self::connect_one(&def, &cb, Some(&tlc_tx)).await;
                (name, result, tx)
            });
        }

        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok((name, Ok(conn), tx)) => {
                    let tool_count = conn.tools.len();
                    info!(name = %name, tools = tool_count, "MCP server connected");
                    ready_count += 1;
                    if let Some(ref tx) = tx {
                        let _ = tx
                            .send(McpStartupEvent::Ready {
                                server_name: name.clone(),
                                tool_count,
                            })
                            .await;
                    }
                    connections.insert(name, conn);
                }
                Ok((name, Err(e), tx)) => {
                    warn!(name = %name, error = %e, "Failed to connect to MCP server, skipping");
                    failed_count += 1;
                    if let Some(ref tx) = tx {
                        let _ = tx
                            .send(McpStartupEvent::Failed {
                                server_name: name,
                                error: e.to_string(),
                            })
                            .await;
                    }
                }
                Err(e) => {
                    warn!(error = %e, "MCP connection task panicked");
                    failed_count += 1;
                }
            }
        }

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(McpStartupEvent::Complete {
                    ready: ready_count,
                    failed: failed_count,
                    skipped: skipped_count,
                })
                .await;
        }

        Self {
            connections,
            circuit_breaker,
            config: config.clone(),
            tool_list_changed_tx,
            tool_list_changed_rx: Some(tool_list_changed_rx),
        }
    }

    /// Connect to a single MCP server with a startup timeout.
    ///
    /// Wraps `connect_one_inner` in `tokio::time::timeout` using the
    /// server's configured `startup_timeout_sec`.
    async fn connect_one(
        server_def: &config::McpServerDef,
        circuit_breaker: &Arc<McpCircuitBreaker>,
        tool_list_changed_tx: Option<&tokio::sync::mpsc::UnboundedSender<String>>,
    ) -> anyhow::Result<McpConnection> {
        let startup_timeout = Duration::from_secs(server_def.startup_timeout_sec);
        let tool_timeout = Duration::from_secs(server_def.tool_timeout_sec);

        tokio::time::timeout(
            startup_timeout,
            Self::connect_one_inner(
                server_def,
                tool_timeout,
                circuit_breaker,
                tool_list_changed_tx,
            ),
        )
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "MCP server '{}' timed out after {}s during startup",
                server_def.name,
                server_def.startup_timeout_sec
            )
        })?
    }

    /// Inner connection logic (no timeout wrapper).
    ///
    /// Process group cleanup is handled by rmcp's `process_wrap` crate,
    /// which sets up process groups and kills the entire tree when the
    /// `TokioChildProcess` is dropped.
    async fn connect_one_inner(
        server_def: &config::McpServerDef,
        tool_timeout: Duration,
        circuit_breaker: &Arc<McpCircuitBreaker>,
        tool_list_changed_tx: Option<&tokio::sync::mpsc::UnboundedSender<String>>,
    ) -> anyhow::Result<McpConnection> {
        let name = &server_def.name;
        let handler = {
            let h = KlyntbotClientHandler::new(name);
            if let Some(tx) = tool_list_changed_tx {
                h.with_tool_list_changed_tx(tx.clone())
            } else {
                h
            }
        };

        let service: McpService = match &server_def.transport {
            McpTransport::Stdio { command, args, env } => {
                let mut cmd = Command::new(command);
                cmd.args(args);
                for (k, v) in env {
                    cmd.env(k, v);
                }
                // Inject OAuth access token as env var for the subprocess
                if let Some(oauth) = &server_def.oauth {
                    cmd.env(&oauth.env_var, oauth.access_token.expose());
                }

                let transport = TokioChildProcess::new(cmd)?;
                handler.serve(transport).await?
            }
            McpTransport::Http { url, headers } => {
                let mut config = StreamableHttpClientTransportConfig::with_uri(url.as_str());

                // Inject OAuth token via rmcp's auth_header (adds Bearer prefix)
                if let Some(oauth) = &server_def.oauth {
                    config = config.auth_header(oauth.access_token.expose());
                }

                if !headers.is_empty() {
                    let custom: std::collections::HashMap<http::HeaderName, http::HeaderValue> =
                        headers
                            .iter()
                            .filter_map(|(k, v)| {
                                let name = http::HeaderName::try_from(k.as_str()).ok()?;
                                let value = http::HeaderValue::try_from(v.as_str()).ok()?;
                                Some((name, value))
                            })
                            .collect();
                    config = config.custom_headers(custom);
                }

                let transport = rmcp::transport::StreamableHttpClientTransport::from_config(config);
                handler.serve(transport).await?
            }
        };

        // Discover tools (handles pagination automatically)
        let tool_defs = service.peer().list_all_tools().await?;
        let peer = Arc::new(service.peer().clone());

        // Filter tools by allowlist/denylist, then wrap as McpTool
        let tools: Vec<Arc<McpTool>> = tool_defs
            .iter()
            .filter(|td| server_def.is_tool_allowed(&td.name))
            .map(|tool_def| {
                Arc::new(McpTool::new(
                    name,
                    tool_def,
                    Arc::clone(&peer),
                    tool_timeout,
                    Arc::clone(circuit_breaker),
                ))
            })
            .collect();

        let filtered_count = tool_defs.len() - tools.len();
        if filtered_count > 0 {
            info!(
                name = %name,
                total = tool_defs.len(),
                filtered = filtered_count,
                "Filtered MCP tools by allowlist/denylist"
            );
        }

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

    /// Get the circuit breaker shared across all MCP connections.
    pub fn circuit_breaker(&self) -> &Arc<McpCircuitBreaker> {
        &self.circuit_breaker
    }

    /// Disconnect a single server without reconnecting.
    ///
    /// Returns `true` if the server was found and disconnected.
    /// Process cleanup is handled by rmcp's `process_wrap` on drop.
    pub async fn disconnect_server(&mut self, name: &str) -> bool {
        if let Some(old) = self.connections.remove(name) {
            info!(name = %name, "Disconnecting MCP server");
            if let Err(e) = old.service.cancel().await {
                warn!(name = %name, error = %e, "Error disconnecting MCP server");
            }
            true
        } else {
            false
        }
    }

    /// Reconnect a single server (disconnect if running, then connect fresh).
    ///
    /// Returns the newly discovered tools, or an empty vec on failure.
    pub async fn reconnect_server(
        &mut self,
        server_def: &config::McpServerDef,
    ) -> Vec<Arc<McpTool>> {
        let name = &server_def.name;

        // Disconnect existing connection if any
        if let Some(old) = self.connections.remove(name) {
            info!(name = %name, "Disconnecting MCP server for reconnect");
            if let Err(e) = old.service.cancel().await {
                warn!(name = %name, error = %e, "Error disconnecting old MCP connection");
            }
        }

        // Connect fresh
        match Self::connect_one(
            server_def,
            &self.circuit_breaker,
            Some(&self.tool_list_changed_tx),
        )
        .await
        {
            Ok(conn) => {
                let tools = conn.tools.clone();
                let tool_count = tools.len();
                info!(name = %name, tools = tool_count, "MCP server reconnected");
                self.connections.insert(name.clone(), conn);
                tools
            }
            Err(e) => {
                warn!(name = %name, error = %e, "Failed to reconnect MCP server");
                Vec::new()
            }
        }
    }

    /// Get all tools for a specific server (for re-registration after reconnect).
    pub fn tools_for_server(&self, server_name: &str) -> Vec<Arc<McpTool>> {
        self.connections
            .get(server_name)
            .map(|conn| conn.tools.clone())
            .unwrap_or_default()
    }

    /// Take the tool-list-changed receiver out of this manager.
    ///
    /// Called once before `start_health_check` to pass the receiver to the
    /// background task. Returns `None` on second call.
    pub fn take_tool_list_changed_rx(
        &mut self,
    ) -> Option<tokio::sync::mpsc::UnboundedReceiver<String>> {
        self.tool_list_changed_rx.take()
    }

    /// Spawn a background task that periodically reconnects downed MCP servers
    /// and reacts to tool-list-changed notifications from connected servers.
    ///
    /// Checks every 30 seconds for servers whose circuit breaker cooldown has expired,
    /// attempts reconnection, and re-registers tools in the `ToolRegistry` on success.
    /// Also listens for `notifications/tools/list_changed` signals from MCP servers
    /// and triggers immediate re-discovery for the affected server.
    ///
    /// The task runs until the `cancel` token is cancelled.
    pub fn start_health_check(
        manager: Arc<tokio::sync::Mutex<Option<McpManager>>>,
        registry: Arc<tokio::sync::RwLock<tools_core::ToolRegistry>>,
        cancel: tokio_util::sync::CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            // Take the tool-list-changed receiver out of the manager (one-shot).
            let mut tool_list_rx = {
                let mut guard = manager.lock().await;
                guard
                    .as_mut()
                    .and_then(|mgr| mgr.take_tool_list_changed_rx())
            };

            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                // Collect servers to reconnect from either timer tick or tool-list-changed notification
                let servers_to_reconnect: Vec<config::McpServerDef> = tokio::select! {
                    _ = cancel.cancelled() => {
                        tracing::debug!("MCP health check task cancelled");
                        break;
                    }
                    _ = interval.tick() => {
                        // Collect servers that need reconnection (brief lock)
                        let guard = manager.lock().await;
                        match guard.as_ref() {
                            Some(mgr) => mgr.config
                                .servers
                                .iter()
                                .filter(|s| {
                                    s.enabled && mgr.circuit_breaker.cooldown_expired(&s.name)
                                })
                                .cloned()
                                .collect(),
                            None => continue,
                        }
                    }
                    Some(server_name) = async {
                        match tool_list_rx.as_mut() {
                            Some(rx) => rx.recv().await,
                            None => std::future::pending().await,
                        }
                    } => {
                        info!(
                            name = %server_name,
                            "Tool list changed notification received, triggering re-discovery"
                        );
                        // Find the server def for this server
                        let guard = manager.lock().await;
                        match guard.as_ref() {
                            Some(mgr) => mgr.config
                                .servers
                                .iter()
                                .filter(|s| s.enabled && s.name == server_name)
                                .cloned()
                                .collect(),
                            None => continue,
                        }
                    }
                };

                for server_def in servers_to_reconnect {
                    info!(
                        name = %server_def.name,
                        "Attempting auto-reconnect for MCP server"
                    );
                    // Reconnect under manager lock, then drop it before
                    // acquiring registry lock to avoid AB/BA deadlock.
                    let (new_tools, server_name) = {
                        let mut guard = manager.lock().await;
                        let mgr = match guard.as_mut() {
                            Some(m) => m,
                            None => continue,
                        };
                        let tools = mgr.reconnect_server(&server_def).await;
                        (tools, server_def.name.clone())
                    };
                    // Manager lock dropped here — safe to acquire registry lock
                    if !new_tools.is_empty() {
                        let mut reg = registry.write().await;
                        for tool in new_tools {
                            reg.register_dyn(tool as Arc<dyn tools_core::Tool>);
                        }
                        drop(reg);
                        let guard = manager.lock().await;
                        if let Some(mgr) = guard.as_ref() {
                            mgr.circuit_breaker.record_success(&server_name);
                        }
                        info!(name = %server_name, "Auto-reconnected MCP server");
                    } else {
                        // Re-record failure so cleanup() doesn't remove the entry
                        let guard = manager.lock().await;
                        if let Some(mgr) = guard.as_ref() {
                            mgr.circuit_breaker.record_failure(&server_name);
                        }
                        warn!(
                            name = %server_name,
                            "Auto-reconnect failed for MCP server (no tools returned)"
                        );
                    }
                }

                // Cleanup stale circuit breaker entries
                let guard = manager.lock().await;
                if let Some(mgr) = guard.as_ref() {
                    mgr.circuit_breaker.cleanup();
                }
            }
        })
    }

    /// Gracefully disconnect all MCP servers concurrently.
    ///
    /// Process cleanup is handled by rmcp's `process_wrap` on drop,
    /// which kills the entire process tree for stdio transports.
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

    fn test_server_def(name: &str, command: &str, enabled: bool) -> config::McpServerDef {
        config::McpServerDef {
            name: name.to_string(),
            transport: McpTransport::Stdio {
                command: command.to_string(),
                args: vec![],
                env: HashMap::new(),
            },
            enabled,
            oauth: None,
            startup_timeout_sec: config::DEFAULT_STARTUP_TIMEOUT_SEC,
            tool_timeout_sec: config::DEFAULT_TOOL_TIMEOUT_SEC,
            enabled_tools: None,
            disabled_tools: None,
        }
    }

    #[tokio::test]
    async fn test_connect_all_with_no_servers() {
        let config = McpConfig::default();
        let manager = McpManager::connect_all(&config, None).await;
        assert_eq!(manager.connected_count(), 0);
        assert!(manager.tools().is_empty());
    }

    #[tokio::test]
    async fn test_connect_all_skips_disabled_servers() {
        let config = McpConfig {
            enabled: true,
            servers: vec![test_server_def("disabled", "nonexistent", false)],
            server: Default::default(),
        };
        let manager = McpManager::connect_all(&config, None).await;
        assert_eq!(manager.connected_count(), 0);
    }

    #[tokio::test]
    async fn test_connect_all_handles_connection_failure_gracefully() {
        let config = McpConfig {
            enabled: true,
            servers: vec![test_server_def(
                "bad-server",
                "nonexistent-binary-that-does-not-exist",
                true,
            )],
            server: Default::default(),
        };
        // Should not panic — logs warning and skips
        let manager = McpManager::connect_all(&config, None).await;
        assert_eq!(manager.connected_count(), 0);
    }

    #[tokio::test]
    async fn test_connect_all_emits_events() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let config = McpConfig {
            enabled: true,
            servers: vec![
                test_server_def("disabled-srv", "cmd", false),
                test_server_def("bad-srv", "nonexistent-binary", true),
            ],
            server: Default::default(),
        };

        let _manager = McpManager::connect_all(&config, Some(tx)).await;

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should have: Skipped + Starting + Failed + Complete
        assert!(events
            .iter()
            .any(|e| matches!(e, McpStartupEvent::Skipped { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, McpStartupEvent::Starting { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, McpStartupEvent::Failed { .. })));
        assert!(events
            .iter()
            .any(|e| matches!(e, McpStartupEvent::Complete { .. })));
    }
}
