# Subsystem Analysis: Channels, CLI & Dashboard

**Crates**: `channels` (Layer 4), `cli` (Layer 6), `dashboard` (Layer 4+)
**Total source files**: 9 (channels) + 43 (cli) + 20 (dashboard) = 72 files
**Total lines of code**: ~2,666 (channels) + ~4,700 (cli) + ~3,500 (dashboard) = ~10,900 LOC

---

## 1. Channels Crate (`crates/channels/`)

### 1.1 Channel Trait — Interface & Lifecycle

**File**: `lib.rs:34-51`

```rust
#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn start(&self, bus: Arc<MessageBus>) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn send(&self, msg: &OutboundMessage) -> Result<()>;
    fn is_allowed(&self, sender_id: &str) -> bool;
}
```

**Lifecycle**: `new()` → `start(bus)` → (long-running event loop) → `stop()`

- `start()` is a blocking long-runner; the `ChannelManager` spawns each in its own `tokio::spawn`
- `stop()` sets `AtomicBool::running` to false; each channel's event loop checks this flag
- `send()` is called by the outbound dispatcher to route agent responses back to users
- `is_allowed()` delegates to the shared `check_allowlist()` helper

**Type alias**: `DynChannel = Arc<dyn Channel>` — all channels are trait-object erased

### 1.2 Shared Infrastructure

#### 1.2.1 Allowlist Checking (`lib.rs:57-78`)

