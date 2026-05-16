# Crate: `mcp`

> **Status:** 🟡 In Progress (server-side approval is always-decline stub)
> **Subsystem:** [11 — Channels, MCP & Activity](../subsystems/11-channels-mcp.md)
> **Status last verified:** 2026-05-16
> **One-liner:** MCP client + server-side bridges; **NOT** the same as `mcp-bridge` (which is a separate Unix-socket IPC layer)

---

## TL;DR

`mcp` is the Model Context Protocol layer. **Server side** exposes Klynt's internal tools to external clients via `ToolRegistryBridge` + `AgentBridge`. **Client side** consumes external MCP servers via `McpManager` with `Stdio` or `Streamable HTTP` transports, circuit breaking, and sampling delegation (LLM-to-LLM). **Three approval channels** (`DesktopApprovalChannel`, `TelegramApprovalChannel`, `BlockingFallbackChannel`) cover non-MCP flows; the fourth, `McpApprovalChannel`, **always declines** — remote MCP clients cannot interactively approve.

The exposed tool list is **runtime-computed**: `default_exposed_tools()` returns empty Vec; `app-core` post-init fills it with `AiFeatureRegistry::tool_names()` ∪ `EXPLICIT_TOOL_ALLOWLIST` (16 hardcoded names including 8 coding-memory recall tools).

**Important distinction:** `mcp` and `mcp-bridge` are *different protocols*. `mcp` speaks the MCP wire format. `mcp-bridge` is bespoke Unix-socket IPC with 4-byte LE length-prefixed JSON — used so the stdio MCP child can receive live Tauri events from the desktop parent.

---

## Module map

```
crates/mcp/src/
├── lib.rs                  ← Re-exports + sanitize() function
├── allowlist.rs            ← McpChannelAllowlist
│
├── client/
│   ├── mod.rs              ← McpManager + McpClientOptions
│   ├── handler.rs          ← KlyntbotClientHandler + SamplingDelegate
│   ├── transport.rs        ← McpTransport (Stdio | Http) variant resolution
│   └── circuit_breaker.rs  ← McpCircuitBreaker
│
└── server/
    ├── mod.rs              ← Server-side re-exports
    └── approval.rs         ← McpApprovalChannel (always-decline stub)

crates/klyntbot-server/src/
├── lib.rs                  ← KlyntbotServerHandler + serve_stdio + serve_http
├── bridge/
│   ├── mod.rs
│   ├── registry.rs         ← ToolRegistryBridge
│   ├── agent.rs            ← AgentBridge + ProgressEmitter
│   └── schema.rs           ← internal Tool → mcp::model::Tool conversion
├── http.rs                 ← Embedded Axum HTTP entry
└── stats.rs                ← Server health stats
```

*(`klyntbot-server` is a sibling crate but is documented here for cohesion.)*

---

## Public API surface (server side, in `klyntbot-server`)

### `KlyntbotServerHandler`

```rust
pub struct KlyntbotServerHandler {
    app: Arc<AppCore>,
    bridge: ToolRegistryBridge,
    agent_bridge: Option<AgentBridge>,
    whitelist: HashSet<String>,
}

impl KlyntbotServerHandler {
    pub fn new(app: Arc<AppCore>, whitelist: Vec<String>) -> Self;
}

// Implements rmcp::handler::server::ServerHandler
#[async_trait]
impl ServerHandler for KlyntbotServerHandler {
    async fn list_tools(&self) -> Result<ListToolsResult, McpError>;
    async fn call_tool(&self, params: CallToolRequestParams) -> Result<CallToolResult, McpError>;
    async fn list_resources(&self) -> Result<ListResourcesResult, McpError>;
    async fn read_resource(&self, params: ReadResourceRequestParams) -> Result<ReadResourceResult, McpError>;
}
```

`list_tools` returns:
- Always: `get_status`
- If `"agent"` in whitelist: `agent` (delegates to full AI pipeline)
- Plus: `bridge.list_tools()` — filtered by whitelist from `ToolRegistry`

`call_tool` dispatches to `handle_get_status`, `agent_bridge.execute`, or `bridge.execute`.

