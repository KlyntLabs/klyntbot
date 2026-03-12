# MCP Server Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn klyntbot into an MCP server so external AI agents (Claude Code, Codex) can access klyntbot's business logic as tools via stdio or HTTP transport.

**Architecture:** Hybrid bridge — `ToolRegistryBridge` reuses internal tools via `ToolRegistry` for headless CRUD, `AgentBridge` calls `AppCore::chat_send()` for natural language delegation. New `klyntbot-server` crate (L7) is both a library (for desktop embedding) and a binary (`klyntbot-mcp`). `AppMode` enum gates init phases so Server mode skips channels/productivity/coaching.

**Tech Stack:** Rust, rmcp 0.17 (MCP protocol), clap (CLI), tokio (async), axum (HTTP via rmcp's streamable-http-server)

**Spec:** `docs/superpowers/specs/2026-03-12-mcp-server-design.md`

---

## Chunk 1: Foundation — AppMode + Crate Skeleton

### Task 1: Add `AppMode` enum to `common` crate

**Files:**
- Modify: `crates/common/src/types.rs`
- Modify: `crates/common/src/lib.rs`

- [ ] **Step 1: Add `AppMode` enum**

In `crates/common/src/types.rs`, add after the `SessionKey` impl block (~line 95):

```rust
/// Runtime mode for the application entry point.
/// Controls which subsystems are initialized during `AppCore::init()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    /// Full desktop app: channels, productivity, coaching, everything.
    #[default]
    Desktop,
    /// Headless MCP server: storage, agent, cron only.
    Server,
}
```

- [ ] **Step 2: Re-export `AppMode`**

In `crates/common/src/lib.rs`, add `AppMode` to the `types` re-export:

```rust
pub use types::{
    AppMode, ChannelName, ChatId, MessageRole, SessionKey, CLI_CHANNEL, SYSTEM_CHANNEL,
    TELEGRAM_RESET_SENDER,
};
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p common`
Expected: compiles with 0 errors

- [ ] **Step 4: Commit**

```bash
git add crates/common/src/types.rs crates/common/src/lib.rs
git commit -m "feat(common): add AppMode enum for Desktop/Server init gating"
```

---

### Task 2: Gate `AppCore::init()` with `AppMode`

**Files:**
- Modify: `crates/app-core/src/init.rs`
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/desktop/src/app_core.rs`

- [ ] **Step 1: Add `mode` field to `AppCore` struct**

In `crates/app-core/src/state.rs`, add `mode: common::AppMode` as the first field of `AppCore`:

```rust
pub struct AppCore {
    pub mode: common::AppMode,
    pub repos: Repos,
    // ... rest unchanged
}
```

- [ ] **Step 2: Change `init()` and `init_with_sender()` signatures**

In `crates/app-core/src/init.rs`, change both signatures:

```rust
pub async fn init(
    mode: common::AppMode,
    config_override: Option<config::Config>,
) -> Result<(Self, EventChannels), String> {
    Self::init_with_sender(mode, config_override, None).await
}

pub async fn init_with_sender(
    mode: common::AppMode,
    config_override: Option<config::Config>,
    notification_sender: Option<Arc<dyn common::NotificationSender>>,
) -> Result<(Self, EventChannels), String> {
```

- [ ] **Step 3: Gate coaching init behind `AppMode::Desktop`**

In `init_with_sender()`, find the coaching init block (~line 358-403). Wrap it:

```rust
// Always create the intervention channel pair (EventChannels requires it).
let (intervention_tx, intervention_rx) =
    mpsc::channel::<feature_coaching::router::DeliveredIntervention>(64);

let (signal_accumulator, pattern_detector, intervention_router, feedback_tracker, coaching_service) =
    if mode == common::AppMode::Desktop {
        // Initialize coaching engine state.
        let signal_accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
        let pattern_detector = Arc::new(Mutex::new(PatternDetector::new()));
        let intervention_router = Arc::new(Mutex::new(InterventionRouter::new(Default::default())));
        let coaching_repo = storage::CoachingStrategyRepo::new(storage_pool.inner().clone());
        let mut tracker = FeedbackTracker::new().with_repo(coaching_repo);
        tracker.load_from_db().await;
        let feedback_tracker = Arc::new(Mutex::new(tracker));

        // Compute real user situation
        {
            let real_situation = build_situation_inputs(
                productivity_repos.as_ref(),
                &repos,
                None,
            )
            .await;
            *user_situation.lock().await = real_situation;
        }

        // Start CoachingService
        let coaching_reasoner: Arc<dyn feature_coaching::CoachingReasonerHandler> =
            if let Some(ref cp) = cognitive_provider {
                let params = providers::cognitive_chat_params(&config, 1024);
                Arc::new(agent::cognitive_handlers::LlmCoachingReasonerHandler::new(
                    cp.clone(),
                    params,
                ))
            } else {
                Arc::new(agent::cognitive_handlers::HeuristicCoachingReasonerHandler)
            };

        let coaching_cancel = shutdown_token.child_token();
        let coaching_service = feature_coaching::CoachingService::start(
            domain_event_bus.subscribe(),
            signal_accumulator.clone(),
            pattern_detector.clone(),
            intervention_router.clone(),
            feedback_tracker.clone(),
            user_situation.clone(),
            coaching_reasoner,
            intervention_tx,
            coaching_cancel,
        );
        info!("coaching service started");

        (
            Some(signal_accumulator),
            Some(pattern_detector),
            Some(intervention_router),
            Some(feedback_tracker),
            Some(coaching_service),
        )
    } else {
        // Server mode: drop intervention_tx so intervention_rx.recv() returns None immediately.
        drop(intervention_tx);
        info!("coaching service skipped (server mode)");
        (None, None, None, None, None)
    };
```

