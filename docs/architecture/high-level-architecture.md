# High-Level Architecture

## Layered Crate Architecture

Klyntbot uses a strict 9-layer architecture with 34 workspace crates. Dependencies flow **upward only** — lower layers have zero knowledge of higher layers.

```
L8  klyntbot, klyntbot-server              Re-export facade, MCP binary
     |
L7  app-core, desktop-shared, desktop      Application core, Tauri adapter
     |
L6  mcp                                    MCP server/client protocol
     |
L5  channels, agent, cognitive             Platform integrations, AI runtime, memory
     |
L4  tools, feature-tasks, feature-finance, Feature packages, plugins,
    feature-notes, feature-productivity,    activity log, autotuner
    feature-coaching, feature-insights,
    feature-launcher, feature-learning,
    activity-log, plugin-runtime, autotuner
     |
L3  providers, session, scheduling,        LLM clients, session persistence,
    context_engine, skill-system            cron, context budgets, skill routing
     |
L2  storage                                SQLite pool, migrations, repos
     |
L1  config, bus, tools-core,               Config schema, event bus,
    tools-core-macros, analytics            Tool/FeaturePackage traits, macros
     |
L0  common, platform-macos                 KlyntbotError, MessageRole, macOS APIs
```

## Dependency Inversion Pattern

Lower layers define traits; upper layers implement them. Injected as `Arc<dyn Trait>`:

```
L3 (cognitive) defines:     L5 (agent) implements:
  ExtractionHandler    -->    LlmExtractionHandler
  ConsolidationHandler -->    LlmConsolidationHandler
  NarrativeHandler     -->    LlmNarrativeHandler

L3 (scheduling) defines:    L5 (agent) implements:
  SpawnHandler         -->    AgentSpawnHandler
  CronHandler          -->    AgentCronHandler

L4 (tools) defines:         L5 (agent) implements:
  DelegationHandler    -->    AgentDelegationHandler
```

This allows the cognitive memory system (L5) to call LLMs for extraction without depending on the provider crate (L3) directly — the trait is injected at application startup.

## Application Core Pattern

```
+------------------+     +-------------------+     +------------------+
|  Desktop (Tauri) |     |  Dev Server (HTTP)|     |  MCP Server      |
|  Thin adapter    |     |  Debug-only       |     |  stdio/HTTP      |
|  #[tauri::cmd]   |     |  port 3456        |     |  rmcp handler    |
+--------+---------+     +--------+----------+     +--------+---------+
         |                        |                         |
         v                        v                         v
+--------+------------------------+-------------------------+----------+
|                             AppCore                                  |
|  +-------------+  +----------------+  +--------------+  +----------+ |
|  | Handlers    |  | AgentLoop      |  | CronService  |  | Mirror   | |
|  | (chat,      |  | (message rx,   |  | (scheduled   |  | Engine   | |
|  |  tasks,     |  |  pipeline,     |  |  jobs)       |  | (self-   | |
|  |  settings)  |  |  session mgmt) |  |              |  |  reflect)| |
|  +-------------+  +----------------+  +--------------+  +----------+ |
|  +-------------+  +----------------+  +--------------+              |
|  | Repos       |  | AgentRuntime   |  | Background   |              |
|  | (storage    |  | (orchestration,|  | Consolidation|              |
|  |  access)    |  |  tool exec)    |  | Service      |              |
|  +-------------+  +----------------+  +--------------+              |
+----------------------------------------------------------------------+
```

**`AppCore`** (35+ fields) is the central state container. It holds:
- `StoragePool` + `Repos` (database access)
- `AgentLoop` (message processing pipeline)
- `CronService` (scheduled jobs)
- `MirrorFacade` (self-reflection)
- `DomainEventBus` (event system)
- `HotConfig` (hot-reloadable settings)
- `ToolRegistry` (shared tool definitions)
- `EmbeddingEngine` (vector embeddings)
- All feature-specific repos and services

Desktop commands are thin adapters that call `AppCore` methods:

```rust
// Desktop command (thin adapter)
#[tauri::command]
async fn task_create(state: State<'_, Arc<AppCore>>, input: CreateTaskInput)
    -> Result<TaskResponse, ApiError> {
    state.handlers.tasks.create(input).await
}
```

The dev server mirrors every Tauri command via generic HTTP dispatch:

```rust
// Dev server dispatch (same handler)
async fn dispatch_dev(cmd: &str, core: &AppCore, body: Value)
    -> Option<Result<Value, ApiError>> {
    match cmd {
        "task_create" => Some(core.handlers.tasks.create(from_value(body)?).await),
        // ...
    }
}
```

A compile-time test (`dev_server_covers_all_tauri_commands`) enforces parity between Tauri commands and dev server dispatch.

## Error Handling

Central `KlyntbotError` enum with typed sub-errors:

```
KlyntbotError
  +-- Tool(ToolError)          -- tool execution failures
  +-- Provider(ProviderError)  -- LLM API errors (rate limit, auth, invalid response)
  +-- Channel(ChannelError)    -- platform integration errors
  +-- Session(SessionError)    -- session management errors
  +-- Config(ConfigError)      -- config loading/parsing errors
  +-- Storage(String)          -- database errors
  +-- StorageNotFound          -- entity not found
  +-- StorageConflict          -- unique constraint violations
  +-- Timeout(String)          -- pipeline timeouts
  +-- Cron(String)             -- scheduling errors
  +-- Bus(String)              -- event bus errors
```

All sub-errors implement `From` for automatic conversion via `?` operator across crate boundaries. `common::Result<T>` is the universal alias.

## Configuration System

```
config.json (camelCase)
     |
     v
+----+------+                    +-------------+
|  Config   | -- hot-reload -->  |  HotConfig  |
|  (full)   |    5s file watch   |  (subset)   |
+-----------+                    +------+------+
                                        |
                                 Arc<RwLock<HotConfig>>
                                        |
                          Shared between AppCore + AgentRuntime
```

**Hot-reloadable fields** (no restart needed): `model`, `temperature`, `max_tokens`, `max_tool_iterations`, `pipeline_timeout_secs`, `monthly_budget_usd`.

**Env overrides**: `KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o` (double-underscore path navigation).

**Config save optimization**: Only diffs from defaults are serialized — minimizes file noise.

## Event System

Four independent event buses:

| Bus | Transport | Pattern | Purpose |
|-----|-----------|---------|---------|
| `DomainEventBus` | `tokio::broadcast` | Fan-out (every subscriber sees every event) | 40+ domain events across all features |
| `MessageBus` | `tokio::mpsc` | Single-consumer (taken once) | Inbound/outbound message routing |
| `ContextUpdateQueue` | `Mutex<Vec>` | Lock-free drain | Live context injection during ReAct loop |
| `LearningEventBus` | `tokio::broadcast` | Fan-out | Adaptive threshold changes |

The `DomainEventBus` carries ~40 event variants covering: productivity, tasks, finance, notes, chat turns, tool calls, memory operations, skill routing, autotuner decisions, and mirror self-reflection events.

## Concurrency Model

- **Tokio multi-thread runtime** — managed by Tauri in desktop mode
- **Per-session mutex** for session state (`Arc<TokioMutex<Session>>`)
- **Per-session cancellation** via `DashMap<String, CancellationToken>` for stream abort
- **Parallel tool execution** via `Semaphore(10)` in ReAct loop
- **Global shutdown** via `CancellationToken` propagated to all background tasks
- **No `Arc<RwLock>` on storage** — `StoragePool` is `Clone+Send+Sync` via internal `Arc<SqlitePool>`
