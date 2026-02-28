# Agent Core

The `agent` crate (Layer 5) is the central orchestration layer of klyntbot. It owns the main processing loop, the execution engine that drives LLM-tool cycles, context assembly, skill discovery, subagent management, and notification dispatch. Everything converges here: messages arrive from channels, pass through an intent pipeline, get executed by the appropriate engine, and responses flow back out.

---

## Section 1: Narrative Overview

### AgentLoop Design

`AgentLoop` is the top-level struct that ties together every subsystem. It holds references to the message bus, session manager, tool registry, context engine, intent pipeline, and all background services (reminders, recurring tasks, learning, cleanup). It does not perform LLM calls directly; instead it delegates to the `IntentPipeline`, which in turn uses `ExecutionCore` for the actual LLM-tool interaction.

**Key fields** (defined at `crates/agent/src/agent_loop/mod.rs:53-87`):

- `bus` -- shared `MessageBus` for inbound/outbound message routing
- `pipeline` -- the `IntentPipeline` that classifies, routes, and executes messages
- `tool_registry` -- `Arc<RwLock<ToolRegistry>>` holding all registered tools
- `context_engine` -- assembles the system prompt from multiple context sources
- `session_manager` -- per-session conversation persistence backed by SQLite
- `skill_manager` -- discovers and loads skill markdown files
- `running` -- atomic boolean controlling the event loop
- `plan_executing` -- atomic boolean tracking whether a plan is currently running
- `history_limit` -- max number of session messages to load per request

Background services held for lifetime management:
- `reminder_engine` -- periodic todo/calendar reminder checks
- `recurring_task_spawner` -- spawns recurring tasks on schedule
- `learning_service` -- adaptive threshold updates from outcome analysis
- `_notification_dispatcher`, `_calendar_adapter` -- held as `Arc` for shared ownership
- `_session_cleanup_token`, `_memory_maintenance_token`, `_plan_cleanup_token` -- cancellation tokens for background cleanup tasks

**Construction** follows a builder pattern. `AgentLoop::builder()` returns an `AgentLoopBuilder` (defined at `crates/agent/src/agent_loop/builder.rs:53-61`). Three fields are required:

1. `bus` -- the `MessageBus`
2. `provider` -- the LLM provider (`DynProvider`)
3. `config` -- the full `Config`

Optional fields enable storage-dependent features:
- `pool` -- `SqlitePool` enables todo, finance, sessions, plans
- `vector_store` -- enables semantic search and conversation embeddings
- `cron_service` -- enables the cron tool
- `notification_handle` -- enables last-active-channel tracking for notifications

The `build()` method (`builder.rs:120-826`) performs a large, sequential initialization:

1. Validates required fields
2. Creates `StoragePool` and `Repos` (falls back to in-memory SQLite if no pool)
3. Loads and filters skills by enabled packs
4. Creates context sources (identity, bootstrap, memory, todo, goal, confidence, skills)
5. Initializes the memory store (with optional embedding support)
6. Creates the session manager from the session repo
7. Builds the subagent manager with a builder of its own
8. Registers all tools into the `ToolRegistry` (see Tool Registry Construction below)
9. Creates the notification dispatcher
10. Wires the calendar adapter and reminder engine
11. Sets up the learning service with event bus subscriber
12. Builds the `IntentPipeline` (analyzer, router with three engines, cost tracker)
13. Spawns background cleanup services (session, memory maintenance, plan visibility)
14. Assembles the final `AgentLoop` struct

### Main Message Processing Flow

When a message arrives, the flow is:

```
InboundMessage -> process_message() -> IntentPipeline -> response -> OutboundMessage
```

**`process_message()`** (`mod.rs:236-306`):

1. **Validate** -- drops oversized messages silently
2. **Reaction handling** -- if the message is a reaction emoji, maps it to a satisfaction score and updates the most recent strategy record (no LLM call)
3. **System messages** -- subagent results route through `process_system_message()`, which parses the origin channel:chat_id from the message and processes it back through the pipeline
4. **Track last active channel** -- for notification routing
5. **Session setup** -- gets or creates a per-session lock, adds the user message, extracts history
6. **Conversation embedding** -- fires a background task to embed the user message (if configured)
7. **Run pipeline** -- calls `run_pipeline()` which assembles the system prompt via `ContextEngine`, converts history to provider messages, and calls `pipeline.process_message()`
8. **Save and respond** -- saves the assistant response to the session (with optional embedding), publishes an `OutboundMessage` to the bus

The `run_pipeline()` method (`mod.rs:464-503`) is the shared entry point for both bus-driven and direct processing. It builds the system prompt, converts session history, gathers tool definitions, and delegates to the pipeline.

**Direct processing** for CLI mode (`process_direct` at `mod.rs:538-551`) returns the response string directly instead of publishing to the bus.

**Streaming** (`process_direct_streaming` at `mod.rs:560-630`) spawns a background task, returns a `StreamingHandle` with channels for events and interaction requests, and supports cancellation.

### ExecutionCore -- the ReAct Loop

`ExecutionCore` (`crates/agent/src/execution/core.rs:140-143`) is the lowest-level execution primitive. It owns a provider and a tool registry, and exposes a single method: `run_cycle()`.

**`run_cycle()`** (`core.rs:158-388`) performs one LLM-tool iteration:

1. Calls `provider.chat()` with messages and tool definitions
2. If the LLM returns **tool calls**:
   - Checks for duplicate tool calls (same name + argument hash) using a `HashSet<String>` tracker. If all calls in a batch are duplicates, appends synthetic "already called" results and skips execution.
   - Records current call signatures for future dedup
   - Appends the assistant message with tool call metadata
   - Executes all tool calls **in parallel** via `futures_util::join_all`, each wrapped in a per-tool timeout (`params.tool_timeout`, default 30s)
   - Emits `AgentEvent::ToolStart` before and `AgentEvent::ToolEnd` after each execution
   - Collects entity cards from tools (via an mpsc channel on the routing context) and emits `AgentEvent::EntityCreated`
   - Appends tool result messages to the conversation
   - Returns `CycleOutcome::ToolsExecuted`
