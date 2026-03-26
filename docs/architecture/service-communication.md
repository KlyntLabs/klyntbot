# Service Communication

## Internal Communication Patterns

### Dependency Inversion (Cross-Layer)

Lower layers define trait interfaces; upper layers implement them. Injected as `Arc<dyn Trait>` at application startup.

```
+-------------------+                    +-------------------+
| L3: cognitive     |                    | L5: agent         |
|                   |                    |                   |
| trait Extraction  |  <--implements--   | LlmExtraction     |
|   Handler         |                    |   Handler         |
|                   |                    |                   |
| trait Consolid-   |  <--implements--   | LlmConsolidation  |
|   ationHandler    |                    |   Handler         |
|                   |                    |                   |
| trait Narrative   |  <--implements--   | LlmNarrative      |
|   Handler         |                    |   Handler         |
+-------------------+                    +-------------------+

+-------------------+                    +-------------------+
| L3: scheduling    |                    | L5: agent         |
|                   |                    |                   |
| trait SpawnHandler|  <--implements--   | AgentSpawn        |
|                   |                    |   Handler         |
| trait CronHandler |  <--implements--   | AgentCron         |
|                   |                    |   Handler         |
+-------------------+                    +-------------------+

+-------------------+                    +-------------------+
| L4: tools         |                    | L5: agent         |
|                   |                    |                   |
| trait Delegation  |  <--implements--   | AgentDelegation   |
|   Handler         |                    |   Handler         |
+-------------------+                    +-------------------+
```

### Storage Access

`StoragePool` wraps `sqlx::SqlitePool` — `Clone+Send+Sync` via internal `Arc`. No `Arc<RwLock>` wrapper needed.

```
AppCore
  |
  +-- pool: StoragePool (Clone)
  |     |
  |     +-- repos: Repos::from_pool(&pool)
  |           |
  |           +-- tasks: TaskRepo
  |           +-- sessions: SessionRepo
  |           +-- finance: FinanceStorage
  |           +-- usage: UsageRepo
  |           +-- cron: CronRepo
  |           +-- ... (23+ typed repos)
  |
  +-- vector_store: Arc<VectorStore>
        |
        +-- connection: Arc<lancedb::Connection>
```

### Tool Registry (Shared Read/Write)

```
Arc<RwLock<ToolRegistry>>
     |
     +--[Read lock]----> AgentRuntime: get_definitions(), prepare()
     |                   (held briefly, released before execute())
     |
     +--[Write lock]---> MCP reconnect: unregister_by_prefix() + re-register
     |                   Plugin load: register new tools
     |
     +--[No lock]------> tool.execute() (runs after prepare() releases lock)
```

The `prepare()` → `execute()` separation prevents deadlocks when tools (like `delegation`) need registry access during execution.

### Event Buses

```
DomainEventBus (broadcast, fan-out)
  +-- Capacity: configurable at construction
  +-- publish(): fire-and-forget (warns on no receivers)
  +-- subscribe(): returns independent Receiver per consumer
  +-- Lag handling: RecvError::Lagged(n) logged as warning, events dropped

MessageBus (mpsc, single-consumer)
  +-- inbound_tx/rx: channels → AgentLoop (taken once)
  +-- outbound_tx/rx: AgentLoop → ChannelManager (taken once)

ContextUpdateQueue (Mutex<Vec>, drain-based)
  +-- push(): any producer (cognitive service, focus manager)
  +-- drain(): LiveContextRefresher at each iteration boundary
  +-- Dedup: 30s window on (reason, content) pairs

LearningEventBus (broadcast, fan-out)
  +-- Carries LearningEvent::AnalysisCompleted
  +-- Producer: learning background analysis loop (feature-learning)
  +-- Consumer: adaptive confidence threshold adjuster
```

## External Service Communication

### LLM Provider APIs

```
AgentRuntime
     |
     v
ExecutionCore
     |
     +--streaming--> provider.chat_stream(messages, tools, params)
     |               Returns LlmStream (async iterator of chunks)
     |               Chunks: TextDelta, ToolCallDelta, Usage
     |
     +--blocking---> provider.chat(messages, tools, params)
                     Returns LlmResponse { content, tool_calls, usage }
```

