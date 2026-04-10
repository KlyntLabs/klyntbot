# Core Infrastructure

Foundation crates (L0-L1) that every other crate in the workspace depends on. Understanding these is prerequisite to contributing anywhere in the codebase.

```
L0  common              Error hierarchy, domain newtypes, utilities
L1  config              Configuration schema, loading, hot-reload
L1  bus                 Event buses (domain, message, context, learning)
L1  tools-core          Tool/FeaturePackage traits, registry, permissions
L1  tools-core-macros   Proc macros for Tool, ToolParams, DomainEnum
L2  storage             SQLite pool, migrations, repos, vector store
```

Dependencies flow strictly upward: `storage` may depend on `tools-core` (for `FeatureMigration`), but never on `agent` or `cognitive`.

---

## 1. common -- Error Types & Core Types

`crates/common/` -- the zero-dependency foundation. Every crate imports `use common::Result`.

### Error hierarchy

```rust
/// Root error -- all domain errors auto-convert via From impls.
pub enum KlyntbotError {
    Bus(String),
    BusDisconnected,
    Tool(#[from] ToolError),
    Provider(#[from] ProviderError),
    Channel(#[from] ChannelError),
    Session(#[from] SessionError),
    Config(#[from] ConfigError),
    Cron(String),
    Storage(String),
    StorageNotFound(String),
    StorageConflict(String),
    Io(#[from] std::io::Error),
    Json(#[from] serde_json::Error),
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, KlyntbotError>;
```

Sub-error types with their variants:

| Error type      | Variants                                                 |
|-----------------|----------------------------------------------------------|
| `ToolError`     | `NotFound`, `InvalidParams`, `ExecutionFailed`, `PermissionDenied` |
| `ProviderError` | `Http`, `InvalidResponse`, `RateLimited { provider, retry_after: Option<u64> }`, `AuthFailed { provider, config_key }` |
| `ChannelError`  | `ConnectionFailed`, `SendFailed`, `InvalidConfig`        |
| `SessionError`  | `NotFound`, `LoadFailed`, `SaveFailed`, `Io`, `Json`     |
| `ConfigError`   | `NotFound`, `Invalid`, `MissingField`, `Io`, `Json`      |

The `storage` crate has its own `StorageError` (Sqlx, Migration, NotFound, Conflict, Vector) with a `From<StorageError> for KlyntbotError` impl that maps `NotFound` to `StorageNotFound` and `Conflict` to `StorageConflict`.

### Domain newtypes

```rust
pub struct ChannelName(String);   // "telegram", "discord", "cli", "mcp"
pub struct ChatId(String);        // Chat identifier within a channel
pub struct SessionKey(String);    // "channel:chat_id" composite

pub enum MessageRole { System, User, Assistant, Tool }
pub enum AppMode { Desktop, Server }
```

`SessionKey` is constructed from `ChannelName` + `ChatId` and can be split back:

```rust
let key = SessionKey::new(&ChannelName::new("telegram"), &ChatId::new("123456"));
assert_eq!(key.as_str(), "telegram:123456");
let (channel, chat_id) = key.split().unwrap();
```

Well-known channel constants: `SYSTEM_CHANNEL`, `CLI_CHANNEL`, `MCP_CHANNEL`.

### Utilities

| Function | Purpose |
|----------|---------|
| `truncate_at_boundary(s, max_bytes)` | UTF-8-safe truncation to byte limit |
| `truncate_chars(s, max_chars, suffix)` | Unicode-aware truncation with suffix |
| `shared_http_client()` | Process-wide `reqwest::Client` via `OnceLock` (avoids 8+ connection pools) |
| `build_http_client(timeout)` | One-off client with custom timeout |
| `cosine_similarity(a, b)` | f32 vector similarity (NaN-safe) |
| `extract_json_array(s)` / `extract_json_object(s)` | Pull JSON from LLM prose output |
| `strip_llm_fences(s)` | Remove `` ```json `` / `` ``` `` wrappers |

### Ports

```rust
#[async_trait]
pub trait NotificationSender: Send + Sync {
    async fn send(&self, title: &str, body: &str) -> Result<()>;
}
```

Defined in L0 so lower layers can consume it. Implemented in `desktop` (Layer 7) where the OS notification API lives.

