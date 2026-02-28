# Tools System

The tools system spans three crates that together provide a complete framework for building, registering, and executing agent tools with JSON Schema validation, permission enforcement, and proc-macro-driven boilerplate elimination.

| Crate | Layer | Purpose |
|-------|-------|---------|
| `tools-core` | 3 | Trait definitions, registry, permissions, search, pagination, proc macro re-exports |
| `tools-core-macros` | 3 | Proc macros: `#[tool_actions]`, `#[derive(ActionParams)]`, `#[derive(DomainEnum)]` |
| `tools` | 3 | 20+ tool implementations, embedding infrastructure, handler trait definitions |

---

## Section 1: Narrative Overview

### Tool Trait Design

The `Tool` trait (`tools-core/src/lib.rs:119-156`) is the central abstraction. Every tool — whether a filesystem reader, a web scraper, or a multi-action domain tool — implements five methods:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;              // JSON Schema
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;
    fn permission_level(&self) -> PermissionLevel { PermissionLevel::Standard }
}
```

The design makes several deliberate choices:

1. **`Value` in, `String` out.** Arguments arrive as `serde_json::Value` (matching what LLM function-calling APIs produce), and results return as plain strings (what LLMs consume). No typed request/response structs at the trait boundary -- that would force every caller to know every tool's types.

2. **JSON Schema for parameters.** Each tool declares its parameter schema as a `Value`, which is used for both LLM function-calling format (`to_schema()`) and runtime validation (`validate_params()`). The built-in validator at `tools-core/src/lib.rs:162-353` handles type checking, required fields, enum constraints, min/max, pattern matching, `oneOf`, `anyOf`, and nested object/array validation.

3. **Permission level as a method.** Rather than metadata, permission level is a trait method with a default (`Standard`). Tools override it to declare their sensitivity. This keeps the permission model close to the tool definition.

4. **`Send + Sync` bound.** Tools are stored as `Arc<dyn Tool>` (`DynTool` type alias at line 159) and shared across async tasks. The bound ensures thread safety.

### ToolRegistry

The `ToolRegistry` (`tools-core/src/registry.rs:12-142`) is a `HashMap<String, DynTool>` with cached schema generation and permission enforcement.

**Registration** accepts either concrete tools (`register()` wraps in `Arc`) or pre-wrapped `DynTool` (`register_dyn()` for `FeaturePackage` tools). Each registration invalidates the definition cache.

**Lookup and execution** (`execute()` at line 83) performs three steps in sequence:
1. Look up the tool by name, returning `ToolError::NotFound` if missing.
2. Check permissions — if a `ToolPermissions` is configured, compare the tool's `permission_level()` against the channel's granted level.
3. Validate parameters against the tool's JSON Schema.
4. Call `tool.execute()`.

**Definition caching** (`get_definitions()` at line 68) uses a `Mutex<Option<Arc<Vec<Value>>>>` for interior mutability. The first call builds and caches all tool schemas; subsequent calls return a cheap `Arc::clone`. Cache invalidation happens automatically on register/unregister.

### Proc Macros

The `tools-core-macros` crate provides three proc macros that eliminate boilerplate in action-based tools and domain enums.

#### `#[tool_actions]` (attribute macro)

Defined at `tools-core-macros/src/tool_actions.rs`. Applied to an `impl` block, it generates the entire `Tool` trait implementation:

```rust
#[tool_actions(name = "test_tool", description = "A test tool")]
impl MyTool {
    #[action(name = "greet")]
    async fn handle_greet(&self, params: GreetParams, ctx: &RoutingContext) -> Result<String> {
        Ok(format!("Hello, {}!", params.name))
    }
}
```

The macro:
- Generates `name()` and `description()` from the attribute arguments.
- Collects all `#[action(name = "...")]` methods and builds a `match action { ... }` dispatcher in the generated `execute()`.
- Merges all action param struct schemas into a single `parameters()` method with an `"action"` enum field listing all action names.
- Strips `#[action]` attributes from the emitted methods so they compile cleanly.

#### `#[derive(ActionParams)]` (derive macro)

Defined at `tools-core-macros/src/action_params.rs`. Generates `json_schema()` and `from_value()` methods for parameter structs:

```rust
#[derive(ActionParams)]
pub struct AddParams {
    /// Task title
    #[param(required)]
    pub title: String,

    /// Priority (1-5)
    #[param(min = 1, max = 5)]
    pub priority: Option<u8>,

    /// Tags
    pub tags: Vec<String>,
}
```

