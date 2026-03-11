# Tools System Architecture

## 1. Overview

The klyntbot tools system uses a **two-layer pattern** that bridges the gap between typed Rust code and the untyped JSON interface that LLMs consume.

**Layer 1 -- `ToolExecute` (typed).** Tool authors write strongly-typed Rust: a params struct with named fields and an `async fn execute` that receives those params. The compiler enforces correctness at build time.

**Layer 2 -- `Tool` (untyped).** The LLM sees JSON function schemas and sends back `serde_json::Value` arguments. The `Tool` trait accepts `Value`, validates it against a JSON Schema, deserializes into the typed params, and delegates to `ToolExecute::execute`.

The derive macro `#[derive(Tool)]` generates the bridging code automatically, so tool authors never write JSON parsing by hand.

```
LLM  ──JSON──>  Tool::execute(Value)  ──deserialize──>  ToolExecute::execute(TypedParams)
                      ^                                        ^
                  untyped boundary                       typed Rust
```

### Key types at a glance

| Type | Crate | Purpose |
|------|-------|---------|
| `Tool` | `tools-core` | Untyped trait the registry and agent runtime operate on |
| `ToolExecute` | `tools-core` | Typed trait tool authors implement |
| `ToolParams` | `tools-core` | Trait for JSON Schema generation + deserialization |
| `ToolRegistry` | `tools-core` | Runtime store of `DynTool` instances |
| `FeaturePackage` | `tools-core` | Self-contained feature: tools + migrations + config |
| `RoutingContext` | `tools-core` | Per-invocation context (channel, chat ID, interaction handles) |
| `PermissionLevel` | `tools-core` | Four-tier authorization model |
| `DynTool` | `tools-core` | Type alias: `Arc<dyn Tool>` |

---

## 2. Tool Trait Anatomy

The `Tool` trait is defined in `crates/tools-core/src/lib.rs`:

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name for function calls (e.g., "read_file")
    fn name(&self) -> &str;

    /// Human-readable description
    fn description(&self) -> &str;

    /// JSON Schema for parameters
    fn parameters(&self) -> Value;

    /// Execute the tool with given arguments and routing context
    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String>;

    /// Permission level required to use this tool.
    /// Defaults to `Standard`. Override for sensitive tools.
    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Standard
    }

    /// Rich metadata for discovery. Override to provide category, tags, etc.
    fn metadata(&self) -> ToolMetadata {
        ToolMetadata::default()
    }

    /// Optional per-tool timeout override.
    fn custom_timeout(&self) -> Option<std::time::Duration> {
        None
    }

    /// Convert to OpenAI function schema format
    fn to_schema(&self) -> Value { /* ... */ }

    /// Validate parameters against JSON Schema
    fn validate_params(&self, params: &Value) -> Vec<String> { /* ... */ }
}
```

### RoutingContext

Passed to every tool invocation. Carries channel identity and optional interaction handles:

```rust
#[derive(Clone)]
pub struct RoutingContext {
    pub channel: ChannelName,
    pub chat_id: ChatId,
    pub interaction_tx: Option<mpsc::Sender<InteractionBundle>>,
    pub is_direct_mode: bool,
    pub delegation_depth: u32,
    pub entity_tx: Option<mpsc::Sender<common::EntityCard>>,
    pub interaction_channel: Option<Arc<dyn InteractionChannel>>,
}
```

### PermissionLevel

Four ordered tiers. The registry checks that the invoking channel's granted level is >= the tool's required level.

```rust
pub enum PermissionLevel {
    ReadOnly  = 0,  // read_file, list_dir, web_search, grep, glob
    Standard  = 1,  // task, project, memory, message
    Elevated  = 2,  // write_file, edit_file, exec, browser
    Admin     = 3,  // spawn
}
```

---

## 3. Derive Macros

Source: `crates/tools-core-macros/src/`.

### `#[derive(ToolParams)]`

Generates `impl ToolParams` from a struct with `#[param(...)]` attributes. Doc comments on fields become JSON Schema `description` values.

**Input:**

```rust
#[derive(Debug, ToolParams)]
pub struct ReadFileParams {
    /// The file path to read
    #[param(required)]
    pub path: String,
}
```

**Generated code (conceptual):**