`check_allowlist(allow_from, sender_id)`:
- Empty allowlist → allow all (permissive default)
- Exact match check
- Compound ID splitting on `|` (e.g., Telegram's `"123456|username"` format)

#### 1.2.2 Reconnect Loop (`lib.rs:82-97`)

```rust
pub async fn reconnect_loop<F, Fut>(name: &str, running: &Arc<AtomicBool>, mut connect: F)
```

Generic retry pattern with 5-second delay between reconnections. Used by Discord, Slack, WhatsApp, and QQ — all WebSocket-based channels. Telegram and Email use their own polling loops instead.

### 1.3 WebSocket Manager (`ws_manager.rs`)

**Purpose**: Consolidates the `connect → heartbeat → read loop → shutdown` pattern shared across Discord, Slack, WhatsApp, and QQ channels.

#### 1.3.1 WsHandler Trait (`ws_manager.rs:71-87`)

```rust
#[async_trait]
pub trait WsHandler: Send + Sync {
    async fn on_connected(&self, write: &Arc<Mutex<WsSink>>) -> Result<Option<HeartbeatStrategy>>;
    async fn on_text_message(&self, text: &str, write: &Arc<Mutex<WsSink>>) -> Result<bool>;
    async fn on_disconnected(&self) {}
}
```

- `on_connected()` — post-connect handshake; can override the heartbeat strategy
- `on_text_message()` — returns `Ok(true)` to continue, `Ok(false)` to disconnect
- `on_disconnected()` — cleanup callback (default no-op)

#### 1.3.2 HeartbeatStrategy (`ws_manager.rs:28-39`)

Two modes:
1. **Timeout** — sends a keepalive (configurable payload) when no message received within `timeout`. Used by WhatsApp (Ping), QQ (JSON heartbeat), Slack (Ping).
2. **None** — handler manages heartbeat externally. Used by Discord (spawns its own heartbeat task from HELLO).

#### 1.3.3 WebSocketManager::run() Flow (`ws_manager.rs:102-125`)

1. Connect with configurable timeout (`connect_async` + `tokio::time::timeout`)
2. Split into `WsSink` + `WsStream`
3. Call `handler.on_connected()` for handshake; handler may override heartbeat strategy
4. Enter read loop with active heartbeat strategy
5. On exit, call `handler.on_disconnected()`
6. Return result to caller (typically wrapped in `reconnect_loop`)

**Test coverage**: 7 unit tests including timeout, invalid URL, running-flag, and mock handler tests.

### 1.4 Channel Implementations

#### 1.4.1 Telegram (`telegram.rs` — 875 lines, largest channel)

**Architecture**: Direct HTTP Bot API (long polling) — no teloxide dependency.

**Key features**:
- **Long polling** via `getUpdates` with 30s timeout
- **Retry logic** with exponential backoff (1s, 2s, 4s) for API calls
- **Proxy support** via configurable `proxy` URL in `TelegramConfig`
- **Voice transcription** via Groq API (`TranscriptionProvider`) — optional, requires Groq API key
- **Media handling**: Downloads photos, voice, audio, documents to `~/.klyntbot/media/`
- **Markdown → HTML conversion**: Full converter with 10 regex passes (code blocks, inline code, headers, blockquotes, links, bold, italic, strikethrough, bullets)
- **Message splitting**: Respects Telegram's 4096 char limit, splits by lines
- **Typing indicators**: Continuous `sendChatAction` every 4 seconds; managed per-chat with `typing_tasks: HashMap<String, JoinHandle<()>>`
- **Bot commands**: `/start`, `/help`, `/reset` (clears session via bus)
- **HTML fallback**: If HTML parsing fails on send, retries as plain text

**Test coverage**: 9 tests (markdown conversion, message splitting)

#### 1.4.2 Discord (`discord.rs` — 554 lines)

**Architecture**: Raw Gateway WebSocket (no serenity) + REST API for sending.

**WebSocket Protocol** (implements `WsHandler`):
- **op 10 (HELLO)**: Extracts heartbeat interval, spawns heartbeat task, sends IDENTIFY
- **op 0 (DISPATCH)**: Routes `READY`, `MESSAGE_CREATE` events
- **op 7 (RECONNECT)**: Returns `Ok(false)` to trigger reconnection
- **op 9 (INVALID SESSION)**: Returns `Ok(false)` to trigger reconnection
- **op 11 (HEARTBEAT ACK)**: Debug log

**Heartbeat**: Self-managed (spawns dedicated `tokio::spawn` from HELLO); overrides WebSocketManager's heartbeat to `None`.

**Message handling**:
- Ignores bot messages (`author.bot == true`)
- Downloads attachments (20MB limit) to `~/.klyntbot/media/`
- Captures metadata: `message_id`, `guild_id`, `reply_to`
- Typing indicators via REST `POST /channels/{id}/typing` every 8 seconds

**Sending**: REST API `POST /channels/{id}/messages` with rate-limit handling (429 → retry after `retry_after` seconds), reply threading support.

#### 1.4.3 Slack (`slack.rs` — 439 lines)

**Architecture**: Socket Mode WebSocket + REST API for sending.

**Connection flow**:
1. `auth.test` → get bot user ID
2. `apps.connections.open` → get ephemeral WebSocket URL
3. Connect via `WebSocketManager` (fresh URL per reconnection)

**Socket Mode protocol**:
- Receives `SocketEnvelope` with `envelope_id`, `type`, `payload`
- Immediately ACKs every envelope (JSON `{"envelope_id": ...}`)
- Handles `events_api` envelopes → `message` and `app_mention` events

**Message filtering**:
- Ignores subtypes (bot/system messages)
- Ignores self-messages
- In channels/groups: only responds to `@mentions` or DMs (`channel_type == "im"`)
- Strips bot mention from message text
- Adds `:eyes:` reaction to received messages (best-effort)
- Preserves `thread_ts` for threaded replies; only threads in non-DM channels

**Heartbeat**: Uses WebSocketManager's Timeout strategy (35s, Ping payload).

#### 1.4.4 WhatsApp (`whatsapp.rs` — 189 lines)

**Architecture**: WebSocket bridge to external Node.js Baileys server.

**Message types**:
- `qr` — QR code for authentication (logged to terminal)
- `message` — incoming user message
- `status` — connection status updates
- `error` — error messages

**Sending**: JSON via WebSocket (`{"type": "send", "to": chat_id, "text": content}`).

**Heartbeat**: Uses WebSocketManager's Timeout strategy (30s, Ping payload).

**Notable**: Simplest WsHandler implementation — stores `WsSink` reference for `send()`, clears on disconnect.

#### 1.4.5 QQ (`qq.rs` — 339 lines)

**Architecture**: Direct WebSocket to QQ Bot API (`wss://api.sgroup.qq.com/websocket`) + REST API.

**Authentication**: `POST /app/getAppAccessToken` with `appId` + `clientSecret`.

**Gateway protocol** (similar to Discord):
- op 10 (HELLO) with `heartbeat_interval`
- op 0 (DISPATCH): `READY`, `C2C_MESSAGE_CREATE`, `DIRECT_MESSAGE_CREATE`
- op 7 (RECONNECT), op 9 (INVALID SESSION)

**Deduplication**: Maintains `VecDeque<String>` of 1000 most recent message IDs to prevent duplicate processing.

**Sending**: REST `POST /v2/users/{openid}/messages` with `QQBot {token}` authorization.

**Heartbeat**: Uses WebSocketManager's Timeout strategy (35s, JSON heartbeat `{"op": 1, "d": null}`).

#### 1.4.6 Email (`email.rs` — 482 lines, feature-gated)

**Architecture**: IMAP polling (inbound) + SMTP (outbound). Feature-gated behind `email` feature flag.

**Inbound (IMAP)**:
- Polls every `poll_interval_seconds` (minimum 5s)
- Supports both TLS and plain TCP connections
- Searches for `UNSEEN` messages
- Parses with `mail_parser::MessageParser`
- Extracts text/plain or converts text/html via `html2text`
- Body truncation at `max_body_chars`
- Tracks `processed_uids` (HashSet, capped at 10K) for deduplication
- Marks messages as seen if `mark_seen` config is enabled
- Stores last subject and message-ID per sender for threading

**Outbound (SMTP)**:
- Uses `lettre` for SMTP transport
- Threading via `In-Reply-To` and `References` headers
- Configurable `from_address`, `subject_prefix`
- Respects `auto_reply_enabled` flag
- SMTP sending runs via `spawn_blocking` (sync lettre API)

**Privacy**: Requires explicit `consent_granted: true` in config before activation. Validates all required fields (IMAP host/username/password, SMTP host/username/password).

### 1.5 ChannelManager (`manager.rs`)

**Role**: Orchestrates initialization, startup, and shutdown of all enabled channels.

**Key mechanics**:
- `init_channel!` macro reduces boilerplate for the enabled-check → create → insert pattern
- `outbound_rx` is taken from `MessageBus` exactly once (panics if taken twice)
- Each channel runs in its own `tokio::spawn`
- Outbound dispatcher: separate `tokio::spawn` that reads from `outbound_rx`, looks up channel by name, and calls `channel.send(msg)`
- `start_all()` blocks until all channel tasks complete (they run forever until stop)
- `stop_all()` iterates channels and calls `channel.stop()`

**Telegram-specific wiring**: Passes Groq API key for voice transcription if available.

---

## 2. CLI Crate (`crates/cli/`)

### 2.1 Command Definitions (`commands.rs`)

Four subcommands via clap:

| Command | Args | Default Port | Description |
|---------|------|-------------|-------------|
| `chat` | `message?`, `--session` | — | Interactive REPL or single-shot |
| `serve` | `--port`, `--verbose` | 18790 | Gateway daemon |
| `init` | — | — | Setup wizard |
| `status` | `--verbose` | — | Configuration display |

`command: Option<Commands>` — no subcommand = brief status display.

### 2.2 Chat Command (`chat.rs` — 506 lines)

#### 2.2.1 Initialization Flow

1. Load config with env var overrides (`config::load_with_env_overrides()`)
2. Print startup banner with model name
3. Create LLM provider via `providers::create_provider()`
4. Create minimal `MessageBus` (capacity 10, not used for routing in CLI)
5. Connect to PostgreSQL (`StoragePool::connect()`)
6. Initialize `AgentLoop` with repos, embeddings

#### 2.2.2 Single Message Mode

`klyntbot chat "Hello"` — sends message, streams response, exits.

#### 2.2.3 Interactive REPL Mode

Built on `rustyline` with:
- History persistence at `~/.klyntbot/history.txt`
- `SlashCommandHelper` for tab completion and hints
- Colored prompt (`> ` with orange background)

**Slash commands**: `/help`, `/paste`, `/history`, `/status`, `/session`, `/clear`, `/exit`, `/quit`

**Exit triggers**: `/exit`, `/quit`, `exit`, `quit`, `:q`, Ctrl+C, Ctrl+D

**Paste mode** (`/paste`):
- Multi-line input collection
- Terminators: `/end` (explicit), Ctrl+D (submit), Ctrl+C (cancel)
- Empty line when buffer is empty cancels
- Concatenated lines sent as single message

#### 2.2.4 Streaming Architecture (`run_with_streaming`)

**Core loop**: `tokio::select!` between two channels:
1. **Agent events** (`event_rx`): `ContentChunk`, `ToolStart`, `ToolEnd`, `IterationStart`, `Done`, `Error`
2. **Interactive questions** (`interaction_rx`): `InteractionBundle { request, response_tx }`

**Rendering**: Uses `StreamRenderer` for:
- Thinking spinner (TTY only)
- Content chunk accumulation
- Tool start/end visualization
- Cancellation tracking
- Elapsed time display with model info

**Ctrl+C handling**: Spawns separate task that calls `cancel_token.cancel()` on signal.

**ask_user integration**: When the agent calls the `ask_user` tool:
1. Spinner stops
2. `StreamRenderer` pauses
3. `ask_user_prompt::prompt_multi_question()` renders tabbed UI
4. User response sent back via `response_tx`
5. `StreamRenderer` resumes with line-count offset

### 2.3 Serve Command (`serve.rs` — 474 lines)

#### 2.3.1 Service Orchestration

The gateway daemon starts and coordinates:

| Service | Purpose |
|---------|---------|
| `AgentLoop` | Core AI loop processing inbound messages |
| `ChannelManager` | All 6 chat platform integrations |
| `CronService` | Scheduled job execution |
| `HeartbeatService` | Periodic agent self-reflection |
| `NotificationDispatcher` | Outbound alert routing |

#### 2.3.2 Shared State

- `TodoStore` — `Arc<RwLock<TodoStore>>` (legacy, used by ContextBuilder, CalendarSyncAdapter)
- `GoalStore` — `Arc<RwLock<GoalStore>>` (SQL-backed via `from_repo`)
- `PlanStore` — `Arc<RwLock<PlanStore>>` (SQL-backed via `from_repo`)
- `CronService` — `Arc<CronService>` (SQL-backed via `from_repo`)
- `NotificationDispatcher` — sends alerts via `bus.outbound_sender()`

#### 2.3.3 Cron Job Registration

Built-in cron jobs:

| Name | Schedule | Purpose |
|------|----------|---------|
| `todo_focus_check` | Every 30 min | Check focus task deadlines (1h/3h/6h reminders) |
| `todo_daily_digest` | `0 9 * * *` | Daily task summary notification |
| `todo_overdue_check` | Every 60 min | Auto-unfocus expired tasks |
| `__klyntbot_weekly_report` | `0 18 * * 0` (Sunday 6pm) | Weekly progress report |
| `__klyntbot_calendar_sync` | Configurable interval | Calendar sync (if any provider enabled) |
| `__klyntbot_daily_planning` | Configurable HH:MM | Daily planning notification (if enabled) |

#### 2.3.4 Shutdown Sequence

1. `Ctrl+C` signal received
2. Set `agent_shutdown` `AtomicBool` to false (no Mutex needed)
3. Stop `CronService` and `HeartbeatService`
4. Wait up to 5 seconds for spawned tasks
5. Abort remaining tasks on timeout

### 2.4 Status Command (`status.rs`)

Two modes:
- **Brief** (`handle_brief_status`): Version, status indicator, active provider/model, top commands
- **Verbose** (`handle_status`): Adds workspace path, config path, channel enable/disable table

Provider detection: checks API keys in priority order (Anthropic → OpenAI → OpenRouter → DeepSeek).

### 2.5 Interactive Features (`interactive.rs`)

**`SlashCommandHelper`** — implements rustyline's:
- `Completer`: Tab-completes slash commands (matches prefix)
- `Hinter`: Shows gray completion hint for partial slash commands
- `Highlighter`: Cyan for slash commands, gray for hints
- `Validator`: No-op (all input is valid)

8 registered commands with descriptions for autocomplete display.

**Test coverage**: 3 tests (creation, slash prefix validation, descriptions non-empty).

### 2.6 Wizard Framework

#### 2.6.1 Core Framework (`wizard/framework.rs`)

**WizardModule trait** (`framework.rs:88-117`):
```rust
pub trait WizardModule {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn is_required(&self) -> bool { true }
    fn is_applicable(&self, _state: &WizardState) -> bool { true }
    fn run(&self, state: &mut WizardState) -> Result<StepResult>;
}
```

**StepResult**: `Next`, `Back`, `Skip`, `Cancel` — controls navigation

**WizardState**: Holds `Config` being built, step counter, fresh-install flag

**WizardRunner**: Orchestrates modules with forward/back navigation. Filters inapplicable modules, tracks step index, saves config on completion.

**Test coverage**: 15 tests covering state, runner, modules, step results.

#### 2.6.2 Wizard Steps (11 total)

| Step | Module | Type | Required |
|------|--------|------|----------|
| 1 | LLM Provider | `ProviderModule` | Yes |
| 2 | Database | `steps::database` (async) | No |
| 3 | Chat Channels | `channels::configure_channels` (async) | No |
| 4 | Tool Permissions | `ToolsModule` | No |
| 5 | Workspace & Notifications | `WorkspaceModule` | Yes |
| 6 | Calendar Sync | `calendar::configure_calendars` (async) | No |
| 7 | Semantic Search | `search::configure_semantic_search` | No |
| 8 | Conversation Memory | `memory::configure_conversation_memory` | No |
| 9 | Learning System | `learning::configure_learning` | No |
| 10 | Background Service | `DaemonModule` (platform-conditional) | No |
| 11 | Review & Confirm | `steps::review::ReviewModule` | Yes |

**Auto-save**: Config saved after each completed step to preserve progress.

**Ctrl+C handling**: Caught at the dispatch level; prints cancellation message, no config changes saved for current step.

#### 2.6.3 Prompt Utilities (`wizard/prompts.rs` — 1524 lines)

Rich interactive prompt library:

| Prompt | Interactive | Fallback |
|--------|-------------|----------|
| `prompt_yes_no` | Single keypress (y/n/Enter) | Line input |
| `prompt_text` | Standard readline | Standard readline |
| `prompt_optional` | Standard readline | Standard readline |
| `prompt_secret` | Masked (`●`) with backspace | Plain text |
| `prompt_secret_with_existing` | Masked with preview | Plain text |
| `prompt_select` | Arrow keys + j/k + Enter | Numbered list |
| `prompt_multi_select` | Space toggle + `a` all + Enter | Comma-separated |
| `prompt_multi_select_with_defaults` | Pre-checked defaults | Standard multi |
| `prompt_select_with_input` | TAB to expand inline text field | "N text" format |
| `prompt_list` | Item-per-line until empty | Item-per-line |

**Raw mode management**: `RawModeGuard` RAII struct ensures raw mode is disabled on drop (even during panics).

**TTY detection**: All interactive prompts fall back to simple line-based input when `!is_interactive()`.

**Secret masking** (`mask_secret`): Recognizes key prefixes (`sk-ant-`, `sk-or-`, `gsk_`, `BSA-`, `sk-`) and shows `prefix****last4`.

**Test coverage**: 20 tests covering mask_secret, option creation, result structures.

#### 2.6.4 ask_user Prompt (`wizard/ask_user_prompt/`)

Tabbed multi-question UI for the `ask_user` tool during streaming. Three files:
- `mod.rs` — main `prompt_multi_question` function
- `box_drawing.rs` — TUI box drawing characters
- `fallback.rs` — non-TTY fallback implementation

#### 2.6.5 Workspace Templates (`wizard/templates.rs`)

6 markdown template files written to workspace during init:
- `AGENTS` — agent capabilities/constraints
- `SOUL` — personality/communication style
- `USER` — user profile/preferences
- `TOOLS` — tool configuration/restrictions
- `IDENTITY` — bot identity information
- `MEMORY` — long-term memory store

**Test coverage**: 12 tests (non-empty, heading structure, content validation, UTF-8, trailing newline).

---

## 3. Dashboard Crate (`crates/dashboard/`)

### 3.1 Architecture Overview

The dashboard is an embedded web application providing:
- **GraphQL API** — async-graphql with Query, Mutation, Subscription roots
- **WebSocket chat** — `/ws/chat` for real-time streaming
- **Event system** — broadcast-based `DashboardEventBus` for real-time updates
- **Config hot-reload** — file system watcher with debounced reload
- **Observable stores** — event-emitting wrappers around TodoStore/ProjectStore

### 3.2 HTTP Server (`server.rs`)

**Framework**: axum with tower-http CORS layer.

**Routes**:

| Method | Path | Handler |
|--------|------|---------|
| GET | `/health` | `{"status": "ok"}` |
| GET | `/ws/chat` | WebSocket upgrade |
| GET | `/` | Minimal HTML (placeholder for Svelte app) |
| GET | `/assets/*` | Static assets (returns 404) |

**CORS**: Allow all origins/methods/headers (localhost-only use).

**Testing**: `start_on_random_port()` for integration tests with OS-assigned port.

### 3.3 GraphQL Schema

#### 3.3.1 DashboardContext (`graphql/mod.rs:34-41`)

Shared context injected into every resolver:
```rust
pub struct DashboardContext {
    pub todo_store: Arc<RwLock<TodoStore>>,
    pub project_store: Arc<RwLock<ProjectStore>>,
    pub goal_store: Arc<RwLock<GoalStore>>,
    pub plan_store: Arc<RwLock<PlanStore>>,
    pub event_tx: broadcast::Sender<DashboardEvent>,
    pub config: Arc<RwLock<Config>>,
}
```

#### 3.3.2 QueryRoot (`graphql/query.rs`)

Merged from two domains via `#[derive(MergedObject)]`:

**TodoProjectQueries** (dev-1):
- `todos(filter, limit, offset)` → paginated todo list
- `todo(id)` → single todo by ID
- `todo_tree(project_id, depth)` → hierarchical tree with recursive `build_tree_node`
- `todo_summary()` → aggregate stats (total, by status, overdue, upcoming, focus active)
- `search_todos(query, mode)` → keyword search (semantic/hybrid are placeholder stubs)
- `projects(filter, limit)` → filtered project list
- `project(id)` → single project by ID

**GoalPlanSystemQueries** (dev-2):
- `goals(filter)` → filtered goal list
- `goal(id)` → single goal by UUID
- `plans(filter)` → filtered plan list (by status or session_key)
- `plan(id)` → single plan by UUID
- `config()` → current hot-reloadable config sections
- `system_status()` → live system snapshot (stub: hardcoded values)
- `cron_jobs()` → configured cron jobs (stub: empty vec)
- `sessions(limit)` → recent chat sessions (stub: empty vec)

#### 3.3.3 MutationRoot (`graphql/mutation.rs`)

**Todo mutations**: create, update, delete, complete, focus/unfocus, move, reorder, log_time, add/remove dependency

**Project mutations**: create, update, archive

**Goal mutations** (dev-2): create, update, update_metric

**Plan mutations** (dev-2): create, approve (Draft→Approved), execute (Approved→Executing), abandon (any→Abandoned)

**Chat mutation** (dev-2): `send_message` — returns user message immediately; agent reply via subscription/WS

**Config mutation** (dev-2): `update_config(section, values)` — partial JSON patch (supported sections: todo, agents, workspace)

**Calendar mutation** (dev-2): `sync_calendar` — stub, returns "not yet configured"

**Event emission**: All write mutations emit `DashboardEvent` via `event_tx.send()` for real-time subscriptions.

**Validation**: Title required + max 200 chars for todos; name required for projects; UUID parsing for goals/plans.

#### 3.3.4 SubscriptionRoot (`graphql/subscription.rs`)

7 real-time subscriptions using `async_stream::stream!`:

| Subscription | Event | Payload |
|-------------|-------|---------|
| `todo_changed` | `TodoChanged` | `GqlTodoChangeEvent` (action + full todo) |
| `project_changed` | `ProjectChanged` | `GqlProjectChangeEvent` (action + full project) |
| `goal_changed` | `GoalChanged` | `GqlGoalChangeEvent` (action + full goal) |
| `plan_changed` | `PlanChanged` | `GqlPlanChangeEvent` (action + full plan) |
| `notification` | `Notification` | `GqlNotification` (title + body) |
| `config_changed` | `ConfigReloaded` | `GqlConfig` (full config snapshot) |
| `system_status` | `AgentEvent` | `GqlSystemStatus` (hardcoded stub) |

**Pattern**: Clone context → subscribe to broadcast → filter for matching variant → yield typed struct.

#### 3.3.5 Input/Filter Types (`graphql/filters.rs`)

- `TodoFilterInput`: status, project_id, parent_id, tag, priority range, due date range
- `ProjectFilterInput`: status, tag
- `CreateTodoInput`, `UpdateTodoInput`, `CreateProjectInput`, `UpdateProjectInput`
- `TodoSortOrder` enum (CreatedAsc, CreatedDesc, PriorityAsc, DueDateAsc)
- `GoalFilter`, `PlanFilter`, `CreateGoalInput`, `UpdateGoalInput`, `CreatePlanInput` (in type modules)

### 3.4 WebSocket Chat (`ws/chat.rs`)

#### 3.4.1 Protocol

**Client → Server**:
```json
{"type": "message", "content": "Hello", "session_id": "web-123"}
```

**Server → Client** (streamed):
```json
{"type": "session", "session_id": "web-<uuid>"}
{"type": "chunk", "content": "Hello! "}
{"type": "tool_start", "name": "todo", "args": {}}
{"type": "tool_end", "name": "todo", "success": true, "duration_ms": 42}
{"type": "done", "content": "Full response here"}
{"type": "error", "message": "..."}
```

#### 3.4.2 Connection Lifecycle

1. WebSocket upgrade via axum's `WebSocketUpgrade`
2. On first message: assign/adopt session ID, subscribe to `DashboardEventBus`, send `Session` message
3. **`tokio::select!` loop** between:
   - Client messages → publish `InboundMessage` to `MessageBus` (channel = "dashboard")
   - Dashboard events → filter `AgentEvent` variants → convert to `ServerMessage` → send to client
4. Connection closes on: `Close` frame, recv error, send error, or bus closed

#### 3.4.3 Event Mapping

`agent_event_to_server_message()` converts:
- `ContentChunk` → `Chunk`
- `ToolStart` → `ToolStart`
- `ToolEnd` → `ToolEnd`
- `Done` → `Done`
- `Error` → `Error`
- All others (iteration, confidence, plan events) → `None` (not forwarded)

### 3.5 Event System (`events/`)

#### 3.5.1 DashboardEventBus (`events/mod.rs`)

`tokio::sync::broadcast`-based event distribution:
- Capacity 256 (handles burst traffic)
- `publish()` — fan-out to all subscribers, no-op if none
- `subscribe()` — independent receiver per subscriber
- `receiver_count()` — active subscriber count

**DashboardEvent variants**: AgentEvent, TodoChanged, ProjectChanged, GoalChanged, PlanChanged, ConfigReloaded, Notification, CronJobRan

**ChangeAction**: Created, Updated, Deleted

#### 3.5.2 Observable Store Wrappers (`events/store_events.rs`)

**ObservableTodoStore**: Wraps `Arc<RwLock<TodoStore>>` + `Arc<DashboardEventBus>`:
- Reads delegate directly (no event overhead)
- Writes emit `DashboardEvent::TodoChanged` with appropriate `ChangeAction`
- Methods: add, get, update, delete, list, summary, focus, unfocus, complete

**ObservableProjectStore**: Same pattern for projects — add, get, update, delete, list.

### 3.6 Config Watcher (`config_watcher.rs`)

**Purpose**: Hot-reload config.json on file system changes.

**Implementation**:
- Uses `notify::RecommendedWatcher` (OS-level FS events)
- Watches parent directory (catches atomic rename-based saves)
- Canonicalizes paths to handle macOS symlinks (`/var` → `/private/var`)
- Filters for modify/create events on the specific config file
- Debounce window (configurable, default 200ms)
- Validates JSON before emitting `ConfigReloaded` event
- Runs in background `tokio::spawn`

---

## 4. Cross-Cutting Patterns

### 4.1 WebSocket Manager Adoption Matrix

| Channel | Uses WsManager | HeartbeatStrategy | Reconnection |
|---------|---------------|-------------------|--------------|
| Telegram | No (HTTP polling) | N/A | Polling retry with 5s delay |
| Discord | Yes | None (self-managed) | `reconnect_loop` |
| Slack | Yes | Timeout(35s, Ping) | `reconnect_loop` (fresh URL each time) |
| WhatsApp | Yes | Timeout(30s, Ping) | `reconnect_loop` |
| QQ | Yes | Timeout(35s, JSON heartbeat) | `reconnect_loop` |
| Email | No (IMAP polling) | N/A | Polling retry on error |

### 4.2 Message Flow: Inbound

```
Chat Platform → Channel.start() → InboundMessage → MessageBus.publish_inbound()
                                                          ↓
Dashboard WS → ws/chat → InboundMessage → MessageBus ↗    ↓
                                                      AgentLoop.run()
```

### 4.3 Message Flow: Outbound

```
AgentLoop response → MessageBus.outbound_sender() → outbound_rx
                                                          ↓
                                            ChannelManager dispatcher
                                                          ↓
                                            channel.send(OutboundMessage)
```

### 4.4 Dashboard Integration Point

The dashboard is designed to embed in `klyntbot serve` but is **not yet wired** in `serve.rs`. The serve command starts:
- AgentLoop
- ChannelManager
- CronService
- HeartbeatService

But does **not** start `DashboardServer`. This is the primary integration gap.

---

## 5. Gap Analysis

### 5.1 Missing Integration

| Gap | Severity | Description |
|-----|----------|-------------|
| Dashboard not wired to serve | **High** | `serve.rs` doesn't create/start `DashboardServer` |
| GraphQL playground not exposed | Medium | POST `/graphql` route registered but not connected to schema |
| `system_status` query is stub | Medium | Returns hardcoded values, not live data |
| `cron_jobs` query is stub | Low | Returns empty vec |
| `sessions` query is stub | Low | Returns empty vec |
| `search_todos` semantic/hybrid stubs | Medium | Returns empty vec for non-keyword modes |
| `sync_calendar` mutation is stub | Low | Returns "not yet configured" |
| `do_config_patch` is stub | Medium | Validates section name but doesn't apply patch |

### 5.2 Test Coverage Gaps

| Area | Current | Gap |
|------|---------|-----|
| Telegram | 9 unit tests | No integration tests, no proxy/media tests |
| Discord | 0 tests | No tests at all |
| Slack | 0 tests | No tests at all |
| WhatsApp | 0 tests | No tests at all |
| QQ | 0 tests | No tests at all |
| Email | 0 tests | No tests at all (hardest to test: IMAP/SMTP) |
| ChannelManager | 0 tests | No tests for init/start/dispatch/stop |
| WebSocketManager | 7 tests | Good coverage for unit behavior |
| CLI chat | 0 tests | Difficult (TTY, streaming) but could test helpers |
| CLI serve | 0 tests | Integration-level testing needed |
| CLI interactive | 3 tests | Minimal coverage |
| Wizard framework | 15 tests | Good trait/state coverage |
| Wizard prompts | 20 tests | Good mask_secret coverage; prompts hard to test |
| Wizard templates | 12 tests | Thorough structural validation |
| Dashboard server | 0 tests | `start_on_random_port` exists for tests but unused |
| Dashboard GraphQL | 0 tests | No resolver tests |
| Dashboard WebSocket | 0 tests | No protocol tests |
| Dashboard events | 0 tests | No event bus tests |
| Dashboard config watcher | 0 tests | No file-watch tests |

### 5.3 Architectural Concerns

1. **Dashboard uses legacy stores**: `DashboardContext` holds `Arc<RwLock<TodoStore>>` and `Arc<RwLock<ProjectStore>>` (legacy JSONL-backed), while the rest of the codebase has migrated to PostgreSQL repos. The dashboard needs to be updated to use `Repos` pattern.

2. **Duplicate send implementations**: Each channel has its own HTTP client setup and retry logic. Could be consolidated into a shared `HttpSender` utility.

3. **Typing indicators aren't uniform**: Telegram and Discord have them; Slack, WhatsApp, QQ, and Email do not.

4. **Email channel has no idle-keepalive**: IMAP connections are opened and closed per poll cycle, which is correct for polling but inefficient for IDLE-based implementations.

5. **QQ heartbeat doesn't include sequence**: The heartbeat payload is `{"op": 1, "d": null}` but QQ protocol may require the current sequence number. The `seq` field exists on `QQChannel` but isn't accessible from the heartbeat closure due to ownership constraints.

6. **No graceful WebSocket close**: Channels set `running = false` but don't send WebSocket close frames. The connections drop on the next read timeout.

### 5.4 Recommended Priorities

1. **Wire dashboard into serve.rs** — create `DashboardServer` alongside existing services
2. **Migrate dashboard to Repos** — replace `Arc<RwLock<TodoStore>>` with SQL repos
3. **Add channel integration tests** — at minimum for Telegram (most complex) and Discord (most popular)
4. **Implement stub resolvers** — `system_status`, `cron_jobs`, `sessions` with real data
5. **Add WebSocket close frames** — graceful disconnect on channel stop
6. **Consolidate retry logic** — extract shared HTTP client with retry/rate-limit handling