Supported `#[param(...)]` attributes: `required`, `min`, `max`, `min_length`, `max_length`. Doc comments become JSON Schema `"description"` fields. Type mapping: `String` to `"string"`, `Option<T>` to the inner type (nullable), `Vec<String>` to `"array"` of strings, `bool` to `"boolean"`, integer types to `"integer"`, float types to `"number"`.

The generated `from_value()` extracts each field from a `&Value`, returning `Err(String)` for missing required fields and defaulting optional/vec fields to `None`/empty.

#### `#[derive(DomainEnum)]` (derive macro)

Defined at `tools-core-macros/src/domain_enum.rs`. Generates `as_str()`, `from_str_loose()`, `Display`, and `FromStr` for unit enums:

```rust
#[derive(DomainEnum)]
pub enum TodoStatus {
    #[aliases("pending", "open")]
    Todo,
    #[canonical("in-progress")]
    InProgress,
    Done,
}
```

- `as_str()`: Returns the canonical string (PascalCase auto-converted to snake_case, or overridden via `#[canonical("...")]`).
- `from_str_loose()`: Case-insensitive matching against canonical name and all `#[aliases("...")]`.
- `Display`: Delegates to `as_str()`.
- `FromStr`: Delegates to `from_str_loose()`, returning an error string for unknown values.

### RoutingContext

`RoutingContext` (`tools-core/src/lib.rs:68-116`) is the execution context passed to every tool call. Rather than using shared mutable state, each tool receives an explicit snapshot of where the request came from and what capabilities are available:

| Field | Type | Purpose |
|-------|------|---------|
| `channel` | `ChannelName` | Source channel (telegram, discord, cli, etc.) |
| `chat_id` | `ChatId` | Source chat/conversation identifier |
| `interaction_tx` | `Option<mpsc::Sender<InteractionBundle>>` | CLI/dashboard oneshot channel for `ask_user` |
| `is_direct_mode` | `bool` | When true, `message` tool returns inline instead of publishing to bus |
| `entity_tx` | `Option<mpsc::Sender<EntityCard>>` | Entity card emission channel for dashboard UI |
| `interaction_channel` | `Option<Arc<dyn InteractionChannel>>` | Platform-native UI (Telegram buttons, Discord selects) |

Two constructors: `new()` for bus-driven (non-interactive) contexts, and `with_interaction()` for CLI/dashboard with user interaction support.

### InteractionChannel

`InteractionChannel` (`tools-core/src/lib.rs:46-60`) is a trait defined at Layer 3 to break the circular dependency between tools (Layer 3) and channels (Layer 4). Channel implementations that support structured UI (Telegram inline keyboards, Discord buttons/selects, Slack Block Kit) implement this trait:

```rust
#[async_trait]
pub trait InteractionChannel: Send + Sync {
    fn supports_interaction(&self) -> bool;
    async fn send_interaction(&self, chat_id: &str, request: &InteractionRequest) -> Result<FormResponse>;
}
```

The `ask_user` tool checks for this in priority order: CLI oneshot channel, then platform-native interaction channel, then text fallback.

### Permission System

Permissions (`tools-core/src/permissions.rs`) use a four-level hierarchy:

| Level | Value | Examples |
|-------|-------|---------|
| `ReadOnly` | 0 | `read_file`, `list_dir`, `web_search`, `glob`, `grep` |
| `Standard` | 1 | `todo`, `project`, `calendar`, `memory`, `agent_task` |
| `Elevated` | 2 | `write_file`, `edit_file`, `browser` |
| `Admin` | 3 | `spawn` |

`ToolPermissions` stores a per-channel permission map plus a default level. The check (`is_allowed()`) grants access when the channel's level >= the tool's required level. When no `ToolPermissions` is configured on the registry, all tools are allowed (backward-compatible).

### Searchable Trait and Hybrid Search (RRF)

The `Searchable` trait (`tools-core/src/search.rs:9-12`) requires a single method `search_id() -> &str` for identifying items in search results.

The `rrf_merge()` function (`tools-core/src/search.rs:27-75`) implements Reciprocal Rank Fusion, merging ranked lists from keyword and semantic search sources. The formula: `score(d) = sum(1 / (k + rank_i + 1))`. Items appearing in both lists get a `"both"` source label, enabling the caller to show provenance. The `k` parameter (typically 60) controls how much weight top-ranked items receive.

In the `tools` crate, `SearchResult` (`tools/src/search_utils.rs:14-36`) is an enum over `Todo` and `ConversationEmbeddingRecord` that implements `Searchable`, enabling unified cross-domain search in the `MemoryTool`.

