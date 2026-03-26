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

Root facade crate has 4 test binaries in `tests/`: `integration/` (cross-crate via facade), `e2e/` (agent loop + reminders), `unit/` (config, providers), `plugins.rs` (WASM, needs `--features plugin-integration` + pre-built plugin). Shared fixtures in `tests/common/`. All tests use ephemeral SQLite (`StoragePool::connect_in_memory()`). No external DB needed.

## Desktop UI (desktop-ui/)

```bash
cd desktop-ui && bun install            # Install deps (always bun, never npm)
cd desktop-ui && bun run dev            # Vite dev server (port 1420)
cd desktop-ui && bun run build          # Production build
cd desktop-ui && bun run lint:fix       # Biome 2.0 auto-fix (lint + format + imports)
cd desktop-ui && bun run test           # Vitest (run once)
cd desktop-ui && bun run test:watch     # Vitest (watch mode)
cd desktop-ui && bun run lint           # Biome check only (no auto-fix)
```

**Path aliases:** `@/*` → `src/*`, `@shared/*` → `src/shared/*`, `@features/*` → `src/features/*`, `@app/*` → `src/app/*`. Always use these in imports, never relative `../../` paths.

**Tailwind v4 + CSS tokens:** All theming in `src/styles/theme.css` via CSS variables + `@theme inline`. No `tailwind.config.js`. Never hardcode hex/rgba — use token utilities (`bg-surface-base`, `text-muted`, `border-border`). For new visual patterns, add a CSS variable to `:root` first, register in `@theme inline`, then use via Tailwind.

**`glass-panel`:** Glassmorphism class for dropdowns/popups/dialogs. Uses `@apply backdrop-blur-[80px] backdrop-saturate-150`.

**Biome 2.0:** Line width 100. Organizes imports automatically. Warnings (not errors) on `noArrayIndexKey`, `noNonNullAssertion`, `noStaticElementInteractions`, `noImportantStyles`.

**React Compiler:** Enabled via `babel-plugin-react-compiler` in `vite.config.ts`. Auto-memoizes components — don't manually wrap with `React.memo`/`useMemo`/`useCallback` unless profiling shows a specific need.

**Data fetching:** `useQuery(cmd, args)` for reads (SWR caching, 30s stale time), `useMutation(cmd)` for writes. Both use `ipc()` which calls Tauri `invoke` in desktop or `fetch("/api/{cmd}")` in browser dev mode. Never call `invoke` directly — always go through `ipc()`.

**CSS gotchas:** (1) Never write raw `backdrop-filter: blur() saturate()` — minifier breaks it. Use Tailwind's `@apply backdrop-blur-* backdrop-saturate-*`. Parent `backdrop-blur` blocks child `backdrop-filter`. (2) Never use `overflow-x-auto`/`overflow: hidden` on containers with absolute dropdown children — clips them. Use portals instead.

## Desktop App (Tauri 2)

```bash
cargo tauri dev                    # Full desktop app (start Vite separately: cd desktop-ui && bun run dev)
```

**Dev/prod isolation:** Set `KLYNTBOT_HOME=~/.klyntbot-dev` (via `.env` file or env var) to run a dev instance with separate config + data from production (`~/.klyntbot/`). Controls where `config.json`, `sessions/`, `workspace/`, `data.db`, `lance/`, `plugins/`, `personas/` all live. A `.env` file at the project root is auto-loaded.

Browser-only dev: run `cd desktop-ui && bun run dev` then `cargo tauri dev` (which starts the embedded HTTP server on `:3456`), then open `localhost:1420`. The dev HTTP server lives in `crates/desktop/src/dev_server/` — no separate `dev-api` crate. Business logic lives in the `app-core` crate; `desktop` is a thin Tauri adapter. Tauri config: `crates/desktop/tauri.conf.json`. Shared IPC types: `desktop-shared` crate.

## Architecture

Rust personal AI agent — single binary connecting 6+ chat platforms to LLMs with task/project management and persistent memory. All state in SQLite + LanceDB.

### Workspace (34 crates, 9 layers)

