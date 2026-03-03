# MCP Integration Design

## Overview

Integrate the Model Context Protocol (MCP) into klyntbot as both a **client** (connecting to external MCP servers like Linear, Notion, Google Calendar) and a **server** (exposing dedicated tools for external AI agents like Claude Code and Cursor).

Uses the official `rmcp` crate (v0.17) for full MCP spec compliance (2025-11-25).

## Decisions

| Decision | Choice |
|----------|--------|
| MCP Role | Both Client + Server |
| Transports | stdio (local subprocess) + Streamable HTTP (remote) |
| Configuration | Config file array in `config.json` |
| Client Features | Full spec: Tools, Resources, Prompts, Sampling, Elicitation, Roots |
| Server Scope | Separate MCP-specific tools (curated API, not raw internal tool exposure) |
| Protocol Library | `rmcp` 0.17 (official Rust MCP SDK) |

## Architecture

### New Crate: `crates/mcp` (Layer 3-4)

```
crates/mcp/
├── Cargo.toml
└── src/
    ├── lib.rs              # Public API: McpManager, McpServerRunner
    ├── config.rs           # McpConfig, McpServerDef, McpTransport
    ├── manager.rs          # McpManager: lifecycle for all client connections
    ├── client/
    │   ├── mod.rs          # McpClient: wraps rmcp service handle
    │   ├── handler.rs      # KlyntbotClientHandler: impl ClientHandler
    │   ├── tool_adapter.rs # McpTool: impl tools_core::Tool
    │   └── resource.rs     # Resource reading helpers
    └── server/
        ├── mod.rs          # McpServerRunner: starts the MCP server
        ├── handler.rs      # KlyntbotServerHandler: impl ServerHandler
        └── tools.rs        # Dedicated MCP-exposed tools
```

### Dependencies

```toml
[dependencies]
rmcp = { version = "0.17", features = [
    "client", "server",
    "transport-io", "transport-child-process",
    "transport-streamable-http-client", "transport-streamable-http-client-reqwest",
    "transport-streamable-http", "transport-sse-server",
    "client-side-sse", "auth", "macros",
] }
tools-core = { path = "../tools-core" }
common = { path = "../common" }
config = { path = "../config" }
providers = { path = "../providers" }
tokio = { version = "1", features = ["process", "sync"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
```

## Config Schema

New top-level `mcp` section in `config.json`:

```json
{
  "mcp": {
    "enabled": true,
    "servers": [
      {
        "name": "linear",
        "transport": "stdio",
        "command": "npx",
        "args": ["-y", "@anthropic/linear-mcp-server"],
        "env": { "LINEAR_API_KEY": "lin_api_..." },
        "enabled": true
      },
      {
        "name": "notion",
        "transport": "http",
        "url": "https://mcp.notion.so/v1",
        "headers": { "Authorization": "Bearer ntn_..." },
        "enabled": true
      }
    ],
    "server": {
      "enabled": false,
      "port": 3100,
      "host": "127.0.0.1"
    }
  }
}
```