```rust
impl ::tools_core::ToolParams for ReadFileParams {
    fn json_schema() -> ::serde_json::Value {
        let mut properties = ::serde_json::Map::new();
        {
            let mut prop = ::serde_json::Map::new();
            prop.insert("type".to_string(), Value::String("string".to_string()));
            prop.insert("description".to_string(),
                Value::String("The file path to read".to_string()));
            properties.insert("path".to_string(), Value::Object(prop));
        }
        let required: Vec<&str> = vec!["path"];
        json!({
            "type": "object",
            "properties": Value::Object(properties),
            "required": required
        })
    }

    fn from_args(args: ::serde_json::Value) -> ::common::Result<Self> {
        let args = &args;
        (|| -> Result<Self, String> {
            Ok(Self {
                path: args.get("path")
                    .and_then(|v| if v.is_null() { None } else { v.as_str() })
                    .ok_or_else(|| "missing required 'path' parameter".to_string())?
                    .to_string(),
            })
        })().map_err(|e| KlyntbotError::Tool(ToolError::InvalidParams(e)))
    }
}
```

**`#[param(...)]` attribute fields:**

| Attribute | Effect |
|-----------|--------|
| `required` | Adds field to JSON Schema `required` array; `from_args` returns error if missing |
| `min = N` | Adds `"minimum": N` to schema (integers/numbers) |
| `max = N` | Adds `"maximum": N` to schema |
| `min_length = N` | Adds `"minLength": N` to schema (strings) |
| `max_length = N` | Adds `"maxLength": N` to schema (strings) |

**Type mapping:**

| Rust type | JSON Schema type | Optional handling |
|-----------|-----------------|-------------------|
| `String` | `"string"` | Required: error if missing. Non-required: `""` default |
| `Option<String>` | `"string"` | `None` if missing |
| `bool` | `"boolean"` | Default `false` unless `required` |
| `i64`, `u32`, etc. | `"integer"` | Default `0` unless `required` |
| `f64`, `f32` | `"number"` | Default `0.0` unless `required` |
| `Vec<String>` | `"array"` of `"string"` | Default `[]` |

### `#[derive(Tool)]`

Generates `impl Tool` by combining `#[tool(...)]` attributes with the struct's `ToolExecute` implementation.

**Input:**

```rust
#[derive(tools_core::Tool)]
#[tool(
    name = "read_file",
    description = "Read the contents of a file at the given path.",
    params = "ReadFileParams",
    permission = "read_only",
    category = "FileSystem",
    tags = "file,read,content",
    cost = "Free"
)]
pub struct ReadFileTool {
    base: FsToolBase,
}
```

**`#[tool(...)]` attribute fields:**

| Field | Required | Values |
|-------|----------|--------|
| `name` | Yes | Tool name string (e.g., `"read_file"`) |
| `description` | Yes | Human-readable description |
| `params` | Yes | Name of the `ToolParams` struct |
| `permission` | No | `"read_only"`, `"standard"` (default), `"elevated"`, `"admin"` |
| `category` | No | `"General"`, `"FileSystem"`, `"Search"`, `"Web"`, `"Communication"`, `"TaskManagement"`, `"Memory"`, `"Finance"`, `"Productivity"`, `"System"`, `"Mcp"`, `"Plugin"` |
| `tags` | No | Comma-separated tag strings |
| `cost` | No | `"Free"` (default), `"Low"`, `"Medium"`, `"High"`, `"Variable"` |

**Generated code (conceptual):**

```rust
#[async_trait]
impl ::tools_core::Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "Read the contents of a file at the given path." }

    fn parameters(&self) -> ::serde_json::Value {
        <ReadFileParams as ::tools_core::ToolParams>::json_schema()
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let params = <ReadFileParams as ::tools_core::ToolParams>::from_args(args)?;
        <Self as ::tools_core::ToolExecute>::execute(self, params, ctx).await
    }

    fn permission_level(&self) -> PermissionLevel { PermissionLevel::ReadOnly }

    fn metadata(&self) -> ToolMetadata {
        ToolMetadata {
            category: ToolCategory::FileSystem,
            tags: vec!["file".to_string(), "read".to_string(), "content".to_string()],
            cost_hint: CostHint::Free,
            ..Default::default()
        }
    }
}
```

### `#[derive(ActionParams)]`

Identical to `ToolParams` in schema generation, but produces inherent methods (`json_schema()`, `from_value()`) instead of a trait impl. Used by `#[tool_actions]` where multiple param structs exist per tool.

