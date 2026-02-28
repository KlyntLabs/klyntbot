# Architecture Overview

Klyntbot is a Rust personal AI agent — a single binary that connects to 6+ chat platforms, calls LLMs, manages tasks/projects, syncs with Apple Calendar, and maintains persistent memory. All state lives in SQLite (relational) + LanceDB (vector embeddings). No external database required.

## System Map

```
                          ┌─────────────┐
                          │   klyntbot   │  Layer 7: Re-export facade + binary
                          └──────┬───────┘
                                 │
                    ┌────────────┴────────────┐
                    │                         │
              ┌─────┴─────┐           ┌───────┴───────┐
              │    cli     │           │   dashboard   │  Layer 6: Entry points
              │  (9,984)   │           │   (4,239)     │
              └─────┬──────┘           └───────┬───────┘
                    │                          │
              ┌─────┴──────────────────────────┘
              │
        ┌─────┴─────┐
        │   agent    │  Layer 5: Integration hub (26,930 LOC — 28% of codebase)
        │            │  Intent pipeline, plan execution, learning, memory,
        │            │  skills, subagents, enrichment, reminders
        └─────┬──────┘
              │
   ┌──────────┼──────────────────────────┐
   │          │                          │
┌──┴───┐ ┌───┴────┐  ┌──────────┐ ┌─────┴──────┐
│ tools │ │channels│  │feature-  │ │  plugin-   │  Layer 3-4: Tools, Channels,
│(17K) │ │ (13K)  │  │todo/fin  │ │  runtime   │  Feature packs, Plugins
└──┬───┘ └───┬────┘  └────┬─────┘ └────────────┘
   │         │            │
   │    ┌────┴──┐    ┌────┴────────────────────────────┐
   │    │       │    │                                  │
┌──┴────┴──┐ ┌──┴────┴──┐ ┌──────────┐ ┌────────────┐ │
│ providers│ │ storage   │ │ calendar │ │context_eng │ │  Layer 1-2: Services
│  (9,130) │ │ (10,964)  │ │ (3,862)  │ │  (4,266)   │ │
└──┬───────┘ └──┬────────┘ └──┬───────┘ └──┬─────────┘ │
   │            │             │            │            │
┌──┴────┐  ┌───┴───┐  ┌──────┴────┐  ┌────┴───┐  ┌────┴──┐
│config │  │ bus   │  │scheduling │  │session │  │ plan  │  Layer 0-1: Foundation
│(5,003)│  │(1,424)│  │ (1,800)   │  │(1,362) │  │(1,266)│
└──┬────┘  └──┬────┘  └───────────┘  └────────┘  └───────┘
   │          │
┌──┴──────────┴──┐
│     common     │  Layer 0: Error types, core types, utilities
│    (4,880)     │
└────────────────┘
```

## Dependency Layers

Dependencies flow **strictly upward**. No circular dependencies — enforced by Cargo.

| Layer | Crates | Purpose |
|-------|--------|---------|
| 0 | `common`, `tools-core-macros` | Error types (12 variants), core types (`MessageRole`, `ChannelName`, `ChatId`, `SessionKey`), proc macros |
| 1 | `config`, `bus`, `tools-core` | Config schema (camelCase JSON), async message bus (tokio::mpsc), tool trait + registry |
| 1.5 | `storage` | SQLite pool (sqlx), auto-migrations, 21 repositories, LanceDB vector store |
| 2 | `providers`, `context_engine`, `calendar`, `scheduling`, `session`, `goal`, `plan` | LLM HTTP clients, token budgets, CalDAV sync, cron service, session persistence, domain types |
| 3 | `tools` | 12+ tool implementations (file I/O, web, task, calendar, plan, memory, learning) |
| 4 | `channels`, `heartbeat`, `feature-todo`, `feature-finance`, `plugin-runtime` | 6 chat platforms, feature packs, WASM plugin host |
| 5 | `agent` | Agent loop, intent pipeline, plan executor, learning, memory, skills, subagents |
| 6 | `cli`, `dashboard` | CLI commands + wizard, REST API + web dashboard |
| 7 | `klyntbot` | Re-export facade + binary entry point |

## Crate Size Distribution