**Resources exposed:**
- `klyntbot://status`
- `klyntbot://memory/recent`
- `klyntbot://tasks/today`
- `klyntbot://config/skills`

### `ToolRegistryBridge`

```rust
pub struct ToolRegistryBridge {
    registry: Arc<RwLock<ToolRegistry>>,
    whitelist: Arc<RwLock<HashSet<String>>>,
    domain_bus: Option<Arc<DomainEventBus>>,
}

impl ToolRegistryBridge {
    pub fn new(
        registry: Arc<RwLock<ToolRegistry>>,
        whitelist: HashSet<String>,
        domain_bus: Option<Arc<DomainEventBus>>,
    ) -> Self;

    pub async fn update_whitelist(&self, whitelist: HashSet<String>);

    pub async fn list_tools(&self) -> Vec<McpTool>;

    pub async fn execute(
        &self,
        tool_name: &str,
        arguments: Value,
        session_id: &str,
    ) -> Result<CallToolResult, McpError>;
}
```

`execute()` flow:
1. Whitelist check
2. Read lock on registry → `registry.prepare(name, args, ctx)` → returns `DynTool` → drop lock
3. `tool.execute(args, &ctx)` with `RoutingContext { channel: MCP_CHANNEL, session_mode: Assistant, chat_id: "mcp:{session_id}" }`
4. Publishes `DomainEvent::ToolCallExecuted` if `domain_bus` set
5. Post-mutation entity updates via `emit_entity_update_for_tool` using `AiFeatureRegistry` or `NON_FEATURE_TOOL_ENTITY_KINDS` fallback

### `AgentBridge`

```rust
pub struct AgentBridge { app: Arc<AppCore> }

impl AgentBridge {
    pub fn new(app: Arc<AppCore>) -> Self;

    /// The `agent` MCP tool — delegates natural language to AppCore::chat_send.
    pub async fn execute(
        &self,
        message: &str,
        session_id: &str,
        progress: Option<&ProgressEmitter>,
    ) -> Result<CallToolResult, McpError>;
}

pub struct ProgressEmitter { /* opaque */ }
```

`execute` calls `AppCore::chat_send(message, session_key, ...)`, collects the full streaming `AgentEvent` sequence:
- **Auto-declines `InteractionBundle` requests** with `FormResponse::Cancelled` (MCP has no interactive prompt)
- Emits `notifications/progress` per `ContentChunk` + `ToolStart` when a `ProgressEmitter` is provided
- Returns the assembled response as `CallToolResult`

### `serve_stdio` + `serve_http`

```rust
pub async fn serve_stdio(app: Arc<AppCore>, whitelist: Vec<String>) -> Result<()>;

pub async fn serve_http(
    app: Arc<AppCore>,
    whitelist: Vec<String>,
    bind: SocketAddr,
    auth_token: Option<String>,
) -> Result<()>;
```

**Stdio mode:** Used by `klyntbot mcp serve --stdio` (called by Claude Code etc.). Uses `rmcp::transport::io::stdio()`. Drains event channels in a separate task. Calls `app.shutdown()` before returning.

**HTTP mode:** Embedded Axum HTTP server, spawned from `desktop::run_desktop_app` if `config.mcp.server.enabled`. Uses `rmcp::transport::streamable_http_server::StreamableHttpService<KlyntbotServerHandler, LocalSessionManager>` mounted at `/mcp`. Optional bearer-token auth middleware (`Authorization: Bearer <token>`).

---

## Public API surface (client side, in `mcp`)

### `McpManager`

```rust
pub struct McpManager { /* opaque */ }

impl McpManager {
    pub fn new(config: McpClientOptions) -> Self;

    /// Connect to an external MCP server.
    pub async fn connect(&self, server_def: &McpServerDef) -> Result<()>;

    /// Disconnect from a server.
    pub async fn disconnect(&self, server_name: &str) -> Result<()>;

    /// List all connected servers' tools, namespaced as `mcp_{server}_{tool}`.
    pub async fn list_tools(&self) -> Vec<DynTool>;

    /// Invoke a tool on a remote server.
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        args: Value,
    ) -> Result<String, ProviderError>;

    pub async fn reconnect_unhealthy(&self);
    pub async fn shutdown(&self);
}

pub struct McpClientOptions {
    pub channel_allowlist: McpChannelAllowlist,
    pub circuit_breaker: McpCircuitBreaker,
    pub sampling_delegate: Option<Arc<dyn SamplingDelegate>>,
    pub auto_reconnect_interval: Duration,
}
```

