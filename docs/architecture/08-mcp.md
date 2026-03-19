# MCP Architecture

## Overview

Klyntbot provides bidirectional MCP (Model Context Protocol) integration:
- **MCP Client**: Connects to external MCP servers (Linear, GitHub, Notion, etc.)
- **MCP Server**: Exposes internal tools to external AI clients (Claude Code, Cursor)

Built on the `rmcp` library for protocol handling, transport layers, and JSON-RPC framing.

## Architecture

```mermaid
graph TB
    subgraph "External AI Clients"
        CC[Claude Code]
        CUR[Cursor]
    end

    subgraph "klyntbot-server (L8)"
        CLI["klyntbot-mcp serve --stdio"]
        HANDLER[KlyntbotServerHandler]
        TRB[ToolRegistryBridge]
        AB[AgentBridge]
    end

    subgraph "mcp crate (L6)"
        MGR[McpManager]
        MCT[McpTool]
        SAN[sanitize module]
    end

    subgraph "External MCP Servers"
        LIN[Linear]
        GH[GitHub]
    end

    subgraph "Core"
        TR[ToolRegistry]
        AP[Agent Pipeline]
    end

    CC & CUR -->|stdio JSON-RPC| CLI
    CLI --> HANDLER
    HANDLER --> TRB & AB
    TRB --> TR
    AB --> AP

    MGR -->|stdio/HTTP| LIN & GH
    MGR --> MCT
    MCT -->|register| TR
```

## MCP Client

### McpManager Lifecycle

```mermaid
sequenceDiagram
    participant Config
    participant MGR as McpManager
    participant Proc as Child Process
    participant Server as External MCP Server
    participant Reg as ToolRegistry

    Config->>MGR: connect_all(config, event_tx)
    par For each enabled server
        MGR->>Proc: spawn(command, args, env)
        Proc->>Server: stdio pipe
        MGR->>Server: initialize handshake
        Server-->>MGR: ServerInfo
        MGR->>Server: tools/list (with pagination)
        Server-->>MGR: Vec<ToolDef>
        MGR->>MGR: filter by allowlist/denylist
        loop For each allowed tool
            MGR->>Reg: register McpTool
        end
    end
```

### McpTool Adapter
Wraps remote MCP tools as `tools_core::Tool`:
- **Name**: `mcp_{sanitized_server}_{sanitized_tool}` (max 64 chars)
- **Permission**: `PermissionLevel::Elevated` (network calls)
- **Timeout**: Per-server configurable (`tool_timeout_sec`, default 120)
- **Execution**: Calls `peer.call_tool()` with original tool name

### Tool Name Sanitization
- Replace non-alphanumeric chars with `_`
- Format: `mcp_{server}_{tool}`
- Truncate at 55 chars + 8-char hex hash suffix if over 64

### Transport Support
- **Stdio**: Spawns child process, communicates via stdin/stdout
- **HTTP**: Streamable HTTP with optional OAuth Bearer token

### Per-Server Tool Filtering
- `enabled_tools` (allowlist) -- when set, only these tools are registered
- `disabled_tools` (denylist) -- excluded tools
- Allowlist takes precedence over denylist

## MCP Server

### Tool Categories
1. **`get_status`** -- Built-in, always present
2. **`agent`** -- Natural language delegation to full AI pipeline (if whitelisted)
3. **Registry tools** -- Internal tools exposed via ToolRegistryBridge

### Server Call Flow

```mermaid
sequenceDiagram
    participant Client as Claude Code
    participant Handler as KlyntbotServerHandler
    participant Bridge as ToolRegistryBridge
    participant Registry as ToolRegistry
    participant Tool as Internal Tool

    Client->>Handler: tools/list
    Handler->>Bridge: list_tools()
    Bridge->>Registry: iterate whitelist
    Registry-->>Bridge: Tool definitions
    Handler-->>Client: [get_status, agent, tasks, project, ...]

    Client->>Handler: tools/call {name: "tasks", args: {action: "list"}}
    Handler->>Bridge: execute("tasks", args)
    Bridge->>Bridge: whitelist check
    Bridge->>Registry: prepare("tasks", args, ctx)
    Registry-->>Bridge: Arc<dyn Tool>
    Note over Bridge: RwLock released
    Bridge->>Tool: execute(args, ctx)
    Tool-->>Bridge: result
    Bridge-->>Handler: CallToolResult::success
    Handler-->>Client: tools/call response
```

### Agent Tool Flow

```mermaid
sequenceDiagram
    participant Client as Claude Code
    participant AB as AgentBridge
    participant App as AppCore
    participant Agent as Agent Pipeline

    Client->>AB: tools/call {name: "agent", args: {action: "chat", message: "..."}}
    AB->>App: chat_send(message, "mcp:{uuid}")
    App->>Agent: Full pipeline execution
    loop Streaming
        Agent-->>AB: ContentChunk, ToolStart, ToolEnd
    end
    Agent-->>AB: Done
    AB-->>Client: text response + [Tools used: ...]
```

Interactive prompts (`ask_user`) are auto-declined since MCP has no interactive capability.

### Entity Update Events
After mutating tool calls, `entity:updated` events are emitted for desktop UI cache invalidation. Read-only actions (list, show, get, search) are skipped.

## Tool Whitelisting

### Server-side (exposing tools)
Configured via `config.json` -> `mcp.server.exposedTools`:

Default whitelist: `tasks`, `project`, `area`, `notes`, `memory`, `okr`, `finance`, `productivity`, `work_context`, `agent`

### Client-side (filtering external tools)
Per-server `enabled_tools` / `disabled_tools` in config.

### Skill-level MCP Access Control
Each skill declares which MCP servers it can access:

| Skill | MCP Access |
|---|---|
| general | `["*"]` (all) |
| task-management | `["google-calendar"]` |
| finance-management | `[]` (none) |
| automation | `[]` (none) |
| communication | `[]` (none) |

## Security

- **Path validation**: `validate_path()` canonicalizes paths, blocks traversal attacks
- **Input sanitization**: `sanitize_input()` strips control chars, truncates to 50,000 chars
- **Namespace isolation**: WASM plugins get `plugin_{id}_` prefix for storage operations

## Claude Code Integration

```json
{
  "mcpServers": {
    "klyntbot": {
      "command": "<path>/target/debug/klyntbot-mcp",
      "args": ["serve", "--stdio"],
      "env": { "KLYNTBOT_HOME": "~/.klyntbot-dev" }
    }
  }
}
```

Tools appear as `mcp__klyntbot__<tool_name>` in Claude Code. Tracing output goes to stderr to keep stdout clean for JSON-RPC.