| Crate | LOC | % of Total | Role |
|-------|-----|-----------|------|
| agent | 26,930 | 28.5% | Integration hub |
| tools | 17,036 | 18.0% | Tool implementations |
| channels | 13,308 | 14.1% | Chat platforms |
| storage | 10,964 | 11.6% | Data persistence |
| cli | 9,984 | 10.6% | CLI + wizard |
| providers | 9,130 | 9.7% | LLM providers |
| feature-finance | 7,259 | 7.7% | Finance tools |
| config | 5,003 | 5.3% | Configuration |
| common | 4,880 | 5.2% | Foundation types |
| feature-todo | 4,838 | 5.1% | Task management |
| context_engine | 4,266 | 4.5% | Context assembly |
| dashboard | 4,239 | 4.5% | REST API |
| calendar | 3,862 | 4.1% | CalDAV sync |
| plugin-runtime | 2,468 | 2.6% | WASM plugins |
| tools-core | 2,150 | 2.3% | Tool trait system |
| scheduling | 1,800 | 1.9% | Cron jobs |
| tools-core-macros | 1,460 | 1.5% | Proc macros |
| bus | 1,424 | 1.5% | Event bus |
| session | 1,362 | 1.4% | Session management |
| plan | 1,266 | 1.3% | Plan types |
| goal | 764 | 0.8% | Goal types |
| heartbeat | 464 | 0.5% | Health monitoring |
| **Total** | **~94,500** | **100%** | |

## Data Flow

### Chat Command Flow

```
User input → CLI (handle_chat)
  → Config + Provider loaded
  → StoragePool::connect(data_dir)
  → AgentLoop::new(provider, config, storage, tools)
  → IntentPipeline::process_message()
    → IntentAnalyzer (heuristics → LLM classifier)
    → ContextEngine (token budget, memory retrieval)
    → ExecutionRouter → Engine (Direct | Reactive | Planned)
    → ResponseValidator
    → CostTracker
  → Response streamed to terminal
```

### Serve Command Flow (Gateway Daemon)

```
klyntbot serve --port 8080
  → Config + Provider loaded
  → StoragePool::connect(data_dir) + VectorStore
  → MessageBus::new(100)  ←──────────────────────────┐
  → CronService::new(storage)                         │
  → ChannelManager::new(config, bus)                   │
    → TelegramChannel  ──→ bus.send_inbound() ────────┤
    → DiscordChannel   ──→ bus.send_inbound() ────────┤
    → SlackChannel     ──→ bus.send_inbound() ────────┤
    → WhatsAppChannel  ──→ bus.send_inbound() ────────┤
    → EmailChannel     ──→ bus.send_inbound() ────────┤
    → QQChannel        ──→ bus.send_inbound() ────────┘
  → AgentLoop listens on bus
    → IntentPipeline::process_message()
    → bus.send_outbound() → ChannelManager dispatches
  → HeartbeatService (health monitoring)
  → DashboardServer on :port (REST API)
  → Signal handler for graceful shutdown
```

### Tool Execution Flow

```
IntentPipeline → ExecutionCore
  → ToolRegistry.get(tool_name)  // Arc<dyn Tool> clone
  → Tool::execute(args, RoutingContext)
    → Tool accesses storage via Repos
    → Tool may call LLM via provider
    → Tool returns String result
  → Result fed back to LLM for next cycle
  → Repeat until LLM produces final response
```

## Key Design Patterns

### Repository Pattern
All persistent state flows through `*Repo` structs in the `storage` crate. Repos hold a `SqlitePool` (Clone + Send + Sync internally via Arc), eliminating `Arc<RwLock<Store>>` wrappers. The `Repos` aggregate provides convenient grouped access.

### Dependency Inversion
Traits defined in lower layers, implemented in higher layers:

| Trait | Defined In | Implemented In |
|-------|-----------|---------------|
| `SpawnHandler` | tools (L3) | agent (L5) |
| `CronHandler` | tools (L3) | agent (L5) |
| `CalendarHandler` | tools (L3) | agent (L5) |
| `EnrichmentHandler` | tools (L3) | agent (L5) |
| `EmbeddingHandler` | feature-todo (L4) | agent (L5) |

Injected as `Arc<dyn Trait>` at construction time. This breaks circular dependencies between tools and agent.