---

## 4. Multi-Action Tools

Many tools expose multiple actions through a single tool name (e.g., `task` has 25+ actions). There are two approaches.

### Approach A: `#[tool_actions]` macro (recommended for new tools)

The macro generates the `Tool` trait impl, including action dispatch and a merged parameter schema. Each method is annotated with `#[action(name = "...")]` and takes a typed `ActionParams` struct.

**Real example from `crates/tools/src/docs.rs`:**

```rust
#[derive(Debug, ActionParams)]
pub struct SearchParams {
    /// Search query for finding documentation or skills
    #[param(required)]
    pub query: String,
    /// Maximum number of results to return (default: 10)
    pub limit: Option<i64>,
}

#[derive(Debug, ActionParams)]
pub struct GetParams {
    /// Document or skill ID to retrieve (e.g. "stripe/api")
    #[param(required)]
    pub id: String,
}

#[derive(Debug, ActionParams)]
pub struct ListParams {}

pub struct DocsTool {
    handler: Option<Arc<dyn ContentRegistryHandler>>,
}

#[tool_actions(
    name = "docs",
    description = "Search and fetch documentation from the content registry.",
    category = "Search",
    tags = "documentation,api,sdk,reference",
    cost = "Free"
)]
impl DocsTool {
    #[action(name = "search")]
    async fn search(&self, params: SearchParams, _ctx: &RoutingContext) -> Result<String> {
        let limit = params.limit.unwrap_or(10) as usize;
        self.handler()?.search(&params.query, limit).await
    }

    #[action(name = "get")]
    async fn get(&self, params: GetParams, _ctx: &RoutingContext) -> Result<String> {
        self.handler()?.get(&params.id).await
    }

    #[action(name = "list")]
    async fn list(&self, _params: ListParams, _ctx: &RoutingContext) -> Result<String> {
        self.handler()?.list().await
    }
}
```

The macro generates a `parameters()` method that produces:

```json
{
  "type": "object",
  "properties": {
    "action": { "type": "string", "enum": ["search", "get", "list"] },
    "query": { "type": "string", "description": "Search query..." },
    "limit": { "type": "integer" },
    "id": { "type": "string", "description": "Document or skill ID..." }
  },
  "required": ["action"]
}
```

The `execute()` method dispatches on the `action` field:

```rust
async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
    let action = args.get("action").and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidParams("missing required 'action'".into()))?;
    match action {
        "search" => {
            let params = SearchParams::from_value(&args)?;
            self.search(params, ctx).await
        }
        "get" => { /* ... */ }
        "list" => { /* ... */ }
        unknown => Err(ToolError::InvalidParams(format!("unknown action: {}", unknown)).into()),
    }
}
```

Another real example using `#[tool_actions]` is `AnnotateTool` in `crates/tools/src/annotate.rs`, with actions: `create`, `get`, `list`, `delete`, `search`.

### Approach B: Manual `Tool` impl (used by complex tools)

Tools that need a hand-crafted JSON Schema (e.g., dozens of shared parameters across actions) implement `Tool` directly with manual `match` dispatch. This is the pattern used by `TaskTool`, `FinanceTool`, `OkrTool`, `AreaTool`, `ProjectTool`, and `MemoryTool`.

**Real example from `crates/feature-todo/src/tool/mod.rs` (TaskTool):**

```rust
#[async_trait]
impl Tool for TaskTool {
    fn name(&self) -> &str { "task" }

    fn description(&self) -> &str {
        "Manage tasks/actions. Actions: add, list, update, complete, delete, show, ..."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "update", "complete", "delete", "show",
                             "summary", "focus", "unfocus", "add_subtask", "move",
                             "attach", "detach", "log_time", "tree", "search",
                             "search_semantic", "search_hybrid", "report",
                             "add_dependency", "remove_dependency",
                             "recur", "list_recurring", "delete_recurring",
                             "enrich", "plan"],
                    "description": "Action to perform"
                },
                "id": { "type": "string", "description": "Task ID" },
                "title": { "type": "string", "description": "Task title" },
                // ... 20+ shared parameters
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;
        match action {
            "add" => self.handle_add(&p, ctx).await,
            "list" => self.handle_list(&p).await,
            "update" => self.handle_update(&p, ctx).await,
            // ... etc
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {}", action)).into()),
        }
    }
}
```

