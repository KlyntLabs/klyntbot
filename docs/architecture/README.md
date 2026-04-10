# Architecture Overview

Klyntbot is a Rust personal AI agent built as a 37-crate workspace organized in 9 strictly-layered tiers. Dependencies flow upward only — no circular dependencies. The system connects multiple chat platforms (Telegram, Discord, Slack, Email) to LLMs with persistent cognitive memory, 20+ tools, and extensibility via WASM plugins and MCP.

This document provides a map of the architecture. Each subsystem has its own deep-dive doc linked below.

## System Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│                        User Interfaces                               │
│  ┌──────────┐  ┌─────────┐  ┌───────┐  ┌───────┐  ┌─────────────┐ │
│  │ Desktop   │  │Telegram │  │Discord│  │ Slack │  │ Email/IMAP  │ │
│  │ (Tauri 2) │  │         │  │       │  │       │  │             │ │
│  └─────┬─────┘  └────┬────┘  └───┬───┘  └───┬───┘  └──────┬──────┘ │
└────────┼──────────────┼──────────┼──────────┼──────────────┼────────┘
         │              └──────────┼──────────┘              │
         ▼                         ▼                         ▼
┌─────────────────┐      ┌─────────────────┐      ┌─────────────────┐
│   App Core       │      │   Message Bus    │      │  Channel Mgr   │
│ (business logic) │◀────▶│  (inbound/out)   │◀────▶│  (routing)     │
└────────┬─────────┘      └────────┬─────────┘      └────────────────┘
         │                         │
         ▼                         ▼
┌──────────────────────────────────────────────────────────────────────┐
│                        Agent Runtime                                  │
│  ┌──────────────┐  ┌────────────────┐  ┌──────────────────────────┐ │
│  │ Context      │  │ Execute Loop   │  │ Tool Registry            │ │
│  │ Engine       │  │ (budget-aware  │  │ (20+ tools, MCP, WASM)   │ │
│  │ (assembly)   │  │  LLM↔tool)     │  │                          │ │
│  └──────┬───────┘  └───────┬────────┘  └────────────┬─────────────┘ │
│         │                  │                         │               │
│         ▼                  ▼                         ▼               │
│  ┌──────────────┐  ┌────────────────┐  ┌──────────────────────────┐ │
│  │ LLM Provider │  │ Skill System   │  │ Feature Packages         │ │
│  │ (Anthropic,  │  │ (orchestrators │  │ (tasks, finance, notes,  │ │
│  │  OpenAI, ...) │  │  activation)   │  │  productivity, ...)      │ │
│  └──────────────┘  └────────────────┘  └────────────┬─────────────┘ │
└──────────────────────────────────────────────────────┼───────────────┘
                                                       │
┌──────────────────────────────────────────────────────┼───────────────┐
│                     Cognitive Layer                    │               │
│  ┌──────────────┐  ┌────────────────┐  ┌─────────────┴─────────────┐│
│  │ Semantic     │  │ Mirror         │  │ Reforge                    ││
│  │ Memory       │  │ (self-         │  │ (strategy                  ││
│  │ (FSRS5,      │  │  reflection)   │  │  reflection)               ││
│  │  12-factor)  │  │                │  │                            ││
│  └──────────────┘  └────────────────┘  └────────────────────────────┘│
└──────────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────┼────────────────────────────────────────┐
│                     Storage Layer                                     │
│  ┌──────────────┐  ┌────────────────┐  ┌──────────────────────────┐ │
│  │ SQLite       │  │ LanceDB        │  │ Config                   │ │
│  │ (WAL mode)   │  │ (vectors)      │  │ (hot-reload)             │ │
│  └──────────────┘  └────────────────┘  └──────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────┘
```

## Crate Hierarchy (9 Layers)

```
L0: common, platform-macos
    Foundation types (KlyntbotError, SessionKey, MessageRole, ChannelName, ChatId)
    macOS native APIs (pasteboard, window management)

L1: config, bus, tools-core, tools-core-macros, analytics
    Config schema (camelCase JSON, hot-reload, Secret<T>)
    Message bus (DomainEventBus, MessageBus, ContextUpdateQueue)
    Tool trait, FeaturePackage trait, derive macros
    FIRE/Monte Carlo pure analytics

L2: storage
    StoragePool (SQLite WAL), migrations, 25+ Repo structs
    Vector store (LanceDB, 384-dim fastembed)

L3: providers, session, scheduling, context_engine, skill-system
    LLM provider abstraction (Anthropic, OpenAI-compat)
    Session persistence (LRU + SQL)
    Cron scheduling, token budgets, skill routing

L4: tools, feature-tasks, feature-finance, feature-notes,
    feature-productivity, feature-coaching, feature-insights,
    feature-launcher, feature-learning, feature-language-learning,
    activity-log, plugin-runtime, autotuner, voice-engine, simulator
    20+ built-in tools, 9 feature packages, WASM plugins
    Self-optimization experiments, voice synthesis, agent simulation