---

## 2. config -- Configuration & Hot-Reload

`crates/config/` -- schema definition, file I/O, environment overrides, hot-reload.

### Config struct

Root struct with 30+ nested sections. All fields `#[serde(default)]` so partial JSON files merge cleanly with defaults.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub agents: AgentsConfig,
    pub channels: ChannelsConfig,       // telegram, discord, slack, email
    pub providers: ProvidersConfig,     // anthropic, openai, openrouter, deepseek, gemini, ...
    pub tools: ToolsConfig,
    pub todo: TodoConfig,
    pub finance: FinanceConfig,
    pub learning: LearningConfig,
    pub notes: NotesConfig,
    pub productivity: ProductivityConfig,
    pub cognitive: CognitiveConfig,
    pub mcp: McpConfig,
    pub packs: PacksConfig,
    pub conversation: ConversationConfig,
    pub confidence: ConfidenceConfig,
    pub autotuner: AutoTunerConfig,
    pub user: UserConfig,
    pub shortcuts: ShortcutsConfig,
    pub embedding: EmbeddingConfig,
    // ... and more
    pub schema_version: u32,
}
```

### Secret<T>

```rust
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret<T>(T);
```

- `Debug` and `Display` print `[REDACTED]`
- Access via `.expose()` / `.into_inner()`
- `.is_empty()` on `Secret<String>`
- Used for all API keys and tokens in config

### Loading and saving

| Function | Signature | Notes |
|----------|-----------|-------|
| `load()` | `async fn load() -> Result<Config>` | From `{KLYNTBOT_HOME}/config.json`, defaults if missing |
| `load_sync()` | `fn load_sync() -> Result<Config>` | Blocking variant for constructors/tests |
| `save(config)` | `async fn save(&Config) -> Result<()>` | **Minimal save** -- only persists fields that differ from `Config::default()` |
| `save_sync(config)` | `fn save_sync(&Config) -> Result<()>` | Blocking variant |
| `load_with_env_overrides()` | `async fn` | Loads `.env`, applies `KLYNTBOT_*` overrides |
| `config_dir()` | `fn -> Result<PathBuf>` | `KLYNTBOT_HOME` env var or `~/.klyntbot/` |
| `init()` | `async fn` | Creates directory structure + workspace templates |

The minimal-save approach uses a recursive `diff_json()` against `Config::default()`, producing a JSON object containing only user-customized fields. This keeps the config file clean and makes defaults upgradeable.

### Schema versioning

```rust
const CURRENT_SCHEMA_VERSION: u32 = 1;

fn migrate_config(raw: Value, from: u32, to: u32) -> Result<Value> {
    // Future migrations match on `from < N` and apply changes
    raw["schemaVersion"] = json!(to);
    Ok(raw)
}
```

When loading, if the file version is less than `CURRENT_SCHEMA_VERSION`, migrations run and the config is re-saved automatically.

### Environment overrides

Pattern: `KLYNTBOT_SECTION__SUBSECTION__FIELD`

```bash
KLYNTBOT_AGENTS__DEFAULTS__MODEL=anthropic/claude-sonnet-4-5
KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-ant-...
KLYNTBOT_DATA_DIR=/custom/data
```

A `.env` file at the project root is auto-loaded via `dotenvy`. `KLYNTBOT_HOME` controls the data directory (config, sessions, workspace, DB, lance, plugins, personas).

### Hot-reload

```rust
pub struct HotConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub max_tool_iterations: u32,
    pub safety_timeout_secs: u64,
    pub monthly_budget_usd: Option<f64>,
}
```

Shared as `Arc<RwLock<HotConfig>>` between `AppCore` and the agent pipeline. A file watcher calls `reload_if_changed()` which:

1. Checks mtime (skips if unchanged)
2. Re-parses the config file
3. Extracts `HotConfig::from(&config)`
4. Computes `HotConfigDiff` against the previous snapshot
5. Updates the shared `Arc<RwLock>` if anything changed

`HotConfigDiff` has boolean flags per field (`model_changed`, `temperature_changed`, etc.) and a `has_changes()` aggregate.

---

## 3. bus -- Event Architecture

`crates/bus/` -- four orthogonal communication channels, all async.

### DomainEventBus

Broadcast channel (`tokio::sync::broadcast`) for cross-feature communication. Multiple subscribers each receive every event independently.

```rust
pub struct DomainEventBus {
    tx: broadcast::Sender<DomainEvent>,
}

