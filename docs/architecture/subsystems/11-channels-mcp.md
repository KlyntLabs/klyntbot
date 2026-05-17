# Subsystem 11 — Channels, MCP & Activity

> **Status:** 🟡 In Progress — notification fan-out to Telegram/Discord/Email is unwired (types exist, dispatcher only routes to OS/Tray); MCP server approval always declines
> **Status last verified:** 2026-05-16
> **Crates:** `channels`, `notifications`, `mcp`, `mcp-bridge`, `activity-log` *(5 crates)*
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

The system's external I/O surface plus the activity graph that observes it.

- **`channels`** — 4 chat platform adapters behind a `Channel` trait, all with *different* transports (Telegram = HTTP long-poll, Discord = raw WS Gateway, Slack = Socket Mode WS, Email = IMAP+SMTP, gated on `consent_granted`). 3 of 4 support interactive forms.
- **`notifications`** — `NotificationDispatcher` subscribes to `AlarmFired` on the bus, gates on quiet hours, fans out to **registered** channels (only `os_native` + `tray` today; Telegram/Discord/Email notification types exist but are **NOT wired**).
- **`mcp`** — MCP client (`McpManager`) and server-side bridges (`ToolRegistryBridge`, `AgentBridge`). Client supports `Stdio` + `Streamable HTTP` transports with circuit breaking and sampling delegation.
- **`mcp-bridge`** — Bespoke Unix-socket IPC (NOT MCP wire format). Pushes desktop Tauri events to a child `klyntbot mcp serve --stdio` process. 4-byte LE length-prefixed JSON.
- **`activity-log`** — Normalizes chat messages, tool calls, and window events into a `WorkContext` / `WorkResource` / `ResourceEdge` graph (8 SQLite tables).

---

## Architecture diagram

```mermaid
flowchart TB
    classDef ch fill:#d1c4e9,stroke:#512da8,color:#311b92
    classDef notif fill:#fff8e1,stroke:#f9a825,color:#f57f17
    classDef mcp fill:#e8eaf6,stroke:#3949ab,color:#1a237e
    classDef bridge fill:#b3e5fc,stroke:#0277bd,color:#01579b
    classDef act fill:#dcedc8,stroke:#7cb342,color:#33691e
    classDef stub fill:#f5f5f5,stroke:#999,color:#616161

    TG[Telegram<br/><i>HTTP long-poll 30s<br/>4 interaction types</i>]:::ch
    DC[Discord<br/><i>raw WS Gateway<br/>HELLO-driven heartbeat</i>]:::ch
    SL[Slack<br/><i>Socket Mode WS<br/>fresh URL per connection</i>]:::ch
    EM[Email<br/><i>IMAP poll + SMTP<br/>consent_granted gate</i>]:::ch

    CM[ChannelManager<br/><i>per-channel mpsc(32)<br/>isolated delivery</i>]:::ch

    ND[NotificationDispatcher<br/><i>subscribes to AlarmFired<br/>quiet hours + held release<br/>retry policy</i>]:::notif
    OSN[os_native<br/>tray]:::notif
    NSTUB[Telegram/Discord/Email<br/>notification channels<br/>STUBS — not wired]:::stub

    QH[QuietHoursPolicy<br/><i>IANA tz · overnight windows</i>]:::notif
    HR[HeldReleaseService<br/><i>writes held_notifications<br/>+ scheduled_fires kind=held_release</i>]:::notif

    MCSRV[MCP Server<br/><i>ToolRegistryBridge<br/>AgentBridge (agent tool)</i>]:::mcp
    MCCLI[MCP Client<br/><i>McpManager · McpTransport<br/>Stdio | Http</i>]:::mcp
    SAMP[SamplingDelegate<br/><i>LLM-to-LLM</i>]:::mcp
    CB[McpCircuitBreaker<br/><i>per-server · cooldown 60s</i>]:::mcp
    WL[ExposedTools whitelist<br/><i>AiFeatureRegistry ∪ EXPLICIT_TOOL_ALLOWLIST</i>]:::mcp

    BR[mcp-bridge<br/><i>Unix socket: $KLYNTBOT_HOME/mcp-events.sock<br/>BridgeFrame: 4-byte LE + JSON, 1MB cap</i>]:::bridge

    AL[activity-log<br/><i>8 tables · WorkContext (8 types)<br/>WorkResource · ResourceEdge (4 types)<br/>3 normalizers</i>]:::act

    TG --> CM
    DC --> CM
    SL --> CM
    EM --> CM
    CM --> BUS[(MessageBus + DomainEventBus)]

    BUS --> ND
    BUS --> AL
    ND --> OSN
    ND -.NOT WIRED.-> NSTUB
    QH --> ND
    ND --> HR

    MCSRV --> WL
    MCCLI --> SAMP
    MCCLI --> CB
    BR -.events.-> MCSRV
```