**When to use which:**

| Criteria | `#[tool_actions]` | Manual `impl Tool` |
|----------|-------------------|---------------------|
| Actions share few parameters | Preferred | Overkill |
| Actions share many parameters | Leads to duplicate fields | Preferred |
| Number of actions | Up to ~10 | Any |
| Schema customization needed | Limited | Full control |

---

## 5. ToolRegistry

Source: `crates/tools-core/src/registry.rs`.

The `ToolRegistry` is the runtime container for all registered tool instances. It stores `DynTool` (`Arc<dyn Tool>`) values in a `HashMap<String, DynTool>`.

### Core operations

```rust
impl ToolRegistry {
    pub fn new() -> Self;

    // Registration
    pub fn register(&mut self, tool: impl Tool + 'static);
    pub fn register_dyn(&mut self, tool: DynTool);
    pub fn unregister(&mut self, name: &str);
    pub fn unregister_by_prefix(&mut self, prefix: &str) -> usize;

    // Lookup
    pub fn get(&self, name: &str) -> Option<DynTool>;
    pub fn has(&self, name: &str) -> bool;
    pub fn tool_names(&self) -> Vec<String>;
    pub fn len(&self) -> usize;

    // Schema generation (cached)
    pub fn get_definitions(&self) -> Arc<Vec<Value>>;

    // Execution pipeline
    pub fn prepare(&self, name: &str, params: &Value, ctx: &RoutingContext) -> Result<DynTool>;
    pub async fn execute(&self, name: &str, params: Value, ctx: &RoutingContext) -> Result<String>;

    // Discovery
    pub fn search_tools(&self, query: &str, limit: usize) -> Vec<(String, f64)>;
    pub fn by_category(&self, category: &ToolCategory) -> Vec<&str>;
    pub fn get_metadata(&self, name: &str) -> Option<&ToolMetadata>;

    // Usage tracking
    pub fn record_usage(&self, name: &str);
    pub fn top_used(&self, n: usize) -> Vec<(String, u64)>;

    // Permission configuration
    pub fn set_permissions(&mut self, permissions: ToolPermissions);
}
```

### Execution pipeline: `prepare()` then `execute()`

The `prepare()` method performs three checks before execution:

1. **Lookup** -- finds the tool by name or returns `ToolError::NotFound`.
2. **Permission check** -- if `ToolPermissions` are configured, verifies the channel's granted level >= the tool's `permission_level()`.
3. **Parameter validation** -- calls `tool.validate_params(params)` against the JSON Schema.

The method returns a cloned `DynTool` (Arc), allowing the caller to drop the registry borrow before calling `tool.execute()`. This prevents deadlocks when a tool (e.g., `delegate`) needs write access to the same registry during execution.

### Search: Reciprocal Rank Fusion (RRF)

`search_tools()` produces three ranked lists from the query terms:

1. **Name matches** -- terms found in tool names
2. **Description matches** -- terms found in tool descriptions
3. **Tag matches** -- terms found in tool metadata tags

These are merged via `rrf_merge_triple()` (k=60) for consistent scoring with the rest of the search infrastructure. Results are returned as `Vec<(String, f64)>` (tool name, RRF score).

### Cached definitions

`get_definitions()` returns `Arc<Vec<Value>>` containing all tool schemas in OpenAI function-calling format. The cache is invalidated on any `register`, `register_dyn`, `unregister`, or `unregister_by_prefix` call. Cache hits are a single atomic reference count increment.

### Usage tracking

`record_usage()` and `top_used()` use interior mutability (`Mutex<HashMap<String, u64>>`) so they can be called with only a shared `&self` reference during the execution loop.

---

## 6. Feature Packages

Source: `crates/tools-core/src/feature.rs`.

A `FeaturePackage` is a self-contained unit that bundles tools, database migrations, config, and health checks.

```rust
#[async_trait]
pub trait FeaturePackage: Send + Sync {
    /// Unique feature name (e.g., "todo", "finance").
    fn name(&self) -> &str;

    /// The tool(s) this feature provides.
    fn tools(&self) -> Vec<DynTool>;

    /// SQL migrations owned by this feature, in order.
    fn migrations(&self) -> Vec<FeatureMigration>;

    /// Config section key (e.g., "todo", "finance").
    fn config_key(&self) -> &str;

    /// Default config value (merged if section is missing).
    fn default_config(&self) -> Value;

    /// Health check (default: healthy).
    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
```