Remove the old standalone `let (intervention_tx, intervention_rx)` line that was inside the coaching block (~line 391-392), since it's now created above.

- [ ] **Step 4: Store `mode` in AppCore construction**

In the `AppCore { ... }` construction block, add `mode,` as the first field. The coaching fields are already `Option<...>`, so the destructured tuple values slot in directly.

- [ ] **Step 5: Update desktop caller**

In `crates/desktop/src/app_core.rs:22`, add the import and change the call:

```rust
// At the top of the file (if not already imported):
use common::AppMode;

// Change line 22:
let (core, channels) = AppCore::init_with_sender(
    AppMode::Desktop,
    None,
    Some(sender),
).await?;
```

- [ ] **Step 6: Verify all existing callers compile**

Run: `cargo build --workspace`
Expected: compiles. If other callers exist (dev_server, tests), update them with `AppMode::Desktop` as first arg.

- [ ] **Step 7: Run tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass (existing behavior unchanged when `AppMode::Desktop`)

- [ ] **Step 8: Commit**

```bash
git add crates/app-core/src/init.rs crates/app-core/src/state.rs crates/desktop/src/app_core.rs
git commit -m "feat(app-core): gate init phases with AppMode — skip coaching/channels in Server mode"
```

---

### Task 3: Add `ToolRegistry` public accessor to `AgentLoop`

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs`

- [ ] **Step 1: Add public accessor method**

In `crates/agent/src/agent_loop/mod.rs`, in the `impl AgentLoop` block (after ~line 84), add:

```rust
/// Public accessor for the tool registry.
/// Used by `klyntbot-server` to bridge internal tools to MCP.
pub fn tool_registry(&self) -> Arc<RwLock<tools::registry::ToolRegistry>> {
    Arc::clone(&self.tool_registry)
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p agent`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs
git commit -m "feat(agent): expose tool_registry() public accessor for MCP bridge"
```

---

### Task 4: Re-export `security` module from `mcp` crate

**Files:**
- Modify: `crates/mcp/src/lib.rs`

- [ ] **Step 1: Add security re-export**

In `crates/mcp/src/lib.rs`, add:

```rust
pub use server::security;
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build -p mcp`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add crates/mcp/src/lib.rs
git commit -m "feat(mcp): re-export server::security module for klyntbot-server"
```

---

### Task 5: Create `klyntbot-server` crate skeleton

**Files:**
- Create: `crates/klyntbot-server/Cargo.toml`
- Create: `crates/klyntbot-server/src/main.rs`
- Create: `crates/klyntbot-server/src/lib.rs`
- Create: `crates/klyntbot-server/src/cli.rs`
- Create: `crates/klyntbot-server/src/logging.rs`
- Create: `crates/klyntbot-server/src/handler.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create `Cargo.toml`**

```toml
[package]
name = "klyntbot-server"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[[bin]]
name = "klyntbot-mcp"
path = "src/main.rs"

[dependencies]
agent.workspace = true
app-core.workspace = true
desktop-shared.workspace = true
mcp.workspace = true
tools-core.workspace = true
common.workspace = true
config.workspace = true
rmcp = { workspace = true, features = [
    "server",
    "transport-io",
    "transport-streamable-http-server",
    "macros",
] }
tokio.workspace = true
clap = { version = "4", features = ["derive"] }
tracing.workspace = true
tracing-subscriber.workspace = true
anyhow.workspace = true
serde_json.workspace = true
uuid.workspace = true
```

- [ ] **Step 2: Create `src/cli.rs`**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "klyntbot-mcp", version, about = "Klyntbot MCP server")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start the MCP server
    Serve {
        /// Use stdio transport (for Claude Code / IDE integration)
        #[arg(long, group = "transport")]
        stdio: bool,

        /// Use HTTP transport
        #[arg(long, group = "transport")]
        http: bool,

        /// HTTP port (default: from config or 3100)
        #[arg(long)]
        port: Option<u16>,

        /// HTTP host (default: from config or 127.0.0.1)
        #[arg(long)]
        host: Option<String>,
    },
    /// Inspect available tools
    Tools {
        /// List all available tools
        #[arg(long)]
        list: bool,

        /// Show schema for a specific tool
        #[arg(long)]
        schema: Option<String>,
    },
}
```

- [ ] **Step 3: Create `src/logging.rs`**

```rust
use tracing_subscriber::EnvFilter;

/// Configure tracing for stdio transport — all output to stderr.
pub fn configure_stdio_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}

/// Configure tracing for HTTP transport — standard output.
pub fn configure_http_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();
}
```

- [ ] **Step 4: Create `src/handler.rs`** (minimal — just `get_status` for now)

```rust
//! MCP server handler — bridges rmcp protocol to klyntbot's AppCore.

use std::sync::Arc;

use app_core::AppCore;
use rmcp::handler::server::tool::{ToolCallContext, ToolRouter};
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router, ErrorData as McpError};

#[derive(Clone)]
pub struct KlyntbotServerHandler {
    #[allow(dead_code)]
    app: Arc<AppCore>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KlyntbotServerHandler {
    pub fn new(app: Arc<AppCore>) -> Self {
        Self {
            app,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get klyntbot's current status, version, and capabilities")]
    async fn get_status(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "status": "running",
                "version": env!("CARGO_PKG_VERSION"),
                "mode": format!("{:?}", self.app.mode),
            })
            .to_string(),
        )]))
    }
}

