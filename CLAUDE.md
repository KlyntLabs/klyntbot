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

Root facade crate has 5 test binaries in `tests/`: `integration/` (cross-crate via facade), `e2e/` (agent loop + reminders), `unit/` (config, providers), `plugins.rs` (WASM, needs `--features plugin-integration` + pre-built plugin), `simulation/` (scenario-based agent smoke tests). Shared fixtures in `tests/common/`. All tests use ephemeral SQLite (`StoragePool::connect_in_memory()`). No external DB needed.

## Dependency hygiene

Run periodically (e.g. before a release):
- `cargo machete` — fast static check for unused deps in `Cargo.toml`
- `cargo +nightly udeps --workspace` — slower but compiler-driven; catches what machete misses

## Desktop UI (desktop-ui/)

```bash
cd desktop-ui && bun install        # Install deps (always bun, never npm)
cd desktop-ui && bun run dev:vite   # Vite dev server (port 1420)
cd desktop-ui && bun run build      # Production build (tsc + vite build)
cd desktop-ui && bun run lint       # ESLint check
cd desktop-ui && bun run typecheck  # tsc --noEmit
cd desktop-ui && bun run test       # Vitest (run once)
cd desktop-ui && bun run test:watch # Vitest (watch mode)
```

**Path aliases** (`vite.config.ts` + `tsconfig.json`):
- `@/*` → `src/*`
- `@app/*` → `src/features/app/*`
- `@settings/*` → `src/features/settings/*`
- `@threads/*` → `src/features/threads/*`
- `@services/*` → `src/services/*`
- `@utils/*` → `src/utils/*`

Always use these in imports, never relative `../../` paths. Note: there is **no** `@shared` or `@features` alias — those were the old UI's conventions.

**Styling:** Plain CSS. No Tailwind. All styles in `src/styles/*.css`, imported through `src/styles/index.css`. Design tokens in `src/styles/ds-tokens.css`; themes in `src/styles/themes.{dark,light,dim,system}.css`. Class naming is BEM-ish (e.g. `sidebar-chat__nav-item`). When adding a new feature with its own CSS file, add an `@import` line to `src/styles/index.css`.

**Typography tokens:** Never hardcode `font-size: Npx` in CSS. Use the scale in `src/styles/ds-tokens.css`: `--fs-2xs` (10.5px) / `--fs-xs` (11.5px) / `--fs-sm` (12.5px, default body — also exposed as `--fs-base`) / `--fs-md` (13.5px) / `--fs-lg` (15px) / `--fs-xl` (17px). Pick by role, not number — default text uses `var(--fs-base)`, secondary/labels step down to `--fs-xs`, headings step up to `--fs-lg`/`--fs-xl`. If no token fits (e.g. display headings ≥20px), add a new `--fs-*` to ds-tokens.css rather than hardcoding.

**Tauri IPC:** Direct `invoke()` from `@/api/client` (which re-exports `@tauri-apps/api/core`). There is no `useQuery` / `useMutation` / `ipc()` wrapper — call `invoke()` from a `useEffect` and manage state with `useState`. For Tauri events, import `listen` from `@tauri-apps/api/event` directly, or use the per-event hubs in `src/services/events.ts`. Endpoint definitions live under `src/api/endpoints/`.

**Markdown rendering:** Reuse `Markdown` from `@/features/messages/components/Markdown` rather than importing `react-markdown` directly.

**Testing:** Vitest + `@testing-library/react`. Test files colocated as `Component.test.tsx`. Mock Tauri APIs per-test via `vi.mock("@tauri-apps/api/core", ...)`.

**Linter:** ESLint via `bun run lint`. No Biome.

## Desktop App (Tauri 2)

```bash
cargo tauri dev                    # Full desktop app (start Vite separately: cd desktop-ui && bun run dev)
```

**Dev/prod isolation:** Set `KLYNTBOT_HOME=~/.klyntbot-dev` (via `.env` file or env var) to run a dev instance with separate config + data from production (`~/.klyntbot/`). Controls where `config.json`, `sessions/`, `workspace/`, `data.db`, `lance/`, `plugins/`, `personas/` all live. A `.env` file at the project root is auto-loaded.

Browser-only dev: run `cd desktop-ui && bun run dev` then `cargo tauri dev` (which starts the embedded HTTP server on `:3456`), then open `localhost:1420`. The dev HTTP server lives in `crates/desktop/src/dev_server/` — no separate `dev-api` crate. Business logic lives in the `app-core` crate; `desktop` is a thin Tauri adapter. Tauri config: `crates/desktop/tauri.conf.json`. Shared IPC types: `desktop-shared` crate.