impl DomainEventBus {
    pub fn new(capacity: usize) -> Self;
    pub fn publish(&self, event: DomainEvent);
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent>;
    pub fn subscriber_count(&self) -> usize;
}
```

Shared as `Arc<DomainEventBus>`. Feature crates publish events without knowing about consumers. The cognitive layer subscribes to all events for pattern extraction, fact discovery, and coaching.

`DomainEvent` is a large enum (85+ variants) organized by domain:

| Domain | Example variants |
|--------|-----------------|
| Productivity | `ActivitySessionCompleted`, `FocusSessionStarted`, `FocusSessionEnded`, `DistractionDetected`, `ProductivityScoreComputed` |
| Tasks | `TaskCreated`, `TaskCompleted { task_id, actual_duration_mins, deviation_pct }`, `TaskDeferred`, `TaskDecomposed`, `TaskStatusChanged`, `TaskExecutionProgress` |
| Finance | `TransactionRecorded { category, amount, is_over_budget }`, `BudgetAlert` |
| Notes | `NoteCreated`, `NoteUpdated`, `NoteContentChanged`, `NoteEditingFinished`, `NoteDeleted` |
| Learning | `UserCorrectedAI { original, correction, kind, strength }`, `KnowledgeAtomCreated`, `AtomFlashcardReviewed`, `FlashcardSessionCompleted` |
| Coaching | `CoachingFeedback`, `CoachingPatternDetected` |
| Agent | `SkillRouted`, `ChatTurnCompleted`, `ToolCallExecuted` |
| Autotuner | `TrialActivated`, `AutotunerDecision` |
| Mirror | `MirrorTrialKilled`, `MirrorSnippetCreated` |
| Lifecycle | `SystemWillSleep`, `SystemDidWake`, `UserBecameIdle`, `UserReturned` |
| Fabric | `CommunityDiscovered`, `CommunityUpdated`, `CommunityWeakened` |

Each event has a `variant_name() -> &'static str` (zero-allocation) and a `domain() -> &str` method for categorization.

Supporting types:
- `CorrectionKind`: `Reaction`, `KeywordPrefix`, `MemoryMiss`
- `FeedbackResponse`: `Helpful`, `Dismissed`, `StopSuggesting`

### MessageBus

MPSC channels (`tokio::sync::mpsc`) for bidirectional channel-to-agent communication.

```rust
pub struct MessageBus {
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: Mutex<Option<mpsc::Receiver<InboundMessage>>>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    outbound_rx: Mutex<Option<mpsc::Receiver<OutboundMessage>>>,
}
```

Key characteristics:
- `take_inbound_rx()` / `take_outbound_rx()` -- one-time consumption (prevents multiple consumers)
- `publish_inbound()` validates `MAX_MESSAGE_SIZE` (64 KB) before sending
- `inbound_sender()` / `outbound_sender()` return cloneable sender handles
- `InboundMessage` includes: `channel`, `sender_id`, `chat_id`, `content`, `timestamp`, `media`, `metadata`, `kind` (Text/Reaction/Voice)
- `OutboundMessage` includes: `channel`, `chat_id`, `content`, `reply_to`, `media`, `metadata`
- `InboundMessage::session_key()` derives `SessionKey` from `channel:chat_id`

### ContextUpdateQueue

Priority-based mid-execution context injection for the LiveContextRefresher.

```rust
pub struct ContextUpdateQueue {
    inner: Mutex<VecDeque<ContextUpdate>>,
}

pub struct ContextUpdate {
    pub reason: ContextUpdateReason,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub priority: UpdatePriority,  // Low, Normal, High
    pub timestamp: DateTime<Utc>,
}
```

- `push()` with 30-second deduplication by `(reason, content)`
- `drain()` atomically takes all pending updates
- MAX_PENDING = 200 (oldest dropped on overflow)
- Reasons: `MemoryPromoted`, `FocusSessionStarted`, `FocusSessionEnded`, `DistractionDetected`, `BudgetThresholdCrossed`, `NoteStructureChanged`, `CommunityDiscovered`, `CommunityUpdated`, `CommunityWeakened`, `Custom(String)`