impl ServerHandler for KlyntbotServerHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability::default()),
                ..Default::default()
            },
            server_info: Implementation {
                name: "klyntbot".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            instructions: Some(
                "Klyntbot MCP server — personal AI agent with task management, memory, and productivity tools.".to_string(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tool_router.list_all(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let ctx = ToolCallContext::new(self, request, context);
        self.tool_router.call(ctx).await
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool_router.get(name).cloned()
    }
}
```

- [ ] **Step 5: Create `src/lib.rs`**

```rust
//! Klyntbot MCP server — library interface.
//!
//! Used by the `klyntbot-mcp` binary (standalone) and by the desktop crate
//! (embedded HTTP server sharing AppCore).

pub mod cli;
pub mod handler;
pub mod logging;

pub use handler::KlyntbotServerHandler;
```

- [ ] **Step 6: Create `src/main.rs`**

```rust
use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use rmcp::service::ServiceExt;

use klyntbot_server::cli::{Cli, Command};
use klyntbot_server::handler::KlyntbotServerHandler;
use klyntbot_server::logging;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve { stdio, http, port, host } => {
            if http {
                logging::configure_http_tracing();
            } else {
                // Default to stdio
                logging::configure_stdio_tracing();
            }

            // Load config
            let config = config::load_with_env_overrides()
                .await
                .map_err(|e| anyhow::anyhow!("config load failed: {e}"))?;

            // Init AppCore in Server mode
            let (app, events) = app_core::AppCore::init(
                common::AppMode::Server,
                Some(config.clone()),
            )
            .await
            .map_err(|e| anyhow::anyhow!("init failed: {e}"))?;
            let app = Arc::new(app);

            // Drain unused EventChannels — both receivers must close before task exits.
            // In Server mode, intervention_tx is dropped (coaching not started) so
            // intervention_rx closes immediately; pipeline_rx closes when domain_event_bus
            // senders are dropped at shutdown.
            tokio::spawn(async move {
                let mut intervention_rx = events.intervention_rx;
                let mut pipeline_rx = events.pipeline_rx;
                let mut intervention_closed = false;
                let mut pipeline_closed = false;
                while !intervention_closed || !pipeline_closed {
                    tokio::select! {
                        msg = intervention_rx.recv(), if !intervention_closed => {
                            if msg.is_none() { intervention_closed = true; }
                        }
                        result = pipeline_rx.recv(), if !pipeline_closed => {
                            if result.is_err() { pipeline_closed = true; }
                        }
                    }
                }
            });

            let handler = KlyntbotServerHandler::new(app.clone());

            if http {
                let bind_host = host.unwrap_or_else(|| config.mcp.server.host.clone());
                let bind_port = port.unwrap_or(config.mcp.server.port);
                tracing::info!("MCP HTTP server listening on {bind_host}:{bind_port}");
                // TODO: Wire rmcp streamable HTTP server (Phase 4)
                eprintln!("HTTP transport not yet implemented. Use --stdio.");
                std::process::exit(1);
            } else {
                tracing::info!("Starting MCP server (stdio)");
                let transport = rmcp::transport::io::stdio();
                let service = handler.serve(transport).await?;

                tokio::select! {
                    result = service.waiting() => {
                        if let Err(e) = result { eprintln!("Server error: {e}"); }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("Shutting down...");
                    }
                }

                app.shutdown().await;
            }
        }
        Command::Tools { list, schema } => {
            // TODO: Implement tool listing (Phase 2)
            if list {
                eprintln!("Tool listing not yet implemented.");
            }
            if let Some(name) = schema {
                eprintln!("Schema for '{name}' not yet implemented.");
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 7: Add to workspace**

In root `Cargo.toml`, add `"crates/klyntbot-server"` to the `members` array. Also add to workspace dependencies:

```toml
klyntbot-server = { path = "crates/klyntbot-server" }
```

- [ ] **Step 8: Verify the full workspace compiles**

Run: `cargo build --workspace`
Expected: compiles with 0 errors

- [ ] **Step 9: Run all tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass

- [ ] **Step 10: Commit**

```bash
git add crates/klyntbot-server/ Cargo.toml
git commit -m "feat(klyntbot-server): add crate skeleton with CLI, handler, and stdio transport"
```

---

### Task 6: Smoke test — run `klyntbot-mcp serve --stdio`

- [ ] **Step 1: Build the binary**

Run: `cargo build -p klyntbot-server`
Expected: binary at `target/debug/klyntbot-mcp`

- [ ] **Step 2: Test `--help`**

Run: `target/debug/klyntbot-mcp --help`
Expected: shows usage with `serve` and `tools` subcommands

- [ ] **Step 3: Test stdio server responds to initialize**

Run: `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | KLYNTBOT_HOME=/tmp/klyntbot-test target/debug/klyntbot-mcp serve --stdio 2>/dev/null | head -1`

Note: Verify the `protocolVersion` matches what rmcp 0.17's `ProtocolVersion::default()` returns. Adjust if needed.

Expected: JSON response with `"result"` containing `"serverInfo"` with `"name":"klyntbot"`

- [ ] **Step 4: Commit any fixes**

If adjustments were needed, commit them:
```bash
git add -A && git commit -m "fix(klyntbot-server): adjust stdio server initialization"
```

---

## Chunk 2: Tool Bridge — Schema Translation + ToolRegistryBridge

### Task 7: Schema translation module

**Files:**
- Create: `crates/klyntbot-server/src/bridge/mod.rs`
- Create: `crates/klyntbot-server/src/bridge/schema.rs`

- [ ] **Step 1: Write schema translation tests**

Create `crates/klyntbot-server/src/bridge/schema.rs`:

```rust
//! Translates internal tool schemas (OpenAI JSON Schema format) to rmcp MCP Tool definitions.

use rmcp::model::Tool as McpTool;

/// Convert an internal tool's schema to an MCP Tool definition.
///
/// Internal tools produce OpenAI-style JSON Schema via `Tool::parameters()`.
/// rmcp's `ToolInputSchema` is serde-deserializable from the same JSON Schema format,
/// so translation is a straightforward `from_value` conversion.
pub fn internal_to_mcp_tool(
    name: &str,
    description: &str,
    parameters: serde_json::Value,
) -> McpTool {
    let input_schema = serde_json::from_value(parameters)
        .unwrap_or_else(|_| {
            serde_json::from_value(serde_json::json!({"type": "object", "properties": {}}))
                .expect("static schema is always valid")
        });

    McpTool {
        name: name.into(),
        description: Some(description.to_string()),
        input_schema,
        annotations: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_schema_translation() {
        let params = serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["list", "create"] },
                "title": { "type": "string" }
            },
            "required": ["action"]
        });

        let tool = internal_to_mcp_tool("task", "Manage tasks", params);
        assert_eq!(tool.name.as_ref(), "task");
        assert_eq!(tool.description.as_deref(), Some("Manage tasks"));
        // Verify schema roundtrips correctly via JSON
        let schema_json = serde_json::to_value(&tool.input_schema).unwrap();
        assert!(schema_json["properties"]["action"].is_object());
        assert!(schema_json["properties"]["title"].is_object());
    }

    #[test]
    fn test_empty_schema_translation() {
        let params = serde_json::json!({});
        let tool = internal_to_mcp_tool("status", "Get status", params);
        assert_eq!(tool.name.as_ref(), "status");
        // Empty/minimal schema — should deserialize without error
        let schema_json = serde_json::to_value(&tool.input_schema).unwrap();
        assert!(schema_json.is_object());
    }
}
```

- [ ] **Step 2: Create bridge mod.rs**

```rust
pub mod schema;
pub mod registry;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p klyntbot-server`
Expected: schema tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/klyntbot-server/src/bridge/
git commit -m "feat(klyntbot-server): add schema translation (OpenAI → MCP Tool)"
```

---

### Task 8: ToolRegistryBridge

**Files:**
- Create: `crates/klyntbot-server/src/bridge/registry.rs`
- Modify: `crates/klyntbot-server/src/lib.rs`

- [ ] **Step 1: Write ToolRegistryBridge with tests**

Create `crates/klyntbot-server/src/bridge/registry.rs`:

```rust
//! Bridges klyntbot's internal ToolRegistry to MCP tool calls.
//!
//! Translates MCP `CallToolRequestParams` → internal `Tool::execute()` → `CallToolResult`.

use std::sync::Arc;

use common::{ChannelName, ChatId};
use rmcp::model::{CallToolResult, Content, Tool as McpTool};
use rmcp::ErrorData as McpError;
use tokio::sync::RwLock;
use tools_core::routing::RoutingContext;
use tools_core::registry::ToolRegistry;

use super::schema;

/// Bridges klyntbot's ToolRegistry to MCP protocol.
pub struct ToolRegistryBridge {
    registry: Arc<RwLock<ToolRegistry>>,
    whitelist: Vec<String>,
}

impl ToolRegistryBridge {
    pub fn new(registry: Arc<RwLock<ToolRegistry>>, whitelist: Vec<String>) -> Self {
        Self { registry, whitelist }
    }

    /// List all whitelisted tools as MCP Tool definitions.
    pub async fn list_tools(&self) -> Vec<McpTool> {
        let reg = self.registry.read().await;
        let mut tools = Vec::new();
        for name in &self.whitelist {
            if let Some(tool) = reg.get(name) {
                let params = tool.parameters();
                tools.push(schema::internal_to_mcp_tool(
                    tool.name(),
                    tool.description(),
                    params,
                ));
            }
        }
        tools
    }

    /// Execute a tool call via the internal registry.
    pub async fn execute(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        // Whitelist check
        if !self.whitelist.iter().any(|w| w == tool_name) {
            return Err(McpError::invalid_request(
                format!("Tool '{tool_name}' is not exposed via MCP"),
                None,
            ));
        }

        // Build MCP routing context — is_direct_mode: true per spec (responses
        // go directly to event stream, not via bus).
        let ctx = RoutingContext {
            channel: ChannelName::new("mcp"),
            chat_id: ChatId::new("mcp-session"),
            interaction_tx: None,
            is_direct_mode: true,
            delegation_depth: 0,
            entity_tx: None,
            interaction_channel: None,
        };

        // Execute via ToolRegistry (uses prepare() internally for validation)
        let reg = self.registry.read().await;
        match reg.execute(tool_name, arguments, &ctx).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result)])),
            Err(e) => {
                // Map tool errors to MCP responses
                match &e {
                    common::KlyntbotError::Tool(tools_core::ToolError::InvalidParams(msg)) => {
                        Err(McpError::invalid_params(msg.clone(), None))
                    }
                    _ => {
                        // Return as tool result with is_error flag, not protocol error
                        Ok(CallToolResult::error(vec![Content::text(e.to_string())]))
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitelist_rejects_unexposed_tool() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let registry = Arc::new(RwLock::new(ToolRegistry::new()));
            let bridge = ToolRegistryBridge::new(registry, vec!["task".into()]);

            let result = bridge
                .execute("read_file", serde_json::json!({}))
                .await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_whitelist_allows_exposed_tool() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let registry = Arc::new(RwLock::new(ToolRegistry::new()));
            let bridge = ToolRegistryBridge::new(registry, vec!["task".into()]);

            // Tool passes whitelist but is not registered → NotFound maps to
            // Ok(CallToolResult) with is_error=true (not Err(McpError)).
            let result = bridge
                .execute("task", serde_json::json!({"action": "list"}))
                .await;
            assert!(result.is_ok());
            let tool_result = result.unwrap();
            assert!(tool_result.is_error.unwrap_or(false));
        });
    }

    #[test]
    fn test_empty_whitelist_rejects_everything() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let registry = Arc::new(RwLock::new(ToolRegistry::new()));
            let bridge = ToolRegistryBridge::new(registry, vec![]);

            let result = bridge.execute("task", serde_json::json!({})).await;
            assert!(result.is_err());
        });
    }
}
```

- [ ] **Step 2: Update `src/lib.rs`**

Add `pub mod bridge;` to `src/lib.rs`:

```rust
pub mod bridge;
pub mod cli;
pub mod handler;
pub mod logging;

