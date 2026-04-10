# Klyntbot

A Rust-based personal AI agent that connects multiple chat platforms to LLM providers with cognitive memory, budget-aware execution, and extensibility via WASM plugins and MCP.

## What Makes This Different

This is not an LLM wrapper. Klyntbot is a full agent runtime with:

- **Budget-aware execution** — Every LLM interaction runs within explicit token/turn budgets (Normal/DeepThink/Ultra depth modes) with mid-loop compression, live context refresh, and graceful degradation
- **Cognitive memory** — Bi-temporal semantic facts with FSRS5 spaced-repetition decay and 12-factor relevance scoring. Memories strengthen with use and fade naturally — no manual curation
- **Self-reflection** — Mirror system watches the agent's own behavior patterns; Reforge reviews strategy files nightly and suggests improvements
- **37-crate layered architecture** — 9 strictly-layered tiers with upward-only dependencies. Each feature is a self-contained package with its own tools, migrations, config, and health checks

## Highlights

- **Multi-platform** — Telegram, Discord, Slack, Email (IMAP/SMTP), and a native desktop app (Tauri 2)
- **LLM providers** — Anthropic (native), OpenAI-compatible (GPT-4, DeepSeek-R1, Kimi, local llama.cpp, any compatible endpoint)
- **20+ built-in tools** — Tasks (agentic execution, decomposition, forecasting), finance (FIRE/Monte Carlo), notes, productivity coaching, and more
- **Feature packages** — Tasks (14+ actions), Finance (40+ actions), Notes, Productivity, Coaching, Insights, Learning, Language Learning, Launcher
- **Cognitive memory** — Bi-temporal semantic facts, episodic memories, procedural rules, knowledge graph, FSRS5 decay
- **Extensible** — WASM plugins (Extism), MCP client/server, custom skills with tool/MCP authorization
- **Desktop app** — Tauri 2 + React 19 with OKLch theming, glassmorphism UI, multi-window
- **Local-first** — All data in SQLite (WAL) + LanceDB (vectors). No cloud dependency for storage

## Quick Start

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust (stable) | >= 1.93 | `rustup install stable` |
| cargo-nextest | latest | `cargo install cargo-nextest` |
| bun | latest | `curl -fsSL https://bun.sh/install \| bash` |
| cargo-tauri | v2 | `cargo install tauri-cli@^2` |

### Setup

```bash
git clone <repo-url> && cd klyntbot
cargo build --workspace
cd desktop-ui && bun install && cd ..
```

### Configure

Create `~/.klyntbot/config.json` with at least one provider:

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-..."
    }
  }
}
```

Environment variable overrides: `KLYNTBOT_SECTION__SUBSECTION__FIELD` (e.g., `KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-ant-...`).

### Run

**Browser dev mode** (recommended for UI work):

```bash
# Terminal 1 — Rust backend + dev HTTP server on :3456
cargo tauri dev

# Terminal 2 — Vite dev server on :1420
cd desktop-ui && bun run dev
# Open http://localhost:1420
```

**Full desktop app**:

```bash
cargo tauri dev
```

**Dev/prod isolation**: Set `KLYNTBOT_HOME=~/.klyntbot-dev` (via `.env` file or env var) to run a dev instance with separate config + data from production.

## Build & Test

```bash
cargo build --workspace                              # Build all crates
cargo nextest run --workspace                        # Run all tests (parallel)
cargo nextest run -p agent                           # Single crate
cargo nextest run -E 'test(session_persistence)'     # Pattern match
cargo test --workspace --doc                         # Doctests only
cargo clippy --workspace --all-targets --all-features  # Lint (0 warnings)
cargo fmt --all --check                              # Format check
```

All tests use ephemeral SQLite (`StoragePool::connect_in_memory()`). No external DB needed.

### Desktop UI

```bash
cd desktop-ui
bun install              # Always bun, never npm
bun run dev              # Vite dev server (port 1420)
bun run build            # Production build
bun run lint:fix         # Biome 2.0 auto-fix
bun run test             # Vitest
```

## Architecture

Klyntbot is a 37-crate Rust workspace organized in 9 strictly-layered tiers. Dependencies flow upward only.

```
L0  common, platform-macos          Foundation types, errors, macOS APIs
L1  config, bus, tools-core,        Config (hot-reload), messaging, tool framework,
    tools-core-macros, analytics    derive macros, FIRE analytics