### `McpTransport`

```rust
// Defined in crates/config/src/schema/mcp.rs
#[serde(tag = "transport", rename_all = "camelCase")]
pub enum McpTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    Http {
        url: String,
        headers: HashMap<String, String>,
    },
}
```

**Stdio** uses `rmcp::transport::TokioChildProcess` — process group cleanup is automatic via `process_wrap` (auto-kill on drop).

**HTTP** uses `rmcp::transport::StreamableHttpClientTransport`. Supports streaming + bidirectional sampling.

### `SamplingDelegate`

```rust
#[async_trait]
pub trait SamplingDelegate: Send + Sync {
    async fn sample(&self, params: CreateMessageRequestParams) -> Result<CreateMessageResult, McpError>;
}
```

Set via `McpClientOptions::sampling_delegate`. When an external MCP server sends `sampling/createMessage`, `KlyntbotClientHandler::create_message` invokes the delegate. Returns `method_not_found` if none configured.

The delegate is typically `Arc<DynProvider>` wrapped to translate MCP `CreateMessageRequest` ↔ `providers::ChatParams` — Klynt as the LLM for an MCP server's sampling needs.

### `McpCircuitBreaker`

```rust
pub struct McpCircuitBreaker {
    state: DashMap<String, BreakerState>,
    config: BreakerConfig,
}

impl McpCircuitBreaker {
    pub fn new(threshold: u32, cooldown_secs: u64) -> Self;

    pub fn is_open(&self, server: &str) -> bool;
    pub fn record_failure(&self, server: &str);
    pub fn record_success(&self, server: &str);
    pub fn force_reset(&self, server: &str);

    /// Background task: every 30s, polls servers with cooldown_expired() and reconnects.
    /// Also reacts to notifications/tools/list_changed signals.
    pub fn start_health_check(self: &Arc<Self>, manager: Arc<McpManager>) -> JoinHandle<()>;
}

pub struct BreakerConfig {
    pub threshold: u32,           // Opens after N consecutive failures
    pub cooldown: Duration,        // Wait before half-open probe
    pub probe_timeout: Duration,   // Time given to half-open probe
}
```

### `McpChannelAllowlist`

```rust
pub struct McpChannelAllowlist {
    map: HashMap<String, HashSet<String>>,
}

impl McpChannelAllowlist {
    pub fn new(map: HashMap<String, HashSet<String>>) -> Self;

    /// Unconfigured channels allow all servers.
    pub fn is_server_allowed(&self, channel: &str, server: &str) -> bool;
}
```

Per-server `enabled_tools`/`disabled_tools` (in `McpServerDef`) filter individual tools at discovery time.

Agent profiles have a `mcp_tools: Vec<String>` field — empty denies all, `["*"]` allows all.

### `McpApprovalChannel` (server-side, stub)

```rust
// crates/mcp/src/server/approval.rs
pub struct McpApprovalChannel;

#[async_trait]
impl ApprovalChannel for McpApprovalChannel {
    async fn request(&self, req: ApprovalRequest) -> Result<ApprovalDecision> {
        let reason = serde_json::json!({
            "code": "approval-required",
            "tool": req.tool,
            "action": req.action,
            "class": req.class,
            "message": "Open Klynt on desktop to confirm."
        });
        Ok(ApprovalDecision::Decline { reason: reason.to_string() })
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            supports_classes: HashSet::from([ApprovalClass::Destructive, ApprovalClass::Admin]),
            supports_action_responses: false,
        }
    }
}
```

**Always declines.** Wraps the reason as a structured JSON error so the MCP client can route the user to the desktop app. See [`subsystems/11-channels-mcp.md`](../subsystems/11-channels-mcp.md) for the consumer-side translation (`deny_to_mcp_error`).

### `sanitize` (tool-name namespacing)

```rust
pub fn sanitize(server: &str, tool: &str) -> String;
```

