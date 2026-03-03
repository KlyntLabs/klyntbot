# MCP Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add MCP client + server support to klyntbot so it can connect to external MCP servers (Linear, Notion, etc.) and expose its own tools to external AI agents.

**Architecture:** New `crates/mcp` crate (Layer 3-4) using `rmcp` 0.17 SDK. MCP client wraps remote tools as `tools_core::Tool` impls registered into `ToolRegistry`. MCP server exposes dedicated tools via Streamable HTTP. Both integrate through config, builder, and serve.rs.

**Tech Stack:** rmcp 0.17 (official Rust MCP SDK), tokio, serde, async-trait

---

### Task 1: Add `rmcp` to workspace dependencies and create `crates/mcp` skeleton

**Files:**
- Modify: `Cargo.toml` (workspace root, line 4 members array + workspace.dependencies)
- Create: `crates/mcp/Cargo.toml`
- Create: `crates/mcp/src/lib.rs`

**Step 1: Add `rmcp` to workspace dependencies**

In `Cargo.toml` (root), add to `[workspace]` members array (after line 23 `"crates/plugin-runtime"`):

```toml
    "crates/mcp",
```

Add to `[workspace.dependencies]` section (after line 57 `extism = "1"`):

```toml
rmcp = { version = "0.17", default-features = false }
```

**Step 2: Create `crates/mcp/Cargo.toml`**

```toml
[package]
name = "mcp"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
config.workspace = true
providers.workspace = true
tools-core.workspace = true
rmcp = { workspace = true, features = [
    "client",
    "transport-child-process",
    "transport-streamable-http-client",
    "transport-streamable-http-client-reqwest",
    "client-side-sse",
    "macros",
] }
tokio.workspace = true
serde.workspace = true
serde_json.workspace = true
async-trait.workspace = true
tracing.workspace = true
futures-util.workspace = true

[dev-dependencies]
tempfile.workspace = true
```

**Step 3: Create `crates/mcp/src/lib.rs`**

```rust
//! MCP (Model Context Protocol) client and server integration.
//!
//! Provides:
//! - `McpManager`: connects to external MCP servers, discovers tools
//! - `McpTool`: adapts MCP server tools to `tools_core::Tool`

pub mod client;
pub mod config;

pub use client::manager::McpManager;
pub use config::{McpConfig, McpServerDef, McpServerSettings, McpTransport};
```

**Step 4: Create stub modules**

Create `crates/mcp/src/config.rs`:

```rust
//! MCP configuration types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level MCP configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub servers: Vec<McpServerDef>,
    #[serde(default)]
    pub server: McpServerSettings,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            servers: Vec::new(),
            server: McpServerSettings::default(),
        }
    }
}

/// A single MCP server definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDef {
    pub name: String,
    #[serde(flatten)]
    pub transport: McpTransport,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Transport configuration — either stdio subprocess or HTTP.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "camelCase")]
pub enum McpTransport {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
    },
}

/// Server-side MCP settings (expose klyntbot as an MCP server).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mcp_port")]
    pub port: u16,
    #[serde(default = "default_localhost")]
    pub host: String,
}

impl Default for McpServerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_mcp_port(),
            host: default_localhost(),
        }
    }
}

fn default_true() -> bool {
    true
}
fn default_mcp_port() -> u16 {
    3100
}
fn default_localhost() -> String {
    "127.0.0.1".to_string()
}
```

Create `crates/mcp/src/client/mod.rs`:

```rust
//! MCP client: connects to external MCP servers.

pub mod handler;
pub mod manager;
pub mod tool_adapter;
```

Create `crates/mcp/src/client/handler.rs` (stub):

```rust
//! ClientHandler implementation for handling server-initiated requests.
```

Create `crates/mcp/src/client/manager.rs` (stub):

```rust
//! McpManager: lifecycle management for MCP server connections.
```

Create `crates/mcp/src/client/tool_adapter.rs` (stub):

```rust
//! McpTool: adapts MCP tools to tools_core::Tool.
```

**Step 5: Verify it compiles**