### LearningEventBus

Separate broadcast bus for behavioral signal collection.

```rust
pub enum LearningEvent {
    ThresholdChanged { old_threshold: f32, new_threshold: f32, reason: String },
    AnalysisCompleted { total_outcomes: usize, suggested_threshold: f32 },
}
```

Used by `LearningService` to notify the agent loop and dashboards when the adaptive confidence threshold changes.

---

## 4. storage -- SQLite + Vector Store

`crates/storage/` -- persistence layer. All state lives in SQLite (relational) + LanceDB (vectors).

### StoragePool

```rust
#[derive(Clone)]
pub struct StoragePool(sqlx::SqlitePool);
```

| Method | Description |
|--------|-------------|
| `connect(data_dir)` | Opens `{data_dir}/data.db`, enables WAL + FK + busy_timeout, runs migrations |
| `connect_in_memory()` | In-memory DB with migrations (tests + fallback) |
| `from_existing(pool)` | Wraps a pre-migrated pool (skips migrations) |
| `run_feature_migrations(pool, migrations)` | Runs `FeatureMigration` entries not yet applied |
| `optimize()` | `PRAGMA optimize` for query stats (call on shutdown) |
| `inner()` | Access underlying `sqlx::SqlitePool` |

SQLite PRAGMAs applied on connect:
- `journal_mode=WAL` -- concurrent readers
- `foreign_keys=ON`
- `busy_timeout=5000` -- 5s retry on lock contention
- `cache_size=-2000` -- ~2MB per connection (single-user app)
- `wal_autocheckpoint=1000` -- prevent unbounded WAL growth
- Max 5 connections

Feature migrations are tracked in `_feature_migrations` table. Each migration runs inside an explicit transaction (SQL + tracking INSERT) to prevent partial application.

### StorageError

```rust
pub enum StorageError {
    Sqlx(#[from] sqlx::Error),
    Migration(String),
    NotFound(String),
    Conflict(String),
    Vector(String),
}
```

The `OptionExt` trait adds `.ok_or_not_found(label)` to `Option<T>` for ergonomic NotFound conversion:

```rust
fetch_optional(...).await?.ok_or_not_found("task abc-123")?;
```

### Repos aggregate

```rust
pub struct Repos {
    pool: sqlx::SqlitePool,
    pub tasks: TaskRepo,
    pub projects: ProjectRepo,
    pub areas: AreaRepo,
    pub sessions: SessionRepo,
    pub objectives: ObjectiveRepo,
    pub key_results: KeyResultRepo,
    pub finance: FinanceStorage,
    pub usage: UsageRepo,
    pub cron: CronRepo,
    pub tool_usage: ToolUsageRepo,
    pub session_context: SessionContextRepo,
    pub session_memory: SessionMemoryRepo,
    pub interaction_log: InteractionLogRepo,
    pub learning_state: LearningStateRepo,
    // ... 25+ repos total
}

impl Repos {
    pub fn from_pool(pool: &StoragePool) -> Self;
    pub fn pool(&self) -> &sqlx::SqlitePool;
    pub async fn cleanup_analytics(&self) -> Result<u64, StorageError>;
}
```

All repos are `Clone + Send + Sync` (wrapping `SqlitePool` which is `Arc`-based). Construct once via `Repos::from_pool()`, then clone freely.

### Repository pattern conventions

- Each repo wraps `SqlitePool` with a `new(pool)` constructor
- Standard CRUD methods: `create()`, `get_by_id()`, `update()`, `delete()`, `list()`
- Partial updates use `*Patch` structs: `Option<T>` means "don't touch", `Option<Option<T>>` means "set or clear a nullable field"
- Row structs: `#[derive(FromRow, Serialize)]` with `#[serde(rename_all = "camelCase")]`
- Specialized queries per domain (e.g., `TaskRepo::list_by_status()`, `FinanceTransactionRepo::sum_by_category()`)

### Vector store