### Rust Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub servers: Vec<McpServerDef>,
    #[serde(default)]
    pub server: McpServerSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerDef {
    pub name: String,
    #[serde(flatten)]
    pub transport: McpTransport,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

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
        #[serde(default)]
        auth: Option<McpAuth>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAuth {
    pub client_id: String,
    pub client_secret: Secret<String>,
    pub auth_url: String,
    pub token_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mcp_port")]
    pub port: u16,
    #[serde(default = "default_localhost")]
    pub host: String,
}
```

## MCP Client

### McpManager

Central lifecycle manager for all MCP server connections.

```rust
pub struct McpManager {
    clients: HashMap<String, McpClient>,
    config: McpConfig,
    provider: DynProvider,  // For sampling requests
}

impl McpManager {
    pub fn new(config: &McpConfig, provider: DynProvider) -> Self;
    pub async fn connect_all(&mut self) -> Result<()>;
    pub fn tools(&self) -> Vec<Arc<McpTool>>;
    pub async fn disconnect_all(&mut self) -> Result<()>;
    pub async fn reconnect(&mut self, name: &str) -> Result<()>;
}
```

**Startup flow per server:**
1. Create transport: stdio via `TokioChildProcess::new(Command)` or HTTP via `StreamableHttpClientTransport`
2. `KlyntbotClientHandler::new(provider.clone()).serve(transport).await` — MCP initialize handshake
3. `service.list_tools()` — discover all tools
4. Wrap each as `McpTool` and store
5. Listen for `on_tool_list_changed` to refresh dynamically

**Reconnection:** Background task pings each server. On disconnect, exponential backoff (1s, 2s, 4s, 8s, max 60s).

### McpTool (implements tools_core::Tool)

Bridge between MCP tool definitions and klyntbot's tool system.

```rust
pub struct McpTool {
    server_name: String,
    tool_def: rmcp::model::Tool,
    peer: Arc<Peer<RoleClient>>,
}

impl tools_core::Tool for McpTool {
    fn name(&self) -> &str;          // "mcp_{server}_{tool}" namespaced
    fn description(&self) -> &str;   // From MCP tool definition
    fn parameters(&self) -> Value;   // Convert inputSchema to OpenAI function format
    fn permission_level(&self) -> PermissionLevel; // Elevated (network calls)

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;
}
```

**Tool naming convention:** `mcp_{server_name}_{tool_name}` — e.g., `mcp_linear_list_issues`. Prevents collisions with built-in tools.

**Execute flow:**
1. `peer.call_tool(CallToolRequestParam { name, arguments })` — JSON-RPC call to MCP server
2. Serialize `CallToolResult.content` to string (text content concatenated, images as descriptions)
3. Return as `Result<String>` matching klyntbot's tool result convention

### KlyntbotClientHandler (implements rmcp::ClientHandler)

Handles server-initiated requests:

| Method | Implementation |
|--------|---------------|
| `create_message` (sampling) | Routes to `DynProvider::chat()` — MCP server can request LLM completions |
| `list_roots` | Returns klyntbot workspace directory |
| `create_elicitation` | Routes through chat channel via ask_user pattern |
| `on_tool_list_changed` | Triggers tool re-discovery and ToolRegistry update |
| `on_logging_message` | Routes to tracing |
| `on_resource_updated` | Cache invalidation for subscribed resources |

## MCP Server

### KlyntbotServerHandler

Dedicated tools exposed to external AI agents (Claude Code, Cursor, etc.):

| Tool | Description |
|------|-------------|
| `ask_klyntbot` | Send a natural language message and get a response (conversational proxy) |
| `manage_tasks` | CRUD operations on the task system |
| `search_memory` | Search knowledge base / memory notes |
| `check_calendar` | Query calendar events |
| `get_status` | Current agent status, active tasks, recent activity |

Implemented via `rmcp`'s `#[tool_router]` macro:

```rust
#[derive(Clone)]
pub struct KlyntbotServerHandler {
    // Dependencies injected at construction
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl KlyntbotServerHandler {
    #[tool(description = "Send a message to Klyntbot and get a response")]
    async fn ask_klyntbot(&self, message: String) -> Result<CallToolResult, McpError> { ... }

    #[tool(description = "Manage tasks: create, update, list, complete")]
    async fn manage_tasks(&self, action: String, ...) -> Result<CallToolResult, McpError> { ... }

    // ... other tools
}
```

### Server startup

- Runs if `config.mcp.server.enabled`
- Binds to `{host}:{port}` via Streamable HTTP transport
- Also supports stdio (for subprocess mode via Claude Desktop)
- Background `tokio::spawn` task alongside channel manager
- Graceful shutdown in the `tokio::select!` block

## Integration Points

### AgentLoopBuilder (builder.rs)

After WASM plugin registration (~line 579):

```rust
// MCP servers (client side)
let mcp_manager = if config.mcp.enabled && !config.mcp.servers.is_empty() {
    let mut manager = McpManager::new(&config.mcp, provider.clone());
    manager.connect_all().await?;
    for tool in manager.tools() {
        tool_registry.register_dyn(tool);
    }
    Some(manager)
} else {
    None
};
```

### serve.rs

After `AgentLoop::builder().build()`, before spawning the agent:

```rust
// MCP Server (expose tools to external agents)
if config.mcp.server.enabled {
    let mcp_server = McpServerRunner::new(&config.mcp.server, /* deps */);
    tokio::spawn(mcp_server.run());
}
```

### Config (schema/core.rs)

Add field to root Config struct:

```rust
#[serde(default)]
pub mcp: McpConfig,
```

## Error Handling & Resilience

| Scenario | Behavior |
|----------|----------|
| Connection failure at startup | Log warning, skip server, continue. Don't block agent. |
| Tool call failure | Map `rmcp::ServiceError` → `KlyntbotError::Tool`. Return error to LLM. |
| Server disconnect | Background health check detects via ping. Auto-reconnect with backoff. |
| Subprocess crash (stdio) | Detect via process exit. Auto-restart with backoff. |
| Tool call timeout | 30s default (matches existing tool timeout). |
| Tool list change | Re-discover tools, update ToolRegistry under write lock. |

## Testing Strategy

- Unit tests: Mock MCP server using `rmcp`'s test utilities
- Integration tests: Spawn a simple test MCP server (counter example) via stdio, verify tool discovery and calling
- No external services required for tests (consistent with project's ephemeral SQLite test pattern)
