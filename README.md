# Klyntbot

A Rust-based personal AI agent platform that connects multiple chat platforms to LLM providers with task management, persistent memory, and extensibility via WASM plugins and MCP.

## Highlights

- **Multi-platform** — Telegram, Discord, Slack, Email (IMAP/SMTP), CLI, WebSocket, and a native desktop app
- **12 LLM providers** — Anthropic, OpenAI, DeepSeek, Gemini, OpenRouter, Groq, and more
- **Intelligent routing** — Intent analysis selects Direct (single LLM call) or Reactive (ReAct tool-use loop) execution
- **20+ built-in tools** — File I/O, web search, shell exec, cron scheduling, and more
- **Feature packages** — Todo (OKR + PARA), finance (FIRE tracking), notes, productivity coaching
- **Cognitive memory** — Bi-temporal semantic facts with FSRS spaced-repetition decay, FTS5 + vector hybrid search
- **Extensible** — WASM plugins (Extism), MCP client/server (rmcp), custom agent profiles with skills
- **Desktop app** — Tauri 2 + React 19 with glassmorphism UI, multi-window (main, launcher, tray, distraction overlay)
- **Local-first** — All data in SQLite + LanceDB. No cloud dependency for storage

## Quick Start

### Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust (stable) | >= 1.75 | `rustup install stable` |
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

See the [Configuration Reference](docs/configuration/reference.md) for all options.

### Run

**Browser dev mode** (recommended for UI work):

```bash
# Terminal 1
cargo run -p dev-api

# Terminal 2
cd desktop-ui && bun run dev
# Open http://localhost:1420
```

**Full desktop app**:

```bash
cargo tauri dev
```

See the [Getting Started Guide](docs/development/getting-started.md) for details.

## Architecture

Klyntbot is a 26-crate Rust workspace organized in 9 strictly-layered tiers. Dependencies flow upward only — no circular dependencies.

```
L0  common                          Foundation types, errors
L1  config, bus, tools-core         Config, messaging, tool abstractions
L2  storage, domain                 SQLite, LanceDB, domain models
L3  providers, session, scheduling  LLM clients, persistence, cron
    context_engine
L4  tools, feature-*, plugin-runtime Built-in tools, features, WASM
L5  channels, agent, cognitive      Platform adapters, runtime, memory
L6  mcp                            MCP server/client
L7  app-core, desktop-shared,      Application layer, Tauri desktop
    desktop
L8  klyntbot                       Re-export facade
```

### Agent Runtime Pipeline

```
Message → AgentManager (profile selection)
        → IntentAnalyzer (heuristic + LLM classifier)
        → ContextEngine (token budget allocation)
        → Tool Filtering (per-agent allowlists)
        → ExecutionRouter (Direct or Reactive)
        → ResponseValidator → CostTracker
        → Response
```

See the [Architecture Overview](docs/architecture/overview.md) for the full picture.

## Build & Test

```bash
cargo build --workspace                              # Build all crates
cargo nextest run --workspace                        # Run all tests
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
```

## Project Structure

```
klyntbot/
├── agents/              # Agent profiles (AGENT.md + skills/)
│   ├── general/         #   Default agent
│   ├── task/            #   Task management agent
│   ├── finance/         #   Finance agent
│   ├── automation/      #   Automation agent
│   └── communication/   #   Communication agent
├── crates/              # 26 Rust crates (see architecture docs)
├── desktop-ui/          # React 19 frontend (Tailwind v4, Vite 6)
├── workspace/           # Agent runtime workspace files
├── docs/                # Technical documentation
├── Cargo.toml           # Workspace root
└── CLAUDE.md            # AI assistant instructions
```

## Documentation

### Architecture

| Document | Description |
|----------|-------------|
| [Architecture Overview](docs/architecture/overview.md) | System architecture, crate layers, message flow, design patterns |
| [Crate Dependency Map](docs/architecture/crate-dependency-map.md) | All 26 crates with purpose, layer, and dependency relationships |
| [Agent Runtime](docs/architecture/agent-runtime.md) | Intent analysis, context assembly, execution routing pipeline |
| [Tools System](docs/architecture/tools-system.md) | Tool trait, derive macros, feature packages, registry |
| [Storage](docs/architecture/storage.md) | SQLite schema, LanceDB vectors, migrations, repo pattern |
| [Desktop App](docs/architecture/desktop-app.md) | Tauri setup, dual-mode IPC, AppCore pattern, streaming |
| [Channels](docs/architecture/channels.md) | Platform integrations, Channel trait, message bus |
| [MCP](docs/architecture/mcp.md) | MCP client/server, tool namespacing, security |
| [Context Engine](docs/architecture/context-engine.md) | Token budgets, context sources, history compression |
| [Cognitive Memory](docs/architecture/cognitive-memory.md) | Semantic memory, FSRS decay, consolidation pipeline |
| [Plugins](docs/architecture/plugins.md) | WASM plugin system (Extism), SDK, permissions |
| [Scheduling](docs/architecture/scheduling.md) | CronService, job types, persistence |
| [Architecture Decisions](docs/architecture/decisions.md) | 10 ADRs documenting key design choices |

### Configuration

| Document | Description |
|----------|-------------|
| [Configuration Reference](docs/configuration/reference.md) | Complete config.json reference — all 24 sections, env vars |
| [Agent Profiles](docs/configuration/agent-profiles.md) | AGENT.md format, skills, tool allowlists, matching |

### Development

| Document | Description |
|----------|-------------|
| [Getting Started](docs/development/getting-started.md) | Prerequisites, setup, dev modes, first run |
| [Testing](docs/development/testing.md) | Test strategy, patterns, nextest, clippy |

### Frontend

| Document | Description |
|----------|-------------|
| [Frontend Architecture](docs/frontend/architecture.md) | React structure, routing, hooks, IPC, streaming |
| [Design System](docs/frontend/design-system.md) | Theme tokens, glassmorphism, Tailwind v4 conventions |

### Operations

| Document | Description |
|----------|-------------|
| [Security](docs/operations/security.md) | Secret handling, permissions, sandboxing, input sanitization |
| [Troubleshooting](docs/operations/troubleshooting.md) | Common issues, gotchas, debugging tips |

## Tech Stack

### Backend (Rust)

- **Async runtime**: tokio 1.49
- **Database**: sqlx (SQLite, WAL mode, FTS5)
- **Vectors**: LanceDB + fastembed (384-dim embeddings)
- **LLM**: Custom provider abstraction (12 providers)
- **MCP**: rmcp 0.17
- **WASM**: Extism 1.x
- **HTTP**: axum 0.8
- **Desktop**: Tauri 2

### Frontend (TypeScript)

- **Framework**: React 19
- **Routing**: React Router 7
- **Styling**: Tailwind v4 (CSS-first)
- **Rich text**: TipTap 3
- **Charts**: Recharts 3
- **Build**: Vite 6, Biome 2.0

## Configuration

Config file: `~/.klyntbot/config.json` (camelCase JSON, all fields optional)

Environment variable overrides: `KLYNTBOT_SECTION__SUBSECTION__FIELD`

```bash
KLYNTBOT_AGENTS__DEFAULTS__MODEL=anthropic/claude-sonnet-4-20250514
KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-ant-...
KLYNTBOT_CHANNELS__TELEGRAM__TOKEN=bot...
```

See [Configuration Reference](docs/configuration/reference.md) for the complete guide.

## License

MIT