---

## Mental model

Three roles, sometimes confused:

1. **Chat channels** (`channels`) carry **bidirectional user messages** — user types, bot responds. 4 adapters, each with its own transport quirks.
2. **Notification channels** (`notifications`) are **outbound-only push** — alarm fires, system notifies. Today only `os_native` and `tray` are wired; chat-platform notification types exist but go nowhere.
3. **MCP** (`mcp` + `mcp-bridge`) is the **tool-exposure surface for external AI clients** (Claude Code, Cursor, …). Server side exposes Klynt's internal tools; client side consumes external MCP servers.

`activity-log` is orthogonal — it observes everything passing through and builds a navigable work-context graph in SQLite.

### One non-obvious distinction

**`mcp` and `mcp-bridge` are *different protocols*.** `mcp` speaks the MCP JSON-RPC wire format (stdio + Streamable HTTP). `mcp-bridge` is a bespoke Unix-socket IPC with 4-byte LE length-prefixed JSON, used so the standalone `klyntbot mcp serve --stdio` child process can receive live Tauri events from the desktop parent. **They share a name but nothing else.** Documenting them in the same crate would be misleading; documenting them in the same subsystem is fine because they cooperate.

---

## Reference

### `channels` — Channel trait

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, bus: Arc<MessageBus>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn send(&self, msg: &OutboundMessage) -> Result<()>;
    fn is_allowed(&self, sender_id: &str) -> bool;

    async fn send_typing(&self, _chat_id: &str) -> Result<()> { Ok(()) }

    fn supports_interaction(&self) -> bool { false }
    async fn send_interaction(&self, _chat_id: &str, _request: &InteractionRequest) -> Result<FormResponse> {
        Err(...)
    }
}
pub type DynChannel = Arc<dyn Channel>;
```

### 4 adapters with 4 different transports

| Channel | Transport | Reconnect | Interactions | Notes |
|---|---|---|---|---|
| **Telegram** | HTTP long-poll (`getUpdates` 30s timeout); 5s sleep on error | Implicit in poll loop | ✅ `supports_interaction()=true`. 4 types: SingleSelect (rows of 2 buttons), YesNo (2-button), MultiSelect (single-select workaround), FreeText (next-message intercept) | Transcribes voice via Groq if configured. Compound allowlist IDs split on `\|`: `"123\|username"`. |
| **Discord** | Raw WebSocket Gateway (`wss://gateway.discord.gg/?v=10`) via `WebSocketManager` + `reconnect_loop`. `HeartbeatStrategy::None` (HELLO-driven heartbeat spawned per connection) | Via `WebSocketManager` | ✅ Buttons for ≤5 options, `StringSelectMenu` for >5. FreeText via next-message intercept. | Filters bot messages (`author.bot == true`). |
| **Slack** | Socket Mode WebSocket via `WebSocketManager`. Fresh URL from `apps.connections.open` per connection. `HeartbeatStrategy::Timeout { 35s, ping }` | Via `WebSocketManager` | ✅ Block Kit actions. Buttons for select/yes-no, free text via next-message intercept. | Only responds to DMs (`channel_type == "im"`) or `app_mention` events. Strips bot mention with regex. Adds `:eyes:` reaction on receipt (best-effort). |
| **Email** | IMAP polling (default 30s, min 5s) + SMTP outbound | Reconnect on poll error | ❌ `supports_interaction() = false` | **Requires `consent_granted = true`** in config before starting — only channel with explicit consent gate. `#[cfg(feature = "email")]` gated. |

