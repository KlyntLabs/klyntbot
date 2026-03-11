# MCP (Model Context Protocol) Architecture

## Overview

The `mcp` crate provides both client and server implementations of the Model Context Protocol, built on the `rmcp` library. This enables two capabilities:

- **MCP Client** (`McpManager`): Connects to external MCP servers (Linear, Google Calendar, Notion, etc.), discovers their tools, and wraps each as a native klyntbot `Tool` for use in agent workflows.
- **MCP Server** (`McpServerRunner`): Exposes a curated subset of klyntbot's internal tools to external AI agents via MCP.

**Key files:**

- `crates/mcp/src/lib.rs` -- module structure, re-exports
- `crates/mcp/src/client/manager.rs` -- `McpManager` (connection lifecycle)
- `crates/mcp/src/client/tool_adapter.rs` -- `McpTool` (Tool trait adapter)
- `crates/mcp/src/client/sanitize.rs` -- tool name sanitization
- `crates/mcp/src/client/handler.rs` -- `KlyntbotClientHandler` (server-initiated requests)
- `crates/mcp/src/client/events.rs` -- `McpStartupEvent` (startup observability)
- `crates/mcp/src/server/handler.rs` -- `KlyntbotServerHandler`, `McpServerRunner`
- `crates/mcp/src/server/handlers.rs` -- exposed tool list, validation
- `crates/mcp/src/server/security.rs` -- path traversal prevention, input sanitization
- `crates/config/src/schema/mcp.rs` -- `McpConfig`, `McpServerDef`, `McpTransport`

## MCP Client

### McpManager

`McpManager` manages the lifecycle of all MCP server connections. It connects to configured servers at startup, discovers their tools, and provides them for registration in klyntbot's `ToolRegistry`.

#### Connection flow

`McpManager::connect_all(config, event_tx)`:

1. Iterates over `config.servers`. Disabled servers are skipped (with a `Skipped` event).
2. Connects to all enabled servers **in parallel** using `tokio::task::JoinSet`.
3. Each connection attempt is wrapped in `tokio::time::timeout` using the server's `startup_timeout_sec` (default: 10s).
4. On success, discovered tools are filtered by allowlist/denylist and wrapped as `McpTool` instances.
5. On failure, the error is logged as a warning. Other servers continue connecting.
6. Emits `McpStartupEvent` values for UI/logging observability.

#### Transport types

Each MCP server connection uses one of two transports:

**Stdio** -- Spawns the MCP server as a subprocess:
```json
{
  "name": "linear",
  "transport": "stdio",
  "command": "npx",
  "args": ["-y", "@anthropic/linear-mcp-server"],
  "env": {"LINEAR_API_KEY": "..."}
}
```
Uses `rmcp::transport::TokioChildProcess`. Process cleanup is handled automatically by rmcp's `process_wrap` crate, which sets up process groups and kills the entire tree on drop.

**HTTP** -- Connects to a remote MCP server via Streamable HTTP:
```json
{
  "name": "notion",
  "transport": "http",
  "url": "https://mcp.notion.so/v1",
  "headers": {"Authorization": "Bearer ntn_..."}
}
```
Uses `rmcp::transport::StreamableHttpClientTransport`. Custom headers and OAuth auth headers are supported.

#### Tool discovery

After connecting, `McpManager` calls `service.peer().list_all_tools()` to discover all tools exposed by the server (handles pagination automatically). Each tool definition is then:

1. Filtered against the server's `enabled_tools` / `disabled_tools` config
2. Wrapped as an `McpTool` with the namespaced name format `mcp_{server}_{tool}`
3. Collected and returned via `manager.tools()`

#### Reconnection and disconnection

- `reconnect_server(server_def)` -- Disconnects the existing connection (if any) and connects fresh
- `disconnect_server(name)` -- Removes a single server connection
- `disconnect_all()` -- Gracefully disconnects all servers concurrently using `futures_util::future::join_all`

### McpTool

`McpTool` adapts an MCP server tool to klyntbot's `tools_core::Tool` trait, making it indistinguishable from a built-in tool in the agent's tool registry.

Key properties:
- **Name:** Uses the namespaced format `mcp_{server}_{tool}` (see Tool Namespacing below)
- **Description:** Taken from the MCP server's tool definition
- **Parameters:** The MCP server's `inputSchema` is passed through as the tool's JSON Schema
- **Permission level:** `PermissionLevel::Elevated` (MCP tools make network calls to external servers)
- **Timeout:** Uses the server's configured `tool_timeout_sec` (default: 120s) via `custom_timeout()`

#### Execution