3. If the LLM returns **text**:
   - Runs fabrication detection (`is_fabricated_tool_response`) to catch LLMs that skip tool calls and generate fake structured results
   - Returns `CycleOutcome::FinalResponse` or `CycleOutcome::FabricatedResponse`
4. If empty: returns `CycleOutcome::EmptyResponse`

`ExecutionCore` does not loop. The looping is handled by the execution engines (ReactiveEngine, PlannedEngine) in the intent pipeline, which call `run_cycle()` repeatedly until a final response or iteration limit.

**Fabrication detection** (`core.rs:66-133`) uses heuristics to detect when an LLM generates text that mimics tool output instead of actually calling tools. It checks for context-aware patterns: fake hex IDs, structured field patterns (Priority:, Due Date:, etc.), and numbered search result lists. Critically, it only flags patterns that match tools actually available in the registry.

### Tool Registry Construction

The builder (`builder.rs:237-580`) registers tools in this order:

1. **Filesystem tools** -- `register_fs_tools()` or restricted by `allowed_dir`
2. **Search tools** -- `GrepTool`, `GlobTool` (with optional workspace restriction)
3. **Web tools** -- `WebSearchTool` (with Brave API key), `WebFetchTool`
4. **Browser tool** -- conditional on `config.tools.browser.enabled`
5. **Message tool** -- routes outbound messages through the bus
6. **Ask-user tool** -- interactive clarification from users
7. **Spawn tool** -- delegates to `SubagentManager` via `SpawnHandler` trait
8. **Cron tool** -- optional, requires a `CronService`
9. **Feature-todo tool** -- requires a real pool; injects calendar handler, enrichment engine, embedding handler, and search config
10. **Goal tool** -- with plan repo and LLM provider for goal decomposition
11. **Plan tool** -- with plan handler for plan CRUD and execution
12. **Memory tool** -- conditional on conversation search being enabled; wires conversation embedding handler and todo embedding handler
13. **Finance tool** -- conditional on `config.finance.enabled` and a real pool
14. **Plugin tools** -- WASM plugins loaded from `{data_dir}/plugins/`; their cron jobs are registered and their tools are added dynamically
15. **Learning tool** -- conditional on learning being enabled; wires adaptive thresholds

### Context Sources

Before each LLM call, `ContextEngine` assembles the system prompt by querying all registered context sources. Sources are sorted by priority (descending) and their output is concatenated. Each source implements the `ContextSource` trait from the `context_engine` crate.

Sources registered in `builder.rs:190-202`, listed by priority:

| Priority | Source | File | Behavior |
|----------|--------|------|----------|
| 100 | `IdentitySource` | `context_sources/identity.rs:13` | Always fresh. Emits date/time (in configured timezone), OS, workspace path, channel, chat ID. Adjusts messaging instructions based on channel (CLI vs bus). |
| 90 | `BootstrapSource` | `context_sources/bootstrap.rs:25` | Cached permanently via `OnceCell`. Loads `AGENTS.md`, `SOUL.md`, `USER.md`, `TOOLS.md`, `IDENTITY.md`, `RESPONSE.md` from the workspace. |
| 80 | `MemorySource` | `context_sources/memory.rs:14` | TTL-cached (60s). Fetches long-term memory notes. If a user message is available, uses relevance-filtered retrieval (embedding-based ANN). Falls back to full memory context otherwise. |
| 70 | `TodoSource` | `context_sources/todo.rs:13` | TTL-cached (60s). Queries `TodoRepo::to_context_string()` for active tasks summary. |
| 60 | `GoalSource` | `context_sources/goal.rs:12` | TTL-cached (60s). Lists active goals with priority and description. |
| 50 | `ConfidenceSource` | `context_sources/confidence.rs:13` | Always fresh. Emits confidence evaluation instructions with the current threshold (stored as `AtomicU32` for lock-free updates from the learning service). |
| 40 | `SkillSummarySource` | `context_sources/skills.rs:11` | Cached via `OnceLock` inside `SkillManager`. Emits XML summary of all skills (name, availability, description, triggers). |
| 30 | `SkillContentSource` | `context_sources/skills.rs:38` | Emits full markdown content of skills marked `always: true`. |

### Streaming

`StreamingHandle` (`mod.rs:38-47`) is returned by `process_direct_streaming()` and provides:

- `event_rx` -- receives `AgentEvent` values (content chunks, tool start/end, classification, context assembled, errors, done)
- `interaction_rx` -- receives `InteractionBundle` values from the `ask_user` tool, containing questions with oneshot response channels
- `cancel_token` -- a `CancellationToken` for aborting processing
- `handle` -- a `JoinHandle<Result<String>>` for the background task

The streaming flow spawns a tokio task that runs the pipeline with an event sender. The pipeline and execution engines forward events through this channel. On completion, `AgentEvent::Done` is emitted with the final content. On error, `AgentEvent::Error` is emitted.

### AgentEvent

`AgentEvent` (`crates/agent/src/events.rs:11-86`) is a tagged enum serialized with camelCase tags. Variants:

- `ContentChunk { data }` -- streamed LLM output
- `ToolStart { name, args }` -- tool execution beginning
- `ToolEnd { name, success, duration_ms, result }` -- tool execution finished (result truncated to 2KB)
- `IterationStart { iteration, max }` -- new agent iteration in the ReAct loop
- `ClassificationComplete { strategy, confidence, source, duration_ms }` -- intent classification result
- `ContextAssembled { total_tokens, budget, duration_ms }` -- context engine finished assembling
- `ExecutionStarted { engine, max_iterations }` -- execution engine selected
- `Done { content }` -- final response
- `ConfidenceAssessed { score, action }` -- internal confidence check
- `Error { message }` -- processing error
- `PlanStepCompleted { plan_id, step_index, result }` -- single plan step done
- `PlanCompleted { plan_id, summary }` -- entire plan finished
- `EntityCreated(EntityCard)` -- tool created an entity (task, project, goal)

### SubagentManager