### `ChannelManager` API

- `new(config, bus) -> Result<Self>` — takes ownership of `bus.take_outbound_rx()`
- `initialize_channels() -> Result<()>` — creates enabled adapters from config
- `start_all() -> Result<()>` — spawns each channel in its own task; creates per-channel `mpsc::channel(32)` queues for isolated delivery with user-friendly error feedback
- `stop_all() -> Result<()>` — calls `channel.stop()` on all

### `notifications` — registered surfaces

**Active (registered in `ChannelRegistry`):**

| Channel | Bitfield | Description |
|---|---:|---|
| `os_native` | `CHANNEL_OS_NATIVE = 1` | OS desktop notification (`notify-rust` or platform API) |
| `tray` | `CHANNEL_TRAY = 2` | System tray notification |

**Stubs (defined but NOT wired into `NotificationDispatcher`):**

| Channel type | Bitfield | Status |
|---|---:|---|
| `TelegramNotificationChannel` | `CHANNEL_TELEGRAM = 4` | Type exists; not in registry |
| `DiscordNotificationChannel` | `CHANNEL_DISCORD = 8` | Type exists; not in registry |
| `EmailNotificationChannel` | `CHANNEL_EMAIL = 16` | Type exists (feature-gated); not in registry |

The CLAUDE.md "Multi-channel" boast in README is technically misleading — *chat* multi-channel works; *notification fan-out* to those channels doesn't. Per `notifications/channel/mod.rs:64`: `TODO(4.8 / follow-up)`.

`mask_to_names(mask) -> Vec<String>` and `names_to_mask(names) -> u32` are the inverse pair for converting between bitfields and names.

### `NotificationDispatcher`

```rust
NotificationDispatcher::new(
    bus, channels, default_channels, quiet_hours,
    log_repo, held_repo, held_release, retry
) -> Self

start(self) -> NotificationDispatcherHandle  // { join, shutdown }
```

Subscribes to `DomainEvent::AlarmFired`. Per event:
- If `kind == "held_release"` → `handle_held_release` (look up `held_notifications`)
- Otherwise → `handle_alarm_fired` (normal dispatch path)

The dispatcher does **NOT** filter on `kind == "alarm"` specifically — every non-`held_release` kind (including `cron_job`) flows through. The `CronExecutor` typically consumes `cron_job` kinds first, but the dispatcher isn't selective.

### `QuietHoursPolicy`

```rust
QuietHoursPolicy::new(cfg, iana_tz)
   .is_in_quiet_hours(at: Timestamp) -> Result<bool>     // handles overnight windows
   .next_window_end(at: Timestamp) -> Result<Timestamp>  // upcoming release target
   .override_for_urgent() / .enabled()
```

In `handle_alarm_fired`, if quiet hours active and priority != Urgent (or `override_for_urgent` false), calls `HeldReleaseService::hold(...)` instead of dispatching.

### `HeldReleaseService::hold`

```
1. Generate `held_{uuid}` ID
2. INSERT INTO held_notifications (id, alarm_id, channels (JSON), payload (JSON), release_at_ms, ...)
3. fire_store.schedule(FireSpec {
        kind: "held_release",
        ref_id: Some(id.clone()),
        payload: json!({ "held_id": id }),
        dedup_prefix: Some("held:{id}:"),
        ...
   })   ← schedules a scheduled_fires row
```

