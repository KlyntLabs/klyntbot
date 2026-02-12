# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build --workspace              # Build all crates
cargo build --release                # Optimized release build (LTO, stripped)
cargo test --workspace               # Run all ~330 tests
cargo test -p agent                  # Test a single crate
cargo test --test integration_tests  # Run a specific test file
cargo test test_session_persistence  # Run a single test by name
cargo test -- --nocapture            # Show test stdout
cargo clippy --workspace --all-targets --all-features  # Lint (must be 0 warnings)
cargo fmt --all --check              # Check formatting
cargo build --no-default-features    # Build without email channel
```

## Architecture

Klyntbot is a Rust AI agent framework — a single binary that connects to 6+ chat platforms, calls LLMs, executes tools, and manages persistent memory.

### Workspace layout (11 crates in 8 dependency layers)

```
Layer 0: common        — Error types (KlyntbotError with 10 variants), MessageRole, ChannelName, ChatId, SessionKey
Layer 1: config, bus   — Config schema (camelCase JSON serde), async message bus (tokio::mpsc)
Layer 2: providers, session, scheduling — LLM HTTP client, JSONL session persistence, cron service
Layer 3: tools         — Tool trait + 10 implementations (file I/O ×4, shell, web ×2, message, spawn, cron)
Layer 4: channels, heartbeat — Chat platform integrations (Telegram, Discord, WhatsApp, Slack, Email, QQ)
Layer 5: agent         — Agent loop, context builder, memory store, skill manager, subagent manager
Layer 6: cli           — Clap-derived CLI, REPL, command handlers
Layer 7: klyntbot      — Re-export facade (src/lib.rs) + binary entry point (src/main.rs)
```

Dependencies flow strictly upward. No circular dependencies — enforced by Cargo.

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

### Conventions

- Error handling: Use `common::Result<T>` (alias for `Result<T, KlyntbotError>`). Domain errors auto-convert via `From` impls.
- Imports: Use crate names directly (`use common::Result`, `use config::Config`), not `use crate::` for cross-crate refs.
- Tests: Unit tests as `#[cfg(test)] mod tests` inline in each crate. Integration tests in `tests/` use the facade crate. Shared mock provider in `tests/mock_provider.rs`.
- Commits: Conventional format — `feat(providers): add streaming`, `fix(channels): handle rate limits`.
- Zero clippy warnings policy.

## CLI Subcommands

```bash
klyntbot chat "message"          # One-shot chat (or omit message for REPL)
klyntbot chat --session my-sess  # Resume a named session
klyntbot serve --port 8080       # Start HTTP server (default port from config)
klyntbot init                    # Interactive setup wizard
klyntbot status [--verbose]      # Show agent/config status
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