`SubagentManager` (`crates/agent/src/subagent.rs:91-102`) spawns background tasks that run isolated agent loops with restricted tool sets. It uses a `Semaphore` to limit concurrent subagents (configurable, default 3).

**Construction** uses a builder pattern (`SubagentManagerBuilder` at `subagent.rs:105-182`). Required: `provider` and `workspace`. Optional: `inbound_sender`, `model`, `brave_api_key`, `web_max_results`, `task_timeout`, `restrict_to_workspace`, `max_concurrent_subagents`.

**Profiles** (`SubagentProfile` at `subagent.rs:29-38`):

- `General` -- full filesystem + web access, 15 max iterations
- `Research` -- read-only filesystem + web, 10 max iterations
- `Analyst` -- read-only filesystem only, 5 max iterations

**Spawning** (`spawn()` at `subagent.rs:200-307`):

1. Generates a UUID, stores a `SubagentHandle` with cancel token and metadata
2. Spawns a tokio task that acquires a semaphore permit
3. Runs `run_subagent_task()` which builds a profile-specific tool registry, creates an `ExecutionCore` and `ReactiveEngine`, and executes the task
4. On completion, announces the result back to the main agent via the inbound bus as a system message

The `SubagentManager` implements the `SpawnHandler` trait (`subagent.rs:349-371`), enabling the `SpawnTool` in the tools layer to use it without a direct dependency on the agent crate.

### SkillManager

`SkillManager` (`crates/agent/src/skills.rs:73-77`) discovers, parses, and manages skill markdown files. Skills have YAML frontmatter with metadata (description, version, triggers, requirements) and a markdown body.

**Loading** (`load()` at `skills.rs:89-104`):

1. Loads 6 built-in skills compiled into the binary via `include_str!` (cron, daily-planning, skill-creator, summarize, todo, weather)
2. Loads workspace skills from `{workspace}/skills/*/SKILL.md` (these override built-in skills with the same name)
3. Checks requirements (required binaries via `which`, required env vars)

**Filtering** (`filter_by_skills()` at `skills.rs:304-313`): After loading, skills are filtered to only those from enabled feature packs. Workspace-loaded skills (non-builtin) are always kept regardless of pack selection.

**Summary generation** (`generate_summary()` at `skills.rs:252-280`): Produces an XML string with skill names, availability, descriptions, and triggers. Cached via `OnceLock` after first computation.

### NotificationDispatcher

`NotificationDispatcher` (`crates/agent/src/notifications.rs:12-16`) routes notifications to configured targets. It holds an outbound message sender and a reference to the last active channel.

**`notify()`** (`notifications.rs:34-60`) iterates over configured targets:

- `"os_native"` -- sends an OS-level notification via `common::utils::notify`
- Any other string -- treated as a channel name; if it matches the last active channel, sends an `OutboundMessage` with the title and body

### HeartbeatService

`HeartbeatService` (`crates/heartbeat/src/service.rs:67`) is a periodic health check service that wakes the agent to inspect a workspace file (`HEARTBEAT.md`) for pending tasks.

**How it works:**

1. On each tick, the service reads `HEARTBEAT.md` from the configured workspace directory.
2. If the file is missing, empty, or contains only headers/comments/empty checkboxes, the tick is skipped (no agent invocation).
3. If actionable content is found, the service invokes the registered `HeartbeatCallback` with a prompt instructing the agent to read and execute the file's instructions.
4. If the agent's response contains the `HEARTBEAT_OK` token, the tick is logged as "no action needed." Otherwise, it is logged as "completed task."

**Default interval:** 30 minutes (`DEFAULT_HEARTBEAT_INTERVAL_S = 1800`).

**HeartbeatCallback mechanism:** The callback is a synchronous function `Arc<dyn Fn(&str) -> Result<String, Box<dyn Error>> + Send + Sync>` set via `set_callback()`. It receives the heartbeat prompt and returns the agent's response string. The service does not own an agent reference directly -- the callback is wired at construction time in `klyntbot serve`, decoupling the heartbeat crate from the agent crate.

**Integration with `serve`:** The `klyntbot serve` command creates a `HeartbeatService` with the configured workspace path and interval, attaches a callback that routes the prompt through the agent loop, and calls `start()`. The service spawns a background Tokio task that sleeps for the configured interval between ticks. `stop()` aborts the background task. `trigger_now()` allows manual invocation outside the periodic schedule.

### AgentTaskHandler

`AgentTaskHandlerImpl` (`crates/agent/src/agent_task_handler.rs:11-13`) bridges the tools layer to the storage layer. It implements the `AgentTaskHandler` trait (defined in `tools`) by delegating to `AgentTaskRepo` (defined in `storage`). Operations: `list_tasks`, `claim_task`, `update_task`, `complete_task`, `fail_task`.

### Chat Module

The `chat` module (`crates/agent/src/chat/`) provides channel-aware response formatting. It adapts LLM output for each chat platform's message length limits and formatting capabilities.

**`format_for_channel(content, channel)`** (`crates/agent/src/chat/formatter.rs:18`) is the single public entry point. It dispatches on the channel name:

| Channel | Behavior | Max Length |
|---------|----------|------------|
| `"telegram"` | Preserves markdown, truncates at boundary | 4096 chars |
| `"discord"` | Preserves markdown and code blocks, truncates at boundary | 2000 chars |
| `"whatsapp"` | Strips markdown formatting (bold, italic, code, headers), truncates at boundary | 4000 chars |
| Any other (CLI, QQ, etc.) | Pass-through, no modification | Unlimited |

Truncation uses `truncate_with_ellipsis()` which finds the last whitespace boundary before the limit and appends `"..."`. UTF-8 boundary safety is delegated to `common::utils::truncate_at_boundary`. The WhatsApp formatter uses `strip_markdown()` to remove `**`, `__`, backticks, and `#` headers while preserving the underlying text content.

### Scratchpad

`Scratchpad` (`crates/agent/src/execution/scratchpad.rs:19-21`) accumulates `ReasoningTrace` entries across execution cycles. Each trace records the cycle number, thought, planned actions, actual action, optional reflection, and timestamp. The `summarize()` method caps output to the last 20 traces and produces a text summary suitable for context injection.

