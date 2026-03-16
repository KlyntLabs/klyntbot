# CLAUDE.md

## Prerequisites

`rustup`, `cargo-nextest`, `bun`, `cargo-tauri` (Tauri CLI v2). Rust stable toolchain.

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
cd desktop-ui && bun run test           # Vitest (run once)
```

**Tailwind v4 + CSS tokens:** All theming in `src/styles/theme.css` via CSS variables + `@theme inline`. No `tailwind.config.js`. Never hardcode hex/rgba — use token utilities (`bg-surface-base`, `text-muted`, `border-border`). For new visual patterns, add a CSS variable to `:root` first, register in `@theme inline`, then use via Tailwind.

**`glass-panel`:** Glassmorphism class for dropdowns/popups/dialogs. Uses `@apply backdrop-blur-[80px] backdrop-saturate-150`.

**CSS gotchas:** (1) Never write raw `backdrop-filter: blur() saturate()` — minifier breaks it. Use Tailwind's `@apply backdrop-blur-* backdrop-saturate-*`. Parent `backdrop-blur` blocks child `backdrop-filter`. (2) Never use `overflow-x-auto`/`overflow: hidden` on containers with absolute dropdown children — clips them. Use portals instead.

## Desktop App (Tauri 2)

```bash
cargo tauri dev                    # Full desktop app (auto-starts Vite + dev HTTP server on :3456)
```

**Dev/prod isolation:** Set `KLYNTBOT_HOME=~/.klyntbot-dev` (via `.env` file or env var) to run a dev instance with separate config + data from production (`~/.klyntbot/`). Controls where `config.json`, `sessions/`, `workspace/`, `data.db`, `lance/`, `plugins/`, `personas/` all live. A `.env` file at the project root is auto-loaded.

Browser-only dev: run `cargo tauri dev` (starts Vite + embedded HTTP server on `:3456`) then open `localhost:1420`. The dev HTTP server lives in `crates/desktop/src/dev_server/` — no separate `dev-api` crate. Business logic lives in the `app-core` crate; `desktop` is a thin Tauri adapter. Tauri config: `crates/desktop/tauri.conf.json`. Shared IPC types: `desktop-shared` crate.

## Architecture

Rust personal AI agent — single binary connecting 6+ chat platforms to LLMs with task/project management and persistent memory. All state in SQLite + LanceDB.

### Workspace (26 crates, 9 layers)

```
L0: common                — KlyntbotError, MessageRole, ChannelName, ChatId, SessionKey
L1: config, bus, tools-core, tools-core-macros — Config (camelCase JSON), message bus, Tool/FeaturePackage traits, derive macros
L2: storage               — SqlitePool, migrations, *Repo structs, *Row types
L3: providers, session, scheduling, context_engine, skill-system — LLM clients, session persistence, cron, token budgets, skill discovery/routing
L4: tools, feature-tasks, feature-finance, feature-notes, feature-productivity, feature-coaching, activity-log, plugin-runtime — 20+ tools, feature packages, WASM plugins
L5: channels, agent, cognitive — Platform integrations (Telegram/Discord/Slack/Email), agent runtime, cognitive memory system
L6: mcp                   — MCP server/client
L7: app-core, desktop-shared, desktop — Application core (shared handlers), Tauri desktop app
L8: klyntbot              — Re-export facade
```

Dependencies flow strictly upward. `plugin-sdk` and `tests/fixtures/hello_plugin` excluded from workspace.

### Storage

`StoragePool` wraps `SqlitePool` (Clone+Send+Sync, no `Arc<RwLock>` needed). Relational data in `{data_dir}/data.db`, vectors in `{data_dir}/lancedb/`. Data dir defaults to `~/.klyntbot`. Access via `Repos::from_pool(&pool)`. Feature crates add migrations via `FeatureMigration`.

### Key patterns

- **App-core + thin adapters:** `app-core` crate holds all shared business logic (handlers). Desktop `commands/*.rs` files are thin Tauri adapters that delegate to `AppCore` methods. Mutations use `emit_updates(&app, &updates)` for UI events. Dev server (`dev_server/`) delegates identically but discards entity updates.
- **Derive-based tools:** `#[derive(Tool)]` + `#[derive(ToolParams)]` from `tools-core-macros`. Multi-action: `#[tool_actions]` + `#[derive(ActionParams)]`. See `crates/tools/src/filesystem.rs`.
- **Feature packages:** `feature-*` crates implement `FeaturePackage` (tools + migrations + config + health).
- **Dependency inversion:** Handler traits (`SpawnHandler`, `CronHandler`, etc.) defined in lower layers, implemented in `agent`. Injected as `Arc<dyn Trait>`.
- **Config:** `#[serde(rename_all = "camelCase")]`. File at `~/.klyntbot/config.json`. API keys in `Secret<String>` (access via `.expose()`). Env override: `KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o`.
- **Re-export facade:** `src/lib.rs` re-exports all public types. Use `klyntbot::AgentLoop`, `klyntbot::Config`, etc.

### Skill system & MCP

Five built-in orchestrator skills in `skills/`: general, task-management, finance-management, automation, communication. Each has `SKILL.md` (Agent Skills spec YAML frontmatter) + `references/` folder. Compiled via `include_str!` in `skill-system` crate. `SkillRouter` selects orchestrator per-message via keyword + semantic scoring. MCP tool names: `mcp_{server}_{tool}` (see `mcp::sanitize`). MCP access controlled per-skill via `mcp_tools` field (`["*"]` = all, `[]` = none). Task-management skill has `mcp_tools: ["google-calendar"]`.

Claude Code skills (`.claude/skills/klyntbot-*/SKILL.md`) are a separate layer that teaches Claude Code how to call klyntbot MCP tools. They follow Agent Skills format with `references/` for on-demand detail loading. These are NOT the same as internal skills in `skills/`.

### MCP server — exposing tools to Claude Code

Klyntbot exposes tools to external AI clients (Claude Code, Cursor, etc.) via MCP stdio transport (`klyntbot-mcp serve --stdio`). The desktop app also embeds the MCP server (config: `mcp.server` in `config.json`).

**Architecture:** `ToolRegistryBridge` translates MCP calls → internal `Tool::execute()`. The `agent` tool delegates natural language to the full AI pipeline via `AgentBridge`. Tool names must match the `ToolRegistry` key exactly (e.g. `tasks` not `task`, `notes` not `note`).

**Currently exposed tools:** `tasks`, `project`, `area`, `notes`, `memory`, `okr`, `finance`, `productivity`, `work_context`, `agent` — configured in `default_exposed_tools()` at `crates/config/src/schema/mcp.rs`.

**To expose a new feature tool via MCP:**

1. **Implement the tool** — `#[derive(Tool)]` in a `feature-*` crate, register in `ToolRegistry` via `FeaturePackage::tools()`. The tool's `name()` return value is the registry key.
2. **Add to default whitelist** — append the tool's registry name to `default_exposed_tools()` in `crates/config/src/schema/mcp.rs`. This controls which tools MCP clients can discover and call.
3. **Verify the name matches** — run `cargo nextest run -p klyntbot-server` to confirm the tool appears in `list_tools` and passes the whitelist. Common mistake: the tool registers as plural (`tasks`) but you added singular (`task`).
4. **Test via Claude Code** — rebuild the MCP binary (`cargo build -p klyntbot-mcp`), then in Claude Code call the tool: `mcp__klyntbot__<tool_name>`. No Claude Code config changes needed — it auto-discovers tools via `tools/list`.
5. **Override per-user** — users can customize the whitelist in `config.json` → `mcp.server.exposedTools`. If a user has overridden this array, new defaults won't take effect until they add the tool manually.