### Re-export Facade
`src/lib.rs` re-exports all workspace crate types. Integration tests and external consumers use `klyntbot::AgentLoop`, `klyntbot::StoragePool`, etc.

### Provider Auto-detection
The provider registry matches model name keywords to route to the correct LLM provider (Anthropic, OpenAI-compatible). No external routing library.

### Feature Packs
Domain modules (`feature-todo`, `feature-finance`) implement the `FeaturePackage` trait for standardized setup (tools, migrations, config, health checks). Packs are selected during `klyntbot init` and filter which skills/tools are available at runtime.

## Storage Stack

```
StoragePool::connect(data_dir)
  → Creates/opens {data_dir}/data.db (SQLite)
  → Enables WAL mode + foreign keys
  → Runs auto-migrations
  → Returns StoragePool (wraps SqlitePool)

Repos::from_pool(&pool)
  → TodoRepo, ProjectRepo, PlanRepo, SessionRepo,
    CronRepo, GoalRepo, UsageRepo, FinanceRepo,
    LearningStateRepo, MemoryNoteRepo, CalendarEventCacheRepo,
    DecisionLogRepo, StrategyRepo, OutcomeRepo, AgentTaskRepo

VectorStore (LanceDB)
  → {data_dir}/lancedb/
  → Todo embeddings, conversation embeddings
  → fastembed (paraphrase-multilingual-MiniLM-L12-v2, 384 dims)
```

## Feature Flags

| Feature | Crate | Default | Purpose |
|---------|-------|---------|---------|
| `email` | channels, klyntbot | ON | IMAP/SMTP email channel |
| `semantic-search` | tools | ON | LanceDB + fastembed embeddings |
| `browser-integration` | tools, klyntbot | OFF | Browser automation tool |
| `plugin-integration` | plugin-runtime, klyntbot | OFF | WASM plugin support |

## Document Index

| Doc | Subsystem | Key Files |
|-----|-----------|-----------|
| [01-common](01-common.md) | Error types, core types, utilities | `crates/common/src/` |
| [02-config](02-config.md) | Config schema, loading, env overrides | `crates/config/src/` |
| [03-storage](03-storage.md) | SQLite + LanceDB, repos, migrations | `crates/storage/src/` |
| [04-providers](04-providers.md) | LLM providers, streaming, auto-detection | `crates/providers/src/` |
| [05-context-engine](05-context-engine.md) | Token budgets, context assembly | `crates/context_engine/src/` |
| [06-tools](06-tools.md) | Tool trait, registry, implementations | `crates/tools/src/`, `crates/tools-core/src/` |
| [07-channels](07-channels.md) | Channel trait, 6 platforms | `crates/channels/src/` |
| [08-agent-core](08-agent-core.md) | Agent loop, builder, execution | `crates/agent/src/agent_loop/` |
| [09-intent-pipeline](09-intent-pipeline.md) | Heuristics, classifier, engines, router | `crates/agent/src/intent_pipeline/` |
| [10-planning-engine](10-planning-engine.md) | Plans, steps, backtracking | `crates/agent/src/plan_*/`, `crates/plan/` |
| [11-feature-todo](11-feature-todo.md) | Todo tool, enrichment, search | `crates/feature-todo/src/` |
| [12-feature-finance](12-feature-finance.md) | Finance tool suite | `crates/feature-finance/src/` |
| [13-learning-memory](13-learning-memory.md) | Learning, memory, embeddings | `crates/agent/src/learning/`, `memory.rs` |
| [14-cli-wizard](14-cli-wizard.md) | CLI commands, wizard, packs | `crates/cli/src/` |
| [15-dashboard](15-dashboard.md) | REST API, axum server | `crates/dashboard/src/` |
| [16-plugin-system](16-plugin-system.md) | WASM runtime, SDK | `crates/plugin-runtime/src/` |
| [17-scheduling-reminders](17-scheduling-reminders.md) | Cron, reminders, recurring tasks | `crates/scheduling/src/`, agent reminders |
| [18-calendar](18-calendar.md) | CalDAV, sync engine, providers | `crates/calendar/src/` |
| [19-bus-events](19-bus-events.md) | Message bus, event types | `crates/bus/src/` |