### Session Management

The `session` crate (Layer 2) provides conversation persistence. Every message exchange -- whether from a Telegram chat, Discord channel, CLI REPL, or dashboard WebSocket -- is stored in a session keyed by a `channel:chat_id` string. Sessions are backed by SQLite via `storage::SessionRepo` and cached in memory with LRU eviction.

**SessionManager** (`crates/session/src/manager.rs:154-159`) is the main entry point. It is `Clone + Send + Sync` -- all clones share the same underlying `DashMap` and repo, so it can be stored directly without `Arc<RwLock<>>` wrappers.

Construction happens in the builder (`builder.rs:217-219`):

```rust
let session_manager =
    SessionManager::from_repo(storage::SessionRepo::new(storage_pool.inner().clone()))
        .await;
```

The manager uses a `DashMap<String, Arc<TokioMutex<Session>>>` for concurrent per-session access. Each session has its own `tokio::sync::Mutex`, so operations on different sessions never block each other. LRU eviction order is tracked via a `std::sync::Mutex<VecDeque<String>>` with a default cache size of 1000 entries. When the cache exceeds this limit, the least recently used session is saved to SQL and removed from memory.

**Session lifecycle:**

1. **Create / Resume** -- `get_or_create(key)` checks the in-memory cache first (fast path). On cache miss, it loads from SQLite via `SessionRepo::get_session()` + `get_messages()`. If neither exists, it creates a new session row in the database and returns a fresh `Session`.
2. **Message accumulation** -- `Session::add_message()` appends a `SessionMessage` with a UUID v4 id and timestamp. `add_structured_message()` additionally stores tool call data and metadata.
3. **Saving** -- `save()` upserts the session metadata and batch-inserts all messages in a single SQL round-trip (ON CONFLICT DO NOTHING for idempotency). If the message count exceeds `COMPACTION_THRESHOLD` (1000), it inserts a compaction marker and deletes the oldest messages, keeping `COMPACTION_KEEP` (500).
4. **Deletion** -- `delete(key)` removes from cache and database. `reset_session(key)` does the same but removes from the LRU order as well.
5. **Listing** -- `list()` returns `Vec<SessionInfo>` with key, timestamps, and message count, sorted by most recently updated.

**Session struct** (`crates/session/src/manager.rs:21-37`) holds the conversation state:

- `key` -- session identifier string (e.g., `"telegram:123456"` or `"cli:default"`)
- `messages` -- `Vec<SessionMessage>` containing the full message history
- `created_at` / `updated_at` -- UTC timestamps
- `metadata` -- `HashMap<String, serde_json::Value>` for extensible per-session data

`get_history(max_messages)` returns a slice of the most recent N messages, used by the agent loop to cap context window size.

**SessionMessage** (`crates/session/src/manager.rs:112-137`) represents a single turn in the conversation:

- `id` -- UUID v4 string, auto-generated on creation
- `role` -- one of `"system"`, `"user"`, `"assistant"`, `"tool"`
- `content` -- the message text
- `timestamp` -- when the message was added
- `request_id` -- optional correlation ID for tracing
- `tool_calls` -- optional JSON value holding structured tool call data (function name, arguments, result)
- `metadata` -- optional JSON value for extensible data (reasoning traces, content parts, etc.)

**SessionInfo** (`crates/session/src/manager.rs:428-434`) is the lightweight summary type returned by `list()`:

- `key`, `created_at`, `updated_at`, `message_count`

**Session key format.** Keys follow the pattern `channel:identifier`:

- **CLI sessions**: `cli:{name}` -- the name comes from `--session` flag or defaults to `"default"`. Formed in `crates/cli/src/chat.rs:56` as `format!("cli:{}", session)`.
- **Channel sessions**: `{channel_name}:{chat_id}` -- e.g., `"telegram:123456"`, `"discord:guild123"`. Formed by `InboundMessage::session_key()` which constructs a `SessionKey` from the channel name and chat ID.
- **System sessions**: subagent results are routed back using the original session key parsed from the system message: `format!("{}:{}", origin_channel, origin_chat_id)`.

The `SessionKey` type in `common::types` provides structured construction via `SessionKey::new(&channel, &chat_id)` and parsing via `split()` which returns `(ChannelName, ChatId)`.

**Session cleanup service** (`crates/agent/src/session_cleanup_service.rs:12-17`) is a background tokio task that periodically deletes stale sessions. It is configured via `config.conversation.session`:

- `ttl_days` (default: 30) -- sessions with `updated_at` older than this many days are deleted
- `cleanup_interval_hours` (default: 1) -- how often the service checks for stale sessions

The service is spawned during `AgentLoopBuilder::build()` (`builder.rs:757-769`) when a storage pool is available. It uses a `CancellationToken` for graceful shutdown -- the token is stored as `_session_cleanup_token` on `AgentLoop` and cancelled during `shutdown()`. The first tick is skipped so cleanup does not run immediately on startup.

**Agent loop integration.** The session manager is wired into every message processing path:

1. **Bus-driven messages** (`process_message()` at `mod.rs:270-299`): extracts the session key from the inbound message, calls `get_or_create()`, acquires the per-session lock, adds the user message, extracts `history_limit` recent messages, releases the lock, runs the pipeline, then calls `save_to_session()` which adds the assistant response and persists to SQL.
2. **CLI / direct mode** (`setup_session()` at `mod.rs:509-536` + `process_direct()` at `mod.rs:538-549`): same flow but the session key is `"cli:{name}"` and the response is returned directly instead of published to the bus.
3. **Streaming mode** (`process_direct_streaming()` at `mod.rs:560-630`): session setup happens synchronously, then the pipeline runs in a background tokio task with event streaming.
4. **History conversion** (`convert_history()` at `mod.rs:443-453`): maps `SessionMessage` roles to provider `Message` types for LLM input.
5. **Conversation embedding**: after adding each user or assistant message, `spawn_embed_message()` fires a background task to generate a vector embedding of the message (if configured and the channel is not excluded).

---

## Section 2: API Reference

### AgentLoop