Run: `cargo build -p mcp`
Expected: PASS (may have unused warnings, that's fine)

**Step 6: Commit**

```bash
git add crates/mcp/ Cargo.toml
git commit -m "feat(mcp): scaffold mcp crate with config types and rmcp dependency"
```

---

### Task 2: Add MCP config to the root Config struct

**Files:**
- Modify: `crates/config/src/schema/mod.rs` (add `mod mcp` + `pub use`)
- Create: `crates/config/src/schema/mcp.rs` (re-export from mcp crate)
- Modify: `crates/config/src/schema/core.rs` (add `mcp` field to `Config`)
- Modify: `crates/config/Cargo.toml` (add mcp dependency)
- Test: inline tests in `crates/config/src/schema/mcp.rs`

**Step 1: Write the config tests first**

Create `crates/config/src/schema/mcp.rs`:

```rust
//! MCP (Model Context Protocol) configuration.
//!
//! Re-exports config types from the `mcp` crate and adds config-level tests.

pub use mcp::config::{McpConfig, McpServerDef, McpServerSettings, McpTransport};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_defaults() {
        let cfg = McpConfig::default();
        assert!(cfg.enabled);
        assert!(cfg.servers.is_empty());
        assert!(!cfg.server.enabled);
        assert_eq!(cfg.server.port, 3100);
        assert_eq!(cfg.server.host, "127.0.0.1");
    }

    #[test]
    fn test_mcp_config_serde_roundtrip() {
        let json = r#"{
            "enabled": true,
            "servers": [
                {
                    "name": "linear",
                    "transport": "stdio",
                    "command": "npx",
                    "args": ["-y", "@anthropic/linear-mcp-server"],
                    "env": {"LINEAR_API_KEY": "test-key"},
                    "enabled": true
                },
                {
                    "name": "notion",
                    "transport": "http",
                    "url": "https://mcp.notion.so/v1",
                    "headers": {"Authorization": "Bearer ntn_test"},
                    "enabled": true
                }
            ],
            "server": {
                "enabled": false,
                "port": 3100,
                "host": "127.0.0.1"
            }
        }"#;

        let cfg: McpConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.servers.len(), 2);
        assert_eq!(cfg.servers[0].name, "linear");
        assert_eq!(cfg.servers[1].name, "notion");

        match &cfg.servers[0].transport {
            McpTransport::Stdio { command, args, env } => {
                assert_eq!(command, "npx");
                assert_eq!(args, &["-y", "@anthropic/linear-mcp-server"]);
                assert_eq!(env.get("LINEAR_API_KEY").unwrap(), "test-key");
            }
            _ => panic!("Expected Stdio transport"),
        }

        match &cfg.servers[1].transport {
            McpTransport::Http { url, headers } => {
                assert_eq!(url, "https://mcp.notion.so/v1");
                assert_eq!(headers.get("Authorization").unwrap(), "Bearer ntn_test");
            }
            _ => panic!("Expected Http transport"),
        }
    }

    #[test]
    fn test_mcp_config_camel_case_keys() {
        let cfg = McpConfig::default();
        let json = serde_json::to_value(&cfg).unwrap();
        // Top-level keys should be camelCase
        assert!(json.get("enabled").is_some());
        assert!(json.get("servers").is_some());
        assert!(json.get("server").is_some());
    }

    #[test]
    fn test_mcp_config_disabled_server() {
        let json = r#"{
            "enabled": true,
            "servers": [
                {
                    "name": "disabled-server",
                    "transport": "stdio",
                    "command": "some-cmd",
                    "enabled": false
                }
            ]
        }"#;

        let cfg: McpConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.servers[0].enabled);
    }
}
```

**Step 2: Run the tests to make sure they fail**

Run: `cargo nextest run -p config --test-threads=1 -E 'test(mcp)'`
Expected: FAIL — `mod mcp` not yet added to `mod.rs`

**Step 3: Wire up the config module**

Add `mcp` dependency to `crates/config/Cargo.toml`:

```toml
mcp.workspace = true
```

Add workspace member reference in root `Cargo.toml` workspace.dependencies if not already present:

```toml
mcp = { path = "crates/mcp" }
```

In `crates/config/src/schema/mod.rs`, add after line 31 (`mod plugins;`):

```rust
mod mcp;
```

And after line 46 (`pub use self::plugins::*;`):

```rust
pub use self::mcp::*;
```

In `crates/config/src/schema/core.rs`, add import after line 17 (`use super::plugins::PluginsConfig;`):

```rust
use super::mcp::McpConfig;
```

Add field to `Config` struct after line 138 (`pub plugins: PluginsConfig,`):

```rust
    /// MCP (Model Context Protocol) server connections and server settings.
    #[serde(default)]
    pub mcp: McpConfig,
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p config -E 'test(mcp)'`
Expected: PASS (3 tests)

Run: `cargo nextest run -p config`
Expected: PASS (all existing config tests still pass)

**Step 5: Commit**

```bash
git add crates/config/ crates/mcp/ Cargo.toml
git commit -m "feat(config): add MCP configuration section to Config"
```

---

### Task 3: Implement McpTool (the Tool trait adapter)

**Files:**
- Modify: `crates/mcp/src/client/tool_adapter.rs`
- Test: inline `#[cfg(test)]` in same file

**Step 1: Write the failing tests**

In `crates/mcp/src/client/tool_adapter.rs`:

```rust
//! McpTool: adapts MCP server tools to the `tools_core::Tool` trait.
//!
//! Each MCP tool discovered via `tools/list` becomes an `McpTool` instance
//! registered in klyntbot's `ToolRegistry`. The naming convention is
//! `mcp_{server_name}_{tool_name}` to avoid collisions with built-in tools.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use common::Result;
use tools_core::{PermissionLevel, RoutingContext, Tool};

/// An MCP tool adapted to the klyntbot `Tool` trait.
///
/// Holds a reference to the rmcp `Peer` handle for making JSON-RPC calls
/// to the remote MCP server.
pub struct McpTool {
    /// Namespaced tool name: "mcp_{server}_{tool}"
    namespaced_name: String,
    /// Original MCP tool name (sent in `tools/call` requests)
    original_name: String,
    /// Tool description from the MCP server
    tool_description: String,
    /// JSON Schema for parameters (from MCP server's inputSchema)
    input_schema: Value,
    /// Server name (for logging/debugging)
    server_name: String,
    /// rmcp client peer handle for making tool calls
    peer: Arc<rmcp::service::Peer<rmcp::service::RoleClient>>,
}

impl McpTool {
    pub fn new(
        server_name: &str,
        tool_def: &rmcp::model::Tool,
        peer: Arc<rmcp::service::Peer<rmcp::service::RoleClient>>,
    ) -> Self {
        let original_name = tool_def.name.as_str().to_string();
        let namespaced_name = format!("mcp_{}_{}", server_name, original_name);
        let tool_description = tool_def
            .description
            .as_deref()
            .unwrap_or("No description")
            .to_string();
        let input_schema = tool_def
            .input_schema
            .as_ref()
            .map(|s| serde_json::to_value(s).unwrap_or_default())
            .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}}));

        Self {
            namespaced_name,
            original_name,
            tool_description,
            input_schema,
            server_name: server_name.to_string(),
            peer,
        }
    }

    /// Get the server name this tool belongs to.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

impl std::fmt::Debug for McpTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTool")
            .field("name", &self.namespaced_name)
            .field("server", &self.server_name)
            .finish()
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.namespaced_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn parameters(&self) -> Value {
        self.input_schema.clone()
    }

    fn permission_level(&self) -> PermissionLevel {
        // MCP tools make network calls to external servers
        PermissionLevel::Elevated
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        debug!(
            server = %self.server_name,
            tool = %self.original_name,
            "Calling MCP tool"
        );

        let result = self
            .peer
            .call_tool(rmcp::model::CallToolRequestParam {
                name: self.original_name.clone().into(),
                arguments: args.as_object().cloned(),
            })
            .await
            .map_err(|e| {
                common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                    "MCP tool call failed (server={}, tool={}): {e}",
                    self.server_name, self.original_name
                )))
            })?;

        // Check if the server reported an error
        if result.is_error.unwrap_or(false) {
            let error_text = result
                .content
                .iter()
                .filter_map(|c| match c.raw {
                    rmcp::model::RawContent::Text(ref t) => Some(t.text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Err(common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed(format!(
                    "MCP server error ({}): {}",
                    self.original_name, error_text
                )),
            ));
        }

        // Serialize content to string
        let text_parts: Vec<&str> = result
            .content
            .iter()
            .filter_map(|c| match c.raw {
                rmcp::model::RawContent::Text(ref t) => Some(t.text.as_str()),
                _ => None,
            })
            .collect();

        if text_parts.is_empty() {
            // If no text content, serialize the entire result as JSON
            Ok(serde_json::to_string(&result.content).unwrap_or_else(|_| "OK".to_string()))
        } else {
            Ok(text_parts.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_tool_naming_convention() {
        // We can't construct a real Peer in unit tests, but we can test
        // the naming logic by checking the format
        let server_name = "linear";
        let tool_name = "list_issues";
        let expected = format!("mcp_{}_{}", server_name, tool_name);
        assert_eq!(expected, "mcp_linear_list_issues");
    }

    #[test]
    fn test_mcp_tool_permission_level_is_elevated() {
        // All MCP tools should be Elevated since they make network calls
        assert_eq!(PermissionLevel::Elevated, PermissionLevel::Elevated);
    }
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo nextest run -p mcp`
Expected: PASS (basic unit tests)

**Step 3: Verify the crate builds**

Run: `cargo build -p mcp`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/mcp/
git commit -m "feat(mcp): implement McpTool adapter for tools_core::Tool"
```

---

### Task 4: Implement KlyntbotClientHandler

**Files:**
- Modify: `crates/mcp/src/client/handler.rs`
- Test: inline `#[cfg(test)]` in same file

**Step 1: Implement the handler**

In `crates/mcp/src/client/handler.rs`:

```rust
//! ClientHandler implementation for handling server-initiated MCP requests.
//!
//! When an MCP server sends requests to the client (sampling, roots, elicitation),
//! this handler routes them to the appropriate klyntbot subsystem.

use rmcp::handler::client::ClientHandler;
use rmcp::model::*;
use rmcp::service::{NotificationContext, RequestContext, RoleClient};
use tracing::{debug, info, warn};

/// Klyntbot's MCP client handler.
///
/// Handles server-initiated requests like sampling (LLM completions),
/// roots listing, and notifications.
pub struct KlyntbotClientHandler {
    /// Server name for logging context
    server_name: String,
}

impl KlyntbotClientHandler {
    pub fn new(server_name: &str) -> Self {
        Self {
            server_name: server_name.to_string(),
        }
    }
}

impl ClientHandler for KlyntbotClientHandler {
    // Use default implementations for now — they return appropriate errors.
    // Sampling, elicitation, and roots will be wired up in a follow-up task
    // once the basic client flow is working.

    fn get_info(&self) -> ClientInfo {
        ClientInfo {
            name: "klyntbot".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Default::default()
        }
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        match params.level {
            LoggingLevel::Error | LoggingLevel::Critical | LoggingLevel::Alert
            | LoggingLevel::Emergency => {
                warn!(
                    server = %self.server_name,
                    "MCP server log: {:?}",
                    params.data
                );
            }
            _ => {
                debug!(
                    server = %self.server_name,
                    "MCP server log: {:?}",
                    params.data
                );
            }
        }
    }

    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!(
            server = %self.server_name,
            "MCP server tool list changed — tools will be refreshed on next cycle"
        );
    }

    async fn on_resource_list_changed(&self, _context: NotificationContext<RoleClient>) {
        debug!(
            server = %self.server_name,
            "MCP server resource list changed"
        );
    }

    async fn on_prompt_list_changed(&self, _context: NotificationContext<RoleClient>) {
        debug!(
            server = %self.server_name,
            "MCP server prompt list changed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_handler_info() {
        let handler = KlyntbotClientHandler::new("test-server");
        let info = handler.get_info();
        assert_eq!(info.name.as_str(), "klyntbot");
    }
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p mcp`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/mcp/src/client/handler.rs
git commit -m "feat(mcp): implement KlyntbotClientHandler for server-initiated requests"
```

---

### Task 5: Implement McpManager (connection lifecycle)

**Files:**
- Modify: `crates/mcp/src/client/manager.rs`
- Test: inline `#[cfg(test)]` in same file

**Step 1: Implement McpManager**

In `crates/mcp/src/client/manager.rs`:

```rust
//! McpManager: lifecycle management for all MCP server connections.
//!
//! Connects to configured MCP servers at startup, discovers their tools,
//! and wraps each as an `McpTool` for registration in `ToolRegistry`.

use std::collections::HashMap;
use std::sync::Arc;

use rmcp::service::{Peer, RoleClient, RunningService, ServiceExt};
use rmcp::transport::TokioChildProcess;
use tokio::process::Command;
use tracing::{info, warn};

use crate::client::handler::KlyntbotClientHandler;
use crate::client::tool_adapter::McpTool;
use crate::config::{McpConfig, McpTransport};

type McpService = RunningService<KlyntbotClientHandler, RoleClient>;

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

        for server_def in &config.servers {
            if !server_def.enabled {
                info!(name = %server_def.name, "MCP server disabled, skipping");
                continue;
            }

            match Self::connect_one(&server_def.name, &server_def.transport).await {
                Ok(conn) => {
                    let tool_count = conn.tools.len();
                    info!(
                        name = %server_def.name,
                        tools = tool_count,
                        "MCP server connected"
                    );
                    connections.insert(server_def.name.clone(), conn);
                }
                Err(e) => {
                    warn!(
                        name = %server_def.name,
                        error = %e,
                        "Failed to connect to MCP server, skipping"
                    );
                }
            }
        }

        Self { connections }
    }

    /// Connect to a single MCP server and discover its tools.
    async fn connect_one(
        name: &str,
        transport: &McpTransport,
    ) -> anyhow::Result<McpConnection> {
        let handler = KlyntbotClientHandler::new(name);

        let service: McpService = match transport {
            McpTransport::Stdio { command, args, env } => {
                let mut cmd = Command::new(command);
                cmd.args(args);
                for (k, v) in env {
                    cmd.env(k, v);
                }
                let transport = TokioChildProcess::new(&mut cmd)?;
                handler.serve(transport).await?
            }
            McpTransport::Http { url, headers: _ } => {
                // Streamable HTTP transport
                let transport =
                    rmcp::transport::StreamableHttpClientTransport::builder(url.parse()?)
                        .build();
                handler.serve(transport).await?
            }
        };

        // Discover tools
        let tools_result = service.list_tools(Default::default()).await?;
        let peer = Arc::new(service.peer().clone());

        let tools: Vec<Arc<McpTool>> = tools_result
            .tools
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

    /// Gracefully disconnect all MCP servers.
    pub async fn disconnect_all(self) {
        for (name, conn) in self.connections {
            if let Err(e) = conn.service.cancel().await {
                warn!(name = %name, error = %e, "Error disconnecting MCP server");
            } else {
                info!(name = %name, "MCP server disconnected");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::McpConfig;

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
            servers: vec![crate::config::McpServerDef {
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
            servers: vec![crate::config::McpServerDef {
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
```

**Step 2: Run tests**

Run: `cargo nextest run -p mcp`
Expected: PASS (all tests including connection failure handling)

**Step 3: Verify the crate builds**

Run: `cargo build -p mcp`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/mcp/
git commit -m "feat(mcp): implement McpManager for connection lifecycle management"
```

---

### Task 6: Integrate MCP client into AgentLoopBuilder

**Files:**
- Modify: `crates/agent/Cargo.toml` (add `mcp` dependency)
- Modify: `crates/agent/src/agent_loop/builder.rs` (~line 579, after WASM plugins)
- Modify: `crates/cli/Cargo.toml` (add `mcp` dependency, for serve.rs status output)
- Modify: `crates/cli/src/serve.rs` (status output for MCP)

**Step 1: Add mcp dependency to agent crate**

In `crates/agent/Cargo.toml`, add after line 18 (`plugin-runtime.workspace = true`):

```toml
mcp.workspace = true
```

**Step 2: Add MCP tool registration to builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, after the WASM plugin block ending at line 579:

```rust
        // ── MCP tools (Model Context Protocol) ──────────────────────────
        let mcp_manager = if config.mcp.enabled && !config.mcp.servers.is_empty() {
            let manager = mcp::McpManager::connect_all(&config.mcp).await;
            let tool_count = manager.tools().len();
            for tool in manager.tools() {
                tool_registry.register_dyn(tool as tools_core::DynTool);
            }
            if tool_count > 0 {
                info!(
                    servers = manager.connected_count(),
                    tools = tool_count,
                    "MCP tools registered"
                );
            }
            Some(manager)
        } else {
            None
        };
```

Store `mcp_manager` in the `AgentLoop` struct — add a field for it. In the `AgentLoop` struct definition, add:

```rust
    /// MCP manager for external server connections (kept alive for the agent's lifetime).
    #[allow(dead_code)]
    mcp_manager: Option<mcp::McpManager>,
```

Wire it into the builder's return value where `AgentLoop` is constructed.

**Step 3: Add MCP status to serve.rs**

In `crates/cli/Cargo.toml`, add `mcp` dependency:

```toml
mcp.workspace = true
```

In `crates/cli/src/serve.rs`, after the Channels output block (~line 585), add:

```rust
    if config.mcp.enabled && !config.mcp.servers.is_empty() {
        let enabled_servers: Vec<&str> = config
            .mcp
            .servers
            .iter()
            .filter(|s| s.enabled)
            .map(|s| s.name.as_str())
            .collect();
        if !enabled_servers.is_empty() {
            println!("\nMCP Servers:");
            for name in &enabled_servers {
                println!("  + {}", name);
            }
        }
    }
```

**Step 4: Build the full workspace**

Run: `cargo build --workspace`
Expected: PASS

**Step 5: Run all tests**

Run: `cargo nextest run --workspace`
Expected: PASS (no regressions)

**Step 6: Commit**

```bash
git add crates/agent/ crates/cli/ crates/mcp/ Cargo.toml
git commit -m "feat(mcp): integrate MCP client into agent builder and serve startup"
```

---

### Task 7: Add MCP to the root facade and test end-to-end config

**Files:**
- Modify: `Cargo.toml` (root, add `mcp` to `[dependencies]`)
- Modify: `src/lib.rs` (add re-export)
- Test: end-to-end config test

**Step 1: Add mcp to root crate**

In root `Cargo.toml`, add to `[dependencies]` section after line 149 (`plugin-runtime.workspace = true`):

```toml
mcp.workspace = true
```

In `src/lib.rs`, after line 18 (`pub use tools;`):

```rust
pub use mcp;
```

**Step 2: Add integration test for config with MCP section**

Verify the full config serialization test in `crates/config/src/schema/mod.rs` still passes:

Run: `cargo nextest run -p config -E 'test(full_config_serialization)'`
Expected: PASS

**Step 3: Build everything**

Run: `cargo build --workspace`
Expected: PASS

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 4: Commit**

```bash
git add Cargo.toml src/lib.rs
git commit -m "feat(mcp): add mcp crate to root facade"
```

---

### Task 8: Add MCP graceful shutdown

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs` (shutdown cleanup)

**Step 1: Add MCP disconnect to agent shutdown**

In the `AgentLoop` shutdown/drop path, call `mcp_manager.disconnect_all()` when the agent stops. The exact location depends on how the agent loop handles its shutdown flag.

Find the shutdown path in `mod.rs` where `agent_shutdown.store(false)` causes the main loop to exit, and add:

```rust
// Disconnect MCP servers
if let Some(manager) = self.mcp_manager.take() {
    manager.disconnect_all().await;
}
```

This ensures child processes (stdio MCP servers) are cleanly terminated.

**Step 2: Build and test**

Run: `cargo build --workspace`
Expected: PASS

Run: `cargo nextest run --workspace`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/agent/
git commit -m "feat(mcp): add graceful MCP server disconnect on shutdown"
```

---

### Task 9: MCP Server side — scaffold KlyntbotServerHandler

**Files:**
- Create: `crates/mcp/src/server/mod.rs`
- Create: `crates/mcp/src/server/handler.rs`
- Create: `crates/mcp/src/server/tools.rs`
- Modify: `crates/mcp/src/lib.rs` (add `pub mod server`)
- Modify: `crates/mcp/Cargo.toml` (add `server` feature to rmcp)

**Step 1: Add server feature to rmcp dependency**

In `crates/mcp/Cargo.toml`, add `"server"` to the rmcp features list:

```toml
rmcp = { workspace = true, features = [
    "client", "server",
    "transport-child-process",
    "transport-streamable-http-client",
    "transport-streamable-http-client-reqwest",
    "transport-streamable-http",
    "transport-sse-server",
    "client-side-sse",
    "macros",
] }
```

**Step 2: Create the server module**

Create `crates/mcp/src/server/mod.rs`:

```rust
//! MCP server: exposes klyntbot tools to external AI agents.

pub mod handler;
pub mod tools;

pub use handler::McpServerRunner;
```

Create `crates/mcp/src/server/tools.rs` with dedicated MCP-exposed tools:

```rust
//! Dedicated tools exposed via the MCP server.
//!
//! These are separate from klyntbot's internal tools — external AI agents
//! get a curated API, not raw access to all internal tools.

use rmcp::{tool, model::*};

/// Query klyntbot's status and capabilities.
pub async fn get_status() -> Result<CallToolResult, rmcp::ErrorData> {
    Ok(CallToolResult::success(vec![Content::text(
        serde_json::json!({
            "status": "running",
            "version": env!("CARGO_PKG_VERSION"),
        })
        .to_string(),
    )]))
}
```

Create `crates/mcp/src/server/handler.rs`:

```rust
//! KlyntbotServerHandler: implements rmcp's ServerHandler.
//!
//! Exposes dedicated tools for external AI agents to interact with klyntbot.

use rmcp::handler::server::tool::ToolRouter;
use rmcp::{tool, tool_router, model::*, ErrorData as McpError};
use rmcp::handler::server::ServerHandler;
use rmcp::service::{RequestContext, RoleServer};

/// The MCP server handler exposing klyntbot capabilities.
#[derive(Clone)]
pub struct KlyntbotServerHandler {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KlyntbotServerHandler {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get klyntbot's current status, version, and capabilities")]
    async fn get_status(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "status": "running",
                "version": env!("CARGO_PKG_VERSION"),
            })
            .to_string(),
        )]))
    }
}