pub use handler::KlyntbotServerHandler;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p klyntbot-server`
Expected: all bridge + schema tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/klyntbot-server/src/bridge/registry.rs crates/klyntbot-server/src/lib.rs
git commit -m "feat(klyntbot-server): add ToolRegistryBridge with whitelist and error mapping"
```

---

### Task 9: Wire bridge into handler — dynamic tool listing + execution

**Files:**
- Modify: `crates/klyntbot-server/src/handler.rs`

- [ ] **Step 1: Rewrite handler to use bridge**

Replace `handler.rs` content to merge the built-in `get_status` tool with bridged tools:

```rust
//! MCP server handler — bridges rmcp protocol to klyntbot's AppCore.
//!
//! Combines built-in tools (get_status) with dynamically bridged tools
//! from klyntbot's internal ToolRegistry.

use std::sync::Arc;

use app_core::AppCore;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::ErrorData as McpError;

use crate::bridge::registry::ToolRegistryBridge;
use crate::bridge::schema;

pub struct KlyntbotServerHandler {
    app: Arc<AppCore>,
    bridge: ToolRegistryBridge,
}

impl KlyntbotServerHandler {
    pub fn new(app: Arc<AppCore>, whitelist: Vec<String>) -> Self {
        let registry = app.agent.tool_registry();
        let bridge = ToolRegistryBridge::new(registry, whitelist);
        Self { app, bridge }
    }

    /// Built-in get_status tool — always available.
    fn status_tool_def() -> Tool {
        serde_json::from_value(serde_json::json!({
            "name": "get_status",
            "description": "Get klyntbot's current status, version, and capabilities",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }))
        .expect("static tool definition is always valid")
    }

    async fn handle_get_status(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "status": "running",
                "version": env!("CARGO_PKG_VERSION"),
                "mode": format!("{:?}", self.app.mode),
            })
            .to_string(),
        )]))
    }
}

impl ServerHandler for KlyntbotServerHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability::default()),
                ..Default::default()
            },
            server_info: Implementation {
                name: "klyntbot".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            instructions: Some(
                "Klyntbot MCP server — personal AI agent with task management, memory, and productivity tools.".to_string(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut tools = vec![Self::status_tool_def()];
        tools.extend(self.bridge.list_tools().await);

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name = request.name.as_ref();
        let params = request.arguments.unwrap_or_default();

        match name {
            "get_status" => self.handle_get_status().await,
            _ => self.bridge.execute(name, params).await,
        }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        if name == "get_status" {
            return Some(Self::status_tool_def());
        }
        None // Dynamic tools are listed but not individually cached
    }
}
```