**File:** `crates/agent/src/agent_loop/mod.rs:53-87`

```rust
pub struct AgentLoop {
    // All fields are pub(crate)
    bus: Arc<MessageBus>,
    inbound_rx: Option<mpsc::Receiver<InboundMessage>>,
    skill_manager: Arc<SkillManager>,
    config: Config,
    context_engine: Arc<context_engine::ContextEngine>,
    session_manager: SessionManager,
    tool_registry: Arc<RwLock<tools::registry::ToolRegistry>>,
    running: Arc<AtomicBool>,
    last_active_channel: Option<LastActiveChannel>,
    reminder_engine: Option<Arc<RwLock<ReminderEngine>>>,
    recurring_task_spawner: Option<Arc<RwLock<RecurringTaskSpawner>>>,
    _notification_dispatcher: Option<Arc<NotificationDispatcher>>,
    _calendar_adapter: Option<Arc<CalendarSyncAdapter>>,
    conversation_embedding_handler: Option<Arc<dyn tools::ConversationEmbeddingHandler>>,
    plan_executing: Arc<AtomicBool>,
    learning_service: Option<Arc<RwLock<LearningService>>>,
    pipeline: Arc<IntentPipeline>,
    strategy_repo: Option<storage::StrategyRepo>,
    history_limit: usize,
    _session_cleanup_token: Option<CancellationToken>,
    _memory_maintenance_token: Option<CancellationToken>,
    _plan_cleanup_token: Option<CancellationToken>,
}
```

**Public methods:**

| Method | Signature | Location | Description |
|--------|-----------|----------|-------------|
| `builder()` | `fn builder() -> AgentLoopBuilder` | `builder.rs:831` | Creates a new builder. |
| `is_plan_executing()` | `fn is_plan_executing(&self) -> bool` | `mod.rs:93` | Checks if a plan is currently running. |
| `take_inbound_rx()` | `fn take_inbound_rx(&mut self) -> Option<Receiver<InboundMessage>>` | `mod.rs:136` | Extracts the inbound receiver (call before wrapping in `Arc`). |
| `run_with_rx()` | `async fn run_with_rx(&self, rx: Receiver<InboundMessage>) -> Result<()>` | `mod.rs:145` | Runs the loop with an externally-provided receiver. Takes `&self`. |
| `run()` | `async fn run(&mut self) -> Result<()>` | `mod.rs:175` | Backward-compatible run that takes `&mut self`. |
| `shutdown_flag()` | `fn shutdown_flag(&self) -> Arc<AtomicBool>` | `mod.rs:187` | Returns the running flag for external shutdown control. |
| `stop()` | `async fn stop(&self)` | `mod.rs:192` | Sets the running flag to false. |
| `shutdown()` | `async fn shutdown(&self) -> Result<()>` | `mod.rs:197` | Gracefully stops all background tasks (reminders, recurring spawner, learning, cleanup services). |
| `process_direct()` | `async fn process_direct(&self, content: String, session_key: String) -> Result<String>` | `mod.rs:538` | Processes a message synchronously (CLI mode). Returns the response text. |
| `process_direct_streaming()` | `async fn process_direct_streaming(self: &Arc<Self>, content: String, session_key: String) -> Result<StreamingHandle>` | `mod.rs:560` | Processes a message with real-time event streaming. Returns a `StreamingHandle`. |
| `skill_manager()` | `fn skill_manager(&self) -> &SkillManager` | `mod.rs:633` | Returns a reference to the skill manager. |
| `list_tools()` | `async fn list_tools(&self) -> Arc<Vec<Value>>` | `mod.rs:638` | Returns all tool definitions (for API status). |
| `tool_names()` | `async fn tool_names(&self) -> Vec<String>` | `mod.rs:643` | Returns all registered tool names. |
| `model_name()` | `fn model_name(&self) -> &str` | `mod.rs:648` | Returns the configured model name. |

### AgentLoopBuilder

**File:** `crates/agent/src/agent_loop/builder.rs:53-61`

```rust
pub struct AgentLoopBuilder {
    bus: Option<Arc<MessageBus>>,
    provider: Option<DynProvider>,
    config: Option<Config>,
    pool: Option<sqlx::SqlitePool>,
    vector_store: Option<storage::VectorStore>,
    cron_service: Option<Arc<scheduling::CronService>>,
    notification_handle: Option<LastActiveChannel>,
}
```

**Builder methods:**

| Method | Signature | Location |
|--------|-----------|----------|
| `new()` | `fn new() -> Self` | `builder.rs:70` |
| `with_bus()` | `fn with_bus(self, bus: Arc<MessageBus>) -> Self` | `builder.rs:82` |
| `with_provider()` | `fn with_provider(self, provider: DynProvider) -> Self` | `builder.rs:87` |
| `with_config()` | `fn with_config(self, config: Config) -> Self` | `builder.rs:92` |
| `with_pool()` | `fn with_pool(self, pool: SqlitePool) -> Self` | `builder.rs:97` |
| `with_vector_store()` | `fn with_vector_store(self, store: VectorStore) -> Self` | `builder.rs:102` |
| `with_cron_service()` | `fn with_cron_service(self, service: Arc<CronService>) -> Self` | `builder.rs:107` |
| `with_notification_handle()` | `fn with_notification_handle(self, handle: LastActiveChannel) -> Self` | `builder.rs:112` |
| `build()` | `async fn build(self) -> Result<AgentLoop>` | `builder.rs:120` |

### ExecutionCore

**File:** `crates/agent/src/execution/core.rs:140-143`

```rust
pub struct ExecutionCore {
    pub provider: DynProvider,
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
}
```

**Methods:**

| Method | Signature | Location | Description |
|--------|-----------|----------|-------------|
| `new()` | `fn new(provider: DynProvider, tool_registry: Arc<RwLock<ToolRegistry>>) -> Self` | `core.rs:146` | Constructs a new core. |
| `run_cycle()` | `async fn run_cycle(&self, messages: &mut Vec<Message>, tools: &[Value], params: &ExecutionParams, routing_ctx: &RoutingContext, event_tx: Option<&Sender<AgentEvent>>, seen_tool_calls: Option<&mut HashSet<String>>) -> Result<(CycleOutcome, Usage)>` | `core.rs:158` | Runs one LLM-tool cycle. Mutates `messages` in place by appending assistant and tool result messages. |

