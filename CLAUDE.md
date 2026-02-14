# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build --workspace              # Build all crates
cargo build --release                # Optimized release build (LTO, stripped)
cargo test --workspace               # Run all 870+ tests
cargo test -p agent                  # Test a single crate
cargo test -p calendar               # Test calendar crate specifically
cargo test --test integration_tests  # Run a specific test file
cargo test test_session_persistence  # Run a single test by name
cargo test -- --nocapture            # Show test stdout
cargo clippy --workspace --all-targets --all-features  # Lint (must be 0 warnings)
cargo fmt --all --check              # Check formatting
cargo build --no-default-features    # Build without email channel
```

## Architecture

Klyntbot is a Rust AI agent framework — a single binary that connects to 6+ chat platforms, calls LLMs, executes tools, manages tasks/projects, syncs with Apple Calendar, and manages persistent memory.

### Workspace layout (12 crates in 8 dependency layers)

```
Layer 0: common        — Error types (KlyntbotError with 11 variants including CalendarError), MessageRole, ChannelName, ChatId, SessionKey
Layer 1: config, bus   — Config schema (camelCase JSON serde + CalendarConfig, ProjectConfig), async message bus (tokio::mpsc)
Layer 2: providers, session, scheduling, calendar — LLM HTTP client, JSONL session persistence, cron service, CalDAV client + sync engine
Layer 3: tools         — Tool trait + 12 implementations (file I/O ×4, shell, web ×2, message, spawn, cron, todo, project, calendar)
Layer 4: channels, heartbeat — Chat platform integrations (Telegram, Discord, WhatsApp, Slack, Email, QQ)
Layer 5: agent         — Agent loop, context builder, memory store, skill manager, subagent manager, calendar handler adapter, reminder engine
Layer 6: cli           — Clap-derived CLI, REPL, command handlers (TodoCommands, ProjectCommands, CalendarCommands)
Layer 7: klyntbot      — Re-export facade (src/lib.rs) + binary entry point (src/main.rs)
```

Dependencies flow strictly upward. No circular dependencies — enforced by Cargo.

**New in v0.2.0:**
- `calendar` crate at Layer 2 for CalDAV sync with Apple Calendar
- `TodoTool` expanded from 9 to 16 actions (hierarchical tasks, attachments, time tracking)
- `ProjectTool` (6 actions) for project management
- `CalendarTool` (4 actions) for sync control
- Extended `Todo` struct with 8 new fields (parent_id, project_id, attachments, time_entries, etc.)
- New JSONL stores: `todos.jsonl`, `projects.jsonl`, `calendar_sync.json`, `calendar_conflicts.jsonl`

### Key patterns

- **Dependency inversion**: `SpawnHandler` and `CronHandler` traits are defined in `tools` (Layer 3) but implemented in `agent` (Layer 5). Injected as `Arc<dyn Trait>` at construction. This breaks what would otherwise be circular deps between tools and agent.
- **Re-export facade**: `src/lib.rs` re-exports all public types from workspace crates. Integration tests and external consumers use `klyntbot::AgentLoop`, `klyntbot::Config`, etc.
- **Provider auto-detection**: The provider registry matches model name keywords to route to the correct LLM provider. No external routing library.
- **Config schema**: All config structs use `#[serde(rename_all = "camelCase")]`. Config file is `~/.klyntbot/config.json`. API keys are wrapped in `Secret<String>` (redacted in Debug/Display, access via `.expose()`).
- **Feature-gated email**: The `email` feature (on by default) gates IMAP/SMTP dependencies in the `channels` crate.

### Extension traits

| Trait | Defined in | Purpose |
|-------|-----------|---------|
| `Tool` | `tools` | `fn name()`, `fn description()`, `fn parameters() -> Value`, `async fn execute()` |
| `LlmProvider` | `providers` | `async fn chat()`, `async fn chat_stream()`, `fn name()`, `fn default_model()` |
| `Channel` | `channels` | `async fn start()`, `async fn stop()`, `async fn send()`, `fn name()`, `fn is_allowed()` |
| `SpawnHandler` | `tools` | Dependency inversion for subagent spawning |
| `CronHandler` | `tools` | Dependency inversion for cron job management |
| `CalendarHandler` | `tools` | Dependency inversion for calendar sync (NEW: `async fn sync_now()`, `async fn get_status()`, `async fn list_events()`, etc.) |

### Conventions

- Error handling: Use `common::Result<T>` (alias for `Result<T, KlyntbotError>`). Domain errors auto-convert via `From` impls.
- Imports: Use crate names directly (`use common::Result`, `use config::Config`), not `use crate::` for cross-crate refs.
- Tests: Unit tests as `#[cfg(test)] mod tests` inline in each crate. Integration tests in `tests/` use the facade crate. Shared mock provider in `tests/mock_provider.rs`.
- Commits: Conventional format — `feat(providers): add streaming`, `fix(channels): handle rate limits`.
- Zero clippy warnings policy.

## CLI Subcommands

```bash
# Chat & Agent
klyntbot chat "message"          # One-shot chat (or omit message for REPL)
klyntbot chat --session my-sess  # Resume a named session
klyntbot serve --port 8080       # Start HTTP server (default port from config)
klyntbot init                    # Interactive setup wizard (includes calendar setup in Step 7)
klyntbot status [--verbose]      # Show agent/config status

# Task Management (16 actions)
klyntbot todo add <title> [--project ID] [--parent ID] [--due DATE]
klyntbot todo list [--project ID] [--status doing|todo|done]
klyntbot todo tree [--project ID] [--depth N]
klyntbot todo focus [ID]         # Start focus session (auto-tracks time)
klyntbot todo unfocus ID
klyntbot todo add-subtask PARENT_ID <title>
klyntbot todo move ID [--parent ID|none] [--project ID|none]
klyntbot todo attach ID --file PATH | --url URL | --note TEXT
klyntbot todo detach ID ATTACHMENT_ID
klyntbot todo log-time ID MINUTES [--note TEXT]
klyntbot todo search QUERY [--include-attachments]
klyntbot todo show ID
klyntbot todo complete ID
klyntbot todo delete ID
klyntbot todo update ID [--title TEXT] [--due DATE] [--priority N]
klyntbot todo summary [--project ID]

# Project Management (6 actions)
klyntbot project create <name> [--description TEXT] [--color COLOR]
klyntbot project list [--status active|paused|completed|archived]
klyntbot project show ID
klyntbot project archive ID
klyntbot project tasks ID [--tree]
klyntbot project report ID --period week|month

# Calendar Sync (4 actions)
klyntbot calendar sync [--force]
klyntbot calendar status
klyntbot calendar list [--from DATE] [--to DATE]
klyntbot calendar conflicts [--limit N]

# Other Commands
klyntbot channels list|start|stop
klyntbot cron list|add|remove
klyntbot config validate|show|set
klyntbot skills list|info
```

## Environment Variables

Config can be overridden via `KLYNTBOT_` prefix with `__` as nesting separator:

```bash
KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o
KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-...
KLYNTBOT_TOOLS__RESTRICT_TO_WORKSPACE=true
```

## Skills

Built-in skills live in `skills/` as `SKILL.md` files (summarize, skill-creator, github, tmux, weather, cron). The `SkillManager` in the agent crate discovers and loads them at runtime.