External MCP tools registered into `ToolRegistry` use the convention `mcp_{sanitized_server}_{sanitized_tool}`:
- Invalid characters → `_`
- Result capped at 64 chars
- If overflow, append an 8-char hash suffix (collision-resistant)

The **original (unsanitized) tool name** is preserved separately and used in actual `tools/call` RPC to the remote server.

---

## `EXPLICIT_TOOL_ALLOWLIST` — runtime exposure

```rust
// crates/config/src/schema/mcp.rs
pub const EXPLICIT_TOOL_ALLOWLIST: &[&str] = &[
    "memory", "agent", "annotate", "cron", "alarm", "mirror", "temporal", "launcher",
    "recall_index", "recall_timeline", "recall_fetch", "trace_causes",
    "check_dead_ends", "recall_facts_as_of", "recall_change_history", "recall_decision_points",
];

pub fn default_exposed_tools() -> Vec<String> { vec![] }
```

**`default_exposed_tools()` returns empty Vec.** App-core post-init fills it:

```rust
// in app-core::init or similar
let mut exposed: HashSet<String> = AiFeatureRegistry::tool_names().into_iter().collect();
for &name in EXPLICIT_TOOL_ALLOWLIST {
    exposed.insert(name.to_string());
}

if config.mcp.server.exposed_tools.is_empty() {
    config.mcp.server.exposed_tools = exposed.into_iter().collect();
    config.mcp.server.exposed_tools_auto_filled = true;
}
```

User can override `mcp.server.exposedTools` in `config.json` to a hand-curated list. If user-provided is empty, auto-fill runs and `exposed_tools_auto_filled = true` for diagnostics.

---

## Internals

### Read-then-execute (drops lock before tool runs)

```rust
// ToolRegistryBridge::execute
let tool: DynTool = {
    let registry = self.registry.read().await;     // ← acquire
    if !self.is_whitelisted(tool_name).await {
        return Err(McpError::method_not_found(tool_name));
    }
    registry.prepare(tool_name, &arguments, &ctx)?  // ← clone Arc<dyn Tool>
};                                                  // ← drop lock here

let result = tool.execute(arguments, &ctx).await?;  // ← no lock held
```

The read lock is held only during the prepare phase. Once we have the `Arc<dyn Tool>`, we drop the lock and execute without blocking other lookups.

### `AgentBridge` auto-declines interactions

MCP doesn't have a generic interactive-prompt primitive (`sampling/createMessage` is for LLM, not UI). When the agent pipeline emits an `InteractionBundle` (e.g., from `ask_user` tool), `AgentBridge` immediately responds with `FormResponse::Cancelled`. The original `ask_user` call sees this as "user cancelled" and the agent loop moves on.

**Implication:** Tools that depend on interactive prompts (`ask_user`, complex form responses) don't work over MCP. Use the desktop app or Telegram.

### Circuit breaker state machine

```
[Closed]      → failure × N → [Open]
   ▲                              │
   │ success                      │ cooldown elapsed
   │                              ▼
[HalfOpen] ← probe — Success → [Closed]
                    ↘ Failure → [Open]
```

`is_open(server)` automatically resets the breaker if cooldown elapsed. `start_health_check` task polls every 30s for servers with `cooldown_expired()` and attempts reconnect. Also reacts to server-sent `notifications/tools/list_changed` to refresh tool list.

### Stdio child process group cleanup

`rmcp::transport::TokioChildProcess` uses the `process_wrap` crate to put the child in its own process group. On parent drop, the entire group is killed — prevents zombie processes if the parent panics or the child spawns sub-processes.

### Streaming HTTP transport

`rmcp::transport::StreamableHttpClientTransport` is HTTP-based but supports streaming (long-lived requests, server-sent events). Used for HTTP-mode MCP servers. Compatible with the `streamable_http_server` on the server side.

### Resource read paths

`klyntbot://status` → calls `app.status_handler.status()` → returns JSON.
`klyntbot://memory/recent` → calls cognitive recall for recent items.
`klyntbot://tasks/today` → calls task handler with today's filter.
`klyntbot://config/skills` → returns skill listing from `SkillStore`.

