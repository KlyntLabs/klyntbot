# Klyntbot MCP Server — Design Spec

## Summary

Turn klyntbot into an MCP server so Claude Code, Codex, and other AI agents can access klyntbot's full business logic as tools — without opening the desktop app. The server supports two bridge paths: **ToolRegistryBridge** (reuses internal tools directly, client orchestrates) and **AgentBridge** (klyntbot runs its full agent pipeline for natural language requests). The desktop app can also embed the MCP HTTP server, sharing the same `AppCore` instance.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Entry Points                    │
│  ┌──────────┐  ┌───────────┐  ┌──────────────┐  │
│  │  Tauri   │  │klyntbot-  │  │  Future CLI  │  │
│  │ Desktop  │  │   mcp     │  │   tools      │  │
│  └────┬─────┘  └─────┬─────┘  └──────┬───────┘  │
│       │              │               │           │
│       ▼              ▼               ▼           │
│  ┌─────────────────────────────────────────────┐ │
│  │   AppCore (AppMode::Desktop / Server)       │ │
│  │   - Repos, Agent, Bus, Cron                 │ │
│  │   - ToolRegistry (all internal tools)       │ │
│  └─────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────┘

MCP Server layer (in klyntbot-server crate):
┌─────────────────────────────────────────────────┐
│  KlyntbotServerHandler                           │
│  ├─ ToolRegistryBridge (headless tools)          │
│  │   └─ Translates MCP ↔ ToolRegistry           │
│  ├─ AgentBridge (agent delegation)               │
│  │   └─ Calls AppCore::chat_send() directly      │
│  └─ get_status (built-in, no AppCore needed)     │
└─────────────────────────────────────────────────┘
```

### Key Decisions

- **`klyntbot-server`** is a new crate — both a library (for desktop embedding) and a binary (`klyntbot-mcp`). No business logic lives here; it's pure MCP wiring.
- **`mcp` crate stays lean** — no `app-core` dependency added. The bridge lives in `klyntbot-server`. The existing `mcp` crate continues to handle the client side (connecting to external MCP servers).
- **Two bridge paths**: `ToolRegistryBridge` for all standard tools (reuses `ToolRegistry`), `AgentBridge` for the `agent` chat tool (calls `AppCore::chat_send()` directly).
- **Tool whitelist** in config (`mcp.server.exposedTools`) controls which internal tools are visible via MCP. Dangerous tools (filesystem, shell) excluded by default.

### Two Runtime Modes

```
Mode 1: Desktop running (shared AppCore)
┌───────────────────────────────┐
│  Tauri Desktop                │
│  AppCore::init(Desktop)       │
│  ├── UI (webview)             │
│  └── MCP HTTP server :3100    │  ← klyntbot_server::start_http_server(app.clone())
│      sharing same Arc<AppCore>│
└───────────────────────────────┘
  Claude Code connects via HTTP → localhost:3100

Mode 2: Headless (standalone binary)
┌───────────────────────────────┐
│  klyntbot-mcp serve --stdio   │
│  AppCore::init(Server)        │
│  └── MCP stdio server         │
└───────────────────────────────┘
  Claude Code spawns process via stdio
```

## Tool Bridging Mechanism

### Schema Translation

Internal tools generate OpenAI function-calling JSON Schema via `Tool::parameters()`. MCP tools use rmcp's `Tool` model (also JSON Schema, wrapped in rmcp types). Translation is mechanical:

```
Internal Tool::parameters() → serde_json::Value (JSON Schema)
                            ↓
                   rmcp::model::Tool {
                       name: tool.name(),
                       description: tool.description(),
                       input_schema: ToolInputSchema::from(parameters_value),
                   }
```

### Execution Flow

```
MCP Client (Claude Code)
  │
  ▼ CallToolRequestParams { name: "task", arguments: {"action":"create","title":"..."} }
  │
KlyntbotServerHandler::call_tool()
  │
  ├─ name == "agent" → AgentBridge::execute()
  │                     └─ AppCore::chat_send() → collect AgentEvents → CallToolResult
  │
  └─ otherwise → ToolRegistryBridge::execute()
                  ├─ Whitelist check (is tool in exposedTools?)
                  ├─ Input sanitization (existing security::sanitize_input)
                  ├─ Construct RoutingContext { channel: "mcp", chat_id: session_id }
                  ├─ ToolRegistry::prepare(name, args, &routing_ctx)
                  ├─ Tool::execute(args, &routing_ctx)
                  └─ Result<String, ToolError> → CallToolResult { content, is_error }