L5: channels, agent, cognitive
    Platform integrations (Telegram, Discord, Slack, Email)
    Agent runtime (execute loop, budget, compression, fabrication detection)
    Cognitive memory (FSRS5, 12-factor relevance, mirror, reforge)

L6: mcp
    MCP client (connect to external servers) and server (expose tools)

L7: app-core, desktop-shared, desktop
    AppCore (transport-agnostic business logic, 180+ handlers)
    IPC types, Tauri 2 desktop app

L8: klyntbot, klyntbot-server
    Re-export facade, standalone MCP server binary
```

**Rule**: Dependencies flow strictly upward. L5 cannot import from L7. L2 cannot import from L4. This is enforced by Cargo's dependency graph.

## Message Flow

```
1. User sends message via Telegram/Discord/Slack/Email/Desktop
2. Channel adapter parses → InboundMessage → MessageBus
3. AgentLoop picks up message, loads/creates Session
4. ContextEngine assembles: system prompt + history + memories + tool schemas
   └── Token budget waterfall: Identity → Task → Tools → History → Memory → Skills
5. Execute loop runs LLM↔tool cycles within budget (Normal/DeepThink/Ultra)
   ├── Mid-loop compression at 70% context usage
   ├── Live context refresh from cognitive/productivity systems
   ├── Fabrication detection for misbehaving models
   └── Streaming events to UI (content chunks, tool status, budget)
6. Response validated, session updated
7. Recording (async): token usage, retrieval feedback, learning signals
8. OutboundMessage → MessageBus → Channel adapter → external API
9. DomainEvents emitted → cognitive layer extracts facts → memory consolidation
```

## Documentation Map

### Core Systems (start here)

| Document | Description | Depth |
|----------|-------------|-------|
| [Agent Runtime](agent-runtime.md) | Execution pipeline, budget system, execute loop, compression, fabrication detection | Deep |
| [Cognitive Memory](cognitive-memory.md) | Memory types, FSRS5 decay, 12-factor relevance, mirror self-reflection, reforge | Deep |
| [Context Engine](context-engine.md) | Context assembly, token budgets, pluggable sources, history compression | Medium |

### Infrastructure

| Document | Description |
|----------|-------------|
| [Core Infrastructure](core-infrastructure.md) | Foundation crates: errors, config, bus, storage, tool framework |
| [Skill System](skill-system.md) | Skills, MCP integration, WASM plugins |

### Application Layer

| Document | Description |
|----------|-------------|
| [Features](features.md) | Feature packages: tasks, finance, notes, productivity, coaching |
| [Channels](channels.md) | Platform integrations: Telegram, Discord, Slack, Email |
| [Desktop App](desktop-app.md) | Tauri 2, AppCore, React 19 frontend |

## Design Principles

1. **Budget-aware execution** — Every LLM interaction operates within explicit token and turn budgets. No unbounded loops, no surprise costs.

2. **Layered crate architecture** — 37 crates in 9 layers with strict upward-only dependencies. Prevents circular deps, enables independent testing.

3. **Transport-agnostic core** — `AppCore` holds all business logic with no knowledge of Tauri, HTTP, or WebSocket. Desktop, dev server, and MCP all adapt the same core.

4. **Event-driven cognition** — `DomainEventBus` broadcasts 85+ event types. The cognitive layer subscribes to all of them for pattern extraction. No direct coupling between features and memory.

5. **Memory that decays** — FSRS5 spaced-repetition decay ensures frequently-useful facts stay accessible while rarely-accessed knowledge gracefully fades. No manual curation needed.

6. **Self-reflection** — The mirror system watches the agent's own behavior, detects patterns, and proposes procedural rules. Reforge reviews strategy files nightly and suggests improvements.

7. **Feature isolation** — Each feature is a self-contained package declaring its tools, migrations, config, and health checks. New features don't require modifying the agent core.

8. **Local-first** — All data in SQLite + LanceDB. No cloud dependency for storage. The agent works offline.

## Tech Stack

### Backend (Rust)

| Component | Technology |
|-----------|-----------|
| Async runtime | tokio 1.x |
| Database | sqlx (SQLite, WAL mode, FTS5) |
| Vectors | LanceDB + fastembed (384-dim) |
| LLM providers | Custom abstraction (Anthropic, OpenAI-compat) |
| MCP | rmcp |
| WASM plugins | Extism |
| HTTP server | axum |
| Desktop | Tauri 2 |

### Frontend (TypeScript)

| Component | Technology |
|-----------|-----------|
| Framework | React 19 |
| Routing | React Router 7 (hash-based) |
| Styling | Tailwind v4 (CSS-first, OKLch) |
| Rich text | TipTap 3 |
| Graphs | d3-force + three.js |
| Build | Vite 6, Biome 2.0 |