| Provider | Endpoint | Auth | Notes |
|----------|----------|------|-------|
| Anthropic | `https://api.anthropic.com/v1/messages` | `x-api-key` header | Native adapter, prompt caching, extended thinking |
| OpenAI | `https://api.openai.com/v1/chat/completions` | `Authorization: Bearer` | OpenAI-compat adapter |
| OpenRouter | `https://openrouter.ai/api/v1/chat/completions` | `Authorization: Bearer` | OpenAI-compat adapter |
| DeepSeek | `https://api.deepseek.com/v1/chat/completions` | `Authorization: Bearer` | OpenAI-compat, reasoning content |
| Groq | `https://api.groq.com/openai/v1/chat/completions` | `Authorization: Bearer` | Also: `/audio/transcriptions` (Whisper) |
| Gemini | Via compatibility endpoint | `Authorization: Bearer` | OpenAI-compat adapter |
| vLLM | Self-hosted URL | `Authorization: Bearer` | OpenAI-compat adapter |

**All use `reqwest`** with features: `json`, `rustls`, `multipart`, `stream`, `charset`, `http2`.

### Channel Platform APIs

**Telegram:**
```
TelegramChannel
     |
     +--[Inbound]---> GET https://api.telegram.org/bot{token}/getUpdates
     |                (30s timeout, offset tracking, 3 retries)
     |
     +--[Outbound]--> POST https://api.telegram.org/bot{token}/sendMessage
     |                POST .../sendPhoto, sendDocument, etc.
     |
     +--[Typing]----> POST .../sendChatAction (every 4s)
     |
     +--[Voice]-----> GET .../getFile → download → Groq Whisper transcription
```

**Discord:**
```
DiscordChannel
     |
     +--[Gateway]---> wss://gateway.discord.gg (tokio-tungstenite)
     |                op 10: HELLO → heartbeat loop + IDENTIFY
     |                op 0: DISPATCH → MESSAGE_CREATE, MESSAGE_REACTION_ADD
     |
     +--[REST]------> POST https://discord.com/api/v10/channels/{id}/messages
     |                (for sending responses)
```

**Slack:**
```
SlackChannel
     |
     +--[Socket]----> WSS via apps.connections.open
     |                Timeout-based heartbeat (30s)
     |                Envelope ACK for each message
     |
     +--[REST]------> POST https://slack.com/api/chat.postMessage
```

**Email:**
```
EmailChannel
     |
     +--[Inbound]---> IMAP over TLS (async-imap + tokio-native-tls)
     |                SELECT INBOX → SEARCH UNSEEN → FETCH BODY[]
     |                html2text for HTML stripping
     |
     +--[Outbound]--> SMTP over TLS (lettre)
     |                Reply threading via In-Reply-To header
```

### MCP Protocol (External Clients)

```
External AI Client (Claude Code, Cursor)
     |
     v
stdin/stdout or HTTP
     |
     v
rmcp (JSON-RPC 2.0)
     |
     +--initialize---------> { capabilities: { tools, resources } }
     |
     +--tools/list----------> [get_status, agent, tasks, notes, ...]
     |
     +--tools/call----------> ToolRegistryBridge or AgentBridge
     |   Arguments: { name: "tasks", arguments: { action: "list" } }
     |   Result: { content: [{ type: "text", text: "..." }] }
     |
     +--resources/list------> 4 static URIs
     +--resources/read------> URI content retrieval
```

### MCP Protocol (External Servers)

```
McpManager::connect_all()
     |
     +--[stdio]-----> TokioChildProcess (spawn subprocess)
     |                Process groups for clean kill-on-drop
     |
     +--[HTTP]------> StreamableHttpClientTransport
     |
     v
Per-server: tools/list → wrap as McpTool → register in ToolRegistry
     |
     +-- Tool names: mcp_{server}_{tool} (via mcp::sanitize)
     +-- Allowlist/denylist per server
     +-- Circuit breaker: threshold=3, cooldown=60s
     +-- Timeouts: startup=10s, tool=120s (configurable per server)
     +-- OAuth: stored in config, injected as env var to subprocess
```