**Secondary windows:** Created via `WebviewWindowBuilder` in `crates/desktop/src/lazy_window.rs`. Pattern: `get_or_create_window(app, label)` with lazy creation on first show. Existing windows: `launcher` (660×580), `tray` (320×600), `distraction-overlay` (340×300), `voice-orb` (200×200) — all draggable + always-on-top + transparent via the shared `hud_effects()` helper (requires `macOSPrivateApi: true` in `tauri.conf.json`). Drag handle pattern: `getCurrentWindow().startDragging()` on a `lc-drag-handle` element. Multi-monitor cursor positioning helper: `shortcuts.rs::position_on_cursor_monitor`.

**Global hotkeys:** `tauri-plugin-global-shortcut = "2"` is the canonical path (already in `crates/desktop/Cargo.toml`). Hotkeys registered in `crates/desktop/src/shortcuts.rs::register_shortcuts`; user-configurable via `ShortcutsConfig` in `config.json`. Don't roll a custom `CFRunLoopSource` thread — the plugin handles modifier+key combos including `Alt+Cmd+Period` cleanly on macOS.

## Architecture

Rust personal AI agent — single binary connecting 6+ chat platforms to LLMs with task/project management and persistent memory. All state in SQLite + LanceDB.

### Workspace (39 crates, 9 layers)

```
L0: common, platform-macos, platform-input, platform-capture — KlyntbotError, MessageRole, ChannelName, ChatId, SessionKey; macOS native APIs (pasteboard, window mgmt, computer-use input + capture); platform-neutral input/capture trait crates
L1: config, bus, tools-core, tools-core-macros, analytics — Config (camelCase JSON), message bus, Tool/FeaturePackage traits, derive macros, FIRE/Monte Carlo analytics
L2: storage               — SqlitePool, migrations, *Repo structs, *Row types
L3: providers, session, scheduling, context_engine, skill-system — LLM clients, session persistence, cron, token budgets, skill discovery/routing
L4: tools, feature-tasks, feature-finance, feature-notes, feature-productivity, feature-coaching, feature-insights, feature-launcher, feature-learning (flashcard generation), feature-language-learning (pronunciation, practice sessions, exam tracking), activity-log, notifications (AlarmFired subscriber, quiet hours, held release, multi-channel fan-out), plugin-runtime, autotuner, voice-engine, simulator — 20+ tools, feature packages, WASM plugins, self-optimization experiments, voice synthesis, agent simulation
L5: channels, agent, cognitive — Platform integrations (Telegram/Discord/Slack/Email), agent runtime, cognitive memory (episodic/semantic extraction, spaced repetition via FSRS5, salience decay, reflection, reforge)
L6: mcp                   — MCP server/client
L7: app-core, desktop-shared, desktop — Application core (shared handlers), Tauri desktop app
L8: klyntbot, klyntbot-server — Re-export facade, standalone MCP server binary
```

Dependencies flow strictly upward. `plugin-sdk` and `tests/fixtures/hello_plugin` excluded from workspace.

### Storage

`StoragePool` wraps `SqlitePool` (Clone+Send+Sync, no `Arc<RwLock>` needed). Relational data in `{data_dir}/data.db`, vectors in `{data_dir}/lance/`. Data dir defaults to `~/.klyntbot`. Access via `Repos::from_pool(&pool)`. Feature crates add migrations via `FeatureMigration`.

### Key patterns

- **App-core + thin adapters:** `app-core` crate holds all shared business logic (handlers). Desktop `commands/*.rs` files are thin Tauri adapters that delegate to `AppCore` methods. Mutations use `emit_updates(&app, &updates)` for UI events. Dev server (`dev_server/`) delegates identically but discards entity updates.
- **Derive-based tools:** `#[derive(Tool)]` + `#[derive(ToolParams)]` from `tools-core-macros`. Multi-action: `#[tool_actions]` + `#[derive(ActionParams)]`. Domain enums (canonical name + alias resolution): `#[derive(DomainEnum)]` with per-variant `#[canonical("...")]` and `#[aliases(...)]`. See `crates/tools/src/domain/docs.rs`. JSON Schema is hand-rolled in `tools-core-macros/src/helpers.rs::classify_type` — supports `String`, `bool`, primitives, `Vec<T>`, `Option<T>`; nested structs panic with a descriptive workaround pointer.
- **Feature packages:** `feature-*` crates implement `FeaturePackage` (tools + migrations + config + health). Exception: some tools (e.g. `TaskTool`) are wired directly in the agent builder, not via `FeaturePackage::tools()` — check the crate's `tools()` return if wiring seems missing.
- **Dependency inversion:** Handler traits (`SpawnHandler`, `CronHandler`, etc.) defined in lower layers, implemented in `agent`. Injected as `Arc<dyn Trait>`.
- **Config:** `#[serde(rename_all = "camelCase")]`. File at `~/.klyntbot/config.json`. API keys in `Secret<String>` (access via `.expose()`). Env override: `KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o`.
- **Re-export facade:** `src/lib.rs` re-exports all public types. Use `klyntbot::AgentLoop`, `klyntbot::Config`, etc.