### Tool Implementations

#### Filesystem Tools

**`read_file`** (`tools/src/filesystem.rs:70-129`) -- Read file contents. Permission: `ReadOnly`. Params: `path` (required). Supports optional directory restriction via `FsToolBase`. Uses `shellexpand::tilde` for `~` expansion and `canonicalize()` for path resolution.

**`write_file`** (`tools/src/filesystem.rs:132-201`) -- Write content to a file, creating parent directories as needed. Permission: `Elevated`. Params: `path`, `content` (both required).

**`edit_file`** (`tools/src/filesystem.rs:204-294`) -- Find-and-replace in a file. Rejects if `old_text` is not found or appears more than once (requires unique match). Permission: `Elevated`. Params: `path`, `old_text`, `new_text` (all required).

**`list_dir`** (`tools/src/filesystem.rs:297-379`) -- List directory contents with type indicators. Permission: `ReadOnly`. Params: `path` (required).

Convenience functions: `register_fs_tools()` registers all four; `register_fs_read_tools()` registers only `read_file` and `list_dir` (used by research/analyst sub-agent profiles).

#### Search Tools

**`glob`** (`tools/src/glob_tool.rs:15-112`) -- Find files by glob pattern, sorted by modification time (most recent first). Permission: `ReadOnly`. Uses `globset` for pattern compilation and `walkdir` for recursive traversal.

**`grep`** (`tools/src/grep.rs:17-191`) -- Search file contents by regex pattern. Permission: `ReadOnly`. Supports glob-based file filtering, configurable `max_results` (default 20), and `context_lines` (0-5). Runs filesystem I/O on a blocking thread via `spawn_blocking`.

#### Web Tools

**`web_search`** (`tools/src/web.rs:14-139`) -- Search via Brave Search API. Params: `query` (required), `count` (1-10). Returns formatted titles, URLs, and snippets.

**`web_fetch`** (`tools/src/web.rs:142-287`) -- Fetch a URL and extract readable content. Handles JSON (pretty-prints), HTML (converts to text via `html2text`), and plain text. Supports `extract_mode` and `max_chars` truncation.

#### Browser Tool

**`browser`** (`tools/src/browser.rs:67-520`) -- Full browser automation via the `agent-browser` CLI subprocess. Permission: `Elevated`. 14 actions: `navigate`, `snapshot`, `click`, `type`, `fill`, `press`, `scroll`, `wait`, `get_text`, `screenshot`, `eval`, `fill_form`, `login_flow`, `submit_and_confirm`.

Implements a trust-level guard system based on `TrustLevel` (from config):
- **Full**: No guards, all actions allowed.
- **Autonomous** (default): Guards dangerous write actions (clicks on "submit"/"buy"/"delete", payment field fills).
- **Strict**: Guards all click/fill/type/submit actions.

Guarded actions return a `[CONFIRMATION_REQUIRED]` message instructing the LLM to use `ask_user` before proceeding.

#### Communication Tools

**`message`** (`tools/src/message.rs:14-89`) -- Send messages to channels via the outbound bus. In direct mode (CLI/dashboard), returns content inline instead of publishing. Params: `content` (required), `channel`, `chat_id` (optional overrides).

**`ask_user`** (`tools/src/ask_user.rs:18-130`) -- Interactive clarification system supporting 4 question types: `single_select`, `multi_select`, `yes_no`, `free_text`. Groups 1-4 questions per call. Three response paths: CLI oneshot channel, platform-native interaction, and text fallback. Produces rich semantic responses with full question context for the LLM.

#### Delegation Tools

**`spawn`** (`tools/src/spawn.rs:34-146`) -- Spawn background subagents. Permission: `Admin`. Actions: `spawn`, `cancel`, `status`. Uses dependency inversion via `SpawnHandler` trait (defined in tools, implemented in agent). Supports 3 profiles: `general`, `research`, `analyst`.

**`cron`** (`tools/src/cron_tool.rs:67-244`) -- Schedule recurring tasks via `CronHandler`. Actions: `add` (with `every_seconds` or `cron_expr`), `list`, `remove`. Emits entity cards on creation.

**`agent_task`** (`tools/src/agent_task_tool.rs:28-120`) -- Subagent task board coordination (only registered in subagent tool registries). Permission: `Standard`. Actions: `list`, `claim`, `update`, `complete`, `fail`. Uses `AgentTaskHandler` for dependency inversion.

#### Domain Tools