### FeatureMigration

```rust
pub struct FeatureMigration {
    pub feature_name: String,
    pub version: i64,
    pub description: String,
    pub sql: String,
}
```

### HealthStatus

```rust
pub enum HealthStatus {
    Healthy,
    Degraded(String),
    Unhealthy(String),
}
```

### Existing feature packages

| Feature | Crate | Tool name | Actions |
|---------|-------|-----------|---------|
| Todo | `feature-todo` | `task` | 25 actions (add, list, update, complete, ...) |
| Finance | `feature-finance` | `finance` | 40+ actions (account_add, tx_add, budget_create, ...) |
| Productivity | `feature-productivity` | `productivity` | 18 actions (focus_start, activity_today, set_goal, ...) |
| Notes | `feature-notes` | `notes` | 10 actions (create_note, search_notes, ...) |

### Wiring in builder.rs

Feature packages are wired in `AgentLoopBuilder::build()` (`crates/agent/src/agent_loop/builder.rs`). The builder does not use `FeaturePackage::tools()` generically -- instead, each feature is constructed with its dependencies and registered individually. This allows dependency injection (embedding handlers, progress handlers, domain event buses, etc.) before registration.

Example for `feature-todo`:

```rust
// In AgentLoopBuilder::build()
let feature_todo_repo = feature_todo::ActionRepo::new(pool_ref.clone());
let mut todo_tool = feature_todo::TaskTool::new(
    feature_todo_repo,
    config.todo.focus.max_slots,
    config.todo.focus.deadline_hours,
    config.timezone.clone(),
);

// Inject optional handlers
if config.todo.enrichment.enabled {
    todo_tool = todo_tool.with_enrichment_handler(Arc::clone(&enrichment_engine) as _);
}
if let (true, Some(vs)) = (config.todo.search.enabled, vector_store.clone()) {
    todo_tool = todo_tool.with_embedding_handler(todo_embed_impl as _)
                         .with_embedding_store(vs);
}
todo_tool = todo_tool.with_progress_handler(Arc::clone(&progress_handler));

tool_registry.register(todo_tool);
```

---

## 7. Built-in Tools

### Core tools (crate: `tools`)

| Name | Description | Permission | Category |
|------|-------------|------------|----------|
| `read_file` | Read file contents | ReadOnly | FileSystem |
| `write_file` | Write content to a file | Elevated | FileSystem |
| `edit_file` | Edit file by replacing text | Elevated | FileSystem |
| `list_dir` | List directory contents | ReadOnly | FileSystem |
| `grep` | Search file contents using regex | ReadOnly | Search |
| `glob` | Find files by glob pattern | ReadOnly | Search |
| `web_search` | Search the web via Brave Search API | Standard | Web |
| `web_fetch` | Fetch URL and extract readable content | Standard | Web |
| `browser` | Browser automation via agent-browser CLI | Elevated | System |
| `message` | Send a message to a channel | Standard | Communication |
| `ask_user` | Ask structured questions to the user | Standard | Communication |
| `spawn` | Spawn a subagent for background tasks | Admin | System |
| `cron` | Schedule reminders and recurring tasks | Standard | System |
| `delegate` | Delegate query to a specialist agent | Standard | System |
| `memory` | Semantic search over conversation history | Standard | Memory |
| `learning` | Query learning system insights | Standard | System |
| `okr` | Manage OKR objectives and key results | Standard | TaskManagement |
| `area` | Manage PARA areas | Standard | Productivity |
| `project` | Manage projects | Standard | TaskManagement |
| `annotate` | CRUD for persistent annotations | Standard | Memory |
| `agent_task` | Subagent task board coordination | Standard | TaskManagement |
| `context_request` | Request additional context mid-execution | Standard | System |
| `docs` | Search/fetch documentation from content registry | Standard | Search |
| `work_context` | Manage inferred work contexts | Standard | Productivity |

### Feature tools (separate crates)

| Name | Crate | Description | Permission | Category |
|------|-------|-------------|------------|----------|
| `task` | `feature-todo` | Full task/action management (25 actions) | Standard | TaskManagement |
| `finance` | `feature-finance` | Personal finance management (40+ actions) | Standard | Finance |
| `productivity` | `feature-productivity` | Productivity tracking and focus sessions (18 actions) | Standard | Productivity |
| `notes` | `feature-notes` | Note and notebook management (10 actions) | Standard | General |