```

### RoutingContext for MCP

```rust
fn build_mcp_routing_context(session_id: &str) -> RoutingContext {
    RoutingContext {
        channel: ChannelName::new("mcp"),
        chat_id: ChatId::new(session_id),
        interaction_channel: None,  // MCP has no interactive prompts (see Elicitation)
        entity_tx: None,            // no UI entity cards
        is_direct_mode: true,       // always direct (no group chat)
        delegation_depth: 0,
    }
}
```

### Error Mapping

| Internal | MCP |
|----------|-----|
| `Ok(text)` | `CallToolResult::success(vec![Content::text(text)])` |
| `Err(ToolError::InvalidParams(msg))` | `McpError::invalid_params(msg, None)` |
| `Err(ToolError::NotFound(msg))` | `CallToolResult` with `is_error: true` |
| `Err(ToolError::ExecutionFailed(msg))` | `CallToolResult` with `is_error: true` |
| `Err(ToolError::PermissionDenied)` | `McpError::invalid_request("Permission denied", None)` |

### Elicitation — Handling `ask_user`

For headless tools (ToolRegistryBridge): not a problem — standard tools don't use `ask_user`.

For agent delegation (AgentBridge): the agent CAN call `ask_user` during its ReAct loop. Solution is capability-aware hybrid:

```
Client connects → ClientCapabilities.elicitation?
  ├─ yes → interaction_rx fires → elicitation/create → relay response back to agent
  └─ no  → interaction_rx fires → auto-respond "User interaction not available via MCP"
```

The agent already handles `ask_user` failures gracefully — it continues with best-effort reasoning. When clients add elicitation support, it automatically upgrades with no code changes.

## Agent Delegation Bridge

Handles the `agent` tool — natural language → full agent pipeline → collected response.

### Execution Flow

```
MCP: agent { action: "chat", message: "What's my progress on Project X?" }
  │
  ▼
AgentBridge::execute()
  │
  ├─ Generate session_key: "mcp:{client_session_id}" or "mcp:{uuid}"
  │
  ├─ AppCore::chat_send(message, session_key, None)
  │   → returns (ChatMessageResponse, ChatStreamInfo)
  │
  ├─ Collect loop on ChatStreamInfo.event_rx:
  │   match event {
  │     AgentEvent::ContentChunk { data } → response.push_str(&data)
  │     AgentEvent::ToolStart { name, .. } → tool_log.push(format!("→ {name}"))
  │     AgentEvent::ToolEnd { name, success, .. } → tool_log.push(format!("✓ {name}"))
  │     AgentEvent::Done { .. } → break
  │     AgentEvent::Error { .. } → return error result
  │     _ → skip (classification, iteration, usage — internal telemetry)
  │   }
  │
  ├─ Handle interaction_rx (capability-aware):
  │   select! on both event_rx and interaction_rx
  │   → elicitation supported? relay to client
  │   → not supported? auto-decline
  │
  └─ Build CallToolResult:
      Content::text(response)  // agent's synthesized answer
      + optional Content::text(tool_log)  // transparency: which tools were called
```

### Session Continuity

The `session_key` allows multi-turn conversations. If the MCP client passes `session_key: "my-session"`, subsequent calls continue the conversation with full history.

### Timeout

Agent pipeline can take 30-60s for complex queries (multiple tool calls + LLM synthesis). Default timeout: 120s, configurable.

### Response Structure

```json
{
  "content": [
    { "type": "text", "text": "Project X is 67% complete. 12 of 18 tasks done..." },
    { "type": "text", "text": "[Tools used: task(list), project(get), memory(facts)]" }
  ],
  "isError": false
}
```

## AppCore Init with AppMode

### AppMode Enum

```rust
// crates/common/src/types.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    Desktop,
    Server,
}
```

### Modified Init Phases

```
AppCore::init(mode: AppMode)
  │
  ├─ Phase 1 (ALWAYS):
  │   config → storage → repos → vector_store → provider → bus
  │   → persona_manager → agent_loop → shutdown_token
  │
  ├─ Phase 2 (Desktop + Server):
  │   cron_service → register_cron_callbacks → ensure_cron_jobs
  │
  ├─ Phase 3 (Desktop ONLY — skipped for Server):
  │   channel_manager.start_all()      ← Telegram/Discord/Slack/Email
  │   productivity_engine              ← focus tracking, session timing
  │   focus_manager                    ← Pomodoro, deep work
  │   nudge_service                    ← "take a break" nudges
  │   distraction_interceptor          ← off-topic detection
  │   coaching_service                 ← signal → pattern → intervention
  │   feedback_tracker                 ← coaching feedback persistence
  │
  └─ Return (AppCore, EventChannels)
      For Server mode: EventChannels are empty/noop — no one listens