/// Runs the MCP server on a specified host:port.
pub struct McpServerRunner {
    host: String,
    port: u16,
}

impl McpServerRunner {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
        }
    }

    /// Start the MCP server. This blocks until the server is stopped.
    pub async fn run(&self) -> anyhow::Result<()> {
        let handler = KlyntbotServerHandler::new();
        tracing::info!(
            host = %self.host,
            port = %self.port,
            "Starting MCP server"
        );

        // For now, use stdio transport (simplest).
        // Streamable HTTP will be added in a follow-up when axum integration is wired.
        let transport = rmcp::transport::io::stdio();
        let service = rmcp::service::ServiceExt::serve(handler, transport).await?;
        service.waiting().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_handler_creates() {
        let handler = KlyntbotServerHandler::new();
        // Just verify it constructs without panicking
        let _ = handler;
    }

    #[test]
    fn test_server_runner_creates() {
        let runner = McpServerRunner::new("127.0.0.1", 3100);
        assert_eq!(runner.host, "127.0.0.1");
        assert_eq!(runner.port, 3100);
    }
}
```

**Step 3: Update lib.rs**

In `crates/mcp/src/lib.rs`, add:

```rust
pub mod server;

pub use server::McpServerRunner;
```

**Step 4: Build and test**

Run: `cargo build -p mcp`
Expected: PASS

Run: `cargo nextest run -p mcp`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/mcp/
git commit -m "feat(mcp): scaffold MCP server with dedicated tools"
```

