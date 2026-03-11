# Getting Started

This guide takes you from a fresh clone to a running klyntbot instance in under 15 minutes.

## Prerequisites

| Tool | Minimum Version | Install |
|------|----------------|---------|
| Rust (via rustup) | 1.75 (stable) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| cargo-nextest | latest | `cargo install cargo-nextest` |
| cargo-tauri | 2.x (Tauri CLI v2) | `cargo install tauri-cli --version '^2'` |
| bun | latest | `curl -fsSL https://bun.sh/install \| bash` |
| TypeScript | 5.7+ | Installed via `bun install` (dev dependency) |

> **Important:** Always use `bun` for frontend package management. Never use `npm` or `yarn`.

## Initial Setup

```bash
# Clone the repository
git clone <repo-url> klyntbot
cd klyntbot

# Build all Rust crates
cargo build --workspace

# Install frontend dependencies
cd desktop-ui && bun install && cd ..
```

The first build downloads and compiles all dependencies (including LanceDB, SQLite, and fastembed). Expect 3-8 minutes depending on your machine.

## Configuration

klyntbot stores all configuration and data under `~/.klyntbot/`:

```
~/.klyntbot/
  config.json     # Main configuration file (camelCase JSON)
  data.db         # SQLite database (created on first run)
  lancedb/        # Vector storage
  sessions/       # Session data
  workspace/      # Agent workspace
```

### Minimal Configuration

Create `~/.klyntbot/config.json` with at least one LLM provider API key:

```json
{
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-your-key-here"
    }
  }
}
```

The config file uses **camelCase** field names (not snake_case). Only fields that differ from defaults are needed -- the system merges your config with sensible defaults automatically.

Supported providers: `anthropic`, `openai`, `openrouter`, `deepseek`, `gemini`, `groq`, `vllm`, `zhipu`, `dashscope`, `moonshot`, `minimax`, `aihubmix`.

The active provider is auto-detected from whichever API key is configured first, or you can set it explicitly:

```json
{
  "agents": {
    "defaults": {
      "provider": "anthropic",
      "model": "anthropic/claude-opus-4-5"
    }
  },
  "providers": {
    "anthropic": {
      "apiKey": "sk-ant-your-key-here"
    }
  }
}
```

Default model: `anthropic/claude-opus-4-5`. Default max tokens: `8192`. Default temperature: `0.7`.

For the full configuration reference, see [docs/configuration/](../configuration/).

## Development Modes

There are two ways to run klyntbot during development.

### Browser-Only Mode (recommended for UI work)

Run two processes in separate terminals:

```bash
# Terminal 1: Start the dev API server on port 3456
cargo run -p dev-api

# Terminal 2: Start the Vite dev server on port 1420
cd desktop-ui && bun run dev
```

Open **http://localhost:1420** in your browser.

How it works: Vite serves the React frontend on port 1420 and proxies `/api` and `/attachments` requests to the dev API server on port 3456 (configured in `desktop-ui/vite.config.ts`). This gives you hot module replacement for the frontend without rebuilding Rust code.

### Full Desktop Mode (Tauri app)

```bash
cargo tauri dev
```

This single command:
1. Starts the Vite dev server on port 1420 (frontend with HMR)
2. Starts an embedded HTTP dev server on port 3456 (debug builds only)
3. Builds and launches the native Tauri window pointing at localhost:1420

The desktop app has a main window (1200x800), a launcher overlay, a tray popup, and a distraction overlay -- all configured in `crates/desktop/tauri.conf.json`.

> **Known issue:** `tauri.conf.json` references `npm` in `beforeBuildCommand`. If `cargo tauri dev` fails with ENOENT, start Vite manually first (`cd desktop-ui && bun run dev`) in a separate terminal, then run `cargo tauri dev`.

## Running Tests

All tests use ephemeral in-memory SQLite. No external database or services needed.

```bash
# Run all tests in parallel (primary test runner)
cargo nextest run --workspace

# Test a single crate
cargo nextest run -p agent

# Run tests matching a pattern
cargo nextest run -E 'test(session_persistence)'

# Run doc tests (nextest does not support these)
cargo test --workspace --doc

# Lint -- zero warnings policy, must pass before merging
cargo clippy --workspace --all-targets --all-features

# Check formatting
cargo fmt --all --check
```

## Project Structure