### ExecutionParams

**File:** `crates/agent/src/execution/types.rs:9-12`

```rust
pub struct ExecutionParams {
    pub tool_timeout: Duration,    // Default: 30s
    pub chat_params: ChatParams,
}
```

**Methods:**

| Method | Signature | Location |
|--------|-----------|----------|
| `new()` | `fn new(model: impl Into<String>) -> Self` | `types.rs:15` |
| `with_timeout()` | `fn with_timeout(self, dur: Duration) -> Self` | `types.rs:22` |

### ToolExecutionResult

**File:** `crates/agent/src/execution/types.rs:29-37`

```rust
pub struct ToolExecutionResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub result: String,
    pub duration_ms: u64,
    pub success: bool,
}
```

### CycleOutcome

**File:** `crates/agent/src/execution/types.rs:41-50`

```rust
pub enum CycleOutcome {
    ToolsExecuted { results: Vec<ToolExecutionResult> },
    FinalResponse { content: String },
    EmptyResponse,
    FabricatedResponse { content: String },
}
```

### StreamingHandle

**File:** `crates/agent/src/agent_loop/mod.rs:38-47`

```rust
pub struct StreamingHandle {
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub interaction_rx: mpsc::Receiver<tools::InteractionBundle>,
    pub cancel_token: CancellationToken,
    pub handle: JoinHandle<Result<String>>,
}
```

- `event_rx` -- poll for `AgentEvent` values during processing
- `interaction_rx` -- poll for `InteractionBundle` values from `ask_user` tool calls
- `cancel_token` -- cancel to abort the background task
- `handle` -- await to get the final response string or error

### AgentEvent

**File:** `crates/agent/src/events.rs:11-86`

```rust
pub enum AgentEvent {
    ContentChunk { data: String },
    ToolStart { name: String, args: Value },
    ToolEnd { name: String, success: bool, duration_ms: u64, result: Option<String> },
    IterationStart { iteration: usize, max: usize },
    ClassificationComplete { strategy: String, confidence: f32, source: String, duration_ms: u64 },
    ContextAssembled { total_tokens: usize, budget: usize, duration_ms: u64 },
    ExecutionStarted { engine: String, max_iterations: usize },
    Done { content: String },
    ConfidenceAssessed { score: f32, action: String },
    Error { message: String },
    PlanStepCompleted { plan_id: Uuid, step_index: usize, result: String },
    PlanCompleted { plan_id: Uuid, summary: String },
    EntityCreated(EntityCard),
}
```

Serialized with `#[serde(tag = "type", rename_all = "camelCase")]`.

### SubagentManager

**File:** `crates/agent/src/subagent.rs:91-102`

**Methods:**

| Method | Signature | Location | Description |
|--------|-----------|----------|-------------|
| `builder()` | `fn builder(provider: DynProvider, workspace: PathBuf) -> SubagentManagerBuilder` | `subagent.rs:186` | Creates a builder. |
| `semaphore_permits()` | `fn semaphore_permits(&self) -> usize` | `subagent.rs:191` | Returns available concurrency permits. |
| `spawn()` | `async fn spawn(&self, task: String, label: Option<String>, profile: SubagentProfile, origin_channel: String, origin_chat_id: String) -> String` | `subagent.rs:200` | Spawns a background subagent. Returns a status message with the short ID. |
| `cancel_subagent()` | `async fn cancel_subagent(&self, agent_id: &str) -> Result<String>` | `subagent.rs:310` | Cancels a running subagent by short ID. |
| `get_status()` | `async fn get_status(&self) -> Result<String>` | `subagent.rs:328` | Lists all running subagents with elapsed time. |

Also implements `SpawnHandler` trait (`subagent.rs:349-371`) with `spawn()`, `cancel()`, `status()`.

### SubagentProfile

**File:** `crates/agent/src/subagent.rs:29-38`

```rust
pub enum SubagentProfile {
    General,   // Full access, 15 iterations
    Research,  // Read-only + web, 10 iterations
    Analyst,   // Read-only only, 5 iterations
}
```

**Methods:**

| Method | Signature | Location |
|--------|-----------|----------|
| `max_iterations()` | `fn max_iterations(&self) -> u32` | `subagent.rs:54` |
| `role_prompt()` | `fn role_prompt(&self) -> &'static str` | `subagent.rs:62` |

Implements `FromStr` (`subagent.rs:40-49`): parses "research", "analyst", or defaults to `General`.

### SubagentManagerBuilder

**File:** `crates/agent/src/subagent.rs:105-115`

| Method | Location |
|--------|----------|
| `new(provider, workspace)` | `subagent.rs:119` |
| `inbound_sender(tx)` | `subagent.rs:133` |
| `model(model)` | `subagent.rs:138` |
| `brave_api_key(key)` | `subagent.rs:143` |
| `web_max_results(max)` | `subagent.rs:148` |
| `task_timeout(timeout)` | `subagent.rs:153` |
| `restrict_to_workspace(restrict)` | `subagent.rs:158` |
| `max_concurrent_subagents(n)` | `subagent.rs:163` |
| `build()` | `subagent.rs:168` |

### SkillManager

**File:** `crates/agent/src/skills.rs:73-77`

**Methods:**

| Method | Signature | Location | Description |
|--------|-----------|----------|-------------|
| `new()` | `fn new() -> Self` | `skills.rs:80` | Creates an empty manager. |
| `load()` | `async fn load(&mut self, workspace_path: PathBuf) -> Result<()>` | `skills.rs:89` | Loads built-in skills, then workspace skills (overrides built-ins). |
| `generate_summary()` | `fn generate_summary(&self) -> &str` | `skills.rs:252` | Returns cached XML skills summary. |
| `get_always_loaded()` | `fn get_always_loaded(&self) -> Vec<&Skill>` | `skills.rs:283` | Returns skills with `always: true` that are available. |
| `get()` | `fn get(&self, name: &str) -> Option<&Skill>` | `skills.rs:291` | Looks up a skill by name. |
| `all()` | `fn all(&self) -> Vec<&Skill>` | `skills.rs:296` | Returns all loaded skills. |
| `filter_by_skills()` | `fn filter_by_skills(&mut self, allowed: &[String])` | `skills.rs:304` | Retains only skills in the allowed list (workspace skills always kept). |

