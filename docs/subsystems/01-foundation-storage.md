# Subsystem Analysis: Foundation & Storage (Layers 0-1.5)

> **Crates analyzed:** `common`, `config`, `bus`, `storage`
> **Analyst:** foundation-analyst
> **Date:** 2026-02-19

---

## Table of Contents

1. [common (Layer 0)](#1-common-layer-0)
2. [config (Layer 1)](#2-config-layer-1)
3. [bus (Layer 1)](#3-bus-layer-1)
4. [storage (Layer 1.5)](#4-storage-layer-15)
5. [Cross-Cutting Observations](#5-cross-cutting-observations)
6. [Gap Analysis & Recommendations](#6-gap-analysis--recommendations)

---

## 1. common (Layer 0)

**Location:** `crates/common/src/`
**Purpose:** Foundation types, error hierarchy, and shared utilities used across the entire workspace. Every other crate in the system depends on `common`.

### 1.1 Module Structure

```
common/src/
  lib.rs          — Re-exports: errors, types, prompts, utils
  error.rs        — Error hierarchy (KlyntbotError + 10 domain errors)
  types.rs        — Newtypes: ChannelName, ChatId, SessionKey, MessageRole
  prompts.rs      — Interactive question types for ask_user tool
  utils/
    mod.rs        — Re-exports all utility submodules
    date.rs       — Natural language date parsing
    helpers.rs    — format_timestamp_ms()
    notify.rs     — Cross-platform OS notifications
    stream_renderer.rs — Real-time LLM output + tool status rendering
    terminal/
      mod.rs      — Terminal utility re-exports
      colors.rs   — ANSI color constants + Unicode-aware display width
      spinners.rs — Braille animation spinner (thread-based)
      tables.rs   — Unicode-aware table drawing
      markdown.rs — Terminal markdown renderer
      boxes.rs    — Box drawing, code blocks, banners, wizard UI
```

### 1.2 Public API Surface

#### Error Hierarchy (`error.rs`)

The top-level error is `KlyntbotError` with 14 variants:

| Variant | Conversion | Source |
|---------|-----------|--------|
| `Bus(String)` | Manual | Message bus failures |
| `BusDisconnected` | Manual | Channel closed |
| `Tool(ToolError)` | `#[from]` auto | Tool execution |
| `Provider(ProviderError)` | `#[from]` auto | LLM provider |
| `Channel(ChannelError)` | `#[from]` auto | Chat platform |
| `Session(SessionError)` | `#[from]` auto | Session management |
| `Config(ConfigError)` | `#[from]` auto | Configuration |
| `Cron(CronError)` | `#[from]` auto | Cron scheduling |
| `Calendar(CalendarError)` | `#[from]` auto | Calendar sync |
| `Goal(GoalError)` | `#[from]` auto | Goal management |
| `Plan(PlanError)` | `#[from]` auto | Plan execution |
| `Storage(String)` | **Manual** (see note) | PostgreSQL storage |
| `Io(io::Error)` | `#[from]` auto | Filesystem I/O |
| `Json(serde_json::Error)` | `#[from]` auto | JSON serialization |

Each domain error has 3-6 variants specific to its domain. All use `thiserror` derive.

**Notable design:** `Storage(String)` is the only domain error that doesn't use `#[from]` auto-conversion. The `From<StorageError> for KlyntbotError` impl lives in the `storage` crate (not `common`), converting via `.to_string()`. This avoids making `common` depend on `storage`.

**Result alias:** `pub type Result<T> = std::result::Result<T, KlyntbotError>;`

#### Domain Types (`types.rs`)

Four newtypes replace raw strings for type safety:

| Type | Inner | Key Methods |
|------|-------|------------|
| `ChannelName(String)` | Channel identifier (e.g., "telegram") | `new()`, `as_str()`, `Display`, `From<String>`, `From<&str>` |
| `ChatId(String)` | Chat/conversation identifier | Same as above |
| `SessionKey(String)` | Composite key `"channel:chat_id"` | `new(&ChannelName, &ChatId)`, `from_parts(&str, &str)`, `split() -> Option<(ChannelName, ChatId)>` |
| `MessageRole` | Enum: System, User, Assistant, Tool | `Display`, `From<&str>` (unknown → User fallback) |

All types derive `Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize`.

#### Interactive Prompts (`prompts.rs`)

Types for the `ask_user` tool's structured question system:

- `InteractionRequest { title: String, questions: Vec<Question> }`
- `Question { id: String, title: String, text: String, answer_type: AnswerType }`
- `AnswerType` — enum: `SingleSelect(Vec<AnswerOption>)`, `MultiSelect(Vec<AnswerOption>)`, `YesNo`, `FreeText`
- `AnswerOption { value: String, label: String, description: Option<String> }`
- `Answer { question_id: String, value: AnswerValue }`
- `AnswerValue` — enum: `Selected(String)`, `MultiSelected(Vec<String>)`, `YesNo(bool)`, `Text(String)`, `Skipped`
- `FormResponse` — enum: `Completed(Vec<Answer>)`, `Cancelled`

All use `serde(rename_all = "snake_case")` for tagged enum serialization.

#### Utilities

| Module | Key Functions | Notes |
|--------|--------------|-------|
| `date.rs` | `parse_datetime(s, fallback_tz) -> Option<DateTime<Utc>>` | Supports RFC3339, ISO, date-only, "YYYY-MM-DD HH:MM", natural language (today, tomorrow, yesterday, next {weekday}, in N days/weeks). Uses `chrono` + `chrono_tz`. |
| `date.rs` | `format_datetime_local(dt, tz)`, `timezone_utc_offset(tz)` | Timezone-aware formatting |
| `helpers.rs` | `format_timestamp_ms(ms: i64) -> String` | Single utility function |
| `notify.rs` | `send_os_notification(title, body) -> io::Result<()>` | macOS (AppleScript with escaping), Linux (notify-send), Windows (PowerShell toast). Sanitizes inputs against injection. |
| `stream_renderer.rs` | `StreamRenderer` struct | Tracks tool status (Running/Completed/Failed), visual line counting with terminal wrapping, pause/resume for interactive prompts, cancel support. Special `format_tool_args()` for ask_user rendering. |
| `terminal/colors.rs` | 16 ANSI constants, `colorize()`, `display_width()`, `pad_to_width()`, `BoxChars` | `colors_enabled()` checks `NO_COLOR` env + TTY. `display_width()` is Unicode-aware using `unicode_width`. |
| `terminal/spinners.rs` | `Spinner` struct | 8 braille frames, thread-based 100ms animation, mutex-controlled, `Drop` cleanup |
| `terminal/tables.rs` | `draw_table(headers, rows)` | Unicode-aware column alignment via `display_width` |
| `terminal/markdown.rs` | `MarkdownRenderer::render(text)` | Code blocks, tables (→ `draw_table`), blockquotes, lists, headers, inline formatting |
| `terminal/boxes.rs` | `draw_box()`, `draw_code_block()`, `draw_banner()` | ASCII logo banner, wizard progress/headers, `display_error()` |

### 1.3 Error Handling Patterns

- All error types use `thiserror::Error` derive
- Domain errors auto-convert to `KlyntbotError` via `#[from]` (except `Storage`)
- Several domain errors also wrap `io::Error` and `serde_json::Error` with their own `#[from]` impls
- `PlanError` has structured variants: `ExecutionStalled { step_index, reason }`, `BacktrackLimitReached(usize)`
- `MessageRole::from("unknown")` silently falls back to `User` — no error on unrecognized role strings

### 1.4 Implementation Quality

**Strengths:**
- Clean separation of concerns — domain types in `types.rs`, errors in `error.rs`, prompts in `prompts.rs`
- Consistent newtype pattern with full trait implementations
- Input sanitization in `notify.rs` prevents injection attacks
- `display_width()` handles ANSI escape codes and Unicode correctly
- Thread-safe spinner with proper `Drop` cleanup

**Test coverage:**
- `error.rs`: 15 tests covering all error type display messages and all `From` conversions
- `types.rs`: 12 tests covering construction, display, from-conversions, serialization, equality
- Unit tests are comprehensive for the types they cover

---

## 2. config (Layer 1)

**Location:** `crates/config/src/`
**Purpose:** Configuration schema definition, file I/O, and environment variable overrides.

### 2.1 Module Structure

```
config/src/
  lib.rs          — Re-exports loader functions and schema types
  loader.rs       — File I/O (async + sync), env overrides, diff_json minimal save
  schema/
    mod.rs        — Re-exports + 23 integration tests
    core.rs       — Config root, Secret<T>, agent/tool/todo/calendar/learning config
    channels.rs   — 9 channel configuration structs
    providers.rs  — 12 provider configurations
```

### 2.2 Public API Surface

#### Secret\<T\> (`schema/core.rs`)

Wrapper that redacts sensitive values:

```rust
pub struct Secret<T>(T);  // #[serde(transparent)]
```

| Method | Purpose |
|--------|---------|
| `new(value)` | Construct |
| `expose() -> &T` | Access inner value |
| `into_inner() -> T` | Consume and extract |
| `is_empty() -> bool` | (String specialization) |

- `Debug` and `Display` print `[REDACTED]`
- `Default` for `Secret<String>` → empty string
- `serde(transparent)` serializes/deserializes as the inner type

#### Root Config (`schema/core.rs`)

```rust
pub struct Config {
    pub agents: AgentsConfig,
    pub channels: ChannelsConfig,
    pub providers: ProvidersConfig,
    pub tools: ToolsConfig,
    pub gateway: GatewayConfig,
    pub todo: TodoConfig,
    pub confidence: ConfidenceConfig,
    pub calendar: CalendarConfig,
    pub project: ProjectConfig,
    pub conversation: ConversationConfig,
    pub learning: LearningConfig,
    pub provider_manager: ProviderManagerConfig,
    pub timezone: String,           // auto-detected via iana_time_zone
    pub database_url: Option<String>,  // skip_serializing_if None
}
```

All fields have `#[serde(default)]` for graceful partial deserialization.

**Config methods:**

| Method | Purpose |
|--------|---------|
| `workspace_path()` | Expand `~` in workspace path |
| `active_provider_name()` | Explicit provider > auto-detect from API keys |
| `is_provider_configured(name)` | Check if provider has non-empty API key |
| `set_provider_key(name, key)` | Set API key by provider name string |
| `todo_store_path()` | Legacy JSONL path: `~/.klyntbot/todos.jsonl` |
| `embedding_store_path()` | Legacy JSONL path |
| `project_store_path()` | Legacy JSONL path |
| `goal_store_path()` | Legacy JSONL path |
| `plan_store_path()` | Legacy JSONL path: `~/.klyntbot/data/plans.jsonl` |
| `learning_outcomes_path()` | Legacy JSONL path |
| `learning_state_path()` | Legacy JSON path |

#### Agent Configuration

```rust
pub struct AgentDefaults {
    pub workspace: String,          // default: "~/.klyntbot/workspace"
    pub model: String,              // default: "anthropic/claude-opus-4-5"
    pub provider: Option<String>,   // explicit provider override
    pub max_tokens: u32,            // default: 8192
    pub temperature: f32,           // default: 0.7
    pub max_tool_iterations: u32,   // default: 20
}
```

#### Todo Configuration (6 sub-configs)

| Struct | Key Fields | Defaults |
|--------|-----------|----------|
| `TodoConfig` | notifications, focus, enrichment, search, daily_planning, creation_mode | All defaulted |
| `TodoNotificationConfig` | targets, focus_reminders, daily_digest, daily_digest_time | `["os_native"]`, true, true, "09:00" |
| `TodoFocusConfig` | max_slots, deadline_hours | 3, 18 |
| `TodoEnrichmentConfig` | enabled, auto_apply_threshold | true, 0.85 |
| `TodoSearchConfig` | enabled, semantic_threshold, embedding_model, rrf_k | true, 0.5, "paraphrase-multilingual-MiniLM-L12-v2", 60 |
| `DailyPlanningConfig` | enabled, planning_time | true, "08:00" |
| `CreationMode` | AskFirst, Yolo, Party | AskFirst |

`CreationMode` uses a custom `deserialize_creation_mode` function that gracefully falls back to `AskFirst` for unknown values.

#### Calendar Configuration (multi-provider)

`CalendarConfig` supports multiple simultaneous providers via a tagged enum:

```rust
pub enum CalendarProviderConfig {
    Apple(AppleCalendarConfig),     // iCloud CalDAV
    Google(GoogleCalendarConfig),   // OAuth2 + CalDAV
    GenericCalDav(GenericCalDavConfig), // Nextcloud, Fastmail, etc.
}
```

Serialized with `#[serde(tag = "type")]` → `{"type": "apple", ...}`.

**CalendarConfig helper methods:** `is_any_enabled()`, `enabled_providers()`, `find_provider(id)`, `apple()`, `google()`, `ensure_apple_mut()`, `ensure_google_mut()`, `min_sync_interval_secs()`.

#### Channels Configuration (9 channels)

| Channel | Key Fields | Special Defaults |
|---------|-----------|-----------------|
| `TelegramConfig` | enabled, token, allow_from, proxy | — |
| `DiscordConfig` | enabled, token, allow_from, gateway_url, intents | `wss://gateway.discord.gg/?v=10&encoding=json`, 37377 |
| `WhatsAppConfig` | enabled, bridge_url, allow_from | `ws://localhost:3001` |
| `SlackConfig` | enabled, bot_token, app_token, allow_from, mode, group_policy, dm | mode: "socket", group_policy: "none" |
| `EmailConfig` | 20 fields (IMAP + SMTP full config) | imap_port: 993, smtp_port: 587, poll_interval: 30s |
| `QQConfig` | enabled, app_id, secret, allow_from | — |
| `FeishuConfig` | enabled, app_id, app_secret, encrypt_key, verification_token, allow_from | — |
| `DingTalkConfig` | enabled, client_id, client_secret, allow_from | — |
| `MochatConfig` | enabled, base_url, socket_url, claw_token, agent_user_id, sessions, panels, allow_from | base_url: "https://mochat.io" |

All channel configs follow the pattern: `enabled` flag + credentials (Secret-wrapped) + `allow_from` list.

#### Providers Configuration (12 providers)

```rust
pub struct ProviderConfig {
    pub api_key: Secret<String>,
    pub api_base: Option<String>,      // Custom API endpoint
    pub extra_headers: HashMap<String, String>,
    pub native: bool,                   // Use native SDK vs OpenAI-compatible
    pub cache_system_prompt: bool,      // Anthropic prompt caching
    pub extended_thinking: ExtendedThinkingConfig,
}
```

Providers: anthropic, openai, openrouter, deepseek, gemini, groq, vllm, zhipu, dashscope, moonshot, minimax, aihubmix.

`ProviderManagerConfig { primary, fallback, classifier_model }` — for routing between providers.

#### Other Configuration

| Struct | Key Fields |
|--------|-----------|
| `ToolsConfig` | web (brave_api_key, max_results:5), exec (timeout:60, allowed_commands), restrict_to_workspace:false |
| `GatewayConfig` | host: "0.0.0.0", port: 18790 |
| `ConfidenceConfig` | threshold: 0.7, enabled: true, log_path: None |
| `LearningConfig` | enabled, analysis_interval_secs: 3600, min_threshold: 0.4, max_threshold: 0.9, min_outcomes_for_adaptation: 50 |
| `ConversationConfig` | embedding (enabled, exclude_channels, exclude_roles), search (enabled, semantic_threshold, max_results) |
| `ProjectConfig` | enabled: true |

### 2.3 Loader (`loader.rs`)

#### File I/O

| Function | Description |
|----------|-------------|
| `config_path() -> Result<PathBuf>` | `~/.klyntbot/config.json` |
| `config_dir() -> Result<PathBuf>` | `~/.klyntbot/` |
| `load() -> Result<Config>` | Async load; returns `Config::default()` if file missing |
| `save(config) -> Result<()>` | Async save with **minimal diff** (see below) |
| `load_sync() -> Result<Config>` | Sync variant for constructors, wizard, tests |
| `save_sync(config) -> Result<()>` | Sync variant |
| `load_with_env_overrides() -> Result<Config>` | Load + apply `KLYNTBOT_*` env vars |
| `init() -> Result<()>` | Create `~/.klyntbot/`, `sessions/`, `workspace/`, default config |
| `exists() -> bool` | Check if config file exists |

#### Minimal Save (diff_json)

`save()` serializes only non-default values:

1. Serialize current config and `Config::default()` to JSON Value
2. `diff_json(actual, default)` recursively prunes matching values
3. Empty objects are pruned
4. Result: only user-customized fields persist to disk

This means a config file that only has Anthropic configured will be ~5 lines, not hundreds.

#### Environment Variable Overrides

Macros: `env_string!`, `env_parse!`, `env_secret!`

Coverage: agent model/workspace/temperature/max_tokens, all 12 provider API keys, database_url, Telegram/Discord/Slack tokens, Brave API key.

Prefix: `KLYNTBOT_` with `__` as nesting separator.

### 2.4 Configuration Patterns

- All structs use `#[serde(rename_all = "camelCase")]`
- Every field has `#[serde(default)]` or `#[serde(default = "fn")]` for graceful partial deserialization
- Sensitive values wrapped in `Secret<T>` — redacted in Debug/Display
- Custom defaults via helper functions (e.g., `default_model()`, `default_gateway_port()`)
- `CreationMode` has graceful fallback deserialization for unknown values

### 2.5 Implementation Quality

**Strengths:**
- Minimal save prevents config bloat — clean user experience
- All fields have serde defaults — forward/backward compatible
- Secret redaction prevents accidental credential leaking
- `CreationMode` gracefully handles unknown enum values
- Comprehensive test coverage: 47 tests in `schema/mod.rs`, 25 tests in `loader.rs`, 13 tests in `channels.rs`, 21 tests in `core.rs`

**Test coverage highlights:**
- Default value verification for every config struct
- Serialization camelCase field name verification
- Round-trip serialization/deserialization
- Minimal save diff correctness (empty diff for defaults, only changes preserved)
- Environment variable override behavior
- Partial config deserialization (missing fields use defaults)

---

## 3. bus (Layer 1)

**Location:** `crates/bus/src/`
**Purpose:** Async message bus for decoupled channel ↔ agent communication. Provides typed event channels for inbound/outbound messages and learning system events.

### 3.1 Module Structure

```
bus/src/
  lib.rs              — Re-exports: InboundMessage, OutboundMessage, MessageBus, LearningEvent, LearningEventBus
  events.rs           — InboundMessage, OutboundMessage structs
  queue.rs            — MessageBus (dual mpsc channels)
  learning_events.rs  — LearningEvent, LearningEventBus (broadcast)
```

### 3.2 Public API Surface

#### InboundMessage (`events.rs`)

Message received from a chat channel:

```rust
pub struct InboundMessage {
    pub channel: ChannelName,
    pub sender_id: String,
    pub chat_id: ChatId,
    pub content: String,
    pub timestamp: DateTime<Utc>,       // default: Utc::now()
    pub media: Vec<String>,             // Media URLs
    pub metadata: HashMap<String, Value>, // Channel-specific metadata
}
```

| Method | Purpose |
|--------|---------|
| `new(channel, sender_id, chat_id, content)` | Constructor (impl Into for all params) |
| `session_key() -> SessionKey` | Derive session key from channel + chat_id |
| `validate() -> Result<(), String>` | Enforce 64KB max content size |

#### OutboundMessage (`events.rs`)

Message to send to a chat channel:

```rust
pub struct OutboundMessage {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub content: String,
    pub reply_to: Option<String>,       // Message ID to reply to
    pub media: Vec<String>,
    pub metadata: HashMap<String, Value>,
}
```

| Method | Purpose |
|--------|---------|
| `new(channel, chat_id, content)` | Constructor |
| `with_reply_to(message_id) -> Self` | Builder: set reply target |
| `with_media(url) -> Self` | Builder: add media URL |

#### MessageBus (`queue.rs`)

Dual-channel message bus using `tokio::mpsc`:

```rust
pub struct MessageBus {
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: Mutex<Option<mpsc::Receiver<InboundMessage>>>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    outbound_rx: Mutex<Option<mpsc::Receiver<OutboundMessage>>>,
}
```

| Method | Purpose |
|--------|---------|
| `new(buffer_size)` | Create bus with specified buffer |
| `take_inbound_rx() -> Option<Receiver>` | Take receiver (one-shot, Mutex-protected) |
| `take_outbound_rx() -> Option<Receiver>` | Take receiver (one-shot) |
| `publish_inbound(msg) -> Result<()>` | Send inbound message (returns `BusDisconnected` on failure) |
| `publish_outbound(msg) -> Result<()>` | Send outbound message |
| `inbound_sender() -> Sender` | Clone sender for distribution |
| `outbound_sender() -> Sender` | Clone sender for distribution |

**Design:** Receivers are wrapped in `Mutex<Option<>>` and taken exactly once via `take()`. This ensures single-consumer semantics — the agent loop takes the inbound receiver, the channel dispatcher takes the outbound receiver.

#### LearningEventBus (`learning_events.rs`)

Broadcast bus for adaptive learning events:

```rust
pub enum LearningEvent {
    ThresholdChanged { old_threshold: f32, new_threshold: f32, reason: String },
    AnalysisCompleted { total_outcomes: usize, suggested_threshold: f32 },
}
```

```rust
pub struct LearningEventBus {
    tx: broadcast::Sender<LearningEvent>,
}
```

| Method | Purpose |
|--------|---------|
| `new(capacity)` | Create bus (recommended: 16) |
| `publish(event)` | Send to all subscribers (no-op if none) |
| `subscribe() -> broadcast::Receiver` | Independent receiver per subscriber |

**Design difference from MessageBus:** Uses `tokio::sync::broadcast` instead of `mpsc`, enabling multiple independent subscribers (agent loop, future dashboards). `publish()` silently succeeds when no subscribers exist.

### 3.3 Implementation Quality

**Strengths:**
- Clean separation of concerns: events vs. bus vs. learning events
- Builder pattern on `OutboundMessage` for ergonomic media/reply construction
- `impl Into` on constructors for flexible parameter types
- 64KB message validation prevents memory abuse
- Broadcast bus for learning events enables fan-out without coupling

**Test coverage:** 20 tests in `queue.rs` covering:
- Basic publish/consume
- Multiple messages and ordering
- Sender cloning
- Inbound/outbound independence
- Buffer overflow behavior (backpressure)
- Concurrent publish/consume (50 messages across 10 publishers)
- Message ordering guarantees
- Empty content and special characters (Unicode, emoji)
- Different channels
- Session key consistency
- Reply-to and media builder methods

---

## 4. storage (Layer 1.5)

**Location:** `crates/storage/src/`
**Purpose:** PostgreSQL persistence layer using sqlx. Provides connection pooling with auto-migration, row structs for deserialization, and a repository pattern for all persistent data.

### 4.1 Module Structure

```
storage/src/
  lib.rs          — Re-exports: StoragePool, StorageError, Repos, all repos, all rows
  pool.rs         — StoragePool newtype around sqlx::PgPool
  error.rs        — StorageError (Sqlx, Migration, NotFound, Conflict)
  repos/
    mod.rs        — Repos aggregate struct + re-exports
    todo_repo.rs  — TodoRepo (most comprehensive — CRUD, focus, deps, attachments, time, templates)
    project_repo.rs — ProjectRepo (CRUD, filter, stats)
    session.rs    — SessionRepo (sessions + messages)
    embedding.rs  — EmbeddingRepo (pgvector operations)
    conv_embedding.rs — ConvEmbeddingRepo (conversation embeddings)
    goal.rs       — GoalRepo (goals + project links)
    plan.rs       — PlanRepo (plans + steps)
    outcome.rs    — OutcomeRepo (learning outcomes + enrichment feedback)
    strategy.rs   — StrategyRepo (strategy records + accuracy)
    usage.rs      — UsageRepo (usage tracking + aggregation)
    cron.rs       — CronRepo (cron job management)
    calendar_sync.rs — CalendarSyncRepo (sync state)
    tests/        — Shared test fixtures
  rows/
    mod.rs        — Re-exports all row structs
    todo.rs       — TodoRow, TodoAttachmentRow, TodoTimeEntryRow, TodoDependencyRow
    project.rs    — ProjectRow
    session.rs    — SessionRow, SessionMessageRow
    embedding.rs  — EmbeddingRow (with pgvector::Vector), ConvEmbeddingRow
    goal.rs       — GoalRow, GoalProjectLinkRow
    plan.rs       — PlanRow, PlanStepRow
    learning.rs   — OutcomeRow, StrategyRecordRow, EnrichmentFeedbackRow
    usage.rs      — UsageRecordRow
    calendar.rs   — CalendarSyncStateRow
    cron.rs       — CronJobRow
  migrations/
    20240101000000_initial.sql     — 17 tables, 7+ indexes, CHECK constraints
    20240101000001_pgvector.sql    — Conditional pgvector: embeddings + IVFFlat indexes
```

### 4.2 Connection Pool (`pool.rs`)

```rust
pub struct StoragePool(sqlx::PgPool);
```

| Method | Purpose |
|--------|---------|
| `connect(database_url) -> Result<Self>` | Connect + run all pending migrations |
| `connect_lazy(database_url) -> Result<Self>` | Deferred connection, no migrations. For tests/dual-mode constructors. |
| `inner() -> &PgPool` | Access underlying pool |

- Custom `Debug` impl using `finish_non_exhaustive()` (doesn't expose pool internals)
- Implements `Clone` (via `PgPool`'s internal `Arc`)

### 4.3 Error Type (`error.rs`)

```rust
pub enum StorageError {
    Sqlx(#[from] sqlx::Error),
    Migration(#[from] sqlx::migrate::MigrateError),
    NotFound(String),
    Conflict(String),
}
```

Manual `From<StorageError> for KlyntbotError` conversion:
```rust
impl From<StorageError> for common::KlyntbotError {
    fn from(e: StorageError) -> Self {
        common::KlyntbotError::Storage(e.to_string())
    }
}
```

This is the bridge between storage errors and the common error hierarchy, converting to a string representation to avoid circular dependencies.

### 4.4 Repos Aggregate (`repos/mod.rs`)

```rust
pub struct Repos {
    pool: sqlx::PgPool,
    pub todos: TodoRepo,
    pub projects: ProjectRepo,
    pub sessions: SessionRepo,
    pub goals: GoalRepo,
    pub plans: PlanRepo,
    pub embeddings: EmbeddingRepo,
    pub conv_embeddings: ConvEmbeddingRepo,
    pub outcomes: OutcomeRepo,
    pub strategies: StrategyRepo,
    pub usage: UsageRepo,
    pub cron: CronRepo,
    pub calendar_sync: CalendarSyncRepo,
}
```

`from_pool(&StoragePool)` clones `PgPool` for each repo. `pool()` provides direct pool access. All repos are `Clone + Send + Sync` because `PgPool` is `Arc`-based internally.

### 4.5 SQL Migrations

#### Migration 1: Initial Schema (`20240101000000_initial.sql`)

**17 tables created:**

| Table | Columns | Notes |
|-------|---------|-------|
| `projects` | id, title, description, status, tags, created_at, updated_at, archived_at | status CHECK, GIN index on tags |
| `todos` | 22 columns | status/priority CHECK, 7 indexes (status, project_id, parent_id, focus, due_date, GIN tags, created_at) |
| `todo_attachments` | id, todo_id, filename, url, content_type, created_at | FK → todos ON DELETE CASCADE |
| `todo_time_entries` | id, todo_id, started_at, ended_at, description | FK → todos ON DELETE CASCADE |
| `todo_dependencies` | id, todo_id, depends_on_id | FK × 2 → todos CASCADE, CHECK(todo_id ≠ depends_on_id), UNIQUE |
| `sessions` | id, channel, chat_id, created_at, updated_at | Unique on (channel, chat_id) |
| `session_messages` | id, session_id, role, content, tool_name, tool_result, metadata, created_at | FK → sessions CASCADE |
| `goals` | id, title, description, status, target_date, metrics, created_at, updated_at | — |
| `goal_project_links` | id, goal_id, project_id | FK × 2, UNIQUE |
| `plans` | id, title, description, status, session_key, goal_id, context, settings, backtrack_history, created_at, updated_at, completed_at | FK → goals SET NULL |
| `plan_steps` | id, plan_id, step_index, title, description, status, tool_hint, result, attempt_count, started_at, completed_at | FK → plans CASCADE |
| `learning_outcomes` | id, tool_name, action, confidence, was_correct, session_key, feedback, created_at | Index on created_at |
| `strategy_records` | id, strategy_name, predicted, actual, confidence, metadata, created_at | — |
| `enrichment_feedback` | id, todo_id, field_name, suggested, accepted, final_value, created_at | FK → todos CASCADE |
| `usage_records` | id, session_key, model, input_tokens, output_tokens, total_tokens, cost_usd, created_at | Index on created_at |
| `cron_jobs` | id, name, expression, command, enabled, last_run_at, next_run_at, created_at, updated_at | UNIQUE on name |
| `calendar_sync_state` | id, provider_id, last_sync_at, sync_token, state, created_at, updated_at | UNIQUE on provider_id |

**Key todo columns:** id, title, description, status (todo/doing/done/blocked/cancelled), priority (1-5), tags (TEXT[]), project_id, parent_id, focus (bool), focus_since, due_date, estimated_minutes, total_tracked_secs, template (bool), is_recurring, recurrence_rule, created_at, updated_at, completed_at.

#### Migration 2: pgvector (`20240101000001_pgvector.sql`)

**Conditional extension creation** — checks `pg_available_extensions` before attempting `CREATE EXTENSION`:

```sql
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_available_extensions WHERE name = 'vector') THEN
        CREATE EXTENSION IF NOT EXISTS vector;
        -- Create tables and indexes
    END IF;
END;
$$;
```

**Tables created (conditionally):**

| Table | Columns | Index |
|-------|---------|-------|
| `todo_embeddings` | id, todo_id (UNIQUE FK), embedding (vector(384)), model, created_at | IVFFlat (lists=100, cosine) |
| `conversation_embeddings` | id, session_key, message_index, content_hash, embedding (vector(384)), model, role, content_preview, created_at | IVFFlat (lists=100, cosine) |

384 dimensions matches the `paraphrase-multilingual-MiniLM-L12-v2` model output.

### 4.6 Repository Details

#### TodoRepo (most comprehensive)

**Filter and Patch types:**

```rust
pub struct TodoFilter {
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub project_id: Option<String>,
    pub priority_min: Option<i32>,
    pub limit: Option<i64>,
    pub templates_only: bool,
}

pub struct TodoPatch {  // Option<Option<T>> for nullable fields
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub priority: Option<Option<i32>>,
    pub tags: Option<Vec<String>>,
    pub project_id: Option<Option<String>>,
    pub parent_id: Option<Option<String>>,
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub estimated_minutes: Option<Option<i32>>,
    pub is_recurring: Option<bool>,
    pub recurrence_rule: Option<Option<String>>,
}

pub struct TodoSummary {
    pub todo: i64, pub doing: i64, pub done: i64, pub total: i64,
}
```

**CRUD methods:**

| Method | SQL Pattern | Notes |
|--------|------------|-------|
| `add(id, title, desc, status, priority, tags, project_id, parent_id, due_date, estimated, template, is_recurring, recurrence_rule)` | INSERT | 13 parameters |
| `get(id) -> Option<TodoRow>` | SELECT WHERE id = $1 | Returns None on miss |
| `get_or_err(id) -> Result<TodoRow>` | get() + StorageError::NotFound | Convenience wrapper |
| `update(id, patch) -> Result<TodoRow>` | UPDATE with COALESCE + boolean flags | `Option<Option<T>>` pattern for nullable fields — see note below |
| `delete(id) -> Result<()>` | DELETE WHERE id = $1 | Returns NotFound if 0 rows affected |
| `list(filter) -> Vec<TodoRow>` | Dynamic SQL with BindValue enum | See dynamic query section |
| `list_templates() -> Vec<TodoRow>` | WHERE template = true | — |
| `search_by_keyword(query) -> Vec<TodoRow>` | ILIKE on title + description | `%query%` pattern |

**Update pattern (COALESCE with nullable fields):**

The `update()` method uses a sophisticated pattern for handling `Option<Option<T>>`:
- `Option<String>` = field not in patch (keep current) vs. `Some("value")` (set)
- `Option<Option<String>>` = `None` (keep current) vs. `Some(None)` (set NULL) vs. `Some(Some("val"))` (set value)

This is implemented with paired boolean flags:
```sql
description = CASE WHEN $3 THEN $4 ELSE description END
```
Where `$3` is the "should update" flag and `$4` is the new value.

**Focus operations:**

| Method | Purpose |
|--------|---------|
| `focus(id, max_slots) -> Result<TodoRow>` | Set focus=true if under max_slots (atomic COUNT check) |
| `unfocus(id) -> Result<TodoRow>` | Set focus=false |
| `list_focused() -> Vec<TodoRow>` | WHERE focus = true ORDER BY focus_since |

**Dependency operations:**

| Method | Purpose |
|--------|---------|
| `add_dependency(todo_id, depends_on_id) -> Result<()>` | Insert with **recursive CTE cycle detection** |
| `remove_dependency(todo_id, depends_on_id) -> Result<()>` | Delete |
| `get_blockers(id) -> Vec<TodoDependencyRow>` | What this task depends on |
| `incomplete_blockers(id) -> Vec<TodoRow>` | Blockers not yet done |
| `get_blocking(id) -> Vec<TodoDependencyRow>` | What this task blocks |
| `get_dependencies(id) -> Vec<(TodoDependencyRow, TodoRow)>` | Full dependency + todo data |

**Cycle detection CTE:**
```sql
WITH RECURSIVE dep_chain(id) AS (
    SELECT depends_on_id FROM todo_dependencies WHERE todo_id = $2
    UNION
    SELECT td.depends_on_id FROM todo_dependencies td
    JOIN dep_chain dc ON td.todo_id = dc.id
)
SELECT EXISTS(SELECT 1 FROM dep_chain WHERE id = $1) as has_cycle
```

**Attachment operations:** `add_attachment`, `remove_attachment`, `list_attachments`

**Time tracking operations:**

| Method | Purpose |
|--------|---------|
| `add_time_entry(id, description) -> Result<TodoTimeEntryRow>` | Insert with started_at = now() |
| `close_time_entry(entry_id) -> Result<TodoTimeEntryRow>` | Set ended_at, **auto-update** todo.total_tracked_secs |
| `list_time_entries(todo_id) -> Vec<TodoTimeEntryRow>` | ORDER BY started_at DESC |

**Hierarchy operations:**

| Method | Purpose |
|--------|---------|
| `get_children(id) -> Vec<TodoRow>` | WHERE parent_id = $1 |
| `get_subtree(id) -> Vec<TodoRow>` | Recursive CTE for full subtree |
| `move_todo(id, new_parent_id) -> Result<TodoRow>` | With parent cycle detection via recursive CTE |
| `cascade_complete(id) -> Result<Vec<String>>` | Recursively mark self + all children as "done" |

**Other operations:**

| Method | Purpose |
|--------|---------|
| `summary() -> TodoSummary` | COUNT by status |
| `overdue() -> Vec<TodoRow>` | due_date < now() AND status NOT IN (done, cancelled) |
| `to_context_string(limit) -> String` | Format recent todos for LLM context injection |
| `add_template(id, title, desc, ...)` | Add with template=true |
| `delete_template(id) -> Result<()>` | Delete where template=true |

**Dynamic query building:**

`list()` builds SQL dynamically based on `TodoFilter` using a `BindValue` enum:

```rust
enum BindValue {
    Text(String),
    TextArray(Vec<String>),
    Int(i32),
    BigInt(i64),
    Bool(bool),
}
```

Conditions are accumulated in a `Vec<String>` and bind values in a `Vec<BindValue>`. The final query is assembled as a string, then executed with manual bind dispatching in a loop:

```rust
for (i, val) in binds.iter().enumerate() {
    query = match val {
        BindValue::Text(v) => query.bind(v.clone()),
        BindValue::TextArray(v) => query.bind(v.clone()),
        // ...
    };
}
```

This avoids a query builder library but requires careful parameter index management.

#### ProjectRepo

Similar pattern to TodoRepo but simpler. Has `ProjectFilter` (status, search term, archived flag, limit), `ProjectPatch`, `ProjectWithStats`. `ProjBindValue` enum for dynamic queries.

| Method | Purpose |
|--------|---------|
| `create(id, title, desc, status, tags)` | INSERT |
| `get/get_or_err/update/delete/archive` | Standard CRUD |
| `list(filter) -> Vec<ProjectRow>` | Dynamic SQL |
| `all() -> Vec<ProjectRow>` | No filter |
| `count_tasks_by_status(project_id) -> HashMap<String, i64>` | Aggregate todo counts |
| `get_with_stats(id) -> Option<ProjectWithStats>` | Project + task counts |

#### SessionRepo

| Method | Purpose |
|--------|---------|
| `create_session(id, channel, chat_id)` | INSERT ON CONFLICT DO UPDATE (upsert) |
| `get_session(id) -> Option<SessionRow>` | — |
| `list_sessions(channel, limit) -> Vec<SessionRow>` | Optional channel filter |
| `add_message(session_id, role, content, tool_name, tool_result, metadata)` | Insert + touch session updated_at |
| `get_messages(session_id) -> Vec<SessionMessageRow>` | All messages in order |
| `get_recent_messages(session_id, limit) -> Vec<SessionMessageRow>` | Subquery for correct ordering (SELECT from subquery with LIMIT, then re-ORDER ASC) |
| `compact_session(session_id, keep_last_n) -> Result<u64>` | DELETE messages except most recent N |
| `delete_session(id) -> Result<()>` | — |

#### EmbeddingRepo

| Method | Purpose |
|--------|---------|
| `upsert(todo_id, embedding: pgvector::Vector, model)` | INSERT ON CONFLICT DO UPDATE |
| `get(todo_id) -> Option<EmbeddingRow>` | — |
| `delete(todo_id) -> Result<()>` | — |
| `search_similar(embedding, threshold, limit) -> Vec<(EmbeddingRow, f64)>` | Cosine distance with threshold (converts similarity→distance) |
| `count() -> i64` | — |
| `bulk_upsert(items: Vec<(String, pgvector::Vector, String)>)` | Batch upsert |
| `upsert_vec(todo_id, embedding: &[f32], model)` | Convenience: `&[f32]` → `pgvector::Vector` |
| `search_similar_vec(embedding: &[f32], threshold, limit)` | Convenience wrapper |

**Threshold conversion:** The similarity threshold (0.0-1.0 where 1.0=identical) is converted to a distance threshold (1.0 - threshold) for pgvector's cosine distance operator `<=>`.

#### ConvEmbeddingRepo

Similar to EmbeddingRepo but for conversation embeddings:

| Method | Purpose |
|--------|---------|
| `insert(session_key, message_index, content_hash, embedding, model, role, content_preview)` | INSERT |
| `get(session_key, message_index) -> Option<ConvEmbeddingRow>` | — |
| `delete(session_key, message_index) -> Result<()>` | — |
| `search_similar(embedding, threshold, limit) -> Vec<(ConvEmbeddingRow, f64)>` | Cosine distance search |

#### GoalRepo

| Method | Purpose |
|--------|---------|
| `create(id, title, desc, status, target_date, metrics)` | INSERT |
| `get/list(status_filter)/update/delete` | Standard CRUD |
| `update_metrics(id, metrics: Value)` | Partial update |
| `link_project(goal_id, project_id)` | Insert link |
| `unlink_project(goal_id, project_id)` | Delete link |
| `get_project_links(goal_id) -> Vec<GoalProjectLinkRow>` | — |

#### PlanRepo

| Method | Purpose |
|--------|---------|
| `create(id, title, desc, status, session_key, goal_id, context, settings)` | INSERT |
| `get/update/delete` | Standard CRUD |
| `list(status, session_key, goal_id) -> Vec<PlanRow>` | Dynamic filter (3 optional params) |
| `update_status(id, new_status) -> Result<PlanRow>` | Sets `completed_at` for terminal states |
| `add_step(id, plan_id, step_index, title, desc, status, tool_hint)` | INSERT |
| `update_step(id, status, result, attempt_count, started_at, completed_at)` | UPDATE |
| `get_steps(plan_id) -> Vec<PlanStepRow>` | ORDER BY step_index |

#### OutcomeRepo

| Method | Purpose |
|--------|---------|
| `create(id, tool_name, action, confidence, was_correct, session_key, feedback)` | INSERT |
| `list_by_date_range(from, to) -> Vec<OutcomeRow>` | — |
| `list_by_tool(tool_name) -> Vec<OutcomeRow>` | — |
| `count_stats() -> (i64, i64, i64)` | (total, correct, incorrect) |
| `create_enrichment_feedback(id, todo_id, field, suggested, accepted, final_value)` | INSERT |
| `list_enrichment_feedback(todo_id) -> Vec<EnrichmentFeedbackRow>` | — |

#### StrategyRepo

| Method | Purpose |
|--------|---------|
| `create(id, strategy_name, predicted, actual, confidence, metadata)` | INSERT |
| `get(id) -> Option<StrategyRecordRow>` | — |
| `list_by_strategy(name) -> Vec<StrategyRecordRow>` | — |
| `get_accuracy(name) -> f64` | predicted==actual fraction |
| `list_by_date_range(from, to) -> Vec<StrategyRecordRow>` | — |

#### UsageRepo

| Method | Purpose |
|--------|---------|
| `create(id, session_key, model, input_tokens, output_tokens, total_tokens, cost_usd)` | INSERT |
| `aggregate_by_model() -> Vec<(model, total_input, total_output, total_tokens, total_cost, count)>` | — |
| `aggregate_by_day() -> Vec<(date, total_tokens, total_cost, count)>` | — |
| `totals_since(since) -> (total_tokens, total_cost, count)` | — |

#### CronRepo

| Method | Purpose |
|--------|---------|
| `upsert(id, name, expression, command, enabled)` | INSERT ON CONFLICT (name) DO UPDATE |
| `get(id) -> Option<CronJobRow>` | — |
| `list() -> Vec<CronJobRow>` | All jobs |
| `list_active() -> Vec<CronJobRow>` | WHERE enabled = true |
| `set_enabled(id, enabled) -> Result<CronJobRow>` | — |
| `update_run_state(id, last_run_at, next_run_at) -> Result<CronJobRow>` | — |
| `delete(id) -> Result<()>` | — |

#### CalendarSyncRepo

| Method | Purpose |
|--------|---------|
| `get(provider_id) -> Option<CalendarSyncStateRow>` | — |
| `upsert(id, provider_id, sync_token, state)` | INSERT ON CONFLICT (provider_id) DO UPDATE |
| `list() -> Vec<CalendarSyncStateRow>` | — |
| `delete(provider_id) -> Result<()>` | — |

### 4.7 Row Structs

All row structs derive `Debug, Clone, sqlx::FromRow, Serialize, Deserialize` with `#[serde(rename_all = "camelCase")]`.

**Notable row types:**

- `TodoRow`: 22 columns including `tags: Vec<String>` (PostgreSQL TEXT[]), `focus: bool`, `template: bool`
- `EmbeddingRow`: contains `embedding: pgvector::Vector` — the only struct with a pgvector type
- `PlanRow`: contains `context: Option<Value>`, `settings: Option<Value>`, `backtrack_history: Option<Value>` — JSON columns
- `SessionMessageRow`: contains `metadata: Option<Value>` for arbitrary channel-specific data

### 4.8 Implementation Quality

**Strengths:**
- Auto-migration on connect prevents schema drift
- Conditional pgvector migration — system works without the extension
- Repos aggregate provides clean access pattern
- `PgPool` is `Clone+Send+Sync` — no `Arc<RwLock<>>` needed
- `connect_lazy()` for tests and dual-mode constructors
- Recursive CTE cycle detection for dependencies and parent moves
- COALESCE pattern for partial updates preserves unmodified fields
- `close_time_entry` auto-updates `total_tracked_secs` in the same operation
- `get_recent_messages` uses subquery pattern for correct ordering with LIMIT

**Patterns worth noting:**
- Dynamic SQL with `BindValue` enum — works but requires manual index tracking
- `Option<Option<T>>` for nullable field patches — correct but complex
- All repos clone `PgPool` — cheap (`Arc` clone) but creates many handles
- `cascade_complete` uses recursive CTE for hierarchical completion

---

## 5. Cross-Cutting Observations

### 5.1 Dependency Flow

```
common ← config ← bus ← storage
  ↑         ↑        ↑       ↑
  └─────────┴────────┴───────┘── (all upper layers depend on these)
```

- `common` has zero internal workspace dependencies
- `config` depends on `common` (for error types)
- `bus` depends on `common` (for types: ChannelName, ChatId, SessionKey, KlyntbotError)
- `storage` depends on `common` (for KlyntbotError conversion)

### 5.2 Serde Consistency

All configuration and row structs consistently use `#[serde(rename_all = "camelCase")]`. JSON field names are camelCase throughout. This is verified by dedicated tests that check serialized output.

### 5.3 Error Conversion Chain

```
StorageError → KlyntbotError::Storage(String)  [manual, via to_string()]
ConfigError  → KlyntbotError::Config            [#[from] auto]
ToolError    → KlyntbotError::Tool              [#[from] auto]
...
```

The storage→common error bridge intentionally uses string conversion to avoid a dependency from `common` on `storage`.

### 5.4 Secret Handling

All sensitive values (API keys, tokens, passwords) use `Secret<String>` with:
- `[REDACTED]` in Debug/Display — prevents accidental log leaking
- `expose()` for explicit access — makes credential usage intentional
- `serde(transparent)` — serializes/deserializes as the inner type

---

## 6. Gap Analysis & Recommendations

### 6.1 Legacy JSONL Paths (P1 — Medium Priority)

**Issue:** `Config` still exposes 7 legacy `*_store_path()` methods that point to JSONL flat files:
- `todo_store_path()` → `~/.klyntbot/todos.jsonl`
- `embedding_store_path()` → `~/.klyntbot/todos_embeddings.jsonl`
- `project_store_path()` → `~/.klyntbot/projects.jsonl`
- `goal_store_path()` → `~/.klyntbot/goals.jsonl`
- `plan_store_path()` → `~/.klyntbot/data/plans.jsonl`
- `learning_outcomes_path()` → `~/.klyntbot/data/outcomes.jsonl`
- `learning_state_path()` → `~/.klyntbot/data/learning_state.json`

**Context:** All persistent data has been migrated to PostgreSQL. These paths may still be referenced by migration code or unused callsites.

**Recommendation:** Audit all callers. If truly unused, deprecate with `#[deprecated]` first, then remove in a follow-up release.

### 6.2 StorageError → KlyntbotError Conversion (P2 — Low Priority)

**Issue:** `Storage(String)` variant converts via `.to_string()`, losing the structured error variant. A `StorageError::NotFound("todo-123")` becomes `KlyntbotError::Storage("Not found: todo-123")` — the upstream handler can't distinguish NotFound from Conflict without string parsing.

**Recommendation:** Consider making `Storage(#[from] StorageError)` in `KlyntbotError`. This requires `common` depending on `storage` (or moving `StorageError` to `common`). Alternative: add `StorageError` to `common` alongside other domain errors, then use `#[from]` in both `common::KlyntbotError` and `storage::StorageError`.

### 6.3 Dynamic Query Builder (P2 — Low Priority)

**Issue:** `TodoRepo::list()` and `ProjectRepo::list()` build SQL strings manually with `BindValue` enum dispatching. This is correct but fragile:
- Parameter indices are tracked manually (risk of off-by-one)
- No compile-time verification of generated SQL
- Each new filter condition requires adding a variant to `BindValue`

**Recommendation:** Consider using a lightweight query builder (e.g., `sea-query` or a custom macro) for complex dynamic queries. This would eliminate manual index management while keeping the repository pattern.

### 6.4 MessageBus Single-Consumer Enforcement (P3 — Informational)

**Issue:** `take_inbound_rx()` / `take_outbound_rx()` return `Option` and can only be called once. If called twice, the second call returns `None`. The caller must handle this, but there's no compile-time enforcement.

**Current mitigation:** The `Mutex<Option<>>` pattern is a well-known Rust idiom and works correctly at runtime.

### 6.5 `MessageRole` Silent Fallback (P3 — Informational)

**Issue:** `MessageRole::from("unknown_value")` silently returns `User`. This could mask bugs where an unexpected role string is passed.

**Recommendation:** Consider using `TryFrom` with an error variant, or at least log a warning on fallback.

### 6.6 Bus Message Validation Gap (P3 — Informational)

**Issue:** `InboundMessage::validate()` checks content size (64KB max) but is not called automatically by `publish_inbound()`. Callers must remember to validate before publishing.

**Recommendation:** Either call `validate()` inside `publish_inbound()` or document that validation is the caller's responsibility.

### 6.7 Missing `#[cfg(test)]` Module in Some Crates (P3 — Informational)

**Issue:** The `bus/learning_events.rs` and `storage/pool.rs` files have no inline tests. Their behavior is tested indirectly through other test modules, but dedicated unit tests would increase confidence.

### 6.8 Timezone Auto-Detection (P3 — Informational)

**Issue:** `Config::default()` calls `iana_time_zone::get_timezone()` which performs system calls. This means creating a default config in tests has a side effect (system timezone lookup). If the system timezone changes during a long-running process, the config won't reflect it (it's captured at construction time).

**Current mitigation:** This is standard practice and unlikely to cause issues. Just worth noting for test reproducibility.

### 6.9 Embedding Dimension Hardcoded (P3 — Informational)

**Issue:** The pgvector migration hardcodes `vector(384)` dimensions. If the embedding model changes (e.g., to a 768-dim model), a new migration would be needed.

**Recommendation:** The config stores `embedding_model` but the SQL schema doesn't adapt to it. Document this coupling clearly. Consider a migration strategy for model upgrades.

---

*End of Foundation & Storage Layer Analysis*