All domain tools follow the same dependency inversion pattern: a handler trait is defined in the tools crate (Layer 3) and implemented in the agent crate (Layer 5), injected as `Arc<dyn Handler>`.

**`calendar`** (`tools/src/calendar_tool.rs:57-160`) -- CalDAV calendar operations. Actions: `sync`, `list_events`, `create_event`, `status`, `push_all`, `pull_all`. Handler: `CalendarHandler`.

**`goal`** (`tools/src/goal_tool.rs:37-291`) -- Strategic goal management spanning multiple projects. Actions: `create`, `list`, `show`, `update`, `delete`, `progress`, `decompose`, `status`, `metrics`. Handler: `GoalHandler`.

**`plan`** (`tools/src/plan_tool.rs:138-351`) -- Multi-step execution plan management. Actions: `create` (with LLM step preview and user approval flow), `show`, `approve`, `abandon`, `status`, `execute`. Handler: `PlanHandler`. The create flow uses `ask_user`-style interaction to get approval before persisting.

**`project`** (`tools/src/project_tool.rs:16-284`) -- Project management backed by `ProjectRepo` and `TodoRepo`. Actions: `create`, `list`, `show`, `update`, `archive`, `tasks`.

**`learning`** (`tools/src/learning_tool.rs:64-138`) -- Learning system insights (confidence thresholds, per-tool success rates, outcome statistics). Actions: `status`, `analyze`, `history`. Handler: `LearningHandler`.

**`memory`** (`tools/src/memory_tool.rs:20-513`) -- Semantic search over conversation history and unified cross-domain search (todos + conversations via RRF). Actions: `search_conversations`, `search_all`, `purge`, `status`. Supports configurable threshold and RRF k parameter.

### Embedding Infrastructure

#### EmbeddingEngine

`EmbeddingEngine` (`tools/src/embedding_engine.rs:26-197`) wraps `fastembed::TextEmbedding` with lazy initialization. The model (`paraphrase-multilingual-MiniLM-L12-v2`, 384 dimensions, ~420MB) downloads on first use. Feature-gated: when compiled without `semantic-search`, all embed methods return errors.

Key methods: `embed()` (single text), `embed_batch()` (multiple texts), `embed_async()` (blocking thread pool wrapper taking `Arc<Self>`), `cosine_similarity()` (static, handles NaN).

#### EmbeddingHandler Trait

`EmbeddingHandler` (`tools/src/embedding_engine.rs:203-213`) provides dependency inversion for embedding operations:

```rust
#[async_trait]
pub trait EmbeddingHandler: Send + Sync {
    async fn embed_todo(&self, todo: &Todo) -> Result<Option<EmbeddingRecord>>;
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;
    fn is_available(&self) -> bool;
}
```

`EmbeddingEngineImpl` (`tools/src/embedding_engine.rs:219-296`) is the production implementation, composing `Arc<EmbeddingEngine>` (vector generation) with `VectorStore` (LanceDB persistence).

#### EmbeddingStore

`EmbeddingStore` (`tools/src/embedding_store.rs:25-66`) is a lightweight in-memory `HashMap<String, EmbeddingRecord>` cache. Persistence is handled by `storage::VectorStore` (LanceDB). Used for quick lookups via `ids_missing_embeddings()`.

### Pagination

`Page<T>` (`tools-core/src/pagination.rs:7-38`) is a generic cursor-based pagination container with `items`, `cursor` (opaque next-page token), and `has_more`. Constructors: `new()`, `empty()`, `single_page()`.

### ConfigPersistence

`ConfigPersistence` (`tools-core/src/config_persistence.rs:14-21`) is a trait for runtime config section reads/writes without depending on the config crate. Features receive it as `Arc<dyn ConfigPersistence>`.

### FeaturePackage Trait

`FeaturePackage` (`tools-core/src/feature.rs:29-50`) is the integration point for self-contained feature crates (like `feature-todo`). Each feature provides its name, tools, SQL migrations, config section key/defaults, and an optional health check. The agent discovers features and registers their tools automatically.

### ParamExtractor

`ParamExtractor` (`tools-core/src/params.rs:15-191`) is a zero-cost wrapper around `&Value` that provides ergonomic parameter extraction:

- **Required extractors** (`required_str`, `required_i64`, `required_u64`, `required_bool`, `required_array`, `required_object`): Return `Err` if absent or wrong type.
- **Optional extractors** (`optional_str`, `optional_i64`, `optional_u64`, `optional_f64`, `optional_bool`, `optional_array`): Return `Ok(None)` if absent, `Err` if present but wrong type.
- **Default extractors** (`str_or`, `i64_or`): Return a default if absent, `Err` if present but wrong type.
- **Utility** (`string_array_or_empty`): Extract string values from an optional JSON array.