### Skill

**File:** `crates/agent/src/skills.rs:33-70`

```rust
pub struct Skill {
    pub name: String,
    pub description: String,
    pub version: String,
    pub always: bool,
    pub triggers: Vec<String>,
    pub requires_bins: Vec<String>,
    pub requires_env: Vec<String>,
    pub path: PathBuf,
    pub content: Option<String>,  // Loaded on-demand (skipped in serde)
    pub available: bool,          // Whether requirements are met (skipped in serde)
}
```

### NotificationDispatcher

**File:** `crates/agent/src/notifications.rs:12-16`

**Methods:**

| Method | Signature | Location | Description |
|--------|-----------|----------|-------------|
| `new()` | `fn new(outbound_tx: Sender<OutboundMessage>, config: TodoNotificationConfig) -> Self` | `notifications.rs:19` | Creates a dispatcher with outbound sender and config. |
| `last_active_handle()` | `fn last_active_handle(&self) -> Arc<RwLock<Option<(ChannelName, ChatId)>>>` | `notifications.rs:29` | Returns a shared handle to the last active channel tracker. |
| `notify()` | `async fn notify(&self, title: &str, body: &str) -> Result<()>` | `notifications.rs:34` | Sends notification to all configured targets (os_native or channel names). |

### HeartbeatService

**File:** `crates/heartbeat/src/service.rs:67`

```rust
pub struct HeartbeatService {
    workspace: PathBuf,
    on_heartbeat: Option<HeartbeatCallback>,
    interval_s: u64,
    enabled: bool,
    running: Arc<RwLock<bool>>,
    task: Arc<RwLock<Option<JoinHandle<()>>>>,
}
```

**Constants:**

| Name | Value | Description |
|------|-------|-------------|
| `DEFAULT_HEARTBEAT_INTERVAL_S` | `1800` (30 min) | Default interval between heartbeat ticks |
| `HEARTBEAT_PROMPT` | (multi-line string) | Prompt sent to the agent: instructs it to read `HEARTBEAT.md` and follow instructions |
| `HEARTBEAT_OK_TOKEN` | `"HEARTBEAT_OK"` | Response token indicating nothing needs attention |

**Types:**

```rust
pub type HeartbeatCallback = Arc<dyn Fn(&str) -> Result<String, Box<dyn Error>> + Send + Sync>;
```

**Methods:**

| Method | Signature | Location | Description |
|--------|-----------|----------|-------------|
| `new()` | `fn new(workspace: impl Into<PathBuf>, interval_s: u64, enabled: bool) -> Self` | `service.rs:78` | Creates a new heartbeat service. |
| `set_callback()` | `fn set_callback(&mut self, callback: HeartbeatCallback)` | `service.rs:90` | Sets the callback invoked on each tick. |
| `heartbeat_file()` | `fn heartbeat_file(&self) -> PathBuf` | `service.rs:95` | Returns the path to `HEARTBEAT.md` in the workspace. |
| `start()` | `async fn start(&self)` | `service.rs:106` | Spawns the background tick loop. No-ops if `enabled` is false. |
| `stop()` | `async fn stop(&self)` | `service.rs:125` | Aborts the background task. |
| `trigger_now()` | `async fn trigger_now(&self) -> Option<String>` | `service.rs:187` | Manually invoke a single heartbeat tick. Returns the agent's response. |

### Scratchpad

**File:** `crates/agent/src/execution/scratchpad.rs:19-21`

**Methods:**

| Method | Signature | Location |
|--------|-----------|----------|
| `new()` | `fn new() -> Self` | `scratchpad.rs:24` |
| `add()` | `fn add(&mut self, trace: ReasoningTrace)` | `scratchpad.rs:29` |
| `traces()` | `fn traces(&self) -> &[ReasoningTrace]` | `scratchpad.rs:34` |
| `last_n()` | `fn last_n(&self, n: usize) -> &[ReasoningTrace]` | `scratchpad.rs:39` |
| `len()` | `fn len(&self) -> usize` | `scratchpad.rs:44` |
| `is_empty()` | `fn is_empty(&self) -> bool` | `scratchpad.rs:48` |
| `summarize()` | `fn summarize(&self) -> String` | `scratchpad.rs:56` |

### ReasoningTrace

**File:** `crates/agent/src/execution/scratchpad.rs:8-15`

```rust
pub struct ReasoningTrace {
    pub cycle: u32,
    pub thought: String,
    pub planned_actions: Vec<String>,
    pub actual_action: String,
    pub reflection: Option<String>,
    pub timestamp: DateTime<Utc>,
}
```

### Context Source Types

All implement `context_engine::ContextSource` with `name() -> &str`, `priority() -> u8`, and `async provide(&self, ctx: &SourceContext) -> Option<String>`.

| Type | File | Priority | Caching |
|------|------|----------|---------|
| `IdentitySource` | `context_sources/identity.rs:13` | 100 | None (always fresh) |
| `BootstrapSource` | `context_sources/bootstrap.rs:25` | 90 | `OnceCell` (permanent) |
| `MemorySource` | `context_sources/memory.rs:14` | 80 | TTL 60s (query-aware) |
| `TodoSource` | `context_sources/todo.rs:13` | 70 | TTL 60s |
| `GoalSource` | `context_sources/goal.rs:12` | 60 | TTL 60s |
| `ConfidenceSource` | `context_sources/confidence.rs:13` | 50 | None (always fresh) |
| `SkillSummarySource` | `context_sources/skills.rs:11` | 40 | `OnceLock` (permanent) |
| `SkillContentSource` | `context_sources/skills.rs:38` | 30 | None |

