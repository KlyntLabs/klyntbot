# Feature Package Architecture Redesign

**Date**: 2026-02-20
**Status**: Approved
**Approach**: Hybrid — Feature Crates + Targeted Derive Macros

## Context

Klyntbot has two main chat-connected features: Todo (26 actions, ~4,000 LOC) and Finance (37+ actions, ~6,800 LOC). Both independently evolved the same patterns (action dispatcher, handler traits, builder pattern, manual JSON schema, ParamExtractor). There is no formal "feature package" abstraction — each feature is hand-wired across 5+ crates (tools, storage, config, agent, common).

Adding a new feature requires 10-15 steps across 5 crates. The system is pre-release, so breaking changes are acceptable.

## Goals

1. **Developer ergonomics**: Adding a new feature = create a crate + one line in agent
2. **Runtime performance**: Parallel queries, cursor pagination, batch embeddings, pool tuning
3. **Code quality**: Eliminate duplication via derive macros, reduce total LOC by 40-50%
4. **Extensibility**: Support both CRUD features (todo, finance) and integration features (GitHub, Jira)

## Workspace Layout

### New crate structure

```
crates/
├── common/              # (unchanged) Error types, MessageRole, ChannelName, etc.
├── config/              # (slimmed) Core config only — provider, database, channels
├── bus/                 # (unchanged) Async message bus
├── storage/             # (slimmed) Pool, migration runner, shared primitives only
├── tools-core/          # (NEW) Tool trait, FeaturePackage trait, ParamExtractor,
│   │                    #        derive macros, RoutingContext, ToolRegistry
│   └── macros/          #        Proc-macro sub-crate
├── feature-todo/        # (NEW) Self-contained: tool + storage + config + types + handler traits
├── feature-finance/     # (NEW) Self-contained: tool + storage + config + types + handler traits
├── providers/           # (unchanged) LLM HTTP client
├── session/             # (unchanged) Session persistence
├── scheduling/          # (unchanged) Cron service
├── calendar/            # (unchanged) CalDAV client
├── context_engine/      # (unchanged) Token budget allocator
├── channels/            # (unchanged) Chat platform integrations
├── agent/               # (slimmed) Agent loop, orchestrator, feature wiring
├── cli/                 # (unchanged) Clap CLI
└── klyntbot/            # (unchanged) Re-export facade
```

### Dependency layers

```
Layer 0: common
Layer 1: config, bus
Layer 1.5: storage (pool + migration runner + shared primitives)
Layer 2: tools-core (Tool trait, FeaturePackage, macros, ParamExtractor, ToolRegistry)
Layer 2.5: feature-todo, feature-finance (depend on common, storage, tools-core)
Layer 3: providers, session, scheduling, calendar, context_engine
Layer 4: channels, heartbeat
Layer 5: agent (imports feature crates, wires handler impls)
Layer 6: cli
Layer 7: klyntbot
```

### Migration map

| Current location | New location |
|---|---|
| `tools/src/todo/` | `feature-todo/src/tool/` |
| `tools/src/todo_types.rs` | `feature-todo/src/types.rs` |
| `tools/src/enrichment.rs` | `feature-todo/src/enrichment.rs` |
| `tools/src/embedding_engine.rs` | `feature-todo/src/embedding.rs` |
| `storage/repos/todo_repo.rs` | `feature-todo/src/storage/repo.rs` |
| `storage/rows/todo.rs` | `feature-todo/src/storage/rows.rs` |
| `config/schema/todo.rs` | `feature-todo/src/config.rs` |
| `agent/enrichment/` | `agent/handlers/todo.rs` |
| `tools/src/finance_tool/` | `feature-finance/src/tool/` |
| `tools/src/finance_types.rs` | `feature-finance/src/types.rs` |
| `tools/src/finance_handler.rs` | `feature-finance/src/handler.rs` |
| `storage/repos/finance_*.rs` | `feature-finance/src/storage/` |
| `config/schema/finance.rs` | `feature-finance/src/config.rs` |

### What stays in tools-core

- `Tool` trait + `DynTool` type alias
- `FeaturePackage` trait (new)
- `ToolRegistry`
- `ParamExtractor`
- `RoutingContext`
- `PermissionLevel` + `ToolPermissions`
- Derive macros
- Generic tools: filesystem, exec, web, message, ask_user, spawn, cron

## FeaturePackage Trait

```rust
#[async_trait]
pub trait FeaturePackage: Send + Sync {
    /// Unique name (e.g., "todo", "finance")
    fn name(&self) -> &str;

    /// The tool(s) this feature provides
    fn tools(&self) -> Vec<DynTool>;

    /// SQL migrations owned by this feature
    fn migrations(&self) -> Vec<Migration>;

    /// Config section key (e.g., "todo", "finance")
    fn config_key(&self) -> &str;

    /// Default config value
    fn default_config(&self) -> Value;

    /// Optional health check
    async fn health_check(&self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}
```

### Feature construction

```rust
pub struct TodoFeature { /* internal state */ }

impl TodoFeature {
    pub async fn new(pool: &PgPool, config: &Value) -> Result<Self> {
        // 1. Parse config into TodoConfig
        // 2. Construct repos from pool
        // 3. Build TodoTool with repos + config
        Ok(Self { /* ... */ })
    }
}
```

### Handler injection

Features define handler traits. Agent creates implementations and injects post-construction:

```rust
// Feature defines what it needs
pub trait TodoHandlers: Send + Sync {
    fn calendar(&self) -> Option<Arc<dyn CalendarHandler>>;
    fn enrichment(&self) -> Option<Arc<dyn EnrichmentHandler>>;
    fn embedding(&self) -> Option<Arc<dyn EmbeddingHandler>>;
}

// Feature accepts handlers
impl TodoFeature {
    pub fn inject_handlers(&mut self, handlers: impl TodoHandlers) { /* ... */ }
}
```

### Agent-side wiring

```rust
let features: Vec<Box<dyn FeaturePackage>> = vec![
    Box::new(TodoFeature::new(&pool, &config["todo"]).await?),
    Box::new(FinanceFeature::new(&pool, &config["finance"]).await?),
];

let mut registry = ToolRegistry::new();
for feature in &features {
    for tool in feature.tools() {
        registry.register_dyn(tool);
    }
}
```

## Derive Macros

### 1. `#[derive(ActionParams)]` — JSON Schema from Rust types

```rust
#[derive(ActionParams)]
pub struct AddParams {
    /// Task title
    #[param(required)]
    pub title: String,

    /// Task priority (1-5)
    #[param(min = 1, max = 5)]
    pub priority: Option<u8>,

    /// Tags for categorization
    #[param(default)]
    pub tags: Vec<String>,
}
```

Generates:
- `fn json_schema() -> Value` — complete JSON Schema
- `fn from_value(args: &Value) -> Result<Self>` — type-safe extraction

### 2. `#[tool_actions]` — Action dispatch routing

```rust
#[tool_actions]
impl TodoTool {
    #[action(name = "add")]
    async fn handle_add(&self, params: AddParams, ctx: &RoutingContext) -> Result<String> {
        // Business logic only
    }

    #[action(name = "list")]
    async fn handle_list(&self, params: ListParams, ctx: &RoutingContext) -> Result<String> {
        // ...
    }
}
```

Generates:
- `Tool::parameters()` — merged action schemas with discriminator
- `Tool::execute()` — routes to correct handler

### 3. `#[derive(DomainEnum)]` — Enum parsing + display

```rust
#[derive(DomainEnum)]
pub enum TodoStatus {
    #[aliases("pending", "open")]
    Todo,
    #[aliases("in_progress", "active")]
    Doing,
    #[aliases("completed", "closed")]
    Done,
    Archived,
}
```

Generates: `from_str_loose()`, `as_str()`, `Display`, `Serialize`/`Deserialize`, `FromStr`

## Storage Architecture

### Slimmed storage crate

```rust
pub struct StoragePool { pool: PgPool }

impl StoragePool {
    pub async fn connect(url: &str, migrations: Vec<FeatureMigration>) -> Result<Self>;
    pub fn pool(&self) -> &PgPool;
}

pub struct FeatureMigration {
    pub feature_name: String,
    pub version: i64,
    pub sql: String,
}
```

### Feature-owned migrations

```
crates/feature-todo/migrations/
├── 001_create_todos.sql
├── 002_add_attachments.sql
└── ...

crates/feature-finance/migrations/
├── 001_create_accounts.sql
├── 002_create_transactions.sql
└── ...
```

Tracked in `_feature_migrations` table (feature_name + version).

### Feature-owned repos

Each feature creates its own repos from the shared PgPool:

```rust
pub struct TodoRepo { pool: PgPool }

impl TodoRepo {
    pub fn new(pool: &PgPool) -> Self { Self { pool: pool.clone() } }
}
```

## Config Architecture

### Core config (slimmed)

```rust
pub struct CoreConfig {
    pub database_url: String,
    pub providers: ProvidersConfig,
    pub channels: ChannelsConfig,
    pub agents: AgentsConfig,
    pub tools: ToolsConfig,
    pub timezone: String,
}
```

### Feature config (owned by each feature)

Each feature defines and parses its own config from the raw JSON:

```rust
// feature-todo/src/config.rs
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TodoConfig {
    pub focus: TodoFocusConfig,
    pub enrichment: TodoEnrichmentConfig,
    // ...
}
```

Loading: Read raw JSON → parse core section → pass feature sections to features.

## Performance Optimizations

### 1. Async query parallelization

Use `tokio::try_join!()` for independent queries within actions.

### 2. Cursor-based pagination

```rust
pub struct Page<T> {
    pub items: Vec<T>,
    pub cursor: Option<String>,
    pub has_more: bool,
}
```

Added to all list operations.

### 3. Embedding batch queue

Queue embeds on mutations, flush periodically or on search.

### 4. Connection pool tuning

Per-feature pool size configuration.

### 5. Index optimization

Review and add composite indexes for common filter combinations.

## Impact Summary

| Aspect | Current | Proposed |
|---|---|---|
| Feature structure | Spread across 5 crates | Self-contained feature crate |
| Adding a feature | 10-15 steps, 5 crates | New crate + 1 line in agent |
| JSON Schema | Manual (50-380 lines) | Derived from Rust types |
| Action dispatch | Manual match statement | Generated by macro |
| Enum parsing | 25+ lines per enum | `#[derive(DomainEnum)]` |
| Param extraction | ParamExtractor calls | Typed params structs |
| Config | Monolithic struct | Feature-owned sections |
| Migrations | Central storage crate | Feature-owned SQL files |
| Storage | Central repos | Feature-owned repos, shared pool |
| Pagination | Missing | Cursor-based, universal |
| Query patterns | Sequential | Parallel where possible |
| Estimated LOC reduction | — | 40-50% of tool code |