---

## Section 2: API Reference

### Tool Trait

**File:** `crates/tools-core/src/lib.rs:119-156`

| Method | Signature | Notes |
|--------|-----------|-------|
| `name` | `fn name(&self) -> &str` | Tool identifier for function calls |
| `description` | `fn description(&self) -> &str` | Human-readable description |
| `parameters` | `fn parameters(&self) -> Value` | JSON Schema for parameters |
| `execute` | `async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>` | Execute with args and routing context |
| `permission_level` | `fn permission_level(&self) -> PermissionLevel` | Default: `Standard` |
| `to_schema` | `fn to_schema(&self) -> Value` | OpenAI function-calling format |
| `validate_params` | `fn validate_params(&self, params: &Value) -> Vec<String>` | Validate against JSON Schema |

### ToolRegistry

**File:** `crates/tools-core/src/registry.rs:12-142`

| Method | Signature | Notes |
|--------|-----------|-------|
| `new` | `fn new() -> Self` | Empty registry |
| `set_permissions` | `fn set_permissions(&mut self, permissions: ToolPermissions)` | Configure permission enforcement |
| `register` | `fn register(&mut self, tool: impl Tool + 'static)` | Register a concrete tool (wraps in Arc) |
| `register_dyn` | `fn register_dyn(&mut self, tool: DynTool)` | Register pre-wrapped dynamic tool |
| `unregister` | `fn unregister(&mut self, name: &str)` | Remove tool by name |
| `get` | `fn get(&self, name: &str) -> Option<DynTool>` | Look up tool by name |
| `has` | `fn has(&self, name: &str) -> bool` | Check if registered |
| `get_definitions` | `fn get_definitions(&self) -> Arc<Vec<Value>>` | Cached OpenAI schemas |
| `execute` | `async fn execute(&self, name: &str, params: Value, ctx: &RoutingContext) -> Result<String>` | Permission check + validate + execute |
| `tool_names` | `fn tool_names(&self) -> Vec<String>` | All registered names |
| `len` | `fn len(&self) -> usize` | Count of tools |
| `is_empty` | `fn is_empty(&self) -> bool` | Whether registry is empty |

### RoutingContext

**File:** `crates/tools-core/src/lib.rs:68-116`

| Field | Type | Description |
|-------|------|-------------|
| `channel` | `ChannelName` | Source channel identifier |
| `chat_id` | `ChatId` | Source chat/user identifier |
| `interaction_tx` | `Option<mpsc::Sender<InteractionBundle>>` | CLI/dashboard ask_user oneshot channel |
| `is_direct_mode` | `bool` | True for CLI/dashboard (inline responses) |
| `entity_tx` | `Option<mpsc::Sender<EntityCard>>` | Entity card emission for dashboard |
| `interaction_channel` | `Option<Arc<dyn InteractionChannel>>` | Platform-native UI support |

| Constructor | Purpose |
|-------------|---------|
| `new(channel, chat_id)` | Non-interactive mode |
| `with_interaction(channel, chat_id, tx)` | Interactive direct mode with user interaction |

### InteractionChannel Trait

**File:** `crates/tools-core/src/lib.rs:46-60`

| Method | Signature | Notes |
|--------|-----------|-------|
| `supports_interaction` | `fn supports_interaction(&self) -> bool` | Whether channel supports structured UI |
| `send_interaction` | `async fn send_interaction(&self, chat_id: &str, request: &InteractionRequest) -> Result<FormResponse>` | Send structured question, wait for response |

### ToolPermissions

**File:** `crates/tools-core/src/permissions.rs:33-63`

| Method | Signature | Notes |
|--------|-----------|-------|
| `new` | `fn new(default_level: PermissionLevel) -> Self` | Create with default level for unconfigured channels |
| `set_channel_level` | `fn set_channel_level(&mut self, channel: impl Into<String>, level: PermissionLevel)` | Override level for a specific channel |
| `is_allowed` | `fn is_allowed(&self, channel: &str, required: PermissionLevel) -> bool` | Check if channel has sufficient permission |

### PermissionLevel

**File:** `crates/tools-core/src/permissions.rs:9-19`