L2  storage                         SQLite (WAL), LanceDB vectors, 25+ repos
L3  providers, session, scheduling, LLM clients, persistence, cron,
    context_engine, skill-system    token budgets, skill routing
L4  tools, feature-*, plugin-       20+ tools, 9 feature packages, WASM,
    runtime, autotuner, voice-      self-optimization, voice, simulation
    engine, simulator, activity-log
L5  channels, agent, cognitive      Platform adapters, agent runtime, memory
L6  mcp                             MCP client/server
L7  app-core, desktop-shared,       Business logic, IPC types, Tauri desktop
    desktop
L8  klyntbot, klyntbot-server       Re-export facade, MCP server binary
```

### Agent Runtime Pipeline

```
Message → Session (load/create)
        → ContextEngine (budget-aware assembly)
        → Execute Loop (LLM↔tool cycles, depth-gated)
           ├── Mid-loop compression (at 70% context)
           ├── Live context refresh (cognitive updates)
           ├── Fabrication detection
           └── Streaming events (content, tools, budget)
        → Response validation
        → Recording (async: usage, feedback, learning)
        → Response
```

## Documentation

### Architecture

| Document | Description |
|----------|-------------|
| [Architecture Overview](docs/architecture/README.md) | System diagram, crate hierarchy, message flow, design principles |
| [Agent Runtime](docs/architecture/agent-runtime.md) | Execution pipeline, budget system, execute loop, compression, streaming |
| [Cognitive Memory](docs/architecture/cognitive-memory.md) | Memory types, FSRS5 decay, 12-factor relevance, mirror, reforge |
| [Context Engine](docs/architecture/context-engine.md) | Token budgets, context sources, history compression, memory retrieval |
| [Core Infrastructure](docs/architecture/core-infrastructure.md) | Foundation crates: errors, config, bus, storage, tool framework |
| [Features](docs/architecture/features.md) | Feature packages: tasks, finance, notes, productivity, coaching |
| [Channels](docs/architecture/channels.md) | Platform integrations: Telegram, Discord, Slack, Email |
| [Desktop App](docs/architecture/desktop-app.md) | Tauri 2, AppCore, React 19 frontend, dev server |
| [Skill System](docs/architecture/skill-system.md) | Skills, MCP integration, WASM plugins |

## Project Structure

```
klyntbot/
├── crates/              # 37 Rust crates (see architecture docs)
│   ├── agent/           #   Agent runtime, execution engine
│   ├── cognitive/       #   Memory, mirror, reforge
│   ├── app-core/        #   Transport-agnostic business logic
│   ├── desktop/         #   Tauri 2 desktop adapter
│   ├── channels/        #   Telegram, Discord, Slack, Email
│   ├── providers/       #   LLM provider abstraction
│   ├── feature-*/       #   Feature packages (tasks, finance, ...)
│   └── ...              #   Storage, config, bus, tools, MCP, ...
├── desktop-ui/          # React 19 frontend (Tailwind v4, Vite 6)
├── skills/              # Built-in orchestrator skills
├── docs/                # Architecture documentation
├── tests/               # Integration, e2e, simulation tests
├── Cargo.toml           # Workspace root
└── CLAUDE.md            # AI assistant instructions
```

## Tech Stack

### Backend (Rust)

| Component | Technology |
|-----------|-----------|
| Async runtime | tokio |
| Database | sqlx (SQLite, WAL mode, FTS5) |
| Vectors | LanceDB + fastembed (384-dim) |
| LLM providers | Custom abstraction (Anthropic, OpenAI-compat) |
| MCP | rmcp |
| WASM plugins | Extism |
| HTTP | axum |
| Desktop | Tauri 2 |

### Frontend (TypeScript)

| Component | Technology |
|-----------|-----------|
| Framework | React 19 |
| Routing | React Router 7 |
| Styling | Tailwind v4 (CSS-first, OKLch color space) |
| Rich text | TipTap 3 |
| Visualization | d3-force, three.js, Recharts |
| Build | Vite 6, Biome 2.0 |

## License

MIT