`VectorStore` wraps LanceDB for embedding-based retrieval. Stored separately from SQLite at `{data_dir}/lance/`. Used by the cognitive layer for semantic fact retrieval, conversation search, entity embeddings, and knowledge fabric communities.

---

## 5. tools-core -- Tool Framework

`crates/tools-core/` -- traits, registry, permissions, and parameter extraction for the tool system.

### Core traits

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;           // JSON Schema
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Standard }
    fn metadata(&self) -> ToolMetadata { ToolMetadata::default() }
    fn custom_timeout(&self) -> Option<Duration> { None }
    fn to_schema(&self) -> Value;            // OpenAI function-calling format
    fn validate_params(&self, params: &Value) -> Vec<String>;
}

pub type DynTool = Arc<dyn Tool>;
```

The typed layer sits on top:

```rust
pub trait ToolParams: Sized {
    fn json_schema() -> Value;
    fn from_args(args: Value) -> Result<Self>;
}

#[async_trait]
pub trait ToolExecute: Send + Sync {
    type Params: ToolParams;
    async fn execute(&self, params: Self::Params, ctx: &RoutingContext) -> Result<String>;
}
```

`#[derive(Tool)]` bridges `ToolExecute` to the untyped `Tool` trait automatically.

### ToolRegistry

```rust
pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
    metadata: HashMap<String, ToolMetadata>,
    usage_counts: Mutex<HashMap<String, u64>>,
    cached_definitions: Mutex<Option<Arc<Vec<Value>>>>,
    permissions: Option<ToolPermissions>,
}
```

Key methods:

| Method | Description |
|--------|-------------|
| `register(tool)` | Add a tool (invalidates definition cache) |
| `register_dyn(Arc<dyn Tool>)` | Add a pre-wrapped dynamic tool |
| `get(name)` | Look up by name (returns cloned `Arc`) |
| `execute(name, params, ctx)` | Look up + permission check + validate + execute |
| `prepare(name, params, ctx)` | Look up + check + validate without executing (avoids holding borrow during execution) |
| `get_definitions()` | All tool schemas, cached as `Arc<Vec<Value>>` |
| `unregister_by_prefix(prefix)` | Bulk remove (e.g., `"mcp_linear_"` for MCP server cleanup) |
| `record_usage(name)` | Increment usage counter (interior mutability) |
| `top_used(n)` | Most-used tools sorted by count |
| `by_category(category)` | Filter tool names by `ToolCategory` |

### RoutingContext

Passed to every `Tool::execute()` call. Carries channel/chat identity and optional interaction channels.

```rust
pub struct RoutingContext {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub interaction_tx: Option<mpsc::Sender<InteractionBundle>>,
    pub is_direct_mode: bool,
    pub delegation_depth: u32,
    pub entity_tx: Option<mpsc::Sender<EntityCard>>,
    pub interaction_channel: Option<Arc<dyn InteractionChannel>>,
    pub squad_id: Option<String>,
    pub champion_params: Option<TrialParams>,
}
```

Constructors: `RoutingContext::new()` (non-interactive), `with_interaction()` (CLI/dashboard), `with_squad()` (multi-persona).

### PermissionLevel

```rust
pub enum PermissionLevel {
    ReadOnly  = 0,   // read_file, list_dir, web_search
    Standard  = 1,   // todo, project, memory
    Elevated  = 2,   // exec, write_file, edit_file
    Admin     = 3,   // spawn
}
```

`ToolPermissions` maps channel names to levels. When set on the registry, every `execute()` / `prepare()` call checks `channel level >= tool level`.

### ToolMetadata

```rust
pub struct ToolMetadata {
    pub category: ToolCategory,  // General, FileSystem, Search, Web, TaskManagement, Memory, Finance, Productivity, System, Mcp, Plugin
    pub tags: Vec<String>,
    pub source: ToolSource,      // Native, Feature(name), Mcp(server), Plugin(name), External
    pub cost_hint: CostHint,     // Free, Low, Medium, High, Variable
}
```

### ParamExtractor

Zero-allocation wrapper for extracting typed values from `serde_json::Value`:

```rust
let p = ParamExtractor::new(&args);
let path = p.required_str("path")?;         // Err if absent or wrong type
let limit = p.i64_or("limit", 10)?;         // Default if absent, Err if wrong type
let tag = p.optional_str("tag")?;           // Ok(None) if absent
let due = p.clearable_str("due_date")?;     // None=don't touch, Some(None)=clear, Some(Some(v))=set
let tags = p.string_array_or_empty("tags")?;
```

### FeaturePackage

```rust
#[async_trait]
pub trait FeaturePackage: Send + Sync {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<DynTool>;
    fn migrations(&self) -> Vec<FeatureMigration>;
    fn config_key(&self) -> &str;
    fn default_config(&self) -> Value;
    async fn health_check(&self) -> Result<HealthStatus> { Ok(HealthStatus::Healthy) }
}
```

Feature crates implement this trait. The agent builder discovers packages and registers their tools + runs their migrations automatically.

### ToolOutput

Opt-in structured output for tools that want to return data alongside a summary:

```rust
pub enum ToolOutput {
    Text(String),
    Structured { summary: String, data: serde_json::Value },
}
```

Backward compatible -- `String` auto-converts to `ToolOutput::Text` via `From`.

---

## 6. tools-core-macros -- Derive Macros

`crates/tools-core-macros/` -- proc macros that eliminate boilerplate for tool definitions.

### #[derive(Tool)]

Generates the full `Tool` trait implementation from metadata attributes + a `ToolExecute` impl.

```rust
#[derive(Tool)]
#[tool(
    name = "read_file",
    description = "Read a file from the filesystem",
    params = "ReadFileParams",
    permission = "read_only",      // optional: read_only, standard, elevated, admin
    category = "FileSystem",       // optional: maps to ToolCategory
    tags = "file,read,content",    // optional: comma-separated
    cost = "Free"                  // optional: Free, Low, Medium, High, Variable
)]
struct ReadFileTool { /* dependencies */ }

#[async_trait]
impl ToolExecute for ReadFileTool {
    type Params = ReadFileParams;
    async fn execute(&self, params: ReadFileParams, ctx: &RoutingContext) -> Result<String> {
        // business logic
    }
}
```

The macro generates the `Tool` trait bridge: `name()`, `description()`, `parameters()` (delegates to `ToolParams::json_schema()`), `execute()` (parses args via `ToolParams::from_args()` then calls `ToolExecute::execute()`), plus `permission_level()` and `metadata()` overrides.

### #[derive(ToolParams)]

Generates `ToolParams` trait impl (JSON Schema + parsing) from a struct.

```rust
#[derive(ToolParams)]
struct ReadFileParams {
    /// File path to read
    #[param(required)]
    path: String,

    /// Maximum number of lines
    #[param(default = "100")]
    max_lines: i64,

    /// Optional encoding override
    #[param]
    encoding: Option<String>,
}
```

- `#[param(required)]` -- field is required in the JSON Schema
- `#[param]` -- optional field
- `#[param(default = "value")]` -- optional with default
- Doc comments become schema `description` fields
- Unit structs generate an empty schema

### #[derive(ActionParams)]

Same as `ToolParams` but generates inherent methods instead of a trait impl. Used for multi-action tool dispatch where each action has its own params struct.

### #[tool_actions]

Multi-action dispatch: routes an `action` parameter to different methods, each with their own params type.

```rust
#[tool_actions(
    name = "tasks",
    description = "Task management tool",
    category = "TaskManagement"
)]
impl TaskTool {
    async fn create(&self, params: CreateParams, ctx: &RoutingContext) -> Result<String> { ... }
    async fn list(&self, params: ListParams, ctx: &RoutingContext) -> Result<String> { ... }
    async fn update(&self, params: UpdateParams, ctx: &RoutingContext) -> Result<String> { ... }
}
```

Generates a `Tool` impl that:
1. Reads `args["action"]` as the dispatch key
2. Merges JSON Schemas from all action params into a single schema with `action` as a required enum field
3. Routes to the correct method

### #[derive(DomainEnum)]

Generates loose string parsing with aliases for domain enums.

```rust
#[derive(DomainEnum)]
enum Priority {
    #[aliases("p0", "critical", "highest")]
    Urgent,
    #[aliases("p1")]
    High,
    #[aliases("p2")]
    Medium,
    #[aliases("p3")]
    Low,
    #[canonical("none")]
    NoPriority,
}
```