```
klyntbot/
  agents/              # Built-in agent profiles (general, task, finance, etc.)
  crates/              # 26 Rust workspace crates (see below)
    common/            # L0 - Shared error types, enums, IDs
    config/            # L1 - Configuration schema and loader
    bus/               # L1 - Internal message bus
    tools-core/        # L1 - Tool and FeaturePackage traits
    tools-core-macros/ # L1 - #[derive(Tool)] and #[derive(ToolParams)] macros
    storage/           # L2 - SQLite pool, migrations, repositories
    domain/            # L2 - OKR + PARA domain types
    providers/         # L3 - LLM provider clients
    session/           # L3 - Session persistence
    scheduling/        # L3 - Cron scheduling
    context_engine/    # L3 - Token budget and context assembly
    tools/             # L4 - 20+ built-in tools
    feature-todo/      # L4 - Todo/task management feature package
    feature-finance/   # L4 - Finance tracking feature package
    feature-notes/     # L4 - Notes feature package
    feature-productivity/ # L4 - Productivity feature package
    feature-coaching/  # L4 - Coaching feature package
    plugin-runtime/    # L4 - WASM plugin runtime
    channels/          # L5 - Platform integrations (Telegram, Discord, Slack, Email)
    agent/             # L5 - Agent runtime, intent analysis, execution
    cognitive/         # L5 - Cognitive memory system
    mcp/               # L6 - MCP server and client
    app-core/          # L7 - Shared business logic (handlers)
    desktop-shared/    # L7 - Shared IPC types for Tauri
    desktop/           # L7 - Tauri desktop app (thin adapter)
    activity-log/      # Activity logging
  desktop-ui/          # React + Vite + Tailwind v4 frontend
  docs/                # Documentation
  src/                 # Root klyntbot crate (re-export facade)
  tests/               # Integration tests
  workspace/           # Default agent workspace
```

Dependencies flow strictly upward (L0 -> L8). The `plugin-sdk` and `tests/fixtures/hello_plugin` crates are excluded from the workspace.

## Common Development Tasks

### Adding a New Tool

1. Create a struct in `crates/tools/src/` with `#[derive(Tool)]` and `#[derive(ToolParams)]`
2. Implement the `execute` method
3. Register it in the tool list
4. Reference implementation: `crates/tools/src/filesystem.rs`

For multi-action tools, use `#[tool_actions]` + `#[derive(ActionParams)]`.

### Adding a Feature Package

1. Create a new `crates/feature-*` crate
2. Implement `FeaturePackage` (provides tools, migrations, config, and health checks)
3. Add it to the workspace `members` in `Cargo.toml`
4. Wire it into the agent runtime

### Adding a Channel Integration

1. Add the platform adapter in `crates/channels/src/`
2. If it requires optional dependencies, gate them behind a feature flag (like the `email` feature)
3. Register the channel in the agent startup flow

## Gotchas and Known Issues

**`StoragePool::from_existing()` skips migrations.** It is only for pools that are already migrated. In tests, always use `StoragePool::connect_in_memory()` which runs all migrations automatically.

**`tauri.conf.json` uses `npm` in `beforeBuildCommand`.** The project requires `bun`. If `cargo tauri dev` fails, start Vite manually (`cd desktop-ui && bun run dev`) then run `cargo tauri dev` in another terminal. Alternatively, use browser-only dev mode.

**Config changes require a restart** of the desktop app. There is no hot-reload for `~/.klyntbot/config.json`.

**Dependency inversion for agent context.** If a new tool needs access to agent-level context (sessions, other tools), inject it via `Arc<dyn Trait>` to avoid circular dependencies between crates. Handler traits like `SpawnHandler` and `CronHandler` are defined in lower layers and implemented in the `agent` crate.

**The `email` feature is on by default.** It gates IMAP/SMTP dependencies in the `channels` crate. If you don't need email and want faster compile times, build with `--no-default-features`.

**Timestamps are always UTC.** Rust code uses `chrono::Utc::now().to_rfc3339()`. On the frontend, never slice ISO strings for display. Always parse with `new Date(iso)` and format with `toLocaleTimeString()`. Use the shared `formatTime()` helper in `desktop-ui/src/lib/dates.ts`.

## Environment Variables

All configuration fields can be overridden via environment variables using the `KLYNTBOT_` prefix with double underscores (`__`) for nesting.

**Pattern:** `KLYNTBOT_{SECTION}__{SUBSECTION}__{FIELD}=value`

### Examples

```bash
# Override the default model
export KLYNTBOT_AGENTS__DEFAULTS__MODEL=anthropic/claude-sonnet-4-5

# Set provider API keys
export KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-ant-...
export KLYNTBOT_PROVIDERS__OPENAI__API_KEY=sk-...
export KLYNTBOT_PROVIDERS__DEEPSEEK__API_KEY=sk-...
export KLYNTBOT_PROVIDERS__GEMINI__API_KEY=...

# Override the data directory
export KLYNTBOT_DATA_DIR=/custom/path

# Set channel tokens
export KLYNTBOT_CHANNELS__TELEGRAM__TOKEN=bot123:ABC...
export KLYNTBOT_CHANNELS__DISCORD__TOKEN=...
export KLYNTBOT_CHANNELS__SLACK__BOT_TOKEN=xoxb-...
export KLYNTBOT_CHANNELS__SLACK__APP_TOKEN=xapp-...

# Set agent defaults
export KLYNTBOT_AGENTS__DEFAULTS__TEMPERATURE=0.5
export KLYNTBOT_AGENTS__DEFAULTS__MAX_TOKENS=4096
export KLYNTBOT_AGENTS__DEFAULTS__WORKSPACE=/path/to/workspace

# Set tool API keys
export KLYNTBOT_TOOLS__WEB__BRAVE_API_KEY=...
```

Environment variables take precedence over values in `config.json`. This is useful for CI, Docker deployments, or keeping secrets out of the config file.