```
L0: common, platform-macos — KlyntbotError, MessageRole, ChannelName, ChatId, SessionKey; macOS native APIs (pasteboard, window mgmt)
L1: config, bus, tools-core, tools-core-macros, analytics — Config (camelCase JSON), message bus, Tool/FeaturePackage traits, derive macros, FIRE/Monte Carlo analytics
L2: storage               — SqlitePool, migrations, *Repo structs, *Row types
L3: providers, session, scheduling, context_engine, skill-system — LLM clients, session persistence, cron, token budgets, skill discovery/routing
L4: tools, feature-tasks, feature-finance, feature-notes, feature-productivity, feature-coaching, feature-insights, feature-launcher, feature-learning (flashcard generation), activity-log, plugin-runtime, autotuner — 20+ tools, feature packages, WASM plugins, self-optimization experiments
L5: channels, agent, cognitive — Platform integrations (Telegram/Discord/Slack/Email), agent runtime, cognitive memory (episodic/semantic extraction, spaced repetition via FSRS5, salience decay, reflection)
L6: mcp                   — MCP server/client
L7: app-core, desktop-shared, desktop — Application core (shared handlers), Tauri desktop app
L8: klyntbot, klyntbot-server — Re-export facade, standalone MCP server binary
```

Dependencies flow strictly upward. `plugin-sdk` and `tests/fixtures/hello_plugin` excluded from workspace.

### Storage

`StoragePool` wraps `SqlitePool` (Clone+Send+Sync, no `Arc<RwLock>` needed). Relational data in `{data_dir}/data.db`, vectors in `{data_dir}/lance/`. Data dir defaults to `~/.klyntbot`. Access via `Repos::from_pool(&pool)`. Feature crates add migrations via `FeatureMigration`.

### Key patterns

- **App-core + thin adapters:** `app-core` crate holds all shared business logic (handlers). Desktop `commands/*.rs` files are thin Tauri adapters that delegate to `AppCore` methods. Mutations use `emit_updates(&app, &updates)` for UI events. Dev server (`dev_server/`) delegates identically but discards entity updates.
- **Derive-based tools:** `#[derive(Tool)]` + `#[derive(ToolParams)]` from `tools-core-macros`. Multi-action: `#[tool_actions]` + `#[derive(ActionParams)]`. See `crates/tools/src/domain/docs.rs`.
- **Feature packages:** `feature-*` crates implement `FeaturePackage` (tools + migrations + config + health). Exception: some tools (e.g. `TaskTool`) are wired directly in the agent builder, not via `FeaturePackage::tools()` — check the crate's `tools()` return if wiring seems missing.
- **Dependency inversion:** Handler traits (`SpawnHandler`, `CronHandler`, etc.) defined in lower layers, implemented in `agent`. Injected as `Arc<dyn Trait>`.
- **Config:** `#[serde(rename_all = "camelCase")]`. File at `~/.klyntbot/config.json`. API keys in `Secret<String>` (access via `.expose()`). Env override: `KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o`.
- **Re-export facade:** `src/lib.rs` re-exports all public types. Use `klyntbot::AgentLoop`, `klyntbot::Config`, etc.

### Skill system & MCP

Five built-in orchestrator skills in `skills/`: general, task-management, finance-management, automation, communication. Each has `SKILL.md` (Agent Skills spec YAML frontmatter) + `references/` folder. Compiled via `include_str!` in `skill-system` crate. `SkillRouter` selects orchestrator per-message via keyword + semantic scoring. MCP tool names: `mcp_{server}_{tool}` (see `mcp::sanitize`). MCP access controlled per-skill via `mcp_tools` field (`["*"]` = all, `[]` = none). Task-management skill has `mcp_tools: ["google-calendar"]`.

**Progressive skill loading:** Orchestrator skills inject their full body on first activation (deduplicated per session). Activated (non-orchestrator) skills inject a summary only — the agent calls `skill_reference` tool to load full instructions when needed. Always-loaded references are filtered by message relevance (single-token refs always load, multi-token refs need a keyword match). This reduces token usage for simple messages.