### Dynamic tools

| Source | Naming | Example |
|--------|--------|---------|
| MCP servers | `mcp_{server}_{tool}` | `mcp_linear_save_issue` |
| WASM plugins | Plugin-defined | Varies |

---

## 8. Tool Registration Flow

Tool registration happens in `AgentLoopBuilder::build()` at `crates/agent/src/agent_loop/builder.rs`. The order:

### Phase 1: Always-available tools

1. **Filesystem tools** -- `read_file`, `write_file`, `edit_file`, `list_dir` via `register_fs_tools()`. Respects `config.tools.restrict_to_workspace` for directory sandboxing.
2. **Search tools** -- `grep`, `glob` with the same `allowed_dir`.
3. **Web tools** -- `web_search` (needs Brave API key), `web_fetch`.
4. **Browser tool** -- conditionally registered if `config.tools.browser.enabled`.
5. **Message tool** -- injected with the outbound message bus sender.
6. **Ask-user tool** -- always registered (stateless).
7. **Spawn tool** -- injected with `SubagentManager` as `SpawnHandler`.
8. **Cron tool** -- conditionally registered if `CronService` is available.

### Phase 2: Domain tools (require `StoragePool`)

9. **Task tool** -- constructed from `ActionRepo`, wired with optional enrichment, embedding, progress, and domain bus handlers.
10. **OKR tool** -- shares the progress handler with task tool.
11. **Area tool** -- from `AreaRepo`.
12. **Project tool** -- from `ProjectRepo` + `ActionRepo`.
13. **Annotate tool** -- from `AnnotationRepo`.
14. **Memory tool** -- conditionally registered if conversation search is enabled. Wired with recall handler, todo repo, and embedding handlers.
15. **Finance tool** -- conditionally registered if `config.finance.enabled` and pool is available.
16. **Notes tool** -- requires pool.
17. **Work context tool** -- conditionally registered if `config.work_context.enabled`.
18. **Productivity tool** -- conditionally registered if productivity repos are available. Runs feature migrations first.

### Phase 3: External tools

19. **WASM plugin tools** -- if `config.plugins.enabled`, loads all plugins from `{data_dir}/plugins/`. Registers cron jobs for plugin-defined schedules, then registers all plugin tools via `register_dyn()`.
20. **MCP tools** -- if `config.mcp.has_active_servers()`, connects to all configured MCP servers via `McpManager::connect_all()` and registers discovered tools via `register_dyn()`.

---

## 9. Creating a New Tool

Step-by-step guide based on the `filesystem.rs` pattern.

### Step 1: Define your params struct

```rust
// crates/tools/src/my_tool.rs

use tools_core::{RoutingContext, ToolParams};
use common::{Result, ToolError};

#[derive(Debug, ToolParams)]
pub struct MyToolParams {
    /// The query to process
    #[param(required)]
    pub query: String,

    /// Maximum number of results (default: 10)
    #[param(min = 1, max = 100)]
    pub limit: Option<i64>,

    /// Whether to include metadata
    pub include_metadata: Option<bool>,
}
```

### Step 2: Define the tool struct with `#[derive(Tool)]`

```rust
#[derive(tools_core::Tool)]
#[tool(
    name = "my_tool",
    description = "Describe what this tool does for the LLM.",
    params = "MyToolParams",
    permission = "standard",
    category = "General",
    tags = "search,query",
    cost = "Low"
)]
pub struct MyTool {
    // Dependencies injected at construction time
    some_repo: SomeRepo,
}

impl MyTool {
    pub fn new(some_repo: SomeRepo) -> Self {
        Self { some_repo }
    }
}
```

### Step 3: Implement `ToolExecute`

```rust
use async_trait::async_trait;

#[async_trait]
impl tools_core::ToolExecute for MyTool {
    type Params = MyToolParams;

    async fn execute(&self, params: MyToolParams, _ctx: &RoutingContext) -> Result<String> {
        let limit = params.limit.unwrap_or(10) as usize;
        let include_meta = params.include_metadata.unwrap_or(false);

        let results = self.some_repo.search(&params.query, limit).await
            .map_err(|e| ToolError::ExecutionFailed(format!("Search failed: {}", e)))?;

        // Return a string that the LLM can understand
        Ok(format!("Found {} results for '{}'", results.len(), params.query))
    }
}
```