**Claude Code MCP config** (`~/.claude.json`):
```json
{
  "mcpServers": {
    "klyntbot": {
      "command": "<path>/target/debug/klyntbot-mcp",
      "args": ["serve", "--stdio"],
      "env": { "KLYNTBOT_HOME": "~/.klyntbot-dev" }
    }
  }
}
```

### Tray countdown

`tray_countdown.rs` shows the next upcoming calendar event or task deadline in the macOS menu bar with a live countdown (e.g. "« 24:57 · Standup"). Only shows items due today (local timezone). Polls the DB every 30s, ticks every 1s. Coordinates with the focus timer via `FOCUS_ACTIVE` atomic flag — when a focus session is running, the focus timer owns the tray title and the countdown yields. On focus end, `notify_focus_ended()` clears the flag and the countdown resumes. Uses `tauri::async_runtime::spawn` (not `tokio::spawn`) because it starts during Tauri's `setup` hook before the tokio runtime is available.

### Agent runtime

`AgentRuntime` → `SkillCatalog` + `SkillRouter` → `IntentAnalyzer` → `ContextEngine` → `ExecutionRouter` → `CostTracker`. Two execution modes: **Direct** (single LLM call, no tools) and **Reactive** (ReAct loop with tool calls, synthesizes at max_iterations). Code in `crates/agent/src/agent_runtime/` and `crates/agent/src/intent_pipeline/`. Skill types in `crates/skill-system/`.