- [ ] **Step 2: Update `main.rs` to pass whitelist**

In `main.rs`, change handler creation:

```rust
let whitelist = config.mcp.server.exposed_tools.clone();
let handler = KlyntbotServerHandler::new(app.clone(), whitelist);
```

Note: `exposed_tools` field doesn't exist yet in config. This will fail to compile until Task 10 adds it. If you're building incrementally, use a hardcoded default for now:

```rust
let whitelist = vec![
    "task", "project", "area", "note", "memory",
    "objective", "key_result", "finance", "productivity", "work_context",
].into_iter().map(String::from).collect();
let handler = KlyntbotServerHandler::new(app.clone(), whitelist);
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p klyntbot-server`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add crates/klyntbot-server/src/handler.rs crates/klyntbot-server/src/main.rs
git commit -m "feat(klyntbot-server): wire ToolRegistryBridge into handler for dynamic tool listing"
```

---

### Task 10: Add `exposed_tools` and `McpAuthConfig` to config

**Files:**
- Modify: `crates/config/src/schema/mcp.rs`
- Modify: `crates/config/src/lib.rs`

- [ ] **Step 1: Add new types and fields**

In `crates/config/src/schema/mcp.rs`, add `McpAuthConfig` before `McpServerSettings`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAuthConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub token: Option<Secret<String>>,
}
```

Update `McpServerSettings` to add new fields:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mcp_port")]
    pub port: u16,
    #[serde(default = "default_localhost")]
    pub host: String,
    #[serde(default = "default_exposed_tools")]
    pub exposed_tools: Vec<String>,
    #[serde(default)]
    pub auth: McpAuthConfig,
}
```

Add the default function:

```rust
fn default_exposed_tools() -> Vec<String> {
    vec![
        "task", "project", "area", "note", "memory",
        "objective", "key_result", "finance",
        "productivity", "work_context",
        "agent",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
```

Update the `Default` impl:

```rust
impl Default for McpServerSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            port: default_mcp_port(),
            host: default_localhost(),
            exposed_tools: default_exposed_tools(),
            auth: McpAuthConfig::default(),
        }
    }
}
```

- [ ] **Step 2: Re-export `McpAuthConfig`**

In `crates/config/src/lib.rs`, add `McpAuthConfig` to the config re-exports.

- [ ] **Step 3: Update `main.rs` to use config whitelist**

In `crates/klyntbot-server/src/main.rs`, replace the hardcoded whitelist:

```rust
let whitelist = config.mcp.server.exposed_tools.clone();
let handler = KlyntbotServerHandler::new(app.clone(), whitelist);
```

- [ ] **Step 4: Run all tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass (new fields have defaults, existing tests unaffected)

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/mcp.rs crates/config/src/lib.rs crates/klyntbot-server/src/main.rs
git commit -m "feat(config): add exposed_tools whitelist and McpAuthConfig to MCP server settings"
```