Claude Code skills (`.claude/skills/klyntbot-*/SKILL.md`) are a separate layer that teaches Claude Code how to call klyntbot MCP tools. They follow Agent Skills format with `references/` for on-demand detail loading. These are NOT the same as internal skills in `skills/`.

### MCP server — exposing tools to Claude Code

Klyntbot exposes tools to external AI clients (Claude Code, Cursor, etc.) via MCP stdio transport (`klyntbot-mcp serve --stdio`). The desktop app also embeds the MCP server (config: `mcp.server` in `config.json`).

**Architecture:** `ToolRegistryBridge` translates MCP calls → internal `Tool::execute()`. The `agent` tool delegates natural language to the full AI pipeline via `AgentBridge`. Tool names must match the `ToolRegistry` key exactly (e.g. `tasks` not `task`, `notes` not `note`).

**Currently exposed tools:** `tasks`, `project`, `area`, `notes`, `memory`, `okr`, `finance`, `productivity`, `work_context`, `agent`, `annotate`, `learning`, `cron`, `mirror` — configured in `default_exposed_tools()` at `crates/config/src/schema/mcp.rs`.

**To expose a new feature tool via MCP:**

1. **Implement the tool** — `#[derive(Tool)]` in a `feature-*` crate, register in `ToolRegistry` via `FeaturePackage::tools()`. The tool's `name()` return value is the registry key.
2. **Add to default whitelist** — append the tool's registry name to `default_exposed_tools()` in `crates/config/src/schema/mcp.rs`. This controls which tools MCP clients can discover and call.
3. **Verify the name matches** — run `cargo nextest run -p klyntbot-server` to confirm the tool appears in `list_tools` and passes the whitelist. Common mistake: the tool registers as plural (`tasks`) but you added singular (`task`).
4. **Test via Claude Code** — rebuild the MCP binary (`cargo build -p klyntbot-mcp`), then in Claude Code call the tool: `mcp__klyntbot__<tool_name>`. No Claude Code config changes needed — it auto-discovers tools via `tools/list`.
5. **Override per-user** — users can customize the whitelist in `config.json` → `mcp.server.exposedTools`. If a user has overridden this array, new defaults won't take effect until they add the tool manually.

**Debug CLI:** `klyntbot-mcp tools --list` (list all exposed tools), `klyntbot-mcp tools --schema <name>` (show tool schema). Useful for verifying tool registration.

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

### Mirror self-reflection layer

`crates/cognitive/src/mirror/` — event-driven self-awareness system. Four subscribers watch domain events reactively, a facade provides the public API, cron jobs handle weekly narrative generation and retention cleanup.

**Subscribers:** `RoutingMirrorSubscriber` (hourly routing snapshots + drift detection), `MetaRuleDetector` (correction streaks → pending rule proposals), `ConfigArchiver` (autotuner promotions → brain version timeline), `TrialPreviewSubscriber` (4h early trial evaluation + kill/continue). All started by `MirrorEngine::start()` which returns `(MirrorFacade, Vec<JoinHandle>, CancellationToken)`.

**MirrorFacade:** Public API for state queries, user actions (approve/dismiss rules, kill/continue trials, revert brain versions), weekly narrative generation, and conversational mirror responses. Wired with optional `EpisodicMemoryRepo` (cross-feature memory ripple) and `Arc<DomainEventBus>` (auto-note on trial kill). Stored as `Option<Arc<MirrorFacade>>` in `AppCore`.

**Storage:** 6 tables in `crates/cognitive/migrations/003_mirror_tables.sql` (routing_snapshots, trend_narratives, snippets, meta_rules, brain_versions, trial_previews). Migration version managed via `cognitive_mirror` feature migration.

**MCP:** `MirrorTool` (multi-action, read-only) registered post-init via `agent.tool_registry()`. Actions: `get_state`, `get_narratives`, `get_routing_history`, `get_brain_versions`, `get_meta_rules`.