```

The optional fields (`productivity_engine`, `focus_manager`, etc.) are already `Option<Arc<...>>`. In Server mode, they're `None`. Handlers that access them already return `Err(ApiError { code: "FEATURE_DISABLED" })`.

`AppMode` is a runtime property of the entry point, not user configuration:

```rust
AppCore::init(AppMode::Server).await   // klyntbot-mcp
AppCore::init(AppMode::Desktop).await  // Tauri
```

## Crate Structure

```
crates/klyntbot-server/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point: CLI parse → init → run server
│   ├── lib.rs               # Library exports: start_http_server(), KlyntbotServerHandler
│   ├── cli.rs               # clap CLI definition
│   ├── handler.rs           # KlyntbotServerHandler (ServerHandler impl)
│   ├── logging.rs           # Tracing setup (stderr for stdio, file for http)
│   └── bridge/
│       ├── mod.rs           # Re-exports
│       ├── registry.rs      # ToolRegistryBridge: MCP ↔ ToolRegistry
│       ├── agent.rs         # AgentBridge: MCP → AppCore::chat_send()
│       └── schema.rs        # OpenAI JSON Schema → rmcp Tool translation
```

### Dependencies

```
klyntbot-server
  ├── app-core     (AppCore, handlers, AppMode)
  ├── mcp          (re-use security module)
  ├── tools-core   (Tool trait, ToolRegistry, RoutingContext)
  ├── common       (types, errors)
  ├── config       (Config, McpServerSettings)
  ├── rmcp         (server handler, transport, protocol types)
  ├── clap         (CLI)
  ├── tokio        (async runtime)
  └── tracing      (logging)
```

### CLI Interface

```bash
klyntbot-mcp serve --stdio                          # Stdio transport
klyntbot-mcp serve --http --port 3100 --host 0.0.0.0  # HTTP transport
klyntbot-mcp tools --list                           # List available tools
klyntbot-mcp tools --schema task                    # Show tool schema
klyntbot-mcp --version
```

### main.rs Flow

```
1. Parse CLI args (clap)
2. Configure tracing (stderr for stdio, file for http)
3. Load config: config::load_with_env_overrides()
4. Init: AppCore::init(AppMode::Server)
5. Build whitelist from config.mcp.server.exposed_tools
6. Create KlyntbotServerHandler { app, whitelist }
7. Match transport:
   ├── --stdio → rmcp::transport::io::stdio()
   └── --http  → rmcp streamable HTTP server on host:port
8. handler.serve(transport).await
9. select! { service.waiting(), ctrl_c() }
10. app.shutdown()
```

## Config Changes

### McpServerSettings

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerSettings {
    pub enabled: bool,               // default false
    pub port: u16,                   // default 3100
    pub host: String,                // default "127.0.0.1"
    pub exposed_tools: Vec<String>,  // NEW — whitelist, default: safe tools
    pub auth: McpAuthConfig,         // NEW — HTTP auth
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpAuthConfig {
    pub enabled: bool,                 // default false
    pub token: Option<Secret<String>>, // Bearer token for HTTP transport
}
```

### Default Exposed Tools

```rust
vec![
    // P0: core productivity
    "task", "project", "area", "note", "memory",
    // P1: extended
    "objective", "key_result", "finance",
    "productivity", "work_context",
    // Agent delegation
    "agent",
]
```

### Not Exposed by Default

- `read_file`, `write_file`, `edit_file`, `list_dir` — filesystem access
- `shell` — arbitrary command execution
- `web_search`, `web_fetch` — network access (client has its own)
- `annotate`, `learning` — internal agent tools

### User Config Example

```json
{
  "mcp": {
    "server": {
      "enabled": true,
      "exposedTools": ["task", "project", "note", "memory", "agent"]
    }
  }
}
```

### Claude Code Config

```json
// Option A: Desktop running (HTTP)
{
  "klyntbot": {
    "url": "http://localhost:3100/mcp"
  }
}

// Option B: Standalone (stdio)
{
  "klyntbot": {
    "command": "klyntbot-mcp",
    "args": ["serve", "--stdio"]
  }
}
```

## HTTP Transport & Production Hardening