`mark_released(held_id)` sets `released = 1` after delivery. Publishes `DomainEvent::HeldNotificationReleased` on bus.

### `notification_log` table (idempotency gate)

`PRIMARY KEY (alarm_id, channel)`. `try_insert(alarm_id, channel, sent_at_ms)` returns `false` (duplicate suppressed) if row exists. `record_ack` + `record_error` update.

### `held_notifications` table (with partial index)

```sql
CREATE INDEX held_pending_idx ON held_notifications (release_at_ms) WHERE released = 0;
```

The partial index is sized only by pending rows — much smaller than a full index, much faster for the "what needs releasing" query.

### `mcp` server side — `ToolRegistryBridge`

Wraps `Arc<RwLock<ToolRegistry>>` + a runtime-updatable `HashSet<String>` whitelist.

- `list_tools() -> Vec<McpTool>` — filters registry by whitelist
- `execute(tool_name, arguments, session_id) -> Result<CallToolResult, McpError>`:
  - Whitelist check
  - `registry.prepare()` (read lock, then dropped)
  - `tool.execute()` with `RoutingContext { channel = MCP_CHANNEL, session_mode = SessionMode::Assistant, chat_id = "mcp:{session_id}" }`
  - Publishes `DomainEvent::ToolCallExecuted` if `domain_bus` set

### `mcp` server side — `AgentBridge`

