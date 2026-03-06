# CLAUDE.md

## Build & Test

```bash
cargo build --workspace                            # Build all crates
cargo nextest run --workspace                      # Run all tests (parallel)
cargo nextest run -p agent                         # Test a single crate
cargo nextest run -E 'test(session_persistence)'   # Run tests matching pattern
cargo test --workspace --doc                       # Doctests only (nextest doesn't support these)
cargo clippy --workspace --all-targets --all-features  # Lint (must be 0 warnings)
cargo fmt --all --check                            # Check formatting
```

All tests use ephemeral SQLite (`StoragePool::connect_in_memory()`). No external DB needed.

## Desktop UI (desktop-ui/)

```bash
cd desktop-ui && bun install            # Install deps (always bun, never npm)
cd desktop-ui && bun run dev            # Vite dev server (port 1420)
cd desktop-ui && bun run build          # Production build
cd desktop-ui && bun run lint:fix       # Biome 2.0 auto-fix (lint + format + imports)
```

**Tailwind v4 + CSS tokens:** All theming in `src/styles/theme.css` via CSS variables + `@theme inline`. No `tailwind.config.js`. Never hardcode hex/rgba — use token utilities (`bg-surface-base`, `text-muted`, `border-border`). For new visual patterns, add a CSS variable to `:root` first, register in `@theme inline`, then use via Tailwind.

**`glass-panel`:** Glassmorphism class for dropdowns/popups/dialogs. Uses `@apply backdrop-blur-[80px] backdrop-saturate-150`.

**CSS gotchas:** (1) Never write raw `backdrop-filter: blur() saturate()` — minifier breaks it. Use Tailwind's `@apply backdrop-blur-* backdrop-saturate-*`. Parent `backdrop-blur` blocks child `backdrop-filter`. (2) Never use `overflow-x-auto`/`overflow: hidden` on containers with absolute dropdown children — clips them. Use portals instead.

## Desktop App (Tauri 2)

```bash
cargo tauri dev                    # Full desktop app (auto-starts Vite)
cargo run -p dev-api               # Lightweight dev API on :3456 (no Tauri needed)
```

Browser-only dev: run `cargo run -p dev-api` + `cd desktop-ui && bun run dev`, open `localhost:1420`. Tauri config: `crates/desktop/tauri.conf.json`. Shared IPC types: `desktop-shared` crate.

## Architecture

Rust personal AI agent — single binary connecting 6+ chat platforms to LLMs with task/project management, Apple Calendar sync, and persistent memory. All state in SQLite + LanceDB.

### Workspace (24 crates, 9 layers)

```
L0: common                — KlyntbotError, MessageRole, ChannelName, ChatId, SessionKey
L1: config, bus, tools-core, tools-core-macros — Config (camelCase JSON), message bus, Tool/FeaturePackage traits, derive macros
L2: storage, domain       — SqlitePool, migrations, *Repo structs, OKR+PARA domain types
L3: providers, session, scheduling, calendar, context_engine — LLM clients, session persistence, cron, CalDAV, token budgets
L4: tools, feature-todo, feature-finance, feature-productivity, plugin-runtime — 20+ tools, feature packages, WASM plugins
L5: channels, agent       — Platform integrations (Telegram/Discord/WhatsApp/Slack/Email/QQ), agent runtime, learning system
L6: cli, mcp              — CLI (serve/init/status/plugin), MCP server
L7: desktop-shared, desktop, dev-api — Tauri desktop app, dev API server
L8: klyntbot              — Re-export facade + binary entry point
```

Dependencies flow strictly upward. `plugin-sdk` and `tests/fixtures/hello_plugin` excluded from workspace.

### Storage

`StoragePool` wraps `SqlitePool` (Clone+Send+Sync, no `Arc<RwLock>` needed). Relational data in `{data_dir}/data.db`, vectors in `{data_dir}/lancedb/`. Data dir defaults to `~/.klyntbot`. Access via `Repos::from_pool(&pool)`. Feature crates add migrations via `FeatureMigration`.

### Key patterns

- **Derive-based tools:** `#[derive(Tool)]` + `#[derive(ToolParams)]` from `tools-core-macros`. Multi-action: `#[tool_actions]` + `#[derive(ActionParams)]`. See `crates/tools/src/filesystem.rs`.
- **Feature packages:** `feature-*` crates implement `FeaturePackage` (tools + migrations + config + health).
- **Dependency inversion:** Handler traits (`SpawnHandler`, `CronHandler`, `CalendarHandler`, etc.) defined in lower layers, implemented in `agent`. Injected as `Arc<dyn Trait>`.
- **Config:** `#[serde(rename_all = "camelCase")]`. File at `~/.klyntbot/config.json`. API keys in `Secret<String>` (access via `.expose()`). Env override: `KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o`.
- **Re-export facade:** `src/lib.rs` re-exports all public types. Use `klyntbot::AgentLoop`, `klyntbot::Config`, etc.

### Agent profiles & MCP

Six built-in agents in `agents/`: general, task, finance, calendar, automation, communication. Each has `AGENT.md` (YAML frontmatter) + `skills/` folder. Compiled via `include_str!`. MCP tool names: `mcp_{server}_{tool}` (see `mcp::sanitize`). MCP access controlled per-agent via `mcp_tools` field (`["*"]` = all, `[]` = none).

### Agent runtime

`AgentRuntime` → `AgentManager` → `IntentAnalyzer` → `ContextEngine` → `ExecutionRouter` → `CostTracker`. Two execution modes: **Direct** (single LLM call, no tools) and **Reactive** (ReAct loop with tool calls, escalates at 80% of max_iterations). Code in `crates/agent/src/agent_runtime/` and `crates/agent/src/intent_pipeline/`.

## Conventions

- Errors: `common::Result<T>` (alias for `Result<T, KlyntbotError>`). Domain errors auto-convert via `From`.
- Imports: Use crate names directly (`use common::Result`), not `use crate::` for cross-crate refs.
- Tests: `#[cfg(test)] mod tests` inline. Integration tests in `tests/` via facade crate.
- Commits: Conventional format — `feat(scope): description`, `fix(scope): description`.
- Zero clippy warnings policy. `desktop` crate has pre-existing exceptions.

## Gotchas

- **`StoragePool::from_existing()` skips migrations** — only for already-migrated pools. Tests must use `connect_in_memory()`.
- **CalDAV sync is async** — use `CalendarTool::sync_now()` for immediate sync.
- **Config changes require restart** of `klyntbot serve`.
- **Dependency inversion** — new tools needing agent context must inject via `Arc<dyn Trait>` to avoid circular deps.
- **`email` feature** (on by default) gates IMAP/SMTP deps in `channels` crate.