**Cron:** `JOB_MIRROR_WEEKLY_NARRATIVE` (Sunday 10am UTC), `JOB_MIRROR_CLEANUP` (Sunday 4am UTC — cleans snapshots, snippets, trial previews older than 90 days).

### Agent runtime

`AgentRuntime` → `SkillCatalog` + `SkillRouter` → `IntentAnalyzer` → `ContextEngine` → `ExecutionRouter` → `CostTracker`. Two execution modes: **Direct** (single LLM call, no tools) and **Reactive** (ReAct loop with tool calls, synthesizes at max_iterations). Code in `crates/agent/src/agent_runtime/` and `crates/agent/src/intent_pipeline/`. Skill types in `crates/skill-system/`.

**Mid-loop context compression:** During Reactive execution, the `MidLoopCompressor` checks token usage after each iteration. When accumulated message tokens exceed 70% of the context window, older `Message::Tool` results are replaced with extractive summaries (first 150 chars + metadata). System messages and recent iterations (last 8 messages) are always preserved verbatim. Emits `AgentEvent::ContextCompressed` for UI transparency.

**Live context refresh:** During Reactive execution, the `LiveContextRefresher` drains a shared `ContextUpdateQueue` (in the `bus` crate) at each iteration boundary. Context updates (e.g., newly promoted memories) are injected as `Message::ContextUpdate` entries with XML-tagged content. Token budget is respected — standard updates can use up to 80% of remaining context (20% reserved for LLM response); high-priority updates can use 90% (10% reserved). Emits `AgentEvent::ContextReassembled` for transparency. Set `pause_context_updates: true` on `ExecutionParams` for frozen-context mode. Phase 1 producer: cognitive background service pushes on memory promotion.

## Workflow

**Parallel sessions:** This workspace benefits from parallel Claude Code sessions. Use separate terminal tabs for independent crate work. For isolated changes, `git worktree` creates parallel checkouts without branch conflicts.

**Plan-then-execute:** For multi-crate changes, start in Plan mode (`/plan`) to design the approach, then switch to execution. Especially important for cross-layer changes (e.g., adding a new feature package that touches L1–L7).

**Subagents for repeatable work:** Use subagents for PR-shaped tasks: "simplify this diff", "verify all tests pass", "check clippy across workspace". Keep the main agent's context clean for architectural decisions.

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
- **Config hot-reload**: Model, temperature, max_tokens, max_iterations, pipeline_timeout, and monthly_budget changes take effect within 5 seconds (file watcher) or immediately (via settings UI). Structural changes (channels, provider init, feature enable/disable) still require restart.
- **Dependency inversion** — new tools needing agent context must inject via `Arc<dyn Trait>` to avoid circular deps.
- **`email` feature** (on by default) gates IMAP/SMTP deps in `channels` crate.
- **`tauri.conf.json` uses `bun`** in `beforeBuildCommand`. Ensure `bun` is installed globally.
- **Timestamps are UTC, display in local time** — Rust stores `chrono::Utc::now().to_rfc3339()`. For user-facing display strings formatted in Rust (e.g. `due_display` in `TodayTaskResponse`), convert to local first via `d.with_timezone(&chrono::Local)`. In the frontend, never `.slice()` ISO strings — parse via `new Date(iso)` and use `toLocaleTimeString()`. Shared helper: `formatTime()` in `desktop-ui/src/shared/lib/dates.ts`.
- **`MirrorEngine::start` takes `Arc<DomainEventBus>`** — not `&DomainEventBus`. The bus is cloned into the facade for `MirrorTrialKilled` event emission. Signature: `start(repo, bus: Arc<DomainEventBus>, narrative_handler, autotuner_bridge, episodic_repo)`. Handles and shutdown token must be stored in `AppCore` (not dropped).
- **Pre-release — no user data to migrate.** All schema changes can be made directly (alter tables, drop and recreate) without writing migration scripts. No need for backwards-compatible migrations until first release. When a migration is consolidated, update the `FeatureMigration` version and SQL in-place rather than adding incremental migration files. After first release, all schema changes require proper versioned migrations with `INSERT OR IGNORE` for idempotency.