All return `ReadResourceResult` with `contents` as a `Vec<ResourceContents>`. Each resource is implemented as a method on `KlyntbotServerHandler::handle_<resource>`.

---

## Workflows

### Server: Claude Code spawns the MCP server child

```
1. Claude Code config has:
   mcp_servers: {
     "klyntbot": { command: "/Applications/Klyntbot.app/Contents/MacOS/Klyntbot",
                    args: ["mcp", "serve", "--stdio"], env: { ... } }
   }
2. Claude Code spawns the binary as a child process
3. desktop main():
   - args[1] = "mcp", args[2] = "serve", args[3] = "--stdio"
   - Cli::parse routes to run_mcp_stdio()
4. run_mcp_stdio:
   - app_core::init(handle) → AppCore::init_with_sender
   - klyntbot_server::serve_stdio(app, whitelist).await
5. serve_stdio:
   - KlyntbotServerHandler::new(app, whitelist)
   - Build rmcp stdio transport from std::io::stdin + std::io::stdout
   - Server loop: rmcp handles initialize, listTools, callTool, readResource, etc.
6. On Ctrl+C / parent disconnect:
   - app.shutdown().await
   - Return from serve_stdio
   - desktop main exits
```

### Server: tool call dispatched

```
Claude Code: tools/call { name: "tasks", arguments: { action: "list", limit: 10 } }
   ↓
KlyntbotServerHandler::call_tool
   ↓
1. Is "tasks" in whitelist? Yes → ToolRegistryBridge::execute
   (If "agent" → AgentBridge::execute)
   (If unknown → handle_get_status or McpError::method_not_found)
2. ToolRegistryBridge::execute:
   - Acquire read lock on registry
   - registry.prepare("tasks", &args, &ctx) → returns DynTool
   - Drop lock
   - tool.execute(args, &ctx).await
3. ctx is built with:
   - channel: MCP_CHANNEL
   - session_mode: Assistant
   - chat_id: "mcp:{session_id}"
4. TaskTool runs through full agent pipeline (approval, db writes, etc.)
5. If tool is destructive + class != Safe:
   - approval gate runs with McpApprovalChannel
   - McpApprovalChannel always declines with structured JSON
   - Tool returns PermissionDenied; ToolRegistryBridge translates to McpError
6. On success:
   - CallToolResult::success(vec![Content::text(result_string)])
   - Optional DomainEvent::ToolCallExecuted published
   - Optional emit_entity_update_for_tool fires entity-updated event
```

### Client: connecting to an external MCP server

```rust
let manager = McpManager::new(McpClientOptions {
    channel_allowlist: …,
    circuit_breaker: McpCircuitBreaker::new(5, 60),
    sampling_delegate: Some(Arc::new(MyProviderDelegate)),
    auto_reconnect_interval: Duration::from_secs(30),
});

// Connect to e.g. a filesystem MCP server
manager.connect(&McpServerDef {
    name: "fs".into(),
    transport: McpTransport::Stdio {
        command: "mcp-fs-server".into(),
        args: vec!["--root", "/Users/me/docs"].into_iter().map(String::from).collect(),
        env: HashMap::new(),
    },
    enabled_tools: None,         // all tools allowed
    disabled_tools: None,
}).await?;

// Tools now appear in registry as:
//   mcp_fs_read_file, mcp_fs_write_file, mcp_fs_list_directory, ...
let tools = manager.list_tools().await;
```

### Client: server returns sampling request

```
External MCP server: sampling/createMessage { prompt: "Summarize this code", … }
   ↓
KlyntbotClientHandler::create_message
   ↓
if let Some(delegate) = &self.sampling_delegate {
    let result = delegate.sample(params).await?;
    return Ok(result);
} else {
    return Err(McpError::method_not_found("sampling/createMessage"));
}

// Delegate typically:
//   1. Maps params.messages → providers::Vec<Message>
//   2. Calls dyn_provider.chat_completion(messages, chat_params)
//   3. Maps providers::LlmResponse → CreateMessageResult
//   4. Returns
```

### Circuit-breaker recovery

