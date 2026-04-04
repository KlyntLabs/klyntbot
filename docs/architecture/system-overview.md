# System Overview

> Klyntbot is a single-user, local-first personal AI agent that connects 6+ chat platforms to LLMs with task/project management, persistent cognitive memory, and self-optimization capabilities.

## Purpose

Klyntbot serves as a personal AI assistant that:

- Receives messages from **Telegram, Discord, Slack, Email, Desktop UI, and MCP clients**
- Routes them through an intelligent skill-based orchestration layer
- Executes tasks using 20+ domain tools (tasks, finance, notes, productivity, OKR, etc.)
- Maintains long-term memory via a cognitive system (episodic + semantic + spaced repetition)
- Self-optimizes its routing and retrieval parameters via an autotuner
- Reflects on its own behavior via a mirror self-reflection layer

## Key Design Principles

| Principle | Implementation |
|-----------|---------------|
| **Local-first** | All state in local SQLite + LanceDB. No cloud backend. Single-user. |
| **Single binary** | Desktop app (Tauri) and MCP server share the same binary. |
| **Layered architecture** | 34 crates across 9 strict layers; dependencies flow upward only. |
| **Dependency inversion** | Handler traits defined in lower layers, implemented in upper layers, injected as `Arc<dyn Trait>`. |
| **Hot-reloadable config** | Model, temperature, budget, and iteration limits change without restart. |
| **Progressive skill loading** | Orchestrator skills inject full body on first activation; subsequent messages use deduplicated references. |
| **Self-optimization** | Autotuner runs shadow experiments on routing/retrieval parameters, promotes winners. |

## Runtime Modes

```
                        +-----------------------+
                        |   klyntbot binary      |
                        +-----------+-----------+
                                    |
                    +---------------+---------------+
                    |               |               |
             Desktop App      MCP Server       CLI Tools
           (Tauri + React)   (stdio/HTTP)    (introspection)
                    |               |
              +-----+-----+   +----+----+
              | Vite UI    |   | Claude  |
              | port 1420  |   | Code    |
              +------------+   | Cursor  |
                               +---------+
```

**Desktop App** — Tauri 2 with React 19 frontend. System tray app (hides from Dock on macOS). 5 windows: main, launcher, tray popup, distraction overlay, voice orb.

**MCP Server** — Exposes tools to external AI clients via JSON-RPC over stdio or HTTP. Same `AppCore` business logic as desktop.

**Dev Server** — Debug-only HTTP server on port 3456 mirroring all Tauri commands for browser-based development.

## Core Components

```
+-------------------+     +------------------+     +-------------------+
|   Chat Channels   |     |   Desktop UI     |     |   MCP Clients     |
| Telegram, Discord |     | React + Tauri    |     | Claude Code, etc. |
| Slack, Email      |     | IPC / SSE        |     | stdio / HTTP      |
+--------+----------+     +--------+---------+     +--------+----------+
         |                         |                         |
         v                         v                         v
+--------+-------------------------+-------------------------+----------+
|                          MessageBus / AppCore                         |
|  InboundMessage -> SessionManager -> AgentRuntime -> OutboundMessage  |
+--+--------------------+--------------------+--------------------+-----+
   |                    |                    |                    |
   v                    v                    v                    v
+--+-------+  +---------+--------+  +-------+--------+  +-------+------+
| Skill    |  | Context Engine   |  | Execution      |  | Cognitive    |
| System   |  | Token budgets    |  | Router         |  | Memory       |
| 5 skills |  | Memory retrieval |  | Direct/Reactive|  | Episodic     |
| Router   |  | Query rewriting  |  | ReAct loop     |  | Semantic     |
+----------+  +------------------+  | Tool registry  |  | FSRS5        |
                                    +----------------+  | Mirror       |
                                                        +--------------+
```

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Language | Rust (MSRV 1.75), TypeScript |
| Desktop | Tauri 2, React 19 (with React Compiler) |
| Frontend build | Vite 6, Tailwind CSS v4, Biome 2 |
| Database | SQLite (via sqlx, WAL mode) |
| Vector store | LanceDB (384-dim embeddings) |
| Async runtime | Tokio (multi-thread) |
| LLM providers | Anthropic, OpenAI, OpenRouter, DeepSeek, Gemini, Groq, vLLM, ZhipuAI, DashScope, Moonshot, MiniMax, AiHubMix |
| MCP protocol | rmcp 0.17 (JSON-RPC over stdio/HTTP) |
| Plugin system | Extism (WASM) |
| Spaced repetition | FSRS-5 (19-parameter algorithm) |

## Data Storage

All data lives under `KLYNTBOT_HOME` (default `~/.klyntbot/`):

```
~/.klyntbot/
  config.json          -- Configuration (camelCase JSON, Secret<String> for API keys)
  data.db              -- SQLite (WAL mode, 5s busy timeout)
  lance/               -- LanceDB vector store (10 tables, 384-dim embeddings)
  plugins/             -- WASM plugins
  sessions/            -- Session data
  workspace/           -- Agent workspace files
  personas/            -- Persona definitions
```

## Crate Count by Layer

| Layer | Crates | Purpose |
|-------|--------|---------|
| L0 | 2 | Primitives (error types, macOS native) |
| L1 | 5 | Infrastructure (config, bus, tool traits, analytics) |
| L2 | 1 | Storage (SQLite pool, repos, migrations) |
| L3 | 5 | AI clients & routing (providers, session, scheduling, context, skills) |
| L4 | 12 | Feature tools & plugins |
| L5 | 3 | Channels, agent runtime, cognitive memory |
| L6 | 1 | MCP protocol |
| L7 | 3 | Application layer (app-core, desktop-shared, desktop) |
| L8 | 2 | Binary facades |
| **Total** | **34** | |