Generates:
- `as_str()` -- canonical snake_case (or `#[canonical("...")]` override)
- `from_str_loose(s)` -- matches canonical name + all aliases (case-insensitive)
- `Display` impl

---

## 7. Design Patterns Summary

| Pattern | Where | How |
|---------|-------|-----|
| **Newtype** | `common::types` | `ChannelName(String)`, `ChatId(String)`, `SessionKey(String)` -- prevent mixing up string parameters |
| **Error hierarchy** | `common::error` | Root `KlyntbotError` with `#[from]` sub-errors, one `Result<T>` type everywhere |
| **Hot-reload** | `config::hot` | `HotConfig` subset in `Arc<RwLock>`, file watcher triggers diff + update |
| **Event sourcing** | `bus::domain_events` | `DomainEventBus` (broadcast) for decoupled cross-feature communication |
| **Repository** | `storage::repos` | One repo per table, all `Clone+Send+Sync`, aggregate via `Repos::from_pool()` |
| **Feature packages** | `tools_core::feature` | Self-contained: tools + migrations + config + health per feature crate |
| **Proc macros** | `tools-core-macros` | `#[derive(Tool)]`, `#[derive(ToolParams)]`, `#[tool_actions]` eliminate Tool boilerplate |
| **Interior mutability** | `ToolRegistry` | `Mutex<Option<Arc<Vec<Value>>>>` for cached definitions, `Mutex<HashMap>` for usage counts |
| **Dependency inversion** | `tools_core::routing` | Port traits (`ProgressHandler`, `InteractionChannel`, `NotificationSender`) defined in L0-L1, implemented in L5-L7 |
| **Minimal persistence** | `config::loader` | `save()` diffs against defaults, only writes changed fields |

---

## 8. Key Files

```
crates/common/src/
    error.rs           KlyntbotError, ToolError, ProviderError, etc.
    types.rs           ChannelName, ChatId, SessionKey, MessageRole, AppMode
    helpers.rs         truncate_at_boundary, cosine_similarity, extract_json_*
    http.rs            shared_http_client, build_http_client
    ports/notification.rs  NotificationSender trait

crates/config/src/
    schema/core.rs     Config struct, Secret<T>, expand_tilde
    schema/hot.rs      HotConfig, HotConfigDiff
    schema/mod.rs      All config section modules
    loader.rs          load, save, init, reload_if_changed, diff_json
    env.rs             load_with_env_overrides, KLYNTBOT_* env vars

crates/bus/src/
    domain_events.rs   DomainEvent enum (85+ variants), DomainEventBus
    events.rs          InboundMessage, OutboundMessage, MAX_MESSAGE_SIZE
    queue.rs           MessageBus (MPSC inbound/outbound)
    context_updates.rs ContextUpdateQueue, ContextUpdate, UpdatePriority
    learning_events.rs LearningEvent, LearningEventBus

crates/storage/src/
    pool.rs            StoragePool (SQLite connect, WAL, migrations)
    error.rs           StorageError, OptionExt
    repos/mod.rs       Repos aggregate (25+ repos)
    vector_store/      LanceDB wrapper for embeddings

crates/tools-core/src/
    lib.rs             Tool, ToolParams, ToolExecute, DynTool, ToolOutput
    registry.rs        ToolRegistry (HashMap + cached definitions)
    feature.rs         FeaturePackage, FeatureMigration, HealthStatus
    permissions.rs     PermissionLevel, ToolPermissions
    metadata.rs        ToolCategory, ToolMetadata, CostHint, ToolSource
    params.rs          ParamExtractor
    routing.rs         RoutingContext, ProgressHandler, InteractionChannel

crates/tools-core-macros/src/
    lib.rs             Proc macro entry points
    tool_derive.rs     #[derive(Tool)]
    tool_params.rs     #[derive(ToolParams)]
    tool_actions.rs    #[tool_actions]
    action_params.rs   #[derive(ActionParams)]
    domain_enum.rs     #[derive(DomainEnum)]
```

---

See also: [agent-runtime.md](agent-runtime.md) (execution pipeline, skill routing), [features.md](features.md) (feature package catalog).