```
Time 0:   server "fs" responds normally
Time T1:  fs.callTool fails → record_failure("fs") count=1
Time T2:  fs.callTool fails again → count=2
...
Time Tn:  count = threshold → state[fs] = Open (cooldown 60s)
Time Tn+1: McpManager.callTool("fs", …) → returns ServerUnhealthy without network call
Time Tn+60: cooldown expired
Time Tn+61: health_check_task picks "fs" → manager.reconnect("fs")
   → if success: state[fs] = Closed
   → if failure: state[fs] = Open with new cooldown
```

---

## Testing approach

### Test `ToolRegistryBridge::execute`

```rust
let mut registry = ToolRegistry::new();
registry.register(MyTool::new());

let bridge = ToolRegistryBridge::new(
    Arc::new(RwLock::new(registry)),
    HashSet::from(["my_tool".to_string()]),
    None,
);

let result = bridge.execute(
    "my_tool",
    serde_json::json!({"arg": "value"}),
    "test-session",
).await.unwrap();
```

### Test whitelist enforcement

```rust
let bridge = ToolRegistryBridge::new(
    Arc::new(RwLock::new(registry)),
    HashSet::from([/* empty */]),  // nothing whitelisted
    None,
);

let result = bridge.execute("my_tool", serde_json::json!({}), "test").await;
assert!(matches!(result, Err(McpError::MethodNotFound(_))));
```

### Test `McpApprovalChannel` always declines

```rust
let channel = McpApprovalChannel;
let result = channel.request(ApprovalRequest {
    tool: "bash".into(),
    action: None,
    class: ApprovalClass::Destructive,
    scope: ApprovalScope::ToolAction,
    ...
}).await.unwrap();

match result {
    ApprovalDecision::Decline { reason } => {
        let parsed: Value = serde_json::from_str(&reason).unwrap();
        assert_eq!(parsed["code"], "approval-required");
    }
    _ => panic!("expected Decline"),
}
```

### Test `sanitize`

```rust
assert_eq!(mcp::sanitize("filesystem", "read_file"), "mcp_filesystem_read_file");
assert_eq!(mcp::sanitize("my-server", "my/tool"), "mcp_my_server_my_tool");

// Overflow case
let long_server = "a".repeat(50);
let long_tool = "b".repeat(50);
let sanitized = mcp::sanitize(&long_server, &long_tool);
assert!(sanitized.len() <= 64);
assert!(sanitized.starts_with("mcp_"));
```

### Test circuit breaker

```rust
let breaker = McpCircuitBreaker::new(3, 1);  // threshold=3, cooldown=1s

assert!(!breaker.is_open("test"));
breaker.record_failure("test");
breaker.record_failure("test");
breaker.record_failure("test");
assert!(breaker.is_open("test"));

tokio::time::sleep(Duration::from_secs(2)).await;
assert!(!breaker.is_open("test"));  // auto-reset after cooldown
```

### Mock `SamplingDelegate`

```rust
struct EchoDelegate;

#[async_trait]
impl SamplingDelegate for EchoDelegate {
    async fn sample(&self, params: CreateMessageRequestParams) -> Result<CreateMessageResult, McpError> {
        Ok(CreateMessageResult {
            content: TextContent { text: "echo".into() },
            model: "test".into(),
            ...
        })
    }
}
```

---

## Extension points

### Add a resource

1. Add a method on `KlyntbotServerHandler::handle_<resource>(uri: &str) -> Result<Vec<ResourceContents>, McpError>`.
2. Add a match arm in `read_resource` dispatch.
3. Add to `list_resources` output.
4. Document the URI scheme.

### Add a server-side built-in tool

If you want to expose something that isn't already a `Tool` in the registry:
1. Add to `list_tools` (returned alongside `get_status` and `agent`).
2. Add a `handle_<tool>` method on `KlyntbotServerHandler`.
3. Add dispatch arm in `call_tool`.

This is a special path — usually you'd add the tool to the regular `ToolRegistry` instead and rely on whitelist.

### Add an MCP transport

`McpTransport` enum is locked to `Stdio` + `Http`. Adding (e.g.) WebSocket transport:
1. Add variant to `McpTransport` enum + serde config.
2. Implement client-side construction in `client/transport.rs` using `rmcp`'s transport traits.
3. (For server) Implement server-side mounting analogous to `StreamableHttpService`.
4. Update config schema.