| Variant | Value | Typical tools |
|---------|-------|--------------|
| `ReadOnly` | 0 | `read_file`, `list_dir`, `web_search`, `glob`, `grep` |
| `Standard` | 1 | `todo`, `project`, `calendar`, `memory`, `agent_task` |
| `Elevated` | 2 | `write_file`, `edit_file`, `browser` |
| `Admin` | 3 | `spawn` |

Implements `PartialOrd` + `Ord` -- comparison is by numeric value.

### Searchable Trait

**File:** `crates/tools-core/src/search.rs:9-12`

| Method | Signature | Notes |
|--------|-----------|-------|
| `search_id` | `fn search_id(&self) -> &str` | Unique identifier for RRF merging |

### rrf_merge Function

**File:** `crates/tools-core/src/search.rs:27-75`

```rust
pub fn rrf_merge<T: Searchable + Clone>(
    keyword_results: &[T],
    semantic_results: &[(String, f64)],
    k: u32,
    items_by_id: &HashMap<String, T>,
) -> Vec<(T, f64, &'static str)>
```

Returns `(item, rrf_score, source)` tuples sorted by score descending. Source is `"keyword"`, `"semantic"`, or `"both"`.

### FeaturePackage Trait

**File:** `crates/tools-core/src/feature.rs:29-50`

| Method | Signature | Default | Notes |
|--------|-----------|---------|-------|
| `name` | `fn name(&self) -> &str` | -- | Unique feature name (e.g., "todo") |
| `tools` | `fn tools(&self) -> Vec<DynTool>` | -- | Tools this feature provides |
| `migrations` | `fn migrations(&self) -> Vec<FeatureMigration>` | -- | SQL migrations in order |
| `config_key` | `fn config_key(&self) -> &str` | -- | Config section key |
| `default_config` | `fn default_config(&self) -> Value` | -- | Default config value |
| `health_check` | `async fn health_check(&self) -> Result<HealthStatus>` | `Ok(Healthy)` | Health check |

### ConfigPersistence Trait

**File:** `crates/tools-core/src/config_persistence.rs:14-21`

| Method | Signature | Notes |
|--------|-----------|-------|
| `load_section` | `async fn load_section(&self, key: &str) -> Result<Value>` | Read config section |
| `save_section` | `async fn save_section(&self, key: &str, value: Value) -> Result<()>` | Write config section |

### Page\<T\>

**File:** `crates/tools-core/src/pagination.rs:7-38`

| Field | Type | Notes |
|-------|------|-------|
| `items` | `Vec<T>` | Page items |
| `cursor` | `Option<String>` | Opaque next-page token |
| `has_more` | `bool` | Whether more pages exist |

Constructors: `new()`, `empty()`, `single_page()`.

### ParamExtractor

**File:** `crates/tools-core/src/params.rs:15-191`

| Method | Signature | On Absent | On Wrong Type |
|--------|-----------|-----------|---------------|
| `required_str` | `fn required_str(&self, name: &str) -> Result<&str>` | `Err` | `Err` |
| `required_i64` | `fn required_i64(&self, name: &str) -> Result<i64>` | `Err` | `Err` |
| `required_u64` | `fn required_u64(&self, name: &str) -> Result<u64>` | `Err` | `Err` |
| `required_bool` | `fn required_bool(&self, name: &str) -> Result<bool>` | `Err` | `Err` |
| `required_array` | `fn required_array(&self, name: &str) -> Result<&Vec<Value>>` | `Err` | `Err` |
| `required_object` | `fn required_object(&self, name: &str) -> Result<&Map<String, Value>>` | `Err` | `Err` |
| `optional_str` | `fn optional_str(&self, name: &str) -> Result<Option<&str>>` | `Ok(None)` | `Err` |
| `optional_i64` | `fn optional_i64(&self, name: &str) -> Result<Option<i64>>` | `Ok(None)` | `Err` |
| `optional_u64` | `fn optional_u64(&self, name: &str) -> Result<Option<u64>>` | `Ok(None)` | `Err` |
| `optional_f64` | `fn optional_f64(&self, name: &str) -> Result<Option<f64>>` | `Ok(None)` | `Err` |
| `optional_bool` | `fn optional_bool(&self, name: &str) -> Result<Option<bool>>` | `Ok(None)` | `Err` |
| `optional_array` | `fn optional_array(&self, name: &str) -> Result<Option<&Vec<Value>>>` | `Ok(None)` | `Err` |
| `str_or` | `fn str_or(&self, name: &str, default: &str) -> Result<&str>` | `Ok(default)` | `Err` |
| `i64_or` | `fn i64_or(&self, name: &str, default: i64) -> Result<i64>` | `Ok(default)` | `Err` |
| `string_array_or_empty` | `fn string_array_or_empty(&self, name: &str) -> Result<Vec<String>>` | `Ok(vec![])` | `Err` |

