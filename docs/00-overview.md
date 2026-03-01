# Klyntbot Architecture Overview

## What is Klyntbot?

Klyntbot is a personal AI agent built as a single Rust binary. It connects to six chat platforms (Telegram, Discord, WhatsApp, Slack, Email, QQ), routes incoming messages through an intent classification pipeline, calls LLMs to generate responses, and executes tool calls against local storage, the filesystem, the web, and external services. It manages tasks, projects, goals, plans, calendars, finances, and persistent memory -- all backed by SQLite and LanceDB with zero external infrastructure dependencies.

It is designed as a personal assistant, not a code execution platform. Users interact with it through their preferred messaging app, and it handles everything from simple greetings to multi-step planned workflows autonomously.

## High-Level Architecture

The system is a single Rust binary composed of 20 workspace crates (plus 2 excluded crates) organized into 8 dependency layers. Each crate has a focused responsibility, and dependencies flow strictly upward -- lower layers never import from higher layers. The root `klyntbot` crate acts as a re-export facade, making all public types available through a single import path.

At runtime, the binary starts a gateway daemon that initializes storage, LLM providers, the message bus, background services, the agent loop, and channel integrations. All components communicate through an async message bus built on `tokio::mpsc` channels, keeping channels and the agent loop fully decoupled.

## Dependency Graph

Dependencies flow strictly upward. A crate at Layer N may only depend on crates at Layer N-1 or below. No circular dependencies exist -- this is enforced by Cargo's build system.

```
Layer 0  common
         Error types, MessageRole, ChannelName, ChatId, SessionKey

Layer 1  config, bus, tools-core, tools-core-macros
         Config schema (camelCase JSON), async message bus (tokio::mpsc),
         tool trait + macros

Layer 2  storage
         SQLite pool (sqlx), auto-migrations, row structs, repository pattern

Layer 3  providers, session, scheduling, calendar, context_engine, domain
         LLM HTTP clients, session persistence, cron service,
         CalDAV sync, token budget allocation + context assembly,
         domain models (Plan, PlanStatus)

Layer 4  tools, feature-todo, feature-finance, plugin-runtime
         Tool trait implementations (file I/O, web, message, spawn, cron,
         calendar, plan, goal, memory, learning), domain feature packages,
         WASM plugin host

Layer 5  channels, agent
         Chat platform integrations (Telegram, Discord, WhatsApp, Slack,
         Email, QQ), agent loop, intent pipeline, execution engines,
         skill manager, subagent manager, plan executor

Layer 6  cli
         Clap CLI with serve, init, status, and plugin commands

Layer 7  klyntbot
         Re-export facade (lib.rs) + binary entry point (main.rs)
```

## Execution Lifecycle

### 1. Startup (`klyntbot serve`)

The `handle_serve()` function in the CLI crate orchestrates the full boot sequence:

1. **Load configuration** -- reads `~/.klyntbot/config.json` with environment variable overrides (`KLYNTBOT_` prefix).
2. **Connect storage** -- opens (or creates) the SQLite database at `{data_dir}/data.db`, enables WAL mode and foreign keys, runs all pending migrations. Creates the `Repos` aggregate for repository access. Connects LanceDB for vector storage.
3. **Initialize LLM provider** -- resolves the configured model name to a provider (Anthropic, OpenAI, Groq, etc.) via keyword matching and creates the provider instance.
4. **Create message bus** -- allocates the dual-channel `MessageBus` with a buffer of 100 messages.
5. **Start cron service** -- loads persisted cron jobs from SQLite, registers built-in jobs (focus checks, daily digest, overdue checks, weekly report, calendar sync, daily planning, finance reviews), and begins the scheduling loop.
6. **Build agent loop** -- constructs the `AgentLoop` via its builder, wiring in the storage pool, cron service, notification dispatcher, intent pipeline, context engine, tool registry, session manager, and all background services (reminders, recurring tasks, learning, session cleanup, memory maintenance, plan cleanup).
7. **Initialize channels** -- creates a `ChannelManager` that instantiates all enabled channel implementations and takes ownership of the outbound bus receiver.
8. **Start heartbeat** -- begins the workspace heartbeat monitor (30-minute interval).
9. **Launch** -- spawns the agent loop and channel manager as background tokio tasks, then blocks on Ctrl+C / SIGTERM for graceful shutdown.

### 2. Message Ingestion

Each channel implementation (Telegram long-polling, Discord WebSocket, etc.) runs in its own tokio task. When a message arrives from a platform API:

1. The channel constructs an `InboundMessage` with channel name, sender ID, chat ID, content, timestamp, and optional media/metadata.
2. The channel publishes it to the bus via `bus.publish_inbound(msg)`.
3. The agent loop receives it from the inbound `mpsc::Receiver` (with a 1-second timeout for shutdown checks).

