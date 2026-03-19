# Layer 8: klyntbot-server -- Standalone MCP Server Binary

## Overview

The `klyntbot-server` crate (`crates/klyntbot-server/`) produces the `klyntbot-mcp` binary, a standalone MCP (Model Context Protocol) server that exposes klyntbot's internal tools to external AI clients such as Claude Code, Cursor, and other MCP-compatible IDEs. It is the primary integration point for AI-to-AI communication with klyntbot.

The crate also serves as a library (`klyntbot_server`) consumed by the desktop app's embedded MCP server, ensuring both standalone and embedded modes share the same handler logic.

**Binary name:** `klyntbot-mcp`
**Crate name:** `klyntbot-server`
**Package path:** `crates/klyntbot-server/Cargo.toml`

## Dependencies

| Dependency | Purpose |
|---|---|
| `app-core` | Application core -- bootstraps storage, agent, tool registry |
| `agent` | Agent runtime (for `AgentEvent` stream types) |
| `mcp` | Internal MCP types (not directly used for protocol; `rmcp` handles that) |
| `tools-core` | `ToolRegistry`, `RoutingContext`, `InteractionBundle` |
| `common` | Error types, `ChannelName`, `ChatId`, `MCP_CHANNEL`, `FormResponse` |
| `config` | Configuration loading with env overrides |
| `desktop-shared` | `EntityKind` enum for UI event emission |
| `rmcp` | MCP protocol implementation (server, stdio/HTTP transport, macros) |
| `clap` | CLI argument parsing |
| `tokio` | Async runtime |
| `tracing` / `tracing-subscriber` | Structured logging |

## Entry Point and Initialization

### `main.rs`

The binary entry point follows this sequence:

1. **Parse CLI** -- `Cli::parse()` via clap
2. **Configure tracing** -- stdio mode writes to stderr (to avoid corrupting the MCP JSON-RPC stream on stdout); HTTP mode writes to stdout
3. **Load config** -- `config::load_with_env_overrides()` reads `~/.klyntbot/config.json` with env var overrides (e.g., `KLYNTBOT_HOME`)
4. **Init AppCore** -- `AppCore::init(AppMode::Server, ...)` bootstraps SQLite, LanceDB, tool registry, agent runtime, and all feature packages. Returns `(AppCore, EventChannels)`
5. **Drain event channels** -- A background task drains the unused `intervention_rx` and `pipeline_rx` channels. In Server mode, coaching is not started, so these must be drained to prevent channel backpressure
6. **Build handler** -- `KlyntbotServerHandler::new(app, whitelist)` constructs the MCP handler with the tool whitelist from config
7. **Start transport** -- For stdio: `rmcp::transport::io::stdio()` creates the stdio transport, and `handler.serve(transport)` starts the MCP service. The process blocks on `service.waiting()` with Ctrl+C handling
8. **Shutdown** -- `app.shutdown()` is called for graceful cleanup

### Startup Modes

| Mode | Transport | Status |
|---|---|---|
| `serve --stdio` | JSON-RPC over stdin/stdout | Production-ready |
| `serve --http` | Streamable HTTP | Not yet implemented (exits with error) |
| `tools --list` | N/A (prints tool list) | Utility command |
| `tools --schema <name>` | N/A | Stub (prints "not yet available") |

## CLI Arguments

Defined in `cli.rs` using clap derive:

```
klyntbot-mcp <COMMAND>

Commands:
  serve    Start the MCP server
  tools    Inspect available tools

serve options:
  --stdio             Use stdio transport (for Claude Code / IDE integration)
  --http              Use HTTP transport
  --port <PORT>       HTTP port (default: from config or 3100)
  --host <HOST>       HTTP host (default: from config or 127.0.0.1)

tools options:
  --list              List all available tools
  --schema <NAME>     Show schema for a specific tool
```

The `--stdio` and `--http` flags are in a clap argument group (mutually exclusive). When neither is specified, stdio is the default.

## MCP Handler (`handler.rs`)

`KlyntbotServerHandler` implements `rmcp::handler::server::ServerHandler` and bridges the MCP protocol to klyntbot internals.

### Tool Categories

The handler exposes three categories of tools:

1. **Built-in tools** -- `get_status` (always present), `agent` (if whitelisted)
2. **Bridged tools** -- Internal `ToolRegistry` tools translated to MCP schema via `ToolRegistryBridge`
3. **Agent tool** -- Special tool that delegates natural language to the full agent pipeline

### ServerHandler Implementation

| Method | Behavior |
|---|---|
| `get_info()` | Returns server name "klyntbot", version from Cargo, tools capability |
| `list_tools()` | Returns `get_status` + `agent` (if whitelisted) + all bridged tools |
| `call_tool()` | Routes by name: `get_status` -> status JSON, `agent` -> `AgentBridge`, everything else -> `ToolRegistryBridge` |
| `get_tool()` | Only resolves built-in tools (`get_status`, `agent`) |

### Entity Update Events

After successful tool calls, `emit_entity_update_for_tool()` fires UI invalidation events so the desktop frontend can refresh. This uses the same event emitter as the desktop app.