When `execute()` is called:

1. Sends a `tools/call` JSON-RPC request to the MCP server via the rmcp `Peer` handle
2. Extracts text content from the response's `content` array
3. If the server reports `is_error: true`, returns the text as a `ToolError::ExecutionFailed`
4. If no text content is found, serializes the entire content array as JSON
5. Otherwise, joins all text parts with newlines

### KlyntbotClientHandler

Handles server-initiated requests from MCP servers. Currently provides:
- `get_info()` -- Returns client info (`"klyntbot"` + version)
- `on_logging_message()` -- Routes server log messages to tracing (errors/critical as `warn!`, others as `debug!`)
- `on_tool_list_changed()` -- Logs a notification (dynamic refresh not yet implemented)
- Default implementations for sampling, elicitation, and roots (return errors)

### Startup events

`McpStartupEvent` provides real-time observability during connection:

| Event | When |
|---|---|
| `Starting { server_name }` | Connection attempt begins |
| `Ready { server_name, tool_count }` | Connected and tools discovered |
| `Failed { server_name, error }` | Connection failed |
| `Skipped { server_name }` | Server disabled in config |
| `Complete { ready, failed, skipped }` | All connection attempts finished |

Events are emitted via an optional `mpsc::Sender<McpStartupEvent>` channel passed to `connect_all`.

## MCP Server

### McpServerRunner

Exposes klyntbot as an MCP server for external AI agents. Uses rmcp's `ServerHandler` trait.

The server currently supports **stdio transport only** (Streamable HTTP is planned). When running on stdio, tracing output is redirected to stderr so stdout remains clean for JSON-RPC messages.

```rust
McpServerRunner::run().await  // Blocks until stopped
```

### KlyntbotServerHandler

Implements `rmcp::handler::server::ServerHandler`. Uses rmcp's `#[tool_router]` and `#[tool]` macros for declarative tool registration.

Currently exposes one built-in tool:
- `get_status` -- Returns klyntbot's status and version

Server info advertises:
- Name: `"klyntbot"`
- Version: from `CARGO_PKG_VERSION`
- Instructions: `"Klyntbot MCP server -- exposes task management and agent capabilities."`

### Exposed tools

The `MCP_EXPOSED_TOOLS` constant in `crates/mcp/src/server/handlers.rs` defines which internal tools are available to external agents:

```rust
pub const MCP_EXPOSED_TOOLS: &[&str] = &[
    "task", "memory", "annotate", "search", "project",
    "area", "okr", "context_request", "learning", "web_search",
];
```

This is a curated subset. External agents do not get raw access to all internal tools. The `validate_tool_call()` function checks that a tool name is in this list before allowing execution, and applies `sanitize_input()` to the parameters.

## Tool Namespacing

MCP tools are registered in klyntbot's `ToolRegistry` using a namespaced naming convention to avoid collisions with built-in tools:

```
mcp_{sanitized_server}_{sanitized_tool}
```

Examples:
- `mcp_linear_list_issues`
- `mcp_github_list_repos`
- `mcp_my_server_get_data` (dots in "my.server" replaced with underscores)

### Sanitization rules

The `sanitize` module (`crates/mcp/src/client/sanitize.rs`) ensures tool names are safe for LLM function-calling APIs:

1. **Character replacement:** Characters outside `[a-zA-Z0-9_-]` are replaced with `_`
2. **Length limit:** Names are capped at 64 characters. If exceeded, the name is truncated and an 8-character hash suffix is appended for uniqueness.
3. **Fast path:** If the input is already clean, no allocation occurs.

Helper functions:
- `build_tool_name(server, tool)` -- Constructs the full namespaced name
- `extract_server_name(tool_name)` -- Extracts the server segment from `"mcp_linear_list_issues"` (returns `"linear"`)
- `server_prefix(server)` -- Returns `"mcp_{server}_"` for bulk unregistration

## Access Control

MCP tool access is controlled at two levels:

### Per-agent access (mcp_tools field)

Each agent profile has an `mcp_tools` field that controls which MCP tools it can use:

| Value | Meaning |
|---|---|
| `["*"]` | All MCP tools available |
| `[]` | No MCP tools available |
| `["google-calendar"]` | Only tools from the `google-calendar` server |

Example: The task agent has `mcp_tools: ["google-calendar"]` for calendar operations.

### Per-server tool filtering

Each `McpServerDef` supports two filter fields:

- `enabled_tools: Option<Vec<String>>` -- Allowlist. When set, only these tools are registered from this server.
- `disabled_tools: Option<Vec<String>>` -- Denylist. When set, these tools are excluded.