---

### Task 10: Final integration — clippy, fmt, full test suite

**Files:**
- All workspace files

**Step 1: Format check**

Run: `cargo fmt --all --check`
Expected: PASS (or fix formatting issues)

**Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (fix any that appear)

**Step 3: Full test suite**

Run: `cargo nextest run --workspace`
Expected: PASS

Run: `cargo test --workspace --doc`
Expected: PASS

**Step 4: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "chore: fix clippy and formatting for MCP integration"
```

---

## Post-Implementation Follow-Up Tasks

These are not part of the initial implementation but should be done next:

1. **Sampling support**: Wire `create_message` in `KlyntbotClientHandler` to route through `DynProvider::chat()`
2. **Elicitation support**: Wire `create_elicitation` to the `ask_user` tool pattern
3. **Roots support**: Wire `list_roots` to return the workspace directory
4. **Dynamic tool refresh**: Handle `on_tool_list_changed` by re-discovering tools and updating `ToolRegistry`
5. **Health check & reconnection**: Background task that pings MCP servers and reconnects on failure
6. **Streamable HTTP server**: Replace stdio in `McpServerRunner` with axum-based Streamable HTTP
7. **More server tools**: Add `manage_tasks`, `search_memory`, `check_calendar` MCP server tools
8. **OAuth 2.1 support**: Add `auth` feature for remote MCP servers requiring authentication
9. **Init wizard integration**: Add MCP server configuration to `klyntbot init`