---

### Task 11: Remove dead `handlers.rs` from `mcp` crate

**Files:**
- Delete: `crates/mcp/src/server/handlers.rs`
- Modify: `crates/mcp/src/server/mod.rs`

- [ ] **Step 1: Remove handlers module**

In `crates/mcp/src/server/mod.rs`, remove both lines:
- `pub mod handlers;` (line 4)
- `pub use handlers::MCP_EXPOSED_TOOLS;` (if present)

Both must be removed — leaving `pub mod handlers;` while deleting the file causes a compile error.

- [ ] **Step 2: Delete the file**

Run: `rm crates/mcp/src/server/handlers.rs`

- [ ] **Step 3: Check for other references**

Run: `cargo build --workspace`
If anything references `MCP_EXPOSED_TOOLS` or `handlers::validate_tool_call`, remove those references.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run --workspace`
Expected: all tests pass (the 5 deleted tests are replaced by whitelist tests in `klyntbot-server`)

- [ ] **Step 5: Commit**

```bash
git add crates/mcp/src/server/mod.rs && git rm crates/mcp/src/server/handlers.rs
git commit -m "refactor(mcp): remove dead MCP_EXPOSED_TOOLS — superseded by config whitelist"
```

---

## Chunk 3: Agent Delegation Bridge

### Task 12: AgentBridge — natural language → agent pipeline → collected response

**Files:**
- Create: `crates/klyntbot-server/src/bridge/agent.rs`
- Modify: `crates/klyntbot-server/src/bridge/mod.rs`
- Modify: `crates/klyntbot-server/src/handler.rs`

- [ ] **Step 1: Write AgentBridge**

Create `crates/klyntbot-server/src/bridge/agent.rs`:

```rust
//! Agent delegation bridge — routes natural language requests through
//! klyntbot's full agent pipeline (intent analysis → tool calls → synthesis).

use std::sync::Arc;

use agent::AgentEvent;
use app_core::AppCore;
use common::FormResponse;
use rmcp::model::{CallToolResult, Content};
use rmcp::ErrorData as McpError;
use tokio::sync::mpsc;

/// Bridges MCP `agent` tool calls to AppCore's chat pipeline.
pub struct AgentBridge {
    app: Arc<AppCore>,
}

impl AgentBridge {
    pub fn new(app: Arc<AppCore>) -> Self {
        Self { app }
    }

    /// Execute an agent chat request and collect the streamed response.
    pub async fn execute(&self, params: serde_json::Value) -> Result<CallToolResult, McpError> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("chat");

        match action {
            "chat" => self.handle_chat(params).await,
            "status" => self.handle_status().await,
            _ => Err(McpError::invalid_params(
                format!("Unknown agent action: {action}"),
                None,
            )),
        }
    }

    async fn handle_chat(&self, params: serde_json::Value) -> Result<CallToolResult, McpError> {
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("'message' is required", None))?
            .to_string();

        let session_key = params
            .get("session_key")
            .and_then(|v| v.as_str())
            .map(|s| format!("mcp:{s}"))
            .unwrap_or_else(|| format!("mcp:{}", uuid::Uuid::new_v4()));

        // Call AppCore's chat pipeline
        let (_msg_response, stream_info) = self
            .app
            .chat_send(message, session_key, None)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Collect streamed events into a single response
        let (response, tool_log) =
            collect_agent_stream(stream_info.event_rx, stream_info.interaction_rx).await;

        // Build result
        let mut content = vec![Content::text(response)];
        if !tool_log.is_empty() {
            content.push(Content::text(format!("[Tools used: {}]", tool_log.join(", "))));
        }

        Ok(CallToolResult::success(content))
    }

    async fn handle_status(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "status": "running",
                "mode": format!("{:?}", self.app.mode),
            })
            .to_string(),
        )]))
    }
}