### Skill system & MCP

Five built-in orchestrator skills in `skills/`: task-management, finance-management, automation, learning, notebook. Each has `SKILL.md` (Agent Skills spec YAML frontmatter); some have `references/` folders. Compiled via `include_str!` in `skill-system` crate. `SkillRouter` selects orchestrator per-message via keyword + semantic scoring. MCP tool names: `mcp_{server}_{tool}` (see `mcp::sanitize`). MCP access controlled per-skill via `mcp_tools` field (`["*"]` = all, `[]` = none). Task-management skill has `mcp_tools: ["google-calendar"]`.

**Progressive skill loading:** Orchestrator skills inject their full body on first activation (deduplicated per session). Activated (non-orchestrator) skills inject a summary only — the agent calls `skill_reference` tool to load full instructions when needed. Always-loaded references are filtered by message relevance (single-token refs always load, multi-token refs need a keyword match). This reduces token usage for simple messages.

Claude Code skills (`.claude/skills/klyntbot-*/SKILL.md`) are a separate layer that teaches Claude Code how to call klyntbot MCP tools. They follow Agent Skills format with `references/` for on-demand detail loading. These are NOT the same as internal skills in `skills/`.

### MCP server — exposing tools to Claude Code

Klyntbot exposes tools to external AI clients (Claude Code, Cursor, etc.) via MCP stdio transport. The MCP server is merged into the desktop binary as a subcommand (`klyntbot mcp serve --stdio`) — there is no separate `klyntbot-mcp` binary. Claude Code spawns the binary as a child process per session; it shares `~/.klyntbot/data.db` with the desktop process via SQLite WAL. The desktop app also embeds an MCP HTTP server (config: `mcp.server` in `config.json`). On first launch the desktop app auto-registers itself with Claude Code via `claude mcp add` if the CLI is detected — gated by a one-time marker file at `~/.klyntbot/.claude-code-integration-offered`.

**Architecture:** `ToolRegistryBridge` translates MCP calls → internal `Tool::execute()`. The `agent` tool delegates natural language to the full AI pipeline via `AgentBridge`. Tool names must match the `ToolRegistry` key exactly (e.g. `tasks` not `task`, `notes` not `note`).

**Currently exposed tools:** `tasks`, `project`, `area`, `notes`, `memory`, `okr`, `finance`, `productivity`, `work_context`, `agent`, `annotate`, `learning`, `cron`, `mirror`, `temporal` — configured in `default_exposed_tools()` at `crates/config/src/schema/mcp.rs`.

**To expose a new tool via MCP:** (1) `#[derive(Tool)]` in a `feature-*` crate, register via `FeaturePackage::tools()`. (2) Add registry name to `default_exposed_tools()` in `crates/config/src/schema/mcp.rs`. (3) Verify: `cargo nextest run -p klyntbot-server`. Common mistake: plural/singular mismatch (`tasks` vs `task`). (4) Rebuild the desktop binary (`cargo build -p desktop`) — the MCP server ships inside it. Users can override the whitelist in `config.json` → `mcp.server.exposedTools`.

**Debug CLI:** `klyntbot mcp tools --list` (lists exposed tools the embedded server would advertise).

### Cognitive subsystems

**Tray countdown** (`tray_countdown.rs`): Live menu bar countdown to next calendar event/task deadline. Coordinates with focus timer via `FOCUS_ACTIVE` atomic flag. Uses `tauri::async_runtime::spawn` (not `tokio::spawn`) — starts during Tauri `setup` hook before tokio runtime is available.