### Step 4: Register in the module's `lib.rs`

```rust
// crates/tools/src/lib.rs
pub mod my_tool;
pub use my_tool::MyTool;
```

### Step 5: Register in `AgentLoopBuilder::build()`

```rust
// crates/agent/src/agent_loop/builder.rs
tool_registry.register(tools::MyTool::new(repos.some_repo.clone()));
```

### Step 6: If the tool needs agent-layer dependencies

Use dependency inversion. Define a handler trait in the tools crate:

```rust
// crates/tools/src/my_tool.rs
#[async_trait]
pub trait MyHandler: Send + Sync {
    async fn do_something(&self, input: &str) -> Result<String>;
}

pub struct MyTool {
    handler: Option<Arc<dyn MyHandler>>,
}

impl MyTool {
    pub fn with_handler(handler: Arc<dyn MyHandler>) -> Self {
        Self { handler: Some(handler) }
    }
}
```

Implement the trait in the agent crate (Layer 5), inject via `Arc<dyn MyHandler>` in the builder.

---

## 10. Creating a Feature Package

Step-by-step guide based on `feature-todo` and `feature-finance`.

### Step 1: Create the crate

```bash
cargo new crates/feature-myfeature --lib
```

Add dependencies to `Cargo.toml`:

```toml
[dependencies]
async-trait = "0.1"
common = { path = "../common" }
serde_json = "1"
storage = { path = "../storage" }
tools-core = { path = "../tools-core" }
```

Add it to the workspace `Cargo.toml`.

### Step 2: Define the tool (see Section 9)

Place tool implementation in `src/tool.rs` or `src/tool/mod.rs`.

### Step 3: Write migrations

Create `migrations/001_create_tables.sql`:

```sql
CREATE TABLE IF NOT EXISTS my_entities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### Step 4: Define config

```rust
// src/config.rs
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MyFeatureConfig {
    pub enabled: bool,
    pub max_items: usize,
}

impl Default for MyFeatureConfig {
    fn default() -> Self {
        Self { enabled: true, max_items: 100 }
    }
}
```

### Step 5: Implement `FeaturePackage`

```rust
// src/lib.rs
use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use std::sync::Arc;
use tools_core::{DynTool, FeatureMigration, FeaturePackage, HealthStatus};

pub mod config;
pub mod tool;

pub use config::MyFeatureConfig;
pub use tool::MyFeatureTool;

pub struct MyFeature {
    tool: Arc<MyFeatureTool>,
}

impl MyFeature {
    pub fn new(tool: MyFeatureTool) -> Self {
        Self { tool: Arc::new(tool) }
    }

    pub fn migration_sql() -> &'static str {
        include_str!("../migrations/001_create_tables.sql")
    }
}

#[async_trait]
impl FeaturePackage for MyFeature {
    fn name(&self) -> &str {
        "myfeature"
    }

    fn tools(&self) -> Vec<DynTool> {
        vec![self.tool.clone()]
    }

    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![FeatureMigration {
            feature_name: "myfeature".to_string(),
            version: 1,
            description: "Create my_entities table".to_string(),
            sql: Self::migration_sql().to_string(),
        }]
    }

    fn config_key(&self) -> &str {
        "myfeature"
    }

    fn default_config(&self) -> Value {
        serde_json::to_value(MyFeatureConfig::default()).unwrap_or(Value::Null)
    }

    async fn health_check(&self) -> Result<HealthStatus> {
        // Check database connectivity, return Degraded or Unhealthy if broken
        Ok(HealthStatus::Healthy)
    }
}
```

### Step 6: Wire in the agent builder

In `crates/agent/src/agent_loop/builder.rs`, add the feature registration:

```rust
// Run feature migrations
if let Some(pool) = &self.pool {
    storage::StoragePool::run_feature_migrations(
        pool,
        &feature_myfeature::MyFeature::migrations_static(),
    ).await?;
}

// Construct and register
let my_tool = feature_myfeature::MyFeatureTool::new(/* deps */);
tool_registry.register(my_tool);
```

### Step 7: Add config section

In `crates/config/src/schema/core.rs`, add the config field:

```rust
pub struct Config {
    // ...
    pub myfeature: MyFeatureConfig,
}
```