`AgentBridge::new(app: Arc<AppCore>)` — the `agent` MCP tool delegates natural-language requests to `AppCore::chat_send(message, session_key, ...)`. Collects the `AgentEvent` stream:
- **Auto-declines `InteractionBundle` requests** (MCP has no interactive prompt — the `ask_user` flow doesn't work over MCP)
- Emits `notifications/progress` per `ContentChunk` and `ToolStart` if `ProgressEmitter` provided

### `McpTransport` enum

```rust
#[serde(tag = "transport", rename_all = "camelCase")]
pub enum McpTransport {
    Stdio { command: String, args: Vec<String>, env: HashMap<String, String> },
    Http  { url: String, headers: HashMap<String, String> },
}
```

- `Stdio` uses `rmcp::transport::TokioChildProcess` — process group cleanup is automatic via `process_wrap` (auto-kill on drop)
- `Http` uses `rmcp::transport::StreamableHttpClientTransport`

### `SamplingDelegate` (LLM-to-LLM via MCP)

```rust
pub trait SamplingDelegate: Send + Sync {
    async fn sample(&self, params: CreateMessageRequestParams) -> Result<CreateMessageResult, McpError>;
}
```

Set via `McpClientOptions::sampling_delegate`. When an external MCP server sends `sampling/createMessage`, `KlyntBotClientHandler::create_message` invokes the delegate. Returns `method_not_found` if none configured.

### `McpCircuitBreaker`

`McpCircuitBreaker::new(threshold: u32, cooldown_secs: u64)`. Per-server state in a `DashMap`.

- Opens after `threshold` failures within `cooldown` window
- `is_open(server)` — auto-resets if cooldown expired
- `McpManager::start_health_check` task polls every 60s for servers with `cooldown_expired()` and reconnects. Reacts to `notifications/tools/list_changed` signals from servers. The `McpCircuitBreaker` itself does not own the health-check task.

### MCP allowlists (multiple layers)

| Layer | Purpose |
|---|---|
| `McpChannelAllowlist` | `HashMap<channel_name, HashSet<server_name>>`. Unconfigured channels allow all servers. |
| Per-server `enabled_tools` / `disabled_tools` | Filter individual tools at discovery time. |
| Agent profile `mcp_tools: Vec<String>` | Empty denies all; `["*"]` allows all. |

### `default_exposed_tools()` — computed at runtime

The config function returns an **empty Vec**. App-core fills it post-init from:

```rust
AiFeatureRegistry::tool_names()
   ∪
EXPLICIT_TOOL_ALLOWLIST = [
    "memory", "agent", "annotate", "cron", "alarm", "mirror", "temporal", "launcher",
    "recall_index", "recall_timeline", "recall_fetch", "trace_causes",
    "check_dead_ends", "recall_facts_as_of", "recall_change_history", "recall_decision_points",
]
```

User can override `mcp.server.exposed_tools` in config. If user-provided list is empty, auto-fill runs and `exposed_tools_auto_filled = true` for diagnostics.

### MCP tool name namespacing

External MCP tools registered into `ToolRegistry` use the convention `mcp_{sanitized_server}_{sanitized_tool}` — invalid chars → `_`, result capped at 64 chars with 8-char hash suffix on overflow. The original (unsanitized) tool name is preserved and used in actual `tools/call` RPC.

### `mcp-bridge` — wire format

```rust
pub struct BridgeFrame {
    pub event: String,           // e.g. "entity:updated", "provider:degraded"
    pub payload: serde_json::Value,
}
```

**Encoding:** 4-byte little-endian length prefix + JSON body. **Max:** `MAX_FRAME_BYTES = 1 << 20` (1 MB). Clean EOF before length prefix → `Ok(None)`. Partial reads or oversized frames → `Err`.

**Socket path:** `${KLYNTBOT_HOME or ~/.klyntbot}/mcp-events.sock` — resolved by `bridge_socket_path()` calling `config::loader::config_dir()`.

**Components:**
- `BridgeServer` — desktop side; listens on socket and fans frames to connected children
- `BridgeClient` — child MCP process side; connects + receives
- `SocketBridgeEmitter` — desktop-side `AppEventEmitter` impl that serializes events as `BridgeFrame`

### `activity-log` — types

```rust
pub struct WorkContext {
    pub id: Ulid,
    pub title: String,
    pub status: WorkContextStatus,           // active | paused | completed | archived
    pub context_type: WorkContextType,       // coding | research | communication | planning |
                                             // review | meeting | learning | general
    pub confidence: f64,
    pub first_seen_at: Timestamp,
    pub last_active_at: Timestamp,
    pub total_duration_secs: u64,
    pub event_count: u64,
    pub linked_project_id: Option<String>,
    pub embedding_id: Option<String>,
}

pub struct WorkResource {
    pub id: String,
    pub resource_type: String,
    pub resource_name: String,
    pub resource_path: String,
    pub resource_uri: Option<String>,
    pub access_count: u64,
}

pub struct ResourceEdge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: ResourceEdgeType,         // co_access | references | derived_from | related
    pub weight: f64,
}
```

### `activity-log` — 8 tables + 3 normalizers

**Tables:** `unified_activity_log`, `work_contexts`, `work_resources`, `resource_edges`, `work_context_resources`, `work_context_actions`, `context_merges`, `inference_state`.

**`ActivityNormalizer` trait** — 3 impls:

| Normalizer | Input | Maps to |
|---|---|---|
| `ChatMessageNormalizer` | `ChatMessageInput { session_key, role, content }` | `source: Chat`, `actor: User\|AiAgent`, `action: "prompt"\|"reply"` |
| `ToolCallNormalizer` | `ToolCallInput { tool_name, args_preview, session_key, duration_ms }` | `source: ToolCall`, `actor: AiAgent`, `action: "run"` |
| `WindowEventNormalizer` | `WindowEventInput { app_name, window_title, url, is_idle, ... }` | `source: OsWindow\|Browser`, `actor: User`, `action: "view"\|"browse"`. Idle events return `None` |

Content is SHA-256 hashed for dedup; previews truncated to 500 chars; IDs are ULIDs (time-sortable).

---

## Workflows

### A Telegram message arrives

```
1. TelegramChannel::poll_updates long-polls getUpdates (timeout=30s)
2. handle_update routes by type (callback_query / message_reaction / message)
3. For a normal message:
   - check_allowlist via allowlist split on '|'
   - Start typing indicator via TypingManager
   - Build content from text + media
   - Create InboundMessage::new("telegram", sender_id, chat_id_str, content)
   - bus.publish_inbound(inbound)
4. MessageBus broadcasts InboundMessage; AgentLoop subscriber receives
5. AgentLoop processes through AI pipeline → produces OutboundMessage
6. OutboundMessage → bus.outbound_tx
7. ChannelManager's dispatcher receives from outbound_rx, routes to "telegram" channel queue
8. TelegramChannel::send:
   - Stop typing indicator
   - Format via TelegramFormatter (Markdown → HTML)
   - Split into ≤4096-char chunks
   - sendMessage with parse_mode: "HTML"
   - Fall back to plain text if HTML rejected
```

### MCP tool exposure to Claude Code

```
1. Claude Code runs: klyntbot mcp serve --stdio
2. serve_stdio(app, whitelist) called
   KlyntBotServerHandler::new(app, whitelist) wraps ToolRegistryBridge + AgentBridge
3. rmcp performs MCP initialization handshake (initialize / initialized)
4. Claude Code: tools/list
   → ToolRegistryBridge::list_tools()
   → filters by whitelist + converts to rmcp::model::Tool
5. Claude Code: tools/call { name: "alarm", arguments: {...} }
   → ToolRegistryBridge::execute("alarm", args, session_id)
   → ToolRegistry.prepare("alarm", &args, &ctx) (read lock, dropped after)
   → tool.execute(args, &ctx)
   → wrap as CallToolResult::success(vec![Content::text(result)])
6. rmcp serializes + returns
```

### Alarm fires + held by quiet hours

```
1. DomainEvent::AlarmFired { fire_id, kind: "alarm", payload_json, fired_at_ms } published
2. NotificationDispatcher event loop receives (kind != "held_release")
3. handle_alarm_fired:
   - parse_payload(payload_json) → title, body, priority, channel_mask
   - resolve_channels: channel_mask == 0 → use default_channels (e.g. ["os_native", "tray"])
4. QuietHoursPolicy.is_in_quiet_hours(now) == true; priority Normal; override_for_urgent false
5. HeldReleaseService.hold(fire_id, &channels, &payload, release_at):
   - INSERT held_notifications (released = 0)
   - fire_store.schedule(FireSpec { kind: "held_release",
                                    payload: json!({"held_id": id}), ... })
   → scheduled_fires row inserted
6. Time passes. AlarmFired { kind: "held_release", payload_json: {"held_id":"..."} }
7. Dispatcher routes to handle_held_release:
   - Extract held_id
   - held_repo.list_pending_before(i64::MAX) → find row
   - Deserialize channels + reconstruct NotificationPayload
8. For each channel: dispatch_one(channel_name, &payload):
   - log_repo.try_insert (idempotency)
   - ch.deliver(payload) with retry
9. On success: held_release.mark_released(held_id) sets released=1
10. Publish DomainEvent::HeldNotificationReleased
```

### `mcp-bridge` event flow

```
Desktop process:
   AppEventEmitter emits "entity:updated" with payload {...}
      ↓
   SocketBridgeEmitter::emit:
      - Serialize as BridgeFrame { event: "entity:updated", payload: {...} }
      - Write 4-byte LE length prefix + JSON
      - Send over Unix socket at ${KLYNTBOT_HOME}/mcp-events.sock

Child klyntbot mcp serve --stdio process:
   BridgeClient receives bytes
      ↓
   Read 4 bytes → length
   Read length bytes → JSON
   Deserialize BridgeFrame
      ↓
   Dispatch to internal handlers (e.g. invalidate cached entity)
```

---

## Internals

### Why `mcp-bridge` exists separately from MCP

The MCP wire protocol is JSON-RPC over stdio (or HTTP). It's request-response. The child MCP process needs **server-pushed events** from the parent desktop (UI state changes, provider degradation, entity updates) without polling. MCP itself doesn't have a clean push channel from server → client outside `notifications/*`. The bespoke Unix socket sidecar is the pragmatic answer: keep MCP for tool calls, use bridge for live events.

### Per-channel `mpsc::channel(32)` isolation

`ChannelManager::start_all` creates a separate `mpsc::channel(32)` per channel. The dispatcher routes outbound messages to the matching channel's queue. If one channel hangs (e.g., Slack WS reconnecting), it can't block delivery to other channels — its queue fills to 32 and the dispatcher logs an error.

### Discord's HELLO-driven heartbeat

`WsConfig::HeartbeatStrategy::None` is set; Discord spawns a separate `tokio::spawn` heartbeat task from the `HELLO (op 10)` handler driven by server-provided `heartbeat_interval`. This avoids a fixed-interval heartbeat from the WS manager.

### Slack's fresh URL per connection

`apps.connections.open` is called each time the WS connection opens, returning a fresh temporary URL. The URL is short-lived. The `WebSocketManager` `reconnect_loop` invokes a fresh URL-fetch callback before each reconnect.

### Email's consent gate

`EmailChannel::validate_config()` errors if `consent_granted != true`. Unique to email — the other channels assume that adding API tokens to config implies consent.

### `NotificationDispatcher` does NOT filter on `kind == "alarm"`

It passes everything that isn't `held_release` to `handle_alarm_fired`. So `kind="cron_job"` would also flow through if it reached the dispatcher. In practice `CronExecutor` consumes those first via its own subscription. **But if `CronExecutor` ever fails to subscribe**, the dispatcher will start firing notifications for raw cron jobs. The two-`kind` naming (`cron_job` vs `cron`) we flagged in [`06-scheduling.md`](./06-scheduling.md) becomes more concerning in this light.

### MCP server runs as a child process of Claude Code, NOT desktop

When Claude Code connects to Klynt's MCP server, it spawns `klyntbot mcp serve --stdio` as a child. This child shares `~/.klyntbot/data.db` with the desktop process via SQLite WAL. Live events (entity updates, provider state) need to reach the child somehow — that's what `mcp-bridge` provides.

### `activity-log` inference is separate from raw events

`unified_activity_log` stores every event. `work_contexts`, `work_resources`, `resource_edges` are computed by an **inference loop** (assigns contexts from activity clustering). The inference state (e.g., `last_run_at`) is keyed in `inference_state` table.

So raw events ≠ work contexts. A user looking for "what was I doing yesterday" reads `work_contexts`; a user looking for "every tool call I made" reads `unified_activity_log`.

---

## Dependencies & extension points

### Upstream deps

- `tokio` + `tokio-tungstenite` (WebSocket for Discord/Slack)
- `reqwest` (HTTP for Telegram, Email API fallback)
- `async-imap` + `lettre` (Email IMAP+SMTP)
- `rmcp` (MCP protocol — `client-side-sse`, `server-side-http` features)
- `process_wrap` (auto-kill on drop for Stdio MCP children)
- `notify-rust` (OS notifications)
- `dashmap` (per-server circuit breaker state)
- `bus` (`DomainEventBus`, `MessageBus`)
- `storage` (`NotificationLogRepo`, `HeldRepo`, `ScheduledFiresRepo`)
- `tools-core` (MCP server uses `ToolRegistry`)
- `approval` (TelegramApprovalChannel for sensitive ops)

### Adding a new chat channel

1. Create `crates/channels/src/adapters/<my_platform>.rs`.
2. Implement `Channel` trait.
3. Pick transport: WS? HTTP poll? IMAP? Add reconnect strategy if needed.
4. Add config schema field under `Config::channels::<my_platform>`.
5. Register in `ChannelManager::initialize_channels`.
6. If interactive: override `supports_interaction` + `send_interaction`.
7. **Decide if you also want notification fan-out.** That's a separate `NotificationChannel` impl in `notifications` crate — and currently nothing past `os_native`/`tray` is wired.

### Exposing a new tool via MCP

1. Add the tool's registry name to `EXPLICIT_TOOL_ALLOWLIST` in `crates/config/src/schema/mcp.rs` — or rely on `AiFeatureRegistry::tool_names()` auto-inclusion if you wired it via `FeaturePackage`.
2. Verify: `cargo nextest run -p klyntbot-server` (advertises the right tool set).
3. Rebuild desktop binary (`cargo build -p desktop`) — MCP server ships inside it.
4. User can override the whitelist in `config.json` → `mcp.server.exposedTools`.

### Adding a new `BridgeFrame` event type

1. Producer (desktop side): `SocketBridgeEmitter::emit("my:event", payload)`.
2. Consumer (child MCP side): handle in `BridgeClient::next_frame` dispatch.
3. **Keep payload size < 1 MB.** Oversized frames return `Err`.

### Adding an `activity-log` normalizer

1. Implement `ActivityNormalizer` trait.
2. Register in `app-core::init::activity_log`.
3. Return `Some(ActivityLogEntry)` for events worth recording; `None` to skip (e.g., idle window events).
4. SHA-256 hash content for dedup; truncate previews to 500 chars; use ULIDs for time-sortable IDs.

---

## Open questions & debt

- **Notification fan-out to Telegram/Discord/Email is unwired.** Types defined, dispatcher routes only to `os_native` + `tray`. README's "Multi-channel" implies chat *and* notifications work everywhere — only chat does.
- **`McpApprovalChannel` always declines.** Documented in [`10-sandboxing-security.md`](./10-sandboxing-security.md); also a debt item here because MCP users hitting `Sensitive` tools see only a "Open Klynt on desktop" error.
- **Email is the only channel with `consent_granted` gating.** Either remove (if API token implies consent) or extend to others for consistency.
- **The dispatcher passes any non-`held_release` `kind` through.** Combined with the `cron_job` vs `cron` naming overlap from [`06-scheduling.md`](./06-scheduling.md), there's a latent bug if `CronExecutor` ever fails to subscribe.
- **`mcp-bridge`'s 1 MB frame cap** is not documented in any user-facing material. Frames larger than that are dropped silently from the consumer's perspective.
- **Each channel uses a different transport** with different reconnect/heartbeat strategies. Worth normalizing OR documenting the trade-off explicitly (today each adapter just does whatever the platform requires).
- **Activity-log inference loop** runs on a schedule but its cadence isn't user-facing. Document or expose.
- **MCP tool name namespacing** truncates at 64 chars with hash suffix. If two long-named server+tool combos collide on the hash, we'd have a silent registration conflict. Worth a test.
- **No mechanism to discover what's actually exposed via MCP** at runtime beyond `klyntbot mcp tools --list`. A `mcp.diagnostics` Tauri command would help debugging.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #2 (stubs — McpApprovalChannel, notification channels), #5 (doc drift — README "Multi-channel"), #8 (naming — two AlarmFired kinds) for specifics.

---

## Cross-references

- [`01-foundations.md`](./01-foundations.md) — `MessageBus`, `DomainEventBus`
- [`02-storage.md`](./02-storage.md) — `notification_log`, `held_notifications`, `unified_activity_log` + activity tables
- [`06-scheduling.md`](./06-scheduling.md) — `AlarmFired` and `held_release` come from here
- [`07-tools-framework.md`](./07-tools-framework.md) — MCP exposes `ToolRegistry`
- [`09-coding-mode.md`](./09-coding-mode.md) — 8 coding-memory MCP tools in `EXPLICIT_TOOL_ALLOWLIST`
- [`10-sandboxing-security.md`](./10-sandboxing-security.md) — `TelegramApprovalChannel`, `McpApprovalChannel`
- [`13-desktop-frontend.md`](./13-desktop-frontend.md) — `mcp-bridge` flows from desktop to MCP child