## Conventions

- Errors: `common::Result<T>` (alias for `Result<T, KlyntbotError>`). Domain errors auto-convert via `From`.
- Imports: Use crate names directly (`use common::Result`), not `use crate::` for cross-crate refs.
- Tests: `#[cfg(test)] mod tests` inline. Integration tests in `tests/` via facade crate.
- Commits: Conventional format — `feat(scope): description`, `fix(scope): description`.
- Zero clippy warnings policy. `desktop` crate has pre-existing exceptions.

## Non-goals

- **Structured observability (OpenTelemetry, Prometheus, metrics dashboards)** — this is a single-user local app. Existing `tracing` logs and `PipelineEvent` SSE stream are sufficient. Don't add observability infrastructure.

## Gotchas

- **MSRV 1.75** — don't use APIs stabilized after 1.75 (e.g., `Option::is_none_or` requires 1.82). Use `map_or`/`map_or_else` instead. Clippy catches this.
- **New Tauri command modules need `DEV_COMMANDS`** — every `crates/desktop/src/commands/*.rs` module with `#[tauri::command]` functions must export `pub const DEV_COMMANDS: &[&str]` and be added to `dev_server/mod.rs` test coverage list. The `dev_server_covers_all_tauri_commands` test enforces this.
- **`StoragePool::from_existing()` skips migrations** — only for already-migrated pools. Tests must use `connect_in_memory()`.

- **Config changes require restart** of the desktop app.
- **Dependency inversion** — new tools needing agent context must inject via `Arc<dyn Trait>` to avoid circular deps.
- **`email` feature** (on by default) gates IMAP/SMTP deps in `channels` crate.
- **`tauri.conf.json` uses `bun`** in `beforeBuildCommand`. Ensure `bun` is installed globally.
- **Timestamps are UTC, display in local time** — Rust stores `chrono::Utc::now().to_rfc3339()`. For user-facing display strings formatted in Rust (e.g. `due_display` in `TodayTaskResponse`), convert to local first via `d.with_timezone(&chrono::Local)`. In the frontend, never `.slice()` ISO strings — parse via `new Date(iso)` and use `toLocaleTimeString()`. Shared helper: `formatTime()` in `desktop-ui/src/shared/lib/dates.ts`.
- **Pre-release — no user data to migrate.** All schema changes can be made directly (alter tables, drop and recreate) without writing migration scripts. No need for backwards-compatible migrations until first release. When a migration is consolidated, update the `FeatureMigration` version and SQL in-place rather than adding incremental migration files. After first release, all schema changes require proper versioned migrations with `INSERT OR IGNORE` for idempotency.