/// Collect AgentEvent stream into (response_text, tool_names_used).
///
/// Uses `select! biased` to prioritize event_rx over interaction_rx.
/// Auto-declines any ask_user interactions with `FormResponse::Cancelled`.
async fn collect_agent_stream(
    mut event_rx: mpsc::Receiver<AgentEvent>,
    mut interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
) -> (String, Vec<String>) {
    let mut response = String::new();
    let mut tool_log = Vec::new();

    loop {
        tokio::select! {
            biased;

            event = event_rx.recv() => {
                match event {
                    Some(AgentEvent::ContentChunk { data }) => {
                        response.push_str(&data);
                    }
                    Some(AgentEvent::ToolStart { name, .. }) => {
                        tool_log.push(name);
                    }
                    Some(AgentEvent::ToolEnd { .. }) => {
                        // Already logged on ToolStart
                    }
                    Some(AgentEvent::Done { .. }) => break,
                    Some(AgentEvent::Error { message }) => {
                        if response.is_empty() {
                            response = format!("Agent error: {message}");
                        }
                        break;
                    }
                    None => break, // Channel closed
                    _ => {} // Skip telemetry events
                }
            }

            interaction = interaction_rx.recv() => {
                if let Some(bundle) = interaction {
                    // Auto-decline: MCP has no interactive prompt capability (yet).
                    // Send Cancelled so the agent's ask_user tool unblocks.
                    let _ = bundle.response_tx.send(FormResponse::Cancelled);
                }
            }
        }
    }

    (response, tool_log)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_collect_content_chunks() {
        let (event_tx, event_rx) = mpsc::channel(16);
        let (_interaction_tx, interaction_rx) = mpsc::channel(1);

        // Simulate agent stream
        event_tx
            .send(AgentEvent::ContentChunk {
                data: "Hello ".into(),
            })
            .await
            .unwrap();
        event_tx
            .send(AgentEvent::ToolStart {
                name: "task".into(),
                args: serde_json::json!({}),
                agent: None,
            })
            .await
            .unwrap();
        event_tx
            .send(AgentEvent::ToolEnd {
                name: "task".into(),
                success: true,
                duration_ms: 50,
                result: None,
                agent: None,
            })
            .await
            .unwrap();
        event_tx
            .send(AgentEvent::ContentChunk {
                data: "world".into(),
            })
            .await
            .unwrap();
        event_tx
            .send(AgentEvent::Done {
                content: String::new(),
            })
            .await
            .unwrap();

        let (response, tools) = collect_agent_stream(event_rx, interaction_rx).await;
        assert_eq!(response, "Hello world");
        assert_eq!(tools, vec!["task"]);
    }

    #[tokio::test]
    async fn test_auto_decline_interaction() {
        let (event_tx, event_rx) = mpsc::channel(16);
        let (interaction_tx, interaction_rx) = mpsc::channel(1);

        // Send interaction before content
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        interaction_tx
            .send(tools_core::InteractionBundle {
                request: common::InteractionRequest {
                    title: "test".into(),
                    questions: vec![],
                },
                response_tx,
            })
            .await
            .unwrap();

        // Then end the stream
        event_tx
            .send(AgentEvent::ContentChunk {
                data: "ok".into(),
            })
            .await
            .unwrap();
        event_tx.send(AgentEvent::Done {
            content: String::new(),
        }).await.unwrap();

        let (response, _) = collect_agent_stream(event_rx, interaction_rx).await;
        assert_eq!(response, "ok");

        // Verify the interaction was auto-declined
        let form_response = response_rx.await.unwrap();
        assert!(matches!(form_response, FormResponse::Cancelled));
    }
}
```

- [ ] **Step 2: Update bridge/mod.rs**

```rust
pub mod agent;
pub mod registry;
pub mod schema;
```

- [ ] **Step 3: Wire agent tool into handler**

In `crates/klyntbot-server/src/handler.rs`, add the agent tool definition and routing:

Add to `KlyntbotServerHandler`:
```rust
use crate::bridge::agent::AgentBridge;