### 3. Intent Classification

The `IntentPipeline` classifies each message through a two-stage process:

1. **Heuristic classifier** -- zero-cost keyword matching that identifies greetings, simple CRUD commands, calendar operations, and other common patterns. If confidence exceeds the threshold (default 0.9), the heuristic result is used directly.
2. **LLM classifier** -- for ambiguous messages, a lightweight LLM call produces a structured classification with execution mode, complexity signals, confidence score, and relevant tool groups.

The result is an `IntentAnalysis` containing the selected `ExecutionMode` (Direct, Reactive, or Planned), complexity signals, confidence, and which tool groups are relevant.

### 4. Execution

The `ExecutionRouter` dispatches to the appropriate engine based on the classified mode:

- **Direct** -- a single LLM call with no tools. Used for greetings, acknowledgments, and simple factual questions. Maximum 1 iteration.
- **Reactive** -- a ReAct loop that interleaves LLM calls with tool execution. The LLM generates tool calls, the router executes them via the `ToolRegistry`, feeds results back, and repeats until the LLM produces a final text response or the iteration limit is reached. Used for task CRUD, searches, calendar operations.
- **Planned** -- multi-step plan generation and execution. The LLM creates a structured plan with ordered steps, then each step is executed through the ReAct loop. Used for complex multi-tool workflows requiring state tracking.

**Escalation chain**: if an engine signals it cannot handle the request (`EngineResult::Escalate`), the router automatically escalates: Direct -> Reactive -> Planned. Maximum escalations are configurable (default: 3).

Before execution, the `ContextEngine` assembles the prompt by allocating a token budget across system prompt, conversation history, tool definitions, and memory context based on the execution strategy.

### 5. Response Delivery

After execution completes:

1. The `ResponseValidator` checks the output (non-empty, within length limits) and produces validation warnings if needed.
2. The `CostTracker` records token usage and the `StrategyRepo` logs the execution outcome (predicted vs. actual mode, escalation count, latency, success).
3. The response is saved to the session (conversation history in SQLite).
4. An `OutboundMessage` is published to the bus via `bus.publish_outbound(msg)`.
5. The `ChannelManager`'s outbound dispatcher receives the message, looks up the target channel by name, and calls `channel.send(&msg)` to deliver it through the platform API.

### 6. Background Services

Several services run independently of the request-response cycle:

- **Cron scheduler** -- triggers time-based jobs (task digests, overdue checks, calendar sync, daily planning, finance reviews) by publishing `InboundMessage`s to the bus as if they were user messages from a "system" channel.
- **Reminder engine** -- monitors upcoming calendar events and task deadlines, dispatches notifications through the bus.
- **Recurring task spawner** -- creates new task instances based on recurrence rules.
- **Conversation embedding** -- asynchronously generates vector embeddings for messages using fastembed, stored in LanceDB for semantic memory retrieval.
- **Plan execution** -- runs multi-step plans in a dedicated worker task, processing one step at a time with backtracking on failure.
- **Session cleanup** -- periodically expires old sessions.
- **Memory maintenance** -- consolidates and prunes memory notes.
- **Plan cleanup** -- removes stale silent/on_failure plans past their retention window.
- **Learning service** -- updates adaptive confidence thresholds based on execution outcome feedback.
- **Heartbeat monitor** -- periodic workspace health checks.

## Data Flow Diagram

```
                           Platform APIs
                    (Telegram, Discord, Slack,
                     WhatsApp, Email, QQ)
                              |
                              v
              +-------------------------------+
              |        Channel Manager        |
              |   (one task per channel)       |
              +-------------------------------+
                    |                   ^
           InboundMessage          OutboundMessage
                    |                   ^
                    v                   |
              +-------------------------------+
              |          Message Bus           |
              |    (tokio::mpsc channels)      |
              |  inbound_tx/rx  outbound_tx/rx |
              +-------------------------------+
                    |                   ^
                    v                   |
              +-------------------------------+
              |          Agent Loop            |
              |   session mgmt, embedding     |
              +-------------------------------+
                    |
                    v
              +-------------------------------+
              |       Intent Pipeline          |
              |  Analyzer -> Context Engine    |
              |  -> Router -> Validator        |
              +-------------------------------+
                    |
                    v
              +-------------------------------+
              |      Execution Engines         |
              |  Direct | Reactive | Planned   |
              +-------------------------------+
                    |                   |
                    v                   v
              +-----------+     +-------------+
              |    LLM    |     |    Tools     |
              | Providers |     |  (Registry)  |
              +-----------+     +-------------+
                                       |
                                       v
                          +------------------------+
                          |       Storage           |
                          |  SQLite     LanceDB     |
                          |  (relational) (vectors) |
                          +------------------------+
```