## Tauri IPC Bridge

### Frontend → Backend

```
React Component
     |
     +--useQuery(cmd, args)---> ipc(cmd, args)
     |  (SWR, 30s stale)            |
     |                        +------+------+
     +--useMutation(cmd)----> | isTauri?    |
        (write + invalidate)  +------+------+
                                |           |
                           [Yes]        [No (browser)]
                                |           |
                                v           v
                         invoke(cmd, args)  fetch("/api/{cmd}", POST)
                         Tauri native IPC   dev server port 3456
```

**Cache:** Module-level `Map<string, CacheEntry>` (survives re-renders). 30s default stale time. In-flight dedup.

**Entity invalidation:** After mutation success, `window.CustomEvent("entity:updated")` in browser mode (mirrors Tauri's `entity:updated` event) triggers cache invalidation across all components.

### Backend → Frontend (Events)

**Tauri mode:**
```
AppCore handler
     |
     v
TauriEmitter::emit_event(name, payload)
     |
     v
tauri::Emitter::emit(name, payload)  -- broadcast to all webviews
     -- or --
WebviewWindow::emit(name, payload)   -- targeted to specific window
```

**Dev mode (browser):**
```
AppCore handler
     |
     v
SseEmitter::emit_event(name, payload)
     |
     v
broadcast::Sender<(String, Value)>
     |
     v
SSE endpoint: GET /api/events/{sessionKey}
     |
     v
text/event-stream response
  data: {"event": "agent:content_chunk", "payload": {...}}
```

### Event Categories

| Category | Events | Purpose |
|----------|--------|---------|
| Agent streaming | `agent:content_chunk`, `agent:done`, `agent:tool_start`, `agent:tool_end`, `agent:error`, `agent:plan_generated` | Real-time chat UI updates |
| Entity updates | `entity:updated` with `{entity_kind, id}` | Cache invalidation |
| Productivity | `focus:tick`, `focus:completed`, `activity:switch`, `score:updated` | Dashboard updates |
| Distraction | `distraction:intervention`, `distraction:detected`, `distraction:verdict` | Overlay trigger |
| MCP | `mcp:oauth_complete`, `mcp:server_status`, `mcp:startup_complete` | Integration status |
| Cognitive | `cognitive:domain_event`, `cognitive:extraction`, `cognitive:consolidation` | Memory pipeline visibility |
| Coaching | `coaching:intervention` | Behavior coaching |

## Activity Ingestion API

External tools can push activity events via HTTP:

```
POST /api/v1/ingest
Authorization: Bearer {capture.ingestion_token}
Content-Type: application/json

{ "source": "shell-hook", "event_type": "command", "data": {...} }
```

```
POST /api/v1/ingest/batch
Authorization: Bearer {capture.ingestion_token}
Content-Type: application/json

{ "events": [...] }
```

Used by: shell hook integration, IDE extensions, browser extension.

## CalDAV Calendar Sync

Standard CalDAV protocol for calendar integration:

```
CalDAV Server
     |
     +--PROPFIND---> Discover calendars
     +--REPORT-----> Fetch events (ctag-based change detection)
     +--GET--------> Fetch individual events
     |
     v
calendar_sync_state (SQLite) -- sync tokens
calendar_event_cache (SQLite) -- cached events
     |
     v
Tray countdown timer -- shows next event countdown in menu bar
```

## Network Security Notes

- **CSP (production):** `connect-src ipc: http://ipc.localhost` — only Tauri IPC in production webview
- **Dev server:** Only accessible from `localhost:1420` (CORS restricted)
- **MCP HTTP auth:** Optional Bearer token (if not configured, server is open on configured port)
- **API keys:** Stored as `Secret<String>` in config, accessed via `.expose()`
- **Email consent:** Requires explicit `consent_granted = true` before any IMAP/SMTP connection
- **Telegram allowlist:** Per-user allowlist in config