pub struct KlyntbotServerHandler {
    app: Arc<AppCore>,
    bridge: ToolRegistryBridge,
    agent_bridge: AgentBridge,
}
```

Update `new()`:
```rust
pub fn new(app: Arc<AppCore>, whitelist: Vec<String>) -> Self {
    let registry = app.agent.tool_registry();
    let bridge = ToolRegistryBridge::new(registry, whitelist.clone());
    let agent_bridge = AgentBridge::new(Arc::clone(&app));
    Self { app, bridge, agent_bridge }
}
```

Add agent tool definition method:
```rust
fn agent_tool_def() -> Tool {
    serde_json::from_value(serde_json::json!({
        "name": "agent",
        "description": "Send a natural language request to klyntbot's agent pipeline. The agent analyzes intent, assembles context from memory/projects/tasks, selects tools, and executes a multi-step plan.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["chat", "status"] },
                "message": { "type": "string", "description": "Natural language request for the agent" },
                "session_key": { "type": "string", "description": "Optional session key for conversation continuity. Omit for one-shot requests." }
            },
            "required": ["action", "message"]
        }
    })).unwrap()
}
```

Update `list_tools`:
```rust
async fn list_tools(&self, ...) -> Result<ListToolsResult, McpError> {
    let mut tools = vec![Self::status_tool_def()];
    // Add agent tool if in whitelist
    if self.bridge.whitelist.iter().any(|w| w == "agent") {
        tools.push(Self::agent_tool_def());
    }
    tools.extend(self.bridge.list_tools().await);
    Ok(ListToolsResult { tools, next_cursor: None, meta: None })
}
```

Update `call_tool`:
```rust
async fn call_tool(&self, request: CallToolRequestParams, ...) -> Result<CallToolResult, McpError> {
    let name = request.name.as_ref();
    let params = request.arguments.unwrap_or_default();
    match name {
        "get_status" => self.handle_get_status().await,
        "agent" => self.agent_bridge.execute(params).await,
        _ => self.bridge.execute(name, params).await,
    }
}
```

- [ ] **Step 4: Make whitelist field accessible**

In `bridge/registry.rs`, change `whitelist` to `pub(crate)`:

```rust
pub struct ToolRegistryBridge {
    registry: Arc<RwLock<ToolRegistry>>,
    pub(crate) whitelist: Vec<String>,
}
```

- [ ] **Step 5: Run all tests**

Run: `cargo nextest run -p klyntbot-server`
Expected: all tests pass (agent stream collection, auto-decline, whitelist)

- [ ] **Step 6: Commit**

```bash
git add crates/klyntbot-server/src/bridge/agent.rs crates/klyntbot-server/src/bridge/mod.rs crates/klyntbot-server/src/handler.rs crates/klyntbot-server/src/bridge/registry.rs
git commit -m "feat(klyntbot-server): add AgentBridge for natural language delegation via chat_send"
```

---

### Task 13: Full workspace build + test verification

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: 0 errors

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 3: Format check**

Run: `cargo fmt --all --check`
Expected: 0 formatting issues

- [ ] **Step 4: Full test suite**

Run: `cargo nextest run --workspace`
Expected: all tests pass

- [ ] **Step 5: Commit any fixes**

```bash
git add -A && git commit -m "fix: address clippy/fmt issues from MCP server implementation"
```

---

## Chunk 4: Desktop Embedding + Production Hardening

### Task 14: Verify config whitelist is wired in main.rs

**Prerequisite check only** — Task 10 should have already wired `config.mcp.server.exposed_tools` into `main.rs`. Verify and move on.

- [ ] **Step 1: Verify main.rs uses `config.mcp.server.exposed_tools`**

Read `crates/klyntbot-server/src/main.rs` and confirm the whitelist comes from config (not hardcoded). If Task 10 was completed correctly, this is already done — skip to Task 15.

---

### Task 15: Desktop embedding — MCP HTTP server in Tauri

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Modify: `crates/desktop/src/main.rs` (Tauri setup closure)

**Context:** The desktop crate is a binary crate. Entry point is `crates/desktop/src/main.rs`. `AppCore` is initialized in `crates/desktop/src/app_core.rs` (called from `main.rs:L69-72` inside the `.setup()` closure). The `core` variable is `Arc<AppCore>`, created at line 69-72 and consumed by `app.manage(core)` at line 83 — so the MCP clone must happen **before** line 83. The `.setup()` closure is **sync**, so async operations require `tauri::async_runtime::block_on()`.

- [ ] **Step 1: Add dependency**

In `crates/desktop/Cargo.toml`, add:
```toml
klyntbot-server.workspace = true
```

- [ ] **Step 2: Spawn MCP HTTP server in Tauri setup**

In `crates/desktop/src/main.rs`, inside the `.setup()` closure, **after** line 72 (where `core` is created) but **before** line 83 (`app.manage(core)`), add — following the existing `dev_server` clone pattern at lines 76-81:

```rust
// Start embedded MCP HTTP server if enabled in config.
// Must clone before app.manage(core) moves the Arc.
{
    let mcp_core = Arc::clone(&core);
    let enabled = tauri::async_runtime::block_on(async {
        mcp_core.config.read().await.mcp.server.enabled
    });
    if enabled {
        tauri::async_runtime::spawn(async move {
            let config = mcp_core.config.read().await;
            let host = config.mcp.server.host.clone();
            let port = config.mcp.server.port;
            let whitelist = config.mcp.server.exposed_tools.clone();
            drop(config);
            tracing::info!("Starting embedded MCP HTTP server on {host}:{port}");
            let handler = klyntbot_server::KlyntbotServerHandler::new(mcp_core, whitelist);
            // TODO: Wire rmcp streamable HTTP server transport
            tracing::warn!("HTTP transport not yet implemented — MCP server not started");
            let _ = handler; // Suppress unused warning until HTTP transport is wired
        });
    }
}
```

Note: The pattern follows the existing `dev_server` spawning at lines 76-81: clone `Arc` before `app.manage()`, then spawn an async task. Config is read inside the async block to avoid sync/async mismatch. The actual HTTP transport wiring (using rmcp's `StreamableHttpServer`) is a follow-up task.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build --workspace`
Expected: compiles with 0 clippy warnings

- [ ] **Step 4: Commit**

```bash
git add crates/desktop/Cargo.toml crates/desktop/src/main.rs
git commit -m "feat(desktop): add hook for embedded MCP HTTP server on Tauri startup"
```

---

### Task 16: Add `PRAGMA busy_timeout` to StoragePool

**Files:**
- Modify: `crates/storage/src/pool.rs`

- [ ] **Step 1: Add busy_timeout pragma**

In `crates/storage/src/pool.rs`, find the `connect()` method where WAL mode and foreign keys are set. Add:

```rust
sqlx::query("PRAGMA busy_timeout = 5000")
    .execute(&pool)
    .await
    .map_err(|e| StorageError::Migration(format!("busy_timeout: {e}")))?;
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p storage`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/storage/src/pool.rs
git commit -m "fix(storage): add PRAGMA busy_timeout=5000 for concurrent MCP+desktop access"
```

---

### Task 17: E2E smoke test

- [ ] **Step 1: Build release binary**

Run: `cargo build -p klyntbot-server`

- [ ] **Step 2: Test initialize + tools/list**

Note: Verify the protocol version string matches what rmcp 0.17 expects. Check `ProtocolVersion::default()` — it may be `"2025-03-26"` or similar. Adjust the test commands accordingly.

Run:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | KLYNTBOT_HOME=/tmp/klyntbot-e2e-test target/debug/klyntbot-mcp serve --stdio 2>/dev/null
```

Expected: Two JSON-RPC responses — `initialize` result with serverInfo, and `tools/list` result containing `get_status` plus bridged tools (task, project, note, etc.)

- [ ] **Step 3: Test a tool call**

Run:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_status","arguments":{}}}' | KLYNTBOT_HOME=/tmp/klyntbot-e2e-test target/debug/klyntbot-mcp serve --stdio 2>/dev/null
```

Expected: Response with `"status":"running"` and `"mode":"Server"`

- [ ] **Step 4: Document any issues and commit fixes**

```bash
git add -A && git commit -m "fix(klyntbot-server): e2e test fixes"
```

---

### Task 18: Final verification

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`

- [ ] **Step 2: Full clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 3: Full test suite**

Run: `cargo nextest run --workspace`

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`

- [ ] **Step 5: Commit if needed**

```bash
git add -A && git commit -m "chore: final cleanup for MCP server implementation"
```