**Mirror** (`crates/cognitive/src/mirror/`): Event-driven self-reflection. Six signal sources (routing snapshots, meta-rule detection, config archiving, trial preview, task focus, finance drift) implementing `MirrorSignalSource` trait. `MirrorEngine::start()` returns `StartedMirror { facade, consumers, flush_handles, shutdown }` — handles must be stored in `AppCore` (not dropped). `MirrorFacade` stored as `Option<Arc<MirrorFacade>>` in `AppCore`. MCP: `MirrorTool` (multi-action, read-only) registered post-init. 8 tables in `003_mirror_tables.sql`.

**Computer Use & Procedural Memory** (in design — see [`docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md`](docs/superpowers/specs/2026-04-28-computer-use-and-procedural-memory-design.md), not yet implemented): Full-OS automation feature with hybrid AVR perception cascade (Accessibility tree → local VLM → cloud VLM, all routed through `ProviderManager`), risk-tier safety gates with NSAlert + `AskUserTool` confirmation, HUD overlay + side panel UI, time-bound `ComputerUseSession` for background automation, and procedural memory (Intent → Stage → Action trajectories distilled into `web_tree_memories` for replay). When implementing: adds `platform-input`/`platform-capture` trait crates at L0, `feature-browser-control` (CDP integration) + `feature-computer-use` at L4, and `WorkflowInductionSignals` as a 7th mirror source. Anthropic adapter gains `ContentPart::ImageData`, the `computer_20251124` tool block, and an `anthropic-beta` header for the `computer-use-2025-11-24` channel — none of which exist today. `MidLoopCompressor` will need an image-aware exception path; `RoutingContext` will gain a `screenshot_tx` sidecar channel mirroring the existing `entity_tx` pattern. Pending implementation plan in `docs/superpowers/plans/`.

**Reforge** (`crates/cognitive/src/services/reforge/`): Nightly self-improvement cycle (cron `JOB_REFORGE_NIGHTLY`, 03:00 local — registered in `app-core/src/init/cron.rs`). Reviews strategy files, collects behavioral feedback, generates rewrite suggestions via LLM. The `service.rs::run_reforge` pipeline has **9 phases** with 3 LLM calls: Collect → Synthesize (LLM JSON) → Coding-Synthesis → Review (LLM JSON) → Rule-Artifact-Generation → Narrate (LLM text) → Apply → Optimize → Graph-Consolidation+Community → Compact. Provider sourced from `config.cognitive.provider` via `DynProvider` (NOT the agent `ProviderManager`); temperature 0.2, max_tokens 4096. Suggestions persist to `reforge_suggestions` table. Extension hooks: `CodingPhaseRunner`, `GraphEnrichmentHandler`, `CommunityIntelligenceHandler` — each is an `Option<&dyn Trait>` parameter on `run_reforge`.

### Agent runtime

`AgentRuntime` → `SkillCatalog` + `SkillRouter` → `IntentAnalyzer` → `ContextEngine` → `ExecutionRouter` → `CostTracker`. Two execution modes: **Direct** (single LLM call, no tools) and **Reactive** (ReAct loop with tool calls, synthesizes at max_iterations). Code in `crates/agent/src/agent_runtime/` and `crates/agent/src/intent_pipeline/`. Execution loop internals in `crates/agent/src/execution/`. Skill types in `crates/skill-system/`.