### AgentTaskHandlerImpl

**File:** `crates/agent/src/agent_task_handler.rs:11-13`

Implements `AgentTaskHandler` trait. Methods: `list_tasks(session_key)`, `claim_task(task_id, agent_id)`, `update_task(task_id, status, result)`, `complete_task(task_id, result)`, `fail_task(task_id, error)`.

### SessionManager

**File:** `crates/session/src/manager.rs:154-159`

```rust
pub struct SessionManager {
    sessions: Arc<DashMap<String, Arc<TokioMutex<Session>>>>,
    lru_order: Arc<StdMutex<VecDeque<String>>>,
    max_cache_size: usize,
    sql_repo: storage::SessionRepo,
}
```

`SessionManager` is `Clone + Send + Sync`. All clones share the same underlying map and repo.

**Methods:**

| Method | Signature | Location | Description |
|--------|-----------|----------|-------------|
| `from_repo()` | `async fn from_repo(repo: SessionRepo) -> Self` | `manager.rs:163` | Creates a manager backed by a SQL repository. |
| `get_or_create()` | `async fn get_or_create(&self, key: impl Into<String>) -> Result<Arc<TokioMutex<Session>>>` | `manager.rs:205` | Returns a per-session lock. Loads from DB on cache miss, creates if not found. |
| `save()` | `async fn save(&self, session: &Session) -> Result<()>` | `manager.rs:268` | Persists session metadata and messages to SQL. Triggers compaction if over threshold. |
| `save_by_key()` | `async fn save_by_key(&self, key: &str) -> Result<()>` | `manager.rs:364` | Locks the session by key, clones it, and persists. |
| `reset_session()` | `async fn reset_session(&self, key: &str) -> Result<()>` | `manager.rs:375` | Removes from cache and LRU order, deletes from database (cascades to messages). |
| `has_session()` | `fn has_session(&self, key: &str) -> bool` | `manager.rs:395` | Checks whether a session exists in the in-memory cache. |
| `delete()` | `async fn delete(&self, key: &str) -> Result<bool>` | `manager.rs:400` | Removes from cache and deletes from database. |
| `list()` | `async fn list(&self) -> Result<Vec<SessionInfo>>` | `manager.rs:411` | Lists all sessions sorted by most recently updated. |

### Session

**File:** `crates/session/src/manager.rs:21-37`

```rust
pub struct Session {
    pub key: String,
    pub messages: Vec<SessionMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

**Methods:**

| Method | Signature | Location | Description |
|--------|-----------|----------|-------------|
| `new()` | `fn new(key: impl Into<String>) -> Self` | `manager.rs:41` | Creates a new empty session. |
| `add_message()` | `fn add_message(&mut self, role: impl Into<String>, content: impl Into<String>)` | `manager.rs:53` | Appends a message with auto-generated UUID and timestamp. |
| `add_message_with_request_id()` | `fn add_message_with_request_id(&mut self, role, content, request_id: Option<String>)` | `manager.rs:58` | Appends a message with an optional correlation ID. |
| `add_structured_message()` | `fn add_structured_message(&mut self, role, content, request_id, tool_calls, metadata)` | `manager.rs:77` | Appends a message with full structured data (tool calls, metadata). |
| `get_history()` | `fn get_history(&self, max_messages: usize) -> &[SessionMessage]` | `manager.rs:98` | Returns the most recent N messages as a slice. |
| `clear()` | `fn clear(&mut self)` | `manager.rs:104` | Removes all messages. |

### SessionMessage

**File:** `crates/session/src/manager.rs:112-137`

```rust
pub struct SessionMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub request_id: Option<String>,
    pub tool_calls: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}
```

**Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID v4, auto-generated via `generate_message_id()`. |
| `role` | `String` | Message role: `"system"`, `"user"`, `"assistant"`, or `"tool"`. |
| `content` | `String` | The message text body. |
| `timestamp` | `DateTime<Utc>` | When the message was created. |
| `request_id` | `Option<String>` | Optional correlation ID for tracing across systems. |
| `tool_calls` | `Option<Value>` | Structured tool call data (function name, arguments, result). Skipped in serialization when `None`. |
| `metadata` | `Option<Value>` | Extensible metadata (reasoning traces, content parts, etc.). Skipped in serialization when `None`. |

### SessionInfo

**File:** `crates/session/src/manager.rs:428-434`

```rust
pub struct SessionInfo {
    pub key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
}
```

Lightweight summary type returned by `SessionManager::list()`. Includes the session key, creation and last-update timestamps, and total message count.

### SessionCleanupService

**File:** `crates/agent/src/session_cleanup_service.rs:12-17`

```rust
pub struct SessionCleanupService {
    repo: SessionRepo,
    ttl_days: u32,
    interval: Duration,
    token: CancellationToken,
}
```

**Methods:**

| Method | Signature | Location | Description |
|--------|-----------|----------|-------------|
| `new()` | `fn new(repo: SessionRepo, ttl_days: u32, cleanup_interval_hours: u32, token: CancellationToken) -> Self` | `session_cleanup_service.rs:20` | Creates the service with TTL and interval configuration. |
| `spawn()` | `fn spawn(self)` | `session_cleanup_service.rs:35` | Consumes the service and spawns it as a background tokio task. |

The service runs on a `tokio::time::interval`. Each tick calls `SessionRepo::delete_stale_sessions(ttl_days)` which deletes sessions with `updated_at` older than the TTL. Logs deletions at `info` level, errors at `warn`. Shuts down when the `CancellationToken` is cancelled.

### Utility Functions

| Function | File | Description |
|----------|------|-------------|
| `accumulate_usage(total, cycle)` | `execution/types.rs:53` | Adds cycle token counts to a running total. |
| `reaction_to_satisfaction(emoji)` | `agent_loop/mod.rs:655` | Maps emoji reactions to satisfaction scores (1.0 for positive, 0.0 for negative, None for unrecognized). |
| `is_fabricated_tool_response(text, tool_names)` | `execution/core.rs:66` | Heuristic detection of LLMs that skip tool calls and generate fake structured results. |