### EmbeddingEngine

**File:** `crates/tools/src/embedding_engine.rs:26-197`

| Method | Signature | Notes |
|--------|-----------|-------|
| `new` | `fn new() -> Self` | No model loaded until first embed |
| `embed` | `fn embed(&self, text: &str) -> Result<Vec<f32>>` | Single text, 384-dim output |
| `embed_batch` | `fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>` | Batch embed |
| `embed_async` | `async fn embed_async(self: Arc<Self>, text: String) -> Result<Vec<f32>>` | Blocking thread pool wrapper |
| `is_available` | `fn is_available(&self) -> bool` | Whether model is initialized |
| `model_name` | `fn model_name(&self) -> &str` | Always `"paraphrase-multilingual-MiniLM-L12-v2"` |
| `cosine_similarity` | `fn cosine_similarity(a: &[f32], b: &[f32]) -> f64` | Static, NaN-safe |

### EmbeddingHandler Trait

**File:** `crates/tools/src/embedding_engine.rs:203-213`

| Method | Signature | Notes |
|--------|-----------|-------|
| `embed_todo` | `async fn embed_todo(&self, todo: &Todo) -> Result<Option<EmbeddingRecord>>` | Embed todo's searchable text |
| `embed_query` | `async fn embed_query(&self, query: &str) -> Result<Vec<f32>>` | Embed a query string |
| `is_available` | `fn is_available(&self) -> bool` | Check engine availability |

### Tool Listing

| Name | File | Permission | Actions | Description |
|------|------|------------|---------|-------------|
| `read_file` | `tools/src/filesystem.rs:83` | ReadOnly | -- | Read file contents |
| `write_file` | `tools/src/filesystem.rs:145` | Elevated | -- | Write content to file |
| `edit_file` | `tools/src/filesystem.rs:217` | Elevated | -- | Find-and-replace in file |
| `list_dir` | `tools/src/filesystem.rs:310` | ReadOnly | -- | List directory contents |
| `glob` | `tools/src/glob_tool.rs:28` | ReadOnly | -- | Find files by glob pattern |
| `grep` | `tools/src/grep.rs:30` | ReadOnly | -- | Search file contents by regex |
| `web_search` | `tools/src/web.rs:34` | Standard | -- | Search via Brave Search API |
| `web_fetch` | `tools/src/web.rs:165` | Standard | -- | Fetch URL, extract readable content |
| `browser` | `tools/src/browser.rs:219` | Elevated | `navigate`, `snapshot`, `click`, `type`, `fill`, `press`, `scroll`, `wait`, `get_text`, `screenshot`, `eval`, `fill_form`, `login_flow`, `submit_and_confirm` | Browser automation via agent-browser CLI |
| `message` | `tools/src/message.rs:25` | Standard | -- | Send messages to channels |
| `ask_user` | `tools/src/ask_user.rs:21` | Standard | -- | Structured questions (single/multi select, yes/no, free text) |
| `spawn` | `tools/src/spawn.rs:59` | Admin | `spawn`, `cancel`, `status` | Spawn background subagents |
| `cron` | `tools/src/cron_tool.rs:92` | Standard | `add`, `list`, `remove` | Schedule recurring tasks |
| `calendar` | `tools/src/calendar_tool.rs:68` | Standard | `sync`, `list_events`, `create_event`, `status`, `push_all`, `pull_all` | CalDAV calendar operations |
| `goal` | `tools/src/goal_tool.rs:58` | Standard | `create`, `list`, `show`, `update`, `delete`, `progress`, `decompose`, `status`, `metrics` | Strategic goal management |
| `plan` | `tools/src/plan_tool.rs:149` | Standard | `create`, `show`, `approve`, `abandon`, `status`, `execute` | Multi-step plan management |
| `project` | `tools/src/project_tool.rs:32` | Standard | `create`, `list`, `show`, `update`, `archive`, `tasks` | Project management |
| `learning` | `tools/src/learning_tool.rs:75` | Standard | `status`, `analyze`, `history` | Learning system insights |
| `memory` | `tools/src/memory_tool.rs:93` | Standard | `search_conversations`, `search_all`, `purge`, `status` | Semantic search over conversation history |
| `agent_task` | `tools/src/agent_task_tool.rs:45` | Standard | `list`, `claim`, `update`, `complete`, `fail` | Subagent task board coordination |