⚠️ Cross-cutting; coordinate with `rmcp` crate capabilities.

### Add a `SamplingDelegate` impl

```rust
struct MyDelegate { provider: Arc<DynProvider> }

#[async_trait]
impl SamplingDelegate for MyDelegate {
    async fn sample(&self, params: CreateMessageRequestParams) -> Result<CreateMessageResult, McpError> {
        // map params → ChatParams
        // call self.provider.chat_completion
        // map LlmResponse → CreateMessageResult
        Ok(...)
    }
}
```

Register via `McpClientOptions::sampling_delegate`.

### Add a tool to `EXPLICIT_TOOL_ALLOWLIST`

Edit `crates/config/src/schema/mcp.rs::EXPLICIT_TOOL_ALLOWLIST` and add the tool's registry name. Recompile.

For tools registered via `FeaturePackage` and discovered through `AiFeatureRegistry::tool_names()`, no allowlist update needed — they're auto-included.

### Handle `notifications/tools/list_changed` from a server

The `McpCircuitBreaker::start_health_check` task watches for this notification. To customize:
1. Modify the health-check task or
2. Add a separate handler in `KlyntbotClientHandler`

---

## Key constants

| Constant | Value | Location |
|---|---|---|
| Default circuit breaker `threshold` | per-config | `circuit_breaker.rs` |
| Default circuit breaker `cooldown` | per-config | `circuit_breaker.rs` |
| Default health-check interval | `30s` | `circuit_breaker.rs::start_health_check` |
| `sanitize` max length | `64 chars` | `lib.rs` |
| Sanitize collision suffix length | `8 chars` | `lib.rs` |
| `EXPLICIT_TOOL_ALLOWLIST` | 16 tool names | `crates/config/src/schema/mcp.rs` |
| Resources exposed | 4 (`status`, `memory/recent`, `tasks/today`, `config/skills`) | `klyntbot-server::lib` |

---

## Open questions

- **`McpApprovalChannel` always declines.** Remote MCP clients cannot get interactive approval. Either implement a callback-based protocol (sampling delegation in reverse?) or document the limitation prominently in user-facing material.
- **`BlockingFallbackChannel.capabilities()` claims `supports_classes: {Destructive, Admin}`** but always declines. Either tighten capabilities or rename the channel.
- **Embedded MCP HTTP server has no `/health` route** — status only via `get_status` tool or `klyntbot://status` resource. External monitors can't probe.
- **MCP tool name namespacing truncates at 64 chars with hash suffix.** If two long-named server+tool combos collide on the hash, silent registration conflict. Add a test.
- **No mechanism to discover what's actually exposed via MCP at runtime** beyond `klyntbot mcp tools --list`. A `mcp.diagnostics` Tauri command would help.
- **`EXPLICIT_TOOL_ALLOWLIST` is hardcoded.** If a new "core" tool wants to be MCP-exposed, requires editing the config crate. Could move to config.
- **`AgentBridge` auto-declines `InteractionBundle`** — silently. Should ideally surface as a server-side notification so the MCP client knows the agent asked something.
- **No retry inside `McpManager.call_tool`.** Single failure trips the breaker. Acceptable today; would matter for high-latency servers.
- **Sampling delegation is one-way.** Klynt's server doesn't request sampling FROM external servers (only the other direction). Could be useful for tool tests.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #2 (stubs) + #5 (drift) for specifics.

---

## Cross-references

- [Subsystem 11 — Channels, MCP & Activity](../subsystems/11-channels-mcp.md) (parent)
- [`crates/tools-core.md`](./tools-core.md) — `ToolRegistry`, `ApprovalClass`, `RoutingContext`
- [`crates/agent.md`](./agent.md) — `AgentBridge` delegates to `AgentLoop::process_direct_streaming`
- [`crates/app-core.md`](./app-core.md) — `KlyntbotServerHandler::new(app: Arc<AppCore>, …)`
- [`crates/desktop.md`](./desktop.md) — `mcp serve` subcommand + embedded HTTP server startup
- [`crates/providers.md`](./providers.md) — `SamplingDelegate` typically wraps `DynProvider`