### Auth for HTTP

```
Request → Check Authorization header
  ├─ auth.enabled == false → pass through
  ├─ header matches auth.token → pass through
  └─ missing or wrong → 401 Unauthorized
```

Stdio transport needs no auth — process isolation is sufficient.

### Concurrent Access Safety

| Concern | Solution | Status |
|---------|----------|--------|
| SQLite concurrent writes | WAL mode + `PRAGMA busy_timeout = 5000` | WAL exists, add busy_timeout |
| Desktop + MCP shared AppCore | Same `Arc<AppCore>` — no contention | By design (Mode 1) |
| Standalone + Desktop separate processes | SQLite WAL handles concurrent readers + serialized writers | Works out of the box |
| Port conflict | Check if port is in use before binding HTTP | Standard TCP behavior |

### Graceful Shutdown

```
klyntbot-mcp (standalone):
  select! {
      server.waiting() => { /* server ended */ }
      ctrl_c()         => { app.shutdown().await }
  }

Desktop embedded:
  Tauri on_exit hook → app.shutdown() → HTTP server drops with AppCore
```

## Testing Strategy

### Unit Tests

- Schema translation: internal `Tool::parameters()` → rmcp `Tool` model
- Whitelist filtering: dangerous tools excluded, safe tools pass
- RoutingContext construction: channel = "mcp", is_direct_mode = true
- AgentEvent collection: ContentChunk aggregation, tool log building
- Error mapping: ToolError variants → MCP error/result

### Integration Tests (in-memory AppCore)

- Task CRUD via ToolRegistryBridge: create → list → verify
- Agent chat via AgentBridge: natural language → collected response
- Unexposed tool rejection: whitelist enforcement
- Auth middleware: valid token passes, invalid rejected

### E2E Tests

- Spawn `klyntbot-mcp serve --stdio`, send JSON-RPC `initialize` + `tools/list`, verify tool catalog
- Full tool call roundtrip over stdio transport
- Claude Code integration: `claude --mcp-config test-mcp.json "List my tasks"`

### Test Utilities

- `create_test_app()` — `AppCore::init(AppMode::Server)` with in-memory storage
- `mcp_routing_ctx()` — RoutingContext with channel "mcp"
- `extract_text(result: &CallToolResult)` — pulls text from Content vec

## Implementation Phases

### Phase 1: Foundation

**Goal:** `klyntbot-mcp serve --stdio` compiles, starts, exposes `get_status` with real AppCore.

New files:
- `crates/klyntbot-server/Cargo.toml`
- `crates/klyntbot-server/src/{main,lib,cli,logging,handler}.rs`

Modified files:
- `Cargo.toml` — add to workspace
- `crates/common/src/types.rs` — add `AppMode`
- `crates/app-core/src/init.rs` — accept `AppMode`, gate Phase 3
- `crates/app-core/src/state.rs` — store `AppMode`

### Phase 2: Tool Bridge

**Goal:** All whitelisted internal tools available via MCP. Full CRUD works.

New files:
- `crates/klyntbot-server/src/bridge/{mod,registry,schema}.rs`

Modified files:
- `crates/klyntbot-server/src/handler.rs` — wire bridge
- `crates/config/src/schema/mcp.rs` — add `exposed_tools`, `McpAuthConfig`

### Phase 3: Agent Delegation

**Goal:** `agent` tool works — natural language → full pipeline → collected response.

New files:
- `crates/klyntbot-server/src/bridge/agent.rs`

Modified files:
- `crates/klyntbot-server/src/handler.rs` — route "agent" to AgentBridge

### Phase 4: Desktop Embedding

**Goal:** Tauri desktop app runs MCP HTTP server when `mcp.server.enabled`.

Modified files:
- `crates/desktop/Cargo.toml` — add `klyntbot-server` dependency
- `crates/desktop/src/lib.rs` — spawn HTTP server on setup

### Phase 5: Production Hardening

**Goal:** Auth, busy_timeout, smart stdio→HTTP fallback.

New files:
- `crates/klyntbot-server/src/auth.rs`

Modified files:
- `crates/storage/src/pool.rs` — add `PRAGMA busy_timeout = 5000`
- `crates/klyntbot-server/src/main.rs` — probe localhost before standalone init

### Estimated Size

- `klyntbot-server` crate: ~800-1200 lines
- `app-core` changes: ~30 lines
- `config` changes: ~40 lines
- `common` changes: ~10 lines
- `storage` changes: ~2 lines