- Read-only actions (`list`, `show`, `get`, `search`, `status`, `stats`, `query`) are skipped
- Mutating actions emit `entity:updated` with the appropriate `EntityKind`
- The `agent` tool emits broad invalidation for `Task`, `Project`, and `Note` entities
- The `okr` tool emits for both `Objective` and `KeyResult`

## Bridge Layer (`bridge/`)

### `ToolRegistryBridge` (`bridge/registry.rs`)

Translates MCP tool calls to internal `Tool::execute()` calls:

- **Whitelist enforcement** -- Only tools in `config.mcp.server.exposedTools` are discoverable and callable. Requests for non-whitelisted tools return `invalid_request`
- **Schema translation** -- `list_tools()` reads the internal `ToolRegistry`, calls `tool.parameters()` (OpenAI JSON Schema format), and converts to MCP `Tool` definitions via `schema::internal_to_mcp_tool()`
- **Execution** -- Acquires a read lock on the registry, calls `prepare()` (validates params + clones `Arc<dyn Tool>`), drops the lock, then calls `execute()`. This ensures long-running tool calls don't block concurrent requests
- **Routing context** -- All MCP calls use `ChannelName::new(MCP_CHANNEL)` and `ChatId::new("mcp-session")` with `is_direct_mode: true`

### `AgentBridge` (`bridge/agent.rs`)

Routes natural language through klyntbot's full agent pipeline:

- **`chat` action** -- Calls `AppCore::chat_send()` with a session key prefixed `mcp:`. Collects the streamed `AgentEvent`s into a single text response plus a tool usage log
- **`status` action** -- Returns agent status JSON
- **Stream collection** -- `collect_agent_stream()` processes events in a `select! biased` loop prioritizing events over interactions. Interactive prompts (`ask_user`) are auto-declined with `FormResponse::Cancelled` since MCP has no interactive prompt capability
- **Session keys** -- Format: `mcp:{uuid}` for one-shot, `mcp:{user-provided}` for continuity

### `schema.rs`

One function: `internal_to_mcp_tool()` converts an internal tool's name, description, and OpenAI-style JSON Schema parameters into an `rmcp::model::Tool`. Handles empty schemas by defaulting to `{"type": "object", "properties": {}}`.

## Logging (`logging.rs`)

| Function | Output | Use case |
|---|---|---|
| `configure_stdio_tracing()` | stderr | Stdio transport (stdout is the MCP JSON-RPC stream) |
| `configure_http_tracing()` | stdout | HTTP transport |

Both use `EnvFilter::from_default_env()` so `RUST_LOG` controls verbosity.

## Default Exposed Tools

Configured in `crates/config/src/schema/mcp.rs` via `default_exposed_tools()`:

```
tasks, project, area, notes, memory, okr, finance, productivity, work_context, agent
```

Users can override in `config.json` at `mcp.server.exposedTools`.

## Relationship to the Desktop App's Embedded MCP Server

The desktop app (`crates/desktop/`) can embed the MCP server by importing `klyntbot_server` as a library:

- **Shared handler** -- Both standalone and embedded modes use `KlyntbotServerHandler`
- **Shared AppCore** -- The desktop app creates `AppCore` once; if embedding MCP, it passes the same `Arc<AppCore>` to `KlyntbotServerHandler`
- **Entity events** -- Both modes emit entity update events through the same `AppEventEmitter` trait, ensuring the UI stays in sync regardless of whether a tool call came from the desktop UI or an external MCP client
- **Config** -- The embedded server reads `mcp.server` from `config.json` for port, host, and tool whitelist

The standalone binary (`klyntbot-mcp`) is the recommended integration path for Claude Code and other IDE clients. It runs as a subprocess managed by the client, communicating over stdio.

## Claude Code Integration

Add to `~/.claude.json`:

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

Tools appear as `mcp__klyntbot__<tool_name>` in Claude Code. The `agent` tool enables natural language delegation to the full klyntbot pipeline.

## Testing

Run tests for this crate:

```bash
cargo nextest run -p klyntbot-server
```

The crate includes unit tests for:
- `bridge/agent.rs` -- Stream collection, error handling, auto-decline of interactions, channel closure
- `bridge/registry.rs` -- Whitelist enforcement (reject unexposed, allow exposed, empty whitelist)
- `bridge/schema.rs` -- Schema translation for populated and empty parameter sets

## Source Files

| File | Purpose |
|---|---|
| `src/main.rs` | Binary entry point, CLI dispatch, transport setup |
| `src/lib.rs` | Library re-exports for embedded use |
| `src/cli.rs` | Clap CLI definition (`Cli`, `Command`) |
| `src/handler.rs` | `KlyntbotServerHandler` implementing `ServerHandler` |
| `src/logging.rs` | Tracing configuration for stdio vs HTTP |
| `src/bridge/mod.rs` | Bridge module declaration |
| `src/bridge/registry.rs` | `ToolRegistryBridge` -- whitelist + tool execution |
| `src/bridge/agent.rs` | `AgentBridge` -- natural language delegation |
| `src/bridge/schema.rs` | OpenAI schema to MCP schema conversion |