Cron jobs, reminders, and heartbeat inject messages into the bus as synthetic `InboundMessage`s from a "system" channel, reusing the same processing pipeline.

## Storage Architecture

All persistent state lives on disk with no external database servers required.

### SQLite (relational data)

A single file at `{data_dir}/data.db` (default `~/.klyntbot/data.db`) stores all structured data:

- **Sessions** -- conversation history per channel:chat_id pair
- **Todos** -- tasks with status, priority, due dates, focus state, time entries, attachments, dependencies
- **Projects** -- project metadata and task associations
- **Goals** -- high-level objectives linked to projects
- **Plans** -- multi-step execution plans with step state, backtrack history, visibility rules
- **Cron jobs** -- persisted scheduled job definitions
- **Usage records** -- LLM token consumption per request
- **Strategy records** -- execution outcome telemetry (predicted vs. actual mode, latency, satisfaction)
- **Learning state** -- adaptive threshold parameters
- **Memory notes** -- persistent agent memory
- **Calendar cache** -- synced calendar events and sync state
- **Finance data** -- accounts, transactions, budgets, investments, liabilities, goals
- **Agent tasks** -- subagent task board entries

The pool uses WAL mode for concurrent read/write access and enables foreign keys for referential integrity. All migrations run automatically on startup via sqlx's embedded migration system. Feature-owned crates (todo, finance) can register their own migrations through the `FeatureMigration` mechanism.

The `Repos` aggregate struct provides convenient access to all repository types from a single `StoragePool`, and every `*Repo` struct holds a cloneable `SqlitePool` internally (no `Arc<RwLock<>>` wrappers needed).

### LanceDB (vector embeddings)

A directory at `{data_dir}/lance/` stores vector embeddings generated by fastembed (paraphrase-multilingual-MiniLM-L12-v2, 384 dimensions):

- **Todo embeddings** -- enables semantic search across tasks ("login bug" finds "authentication issue")
- **Conversation embeddings** -- enables semantic memory retrieval from past conversations

LanceDB provides approximate nearest neighbor (ANN) search with cosine similarity. Hybrid search merges keyword (SQL substring) and semantic (ANN) results via Reciprocal Rank Fusion (RRF).

## Key Design Decisions

### Why a single binary?

Klyntbot is a personal agent -- it runs on one machine for one user. A single binary eliminates container orchestration, inter-service networking, service discovery, and deployment complexity. `cargo build --release` produces one statically-linked executable with LTO and symbol stripping. Deploy by copying a file.

### Why message bus decoupling?

The `MessageBus` (two `tokio::mpsc` channels for inbound and outbound) decouples channels from the agent loop completely. Channels know nothing about the agent -- they publish `InboundMessage`s and consume `OutboundMessage`s. The agent knows nothing about channels -- it reads from one receiver and writes to another. This means:

- Adding a new channel requires zero changes to the agent.
- Background services (cron, heartbeat, reminders) inject messages through the same bus, reusing the full processing pipeline.
- Channels and the agent can be tested independently.

### Why dependency inversion?

Several tools need agent-level functionality (spawning subagents, managing cron jobs, syncing calendars, enriching tasks). Direct imports would create circular dependencies between `tools` (Layer 4) and `agent` (Layer 5). Instead, handler traits (`SpawnHandler`, `CronHandler`, `CalendarHandler`, `EnrichmentHandler`) are defined in `tools` or `tools-core` and implemented in `agent`. The concrete implementations are injected as `Arc<dyn Trait>` during agent construction. This keeps the dependency graph acyclic while allowing tools to access agent capabilities.

### Why SQLite + LanceDB?

**SQLite**: zero-configuration, file-based, embedded. No database server to install or manage. WAL mode provides excellent concurrent performance for a single-user workload. sqlx gives compile-time query checking and automatic migrations. The entire database is a single file that can be backed up by copying.

**LanceDB**: purpose-built for vector search, also file-based and embedded. Provides efficient ANN queries for semantic search without requiring a vector database server. Keeps the "single binary, no infrastructure" property intact.

### Why feature packages?

Feature packs (task-management, productivity, ai-intelligence, finance, weather, skill-creator) let users choose which capabilities to enable. This serves two purposes:

1. **Reduced cognitive load** -- only relevant tools and skills are exposed to the LLM, improving response quality by narrowing the action space.
2. **Progressive disclosure** -- new users start with core task management and add complexity (finance tracking, AI learning) as needed.

Packs are selected during `klyntbot init` and stored in config. At startup, the `SkillManager` filters built-in skills to only those from enabled packs. Feature crates (`feature-todo`, `feature-finance`) register their own tools and migrations, keeping domain logic isolated from the core agent.