**Precedence:** Allowlist takes precedence over denylist. If both are set, only the allowlist is consulted.

```rust
pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
    if let Some(ref allowed) = self.enabled_tools {
        return allowed.iter().any(|n| n == tool_name);
    }
    if let Some(ref denied) = self.disabled_tools {
        return !denied.iter().any(|n| n == tool_name);
    }
    true  // No filters = all tools allowed
}
```

Tool names in these filters use the **original MCP names** (not namespaced), e.g., `"list_issues"` rather than `"mcp_linear_list_issues"`.

## Security

The `security` module (`crates/mcp/src/server/security.rs`) provides two protections for the MCP server (tools exposed to external agents):

### Path traversal prevention

```rust
pub fn validate_path(path: &str, allowed_base: &PathBuf) -> Result<PathBuf, String>
```

- Canonicalizes both the input path and the allowed base directory
- Verifies that the canonicalized path `starts_with` the canonicalized base
- Returns an error if the path escapes the allowed directory (via `../` traversal or absolute paths outside the base)

### Input sanitization

```rust
pub fn sanitize_input(input: &str) -> String
```

- Strips control characters (except `\n` and `\t`, which are valid in tool parameters)
- Truncates to `MAX_INPUT_LENGTH` (50,000 characters)
- Applied to all MCP tool call parameters via `validate_tool_call()`

## OAuth

MCP servers that require OAuth authentication store credentials in the `McpOAuthCredentials` struct alongside the server definition:

```rust
pub struct McpOAuthCredentials {
    pub provider: String,          // e.g., "linear", "github"
    pub access_token: Secret<String>,
    pub refresh_token: Option<Secret<String>>,
    pub expires_at: Option<String>, // ISO-8601 timestamp
    pub env_var: String,           // Environment variable name for subprocess injection
}
```

For **stdio** transport, the OAuth access token is injected as an environment variable into the subprocess using the `env_var` field:
```rust
cmd.env(&oauth.env_var, oauth.access_token.expose());
```

For **HTTP** transport, the access token is set as the authorization header via rmcp's `auth_header()` method (adds `Bearer` prefix automatically).

## Configuration

The MCP config lives under the `mcp` key in `~/.klyntbot/config.json`:

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
        "env": {"LINEAR_API_KEY": "..."},
        "enabled": true,
        "startupTimeoutSec": 10,
        "toolTimeoutSec": 120,
        "enabledTools": ["list_issues", "get_issue"],
        "oauth": {
          "provider": "linear",
          "accessToken": "lin_api_...",
          "refreshToken": null,
          "envVar": "LINEAR_API_KEY"
        }
      },
      {
        "name": "notion",
        "transport": "http",
        "url": "https://mcp.notion.so/v1",
        "headers": {"Authorization": "Bearer ntn_..."},
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

### Top-level fields

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `true` | Master switch for all MCP functionality |
| `servers` | `Vec<McpServerDef>` | `[]` | External MCP server definitions |
| `server` | `McpServerSettings` | see below | Settings for klyntbot's own MCP server |

### McpServerDef fields

| Field | Type | Default | Description |
|---|---|---|---|
| `name` | `String` | required | Server identifier (used in tool namespacing) |
| `transport` | `"stdio"` or `"http"` | required | Transport type (tagged enum, flattened) |
| `enabled` | `bool` | `true` | Whether to connect to this server |
| `command` | `String` | -- | Stdio: subprocess command |
| `args` | `Vec<String>` | `[]` | Stdio: subprocess arguments |
| `env` | `HashMap<String, String>` | `{}` | Stdio: environment variables |
| `url` | `String` | -- | HTTP: server URL |
| `headers` | `HashMap<String, String>` | `{}` | HTTP: custom headers |
| `oauth` | `McpOAuthCredentials?` | `None` | OAuth credentials (optional) |
| `startupTimeoutSec` | `u64` | `10` | Connection + discovery timeout |
| `toolTimeoutSec` | `u64` | `120` | Per-tool-call timeout |
| `enabledTools` | `Vec<String>?` | `None` | Tool allowlist (original MCP names) |
| `disabledTools` | `Vec<String>?` | `None` | Tool denylist (original MCP names) |

### McpServerSettings fields

| Field | Type | Default | Description |
|---|---|---|---|
| `enabled` | `bool` | `false` | Whether to start klyntbot's MCP server |
| `port` | `u16` | `3100` | Server port |
| `host` | `String` | `"127.0.0.1"` | Server bind address |

`has_active_servers()` returns `true` only if `enabled` is `true` and at least one server definition is also enabled.