### Dependency Inversion Handler Traits

All defined in the `tools` crate (Layer 3), implemented in the `agent` crate (Layer 5).

| Trait | File | Key Methods |
|-------|------|-------------|
| `SpawnHandler` | `tools/src/spawn.rs:15-31` | `spawn()`, `cancel()`, `status()` |
| `CronHandler` | `tools/src/cron_tool.rs:55-64` | `add_job()`, `list_jobs()`, `remove_job()` |
| `CalendarHandler` | `tools/src/calendar_tool.rs:16-54` | `sync_calendar()`, `list_events()`, `create_event()`, `get_status()`, `get_event()`, `get_events_for_reconciliation()`, `push_single_task()`, `remove_single_event()`, `pull_all_events()`, `push_all_tasks()` |
| `GoalHandler` | `tools/src/goal_tool.rs:18-33` | `create_goal()`, `get_goal()`, `list_goals()`, `update_goal()`, `delete_goal()`, `calculate_progress()`, `decompose_goal()`, `goal_progress()`, `goal_metrics()` |
| `PlanHandler` | `tools/src/plan_tool.rs:37-60` | `create_plan()`, `get_plan()`, `get_active_plan()`, `approve_plan()`, `abandon_plan()`, `get_step_context()`, `execute_plan()`, `generate_steps()`, `preview_steps()` |
| `PlanCompletionHandler` | `tools/src/plan_tool.rs:17-31` | `on_plan_completed()` |
| `LearningHandler` | `tools/src/learning_tool.rs:52-61` | `get_status()`, `analyze_now()`, `get_threshold_history()` |
| `AgentTaskHandler` | `tools/src/agent_task_tool.rs:15-26` | `list_tasks()`, `claim_task()`, `update_task()`, `complete_task()`, `fail_task()` |
| `EmbeddingHandler` | `tools/src/embedding_engine.rs:203-213` | `embed_todo()`, `embed_query()`, `is_available()` |
| `ConversationEmbeddingHandler` | `tools/src/conversation_embedding.rs` | `embed_message()`, `search()`, `purge()`, `status()`, `is_available()` |

### Proc Macro Usage Examples

#### `#[tool_actions]`

```rust
use tools_core::{tool_actions, ActionParams, RoutingContext};

#[derive(ActionParams)]
pub struct ListParams {
    /// Maximum results
    #[param(min = 1, max = 100)]
    pub limit: Option<u32>,
}

#[derive(ActionParams)]
pub struct AddParams {
    /// Item title
    #[param(required)]
    pub title: String,
}

pub struct MyTool { /* ... */ }

#[tool_actions(name = "my_tool", description = "Manage items")]
impl MyTool {
    #[action(name = "list")]
    async fn handle_list(&self, params: ListParams, ctx: &RoutingContext) -> common::Result<String> {
        // ...
    }

    #[action(name = "add")]
    async fn handle_add(&self, params: AddParams, ctx: &RoutingContext) -> common::Result<String> {
        // ...
    }
}
// Generates: Tool impl with name="my_tool", parameters schema merging both
// ListParams and AddParams properties plus an "action" enum ["list", "add"],
// and execute() dispatching to handle_list/handle_add based on action value.
```

#### `#[derive(ActionParams)]`

```rust
use tools_core::ActionParams;

#[derive(ActionParams)]
pub struct SearchParams {
    /// Search query text
    #[param(required, min_length = 1)]
    pub query: String,

    /// Result limit
    #[param(min = 1, max = 50)]
    pub limit: Option<i64>,

    /// Filter tags
    pub tags: Vec<String>,

    /// Include archived items
    pub include_archived: Option<bool>,
}

// Generated: SearchParams::json_schema() -> JSON Schema Value
// Generated: SearchParams::from_value(&Value) -> Result<Self, String>
```

#### `#[derive(DomainEnum)]`

```rust
use tools_core::DomainEnum;

#[derive(Debug, Clone, PartialEq, Eq, DomainEnum)]
pub enum Priority {
    #[aliases("critical", "p0")]
    Urgent,
    #[aliases("important", "p1")]
    High,
    Medium,
    #[aliases("minor", "nice_to_have")]
    Low,
    #[canonical("no-priority")]
    None,
}

// Generated: Priority::as_str()        -- "urgent", "high", "medium", "low", "no-priority"
// Generated: Priority::from_str_loose() -- case-insensitive, includes aliases
// Generated: Display for Priority       -- delegates to as_str()
// Generated: FromStr for Priority       -- delegates to from_str_loose()
```