**Execution constants** (`crates/agent/src/execution/core.rs`): `MAX_CONCURRENT_TOOLS = 10` (parallel tool fan-out via global semaphore — there's no per-tool serial-only flag today), `MAX_TOOL_RESULT_LENGTH = 50_000` bytes (results truncated past this), `INTERACTIVE_TOOL_TIMEOUT = 600s` (only for `ask_user`; default `params.tool_timeout = 30s`). `MidLoopCompressor` constants: `COMPRESSION_THRESHOLD = 0.70`, `MIN_RECENT_MESSAGES = 8`, `MIN_COMPRESSIBLE_TOKENS = 50`. `ANTHROPIC_CONTEXT_WINDOW = 200_000`.

**Cancellation:** `tokio_util::sync::CancellationToken` carried in `ExecutionParams::cancel_token`. Active sessions tracked in `ActiveStreams = DashMap<String, CancellationToken>` (`crates/agent/src/agent_loop/streaming.rs:27`). Cancel observed at iteration boundary (`execute_loop.rs:113`) — in-flight tool calls run to their timeout before cancellation fires. `chat_cancel()` removes the entry and calls `token.cancel()`.

**Mid-loop context compression:** `MidLoopCompressor` (in `crates/agent/src/execution/`) triggers when message tokens exceed 70% of context window. Replaces older `Message::Tool` results with extractive summaries (~150-char first-snippet + size annotation). Preserves system messages and last 8 messages verbatim. Emits `AgentEvent::ContextCompressed`. **Note:** `Message::Tool.content` is currently a plain `String`; image-bearing tool results have no schema today.

**Tiered History Compression (THC):** `TieredHistoryCompressor` (in `crates/context_engine/src/history_compressor/`) operates at the context-engine level before messages enter the LLM. Groups messages into `ConversationTurn`s, optionally scores via `MemoryScorer`, assigns tiers (Verbatim / Detailed / Condensed), and compresses with tier-specific prompts. Extractive-first: falls back to snippet extraction when LLM summaries aren't needed or fail. Configured via `HistoryCompressionConfig`.

**Live context refresh:** `LiveContextRefresher` (in `crates/agent/src/execution/`) drains `ContextUpdateQueue` (in `bus` crate) at each iteration boundary. Injects `Message::ContextUpdate` entries. Token budget: standard 80%, high-priority 90%. Set `pause_context_updates: true` on `ExecutionParams` for frozen-context mode.

## Behavioral Guidelines

### Think before coding

- State assumptions explicitly. If uncertain, ask — don't guess silently.
- If multiple interpretations exist, present them. Don't pick one without saying so.
- If a simpler approach exists, say so. Push back when warranted.

### Surgical changes

- Don't "improve" adjacent code, comments, or formatting. Match existing style.
- Don't refactor things that aren't broken. If you notice unrelated dead code, mention it — don't delete it.
- Remove imports/variables/functions that YOUR changes made unused. Don't remove pre-existing dead code unless asked.
- **The test:** every changed line should trace directly to the user's request.

### Goal-driven execution

For multi-step tasks, state a brief plan with verification:
```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
```
Transform vague tasks into verifiable goals — "fix the bug" → "write a test that reproduces it, then make it pass".

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
- **Tracing:** every public method on an `AppCore` handler must be annotated with `#[tracing::instrument(skip(self), err)]`. New handler methods inherit the convention. The Tauri command shells in `crates/desktop/src/commands/` are NOT instrumented (thin adapters); the trace span lives one layer down.

## Non-goals

- **Structured observability (OpenTelemetry, Prometheus, metrics dashboards)** — this is a single-user local app. Existing `tracing` logs and `PipelineEvent` SSE stream are sufficient. Don't add observability infrastructure.

## System prompt = KLYNTBOT.md

The system prompt sent to the LLM on every chat turn is built from `ContextEngine::build_system_prompt()` (called by `AgentRuntime::process` at `crates/agent/src/agent_runtime/runtime.rs`). It joins all registered `ContextSource` outputs in priority order; the highest-priority source is `SoulContextSource`, which lives-reads `~/.klyntbot/KLYNTBOT.md` (or `~/.klyntbot-dev/KLYNTBOT.md`). Edits to that file take effect on the next message — no restart, no rebuild. To change the agent's tone, formatting rules, persona, language, or any global behavior, **edit KLYNTBOT.md** rather than hard-coding it in Rust. The default content (used on first run if the file is missing) lives in `crates/skill-system/src/soul.rs::DEFAULT_SOUL`. The squad chat path (`handlers/chat/streaming.rs::execute_direct_address`) uses a different prompt builder (`debate::build_persona_system_prompt`) and does NOT read the soul — squads override personality via persona rows.

## Gotchas

- **MSRV 1.93** — Rust stable 1.93. APIs like `is_some_and`, `is_none_or` are available. Clippy catches MSRV violations.
- **Adding a Tauri command (Plan 6)** — The IPC surface is gated behind two attribute macros in `crates/desktop-macros/`. Direct `#[tauri::command]` is forbidden in `crates/desktop/src/commands/` and `crates/desktop/src/oauth/` (enforced by `no_raw_tauri_command_outside_macros` test).
  - Use `#[klynt_command]` for the happy path (`pub async fn`, no `state` param, bare `T` return).
  - Use `#[klynt_raw_command]` otherwise (sync, non-AppCore state, `rename_all`, etc.).
  - After adding a command, list its path in `desktop_macros::klynt_collect_commands![...]` in `specta_builder.rs`. The macro auto-generates `KLYNT_SPECTA_COMMAND_NAMES` (aliased as `SPECTA_COMMAND_NAMES`) from the same list, so there is no second manual array to maintain. The `registration_drift` test fails until you do.
  - Run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`. The `bindings_are_current` test fails until you do.
- **`StoragePool::from_existing()` skips migrations** — only for already-migrated pools. Tests must use `connect_in_memory()`.
- **Config hot-reload**: Model, temperature, max_tokens, max_iterations, pipeline_timeout, and monthly_budget changes take effect within 5 seconds (file watcher) or immediately (via settings UI). Structural changes (channels, provider init, feature enable/disable) still require restart.
- **Dependency inversion** — new tools needing agent context must inject via `Arc<dyn Trait>` to avoid circular deps.
- **`email` feature** (on by default) gates IMAP/SMTP deps in `channels` crate.
- **`tauri.conf.json` uses `bun`** in `beforeBuildCommand`. Ensure `bun` is installed globally.
- **Timestamps are UTC, display in local time** — Rust stores `jiff::Timestamp::now()` which serialises as RFC 3339 (`2026-04-19T14:30:00Z`) by default via serde. For user-facing display strings formatted in Rust, convert with `ts.to_zoned(jiff::tz::TimeZone::system())` and format via `.strftime("%-I:%M %p")`. In the frontend, parse with `new Date(iso)` and use `toLocaleTimeString()` — never `.slice()` ISO strings. Shared helper: `formatTime()` in `desktop-ui/src/shared/lib/dates.ts`.
- **`MirrorEngine::start` takes `Arc<DomainEventBus>`** — not `&DomainEventBus`. Signature: `start(repo, bus: Arc<DomainEventBus>, narrative_handler, autotuner_bridge, episodic_repo)`. Returns `StartedMirror` with `facade`, `consumers`, `flush_handles`, `shutdown`.
- **Built-in AI task automations removed (2026-04-20).** The task tool no longer supports `plan_day`, `decompose`, `execute`, `suggest` / `apply_suggestion` / `dismiss_suggestion` / `list_suggestions`, `forecast_task` / `forecast_project` / `accuracy_report`, or auto-enrichment (LLM priority/duration/scheduling inference). These LLM-driven behaviors are now meant to be composed by users via cron + skills + the `agent` tool. `TaskCreated` / `TaskCompleted` still publish to the domain bus so cognitive and reforge continue to receive task signals. Design for deepening that integration is parked in `docs/superpowers/brainstorms/2026-04-20-seed-task-cognitive-integration.md`.
- **opencode adapter is poll-only** — no `klyntbot-hook opencode` subcommand fires; the daemon's `OpencodePoller` task drives ingestion. Disable via `coding_memory.opencode.enabled = false`.
- **Coding-memory Phase 7 — multi-CLI ingest.** Four `IngestAdapter` implementations live under `crates/coding-ingest/src/adapters/`: `claude_code`, `codex`, `kimi_cli`, `opencode`. Codex, Kimi, and opencode are poll-only (Codex via `~/.codex/sessions` JSONL; Kimi via `~/.kimi/sessions/<hash>/<uuid>/wire.jsonl`; opencode via SQLite WAL). Hook-driven adapters were removed in 2026-04-29 — `KimiInstaller` no longer exists. All adapters emit `AgentEvent` tagged with `AgentSource` (`ClaudeCode`, `Codex`, `KimiCli`, `OpenCode`, `KlyntCli`). The cross-CLI normalization invariant (Inv 7 — `parse(serialize(event)) == event`) is enforced by the proptest in `crates/coding-ingest/tests/cross_cli_normalization.rs`. Reforge removal-side writes route through `coding_memory::reforge::ReforgeWriter` (re-exported from `reforge/mod.rs`) — raw `DELETE` is rejected; supersede via `valid_until + superseded_by` is the only sanctioned removal.
- **Pre-release — no user data to migrate.** All schema changes can be made directly (alter tables, drop and recreate) without writing migration scripts. No need for backwards-compatible migrations until first release. When a migration is consolidated, update the `FeatureMigration` version and SQL in-place rather than adding incremental migration files. After first release, all schema changes require proper versioned migrations with `INSERT OR IGNORE` for idempotency.


## KCA — Klynt Cognitive Architecture validation gates

The memory system is governed by spec section 7 quality / perf / stability gates. Before any merge to main:

`./scripts/run_kca_validation.sh`

Any gate failure blocks merge. Soak test runs only on tagged release branches (`RUN_SOAK=1`).

Auto-generated game-changer report lives at `docs/architecture/kca-game-changer.md`, refreshed every CI run, archived as artifact.
