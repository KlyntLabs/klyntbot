# CLAUDE.md

> **This file is operational — build commands, conventions, gotchas, behavioral guidelines.**
> **Architecture lives in [`docs/architecture/`](./docs/architecture/).** Start there for any "how does X work" question; jump back here for "how do I build/test/contribute X". If the two disagree, the docs win.

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

Root facade crate has test binaries in `tests/`: `integration/` (cross-crate via facade), `e2e/` (agent loop + reminders), `unit/` (config, providers), `simulation/` (scenario-based agent smoke tests). Shared fixtures in `tests/common/`. All tests use ephemeral SQLite (`StoragePool::connect_in_memory()`). No external DB needed.

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

**Styling:** Hybrid Tailwind + plain CSS. **New components: use Tailwind** — wired via `@tailwindcss/vite` plugin in `vite.config.ts`. **Legacy styles** live in `src/styles/*.css` (imported through `src/styles/index.css`) with BEM-ish naming (e.g. `sidebar-chat__nav-item`). Design tokens are in `src/styles/ds-tokens.css`; themes in `src/styles/themes.{dark,light,dim,system}.css`. When adding a *new* legacy/shared CSS file (rare — prefer Tailwind), add an `@import` to `src/styles/index.css`.

**Typography tokens:** Never hardcode `font-size: Npx` in CSS. Use the scale in `src/styles/ds-tokens.css`: `--fs-2xs` (10.5px) / `--fs-xs` (11.5px) / `--fs-sm` (12.5px, default body — also exposed as `--fs-base`) / `--fs-md` (13.5px) / `--fs-lg` (15px) / `--fs-xl` (17px). Pick by role, not number — default text uses `var(--fs-base)`, secondary/labels step down to `--fs-xs`, headings step up to `--fs-lg`/`--fs-xl`. If no token fits (e.g. display headings ≥20px), add a new `--fs-*` to ds-tokens.css rather than hardcoding.

**Tauri IPC:** Direct `invoke()` from `@/api/client` (which re-exports `@tauri-apps/api/core`). There is no `useQuery` / `useMutation` / `ipc()` wrapper — call `invoke()` from a `useEffect` and manage state with `useState`. For Tauri events, import `listen` from `@tauri-apps/api/event` directly, or use the per-event hubs in `src/services/events.ts`. Endpoint definitions live under `src/api/endpoints/`.

**Markdown rendering:** Reuse `Markdown` from `@/features/messages/components/Markdown` rather than importing `react-markdown` directly.

**Testing:** Vitest + `@testing-library/react`. Test files colocated as `Component.test.tsx`. Mock Tauri APIs per-test via `vi.mock("@tauri-apps/api/core", ...)`.

**Linter:** ESLint via `bun run lint`. No Biome.

## Desktop App (Tauri 2)

```bash
cargo tauri dev                    # Full desktop app (start Vite separately: cd desktop-ui && bun run dev)
```

**Dev/prod isolation:** Set `KLYNTBOT_HOME=~/.klyntbot-dev` (via `.env` file or env var) to run a dev instance with separate config + data from production (`~/.klyntbot/`). Controls where `config.json`, `sessions/`, `workspace/`, `data.db`, `lance/`, `personas/` all live. A `.env` file at the project root is auto-loaded.

Browser-only dev: run `cd desktop-ui && bun run dev` then `cargo tauri dev` (which starts the embedded HTTP server on `:3456`), then open `localhost:1420`. The dev HTTP server lives in `crates/desktop/src/dev_server/` — no separate `dev-api` crate. Business logic lives in the `app-core` crate; `desktop` is a thin Tauri adapter. Tauri config: `crates/desktop/tauri.conf.json`. Shared IPC types: `desktop-shared` crate.

**Secondary windows:** Created via `WebviewWindowBuilder` in `crates/desktop/src/lazy_window.rs`. Pattern: `get_or_create_window(app, label)` with lazy creation on first show. Existing windows: `launcher` (660×580), `tray` (320×600), `distraction-overlay` (340×300), `voice-orb` (200×200) — all draggable + always-on-top + transparent via the shared `hud_effects()` helper (requires `macOSPrivateApi: true` in `tauri.conf.json`). Drag handle pattern: `getCurrentWindow().startDragging()` on a `lc-drag-handle` element. Multi-monitor cursor positioning helper: `shortcuts.rs::position_on_cursor_monitor`.

**Global hotkeys:** `tauri-plugin-global-shortcut = "2"` is the canonical path (already in `crates/desktop/Cargo.toml`). Hotkeys registered in `crates/desktop/src/shortcuts.rs::register_shortcuts`; user-configurable via `ShortcutsConfig` in `config.json`. Don't roll a custom `CFRunLoopSource` thread — the plugin handles modifier+key combos including `Alt+Cmd+Period` cleanly on macOS.

## Architecture

> **Authoritative architecture documentation lives in [`docs/architecture/`](./docs/architecture/).** Start with [`00-overview.md`](./docs/architecture/00-overview.md) — single-file mental model with the subsystem map, three end-to-end sequence diagrams (assistant turn, coding turn, nightly reforge), the 14-subsystem inventory, 11 critical-crate deep-dives, and a glossary. **If this file disagrees with `docs/architecture/`, the docs win** — keep them in sync.

Quick orientation: KlyntBot is a **64-crate Rust workspace** that ships as a single Tauri 2 desktop binary on macOS. Business logic lives in `app-core`; the `desktop` crate is a thin Tauri adapter; the root `klyntbot` crate is a *partial* re-export facade (≈18 of 62 workspace crates). All state in SQLite (WAL) + LanceDB under `~/.klyntbot/`. Sessions are tagged `assistant` or `coding` at creation and the mode is **immutable**; tools declare `allowed_channels = "all" | "non_coding" | "coding_only"`.

| When you're working on… | Open |
|---|---|
| The whole picture | [`docs/architecture/00-overview.md`](./docs/architecture/00-overview.md) |
| A specific subsystem | [`docs/architecture/subsystems/`](./docs/architecture/subsystems/) (14 files) |
| A critical crate's internals | [`docs/architecture/crates/`](./docs/architecture/crates/) (11 crates) |
| Known stubs, drift, anomalies | [`docs/architecture/TECH_DEBT.md`](./docs/architecture/TECH_DEBT.md) |
| Doc-system index & maintenance rules | [`docs/architecture/README.md`](./docs/architecture/README.md) |

### Coding patterns (day-to-day)

- **App-core + thin adapters:** `app-core` holds all shared business logic (handlers). Desktop `commands/*.rs` files are thin Tauri adapters that delegate to `AppCore` methods. Mutations call `emit_updates(&app, &updates)` for UI events. The dev server (`dev_server/`) delegates identically but discards entity updates.
- **Derive-based tools:** `#[derive(Tool)]` + `#[derive(ToolParams)]` from `tools-core-macros`. Multi-action: `#[tool_actions]` + `#[derive(ActionParams)]`. Domain enums: `#[derive(DomainEnum)]` with per-variant `#[canonical("...")]` and `#[aliases(...)]`. See `crates/tools/src/domain/docs.rs`. JSON Schema is hand-rolled in `tools-core-macros/src/helpers.rs::classify_type` — supports `String`, `bool`, primitives, `Vec<T>`, `Option<T>`; nested structs panic with a descriptive workaround pointer. **Four wiring paths exist** (`FeaturePackage`, agent builder, app-core init, subagent) — see the "Extension points" section in the overview.
- **Feature packages:** `feature-*` crates implement `FeaturePackage` (tools + migrations + config + health). Exception: some tools (e.g. `TaskTool`, `AlarmTool`, `LearningTool`) are wired directly in the agent builder, not via `FeaturePackage::tools()` — check the crate's `tools()` return if wiring seems missing.
- **Dependency inversion:** Handler traits (`SpawnHandler`, `CronHandler`, etc.) are defined in lower layers and implemented in `agent`. Injected as `Arc<dyn Trait>` to avoid circular crate deps.
- **Config:** `#[serde(rename_all = "camelCase")]`. File at `~/.klyntbot/config.json`. API keys in `Secret<String>` (access via `.expose()`). Env override: `KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o` (double-underscore = nested key).
- **Approval gate:** Every tool call passes through `approval::ApprovalGate::check`. Declare `approval_class` (Safe / Sensitive / Destructive / Admin) on the `Tool` trait. Coding-mode shell/edit/web_fetch get runtime classification via `CodingApprovalPolicy`. Persistent grants live in the `approval_grants` table.
- **Storage:** `StoragePool` wraps `SqlitePool` (Clone+Send+Sync). Access via `Repos::from_pool(&pool)`. Feature crates add migrations via `FeatureMigration`. **Tests must use `StoragePool::connect_in_memory()`** — `from_existing()` skips migrations.
- **MCP tool exposure:** To expose a new tool via MCP, add its registry name to `default_exposed_tools()` in `crates/config/src/schema/mcp.rs`. Tool names must match the `ToolRegistry` key exactly (e.g. `tasks` not `task`). Rebuild `desktop` — the MCP server ships inside it. Debug with `klyntbot mcp tools --list`.

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

- Errors: `common::Result<T>` (alias for `Result<T, KlyntBotError>`). Domain errors auto-convert via `From`.
- Imports: Use crate names directly (`use common::Result`), not `use crate::` for cross-crate refs.
- Tests: `#[cfg(test)] mod tests` inline. Integration tests in `tests/` via facade crate.
- Commits: Conventional format — `feat(scope): description`, `fix(scope): description`.
- Zero clippy warnings policy. `desktop` crate has pre-existing exceptions.
- **Tracing:** every public method on an `AppCore` handler must be annotated with `#[tracing::instrument(skip(self), err)]`. New handler methods inherit the convention. The Tauri command shells in `crates/desktop/src/commands/` are NOT instrumented (thin adapters); the trace span lives one layer down.

## Non-goals

The canonical list lives in the overview ([What's intentionally not in this system](./docs/architecture/00-overview.md#whats-intentionally-not-in-this-system)). The two most-likely-to-be-mistakenly-added items:

- **Structured observability** (OpenTelemetry, Prometheus, metrics dashboards) — single-user local app; existing `tracing` logs + `PipelineEvent` SSE stream are sufficient. Don't add observability infrastructure.
- **Backwards-compatibility migrations** pre-1.0 — alter schemas in place; see the pre-release gotcha below.

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
- **Built-in AI task automations were removed (2026-04-20).** The task tool no longer supports `plan_day`, `decompose`, `execute`, suggest/forecast actions, or auto-enrichment. These are now meant to be composed via cron + skills + the `agent` tool. `TaskCreated` / `TaskCompleted` still publish to the domain bus.
- **Coding-mode ingest is 5 adapters, mostly poll-only.** `claude_code` (hook-driven), `codex`, `kimi_cli`, `opencode`, `git_post_commit` — all under `crates/coding-ingest/src/adapters/` except `git_post_commit` (top-level `.rs`). Reforge writes go through `coding_memory::reforge::ReforgeWriter`; raw `DELETE` is rejected (supersede via `valid_until + superseded_by`). Full detail: [`docs/architecture/subsystems/09-coding-mode.md`](./docs/architecture/subsystems/09-coding-mode.md) and [`docs/architecture/crates/coding-ingest.md`](./docs/architecture/crates/coding-ingest.md).
- **Process hardening runs at startup** — `crates/desktop/src/main.rs` calls `klynt_process_hardening::pre_main_hardening()` as its first statement. This (a) sets `RLIMIT_CORE = 0` (no core dumps), (b) calls `ptrace(PT_DENY_ATTACH)` on macOS (debuggers cannot attach to a release build), and (c) scrubs `LD_*`/`DYLD_*`/`MallocStackLogging*` env vars. To debug a release build, comment the call out — debug builds are not affected because `PT_DENY_ATTACH` is harmless when no debugger tries to attach.
- **Snapshot rewind has two modes** — `coding_snapshots` rows with non-NULL `ghost_commit_sha` are restored via `klynt_git_utils::restore_ghost_commit` (git working-tree restore); rows with NULL `ghost_commit_sha` use the original BLOB path. The choice is made at snapshot-record time by `try_record_with_ghost` based on whether the file lives in a git repo. Implication: deleting `.git/` between snapshot and rewind makes ghost-mode rewind fail silently (logs a `tracing::error!`). Plan `2026-05-02-klynt-coding-in-chat-phase3-codex-polish.md` introduced this.
- **Pre-release — no user data to migrate.** All schema changes can be made directly (alter tables, drop and recreate) without writing migration scripts. No need for backwards-compatible migrations until first release. When a migration is consolidated, update the `FeatureMigration` version and SQL in-place rather than adding incremental migration files. After first release, all schema changes require proper versioned migrations with `INSERT OR IGNORE` for idempotency.
- **`SessionMode` is creation-time and immutable.** Sessions are tagged `assistant` or `coding` at insert; the column has a CHECK constraint. The legacy `chat_set_mode` Tauri command was removed (2026-05-04). To "switch modes" the user creates a new session via the appropriate entry point (`chat_send` for assistant, `coding_thread_start` for coding). The frontend `useAppMode()` store only controls *which entry point a "New chat" click invokes* — it does NOT mutate existing sessions.
- **Per-mode soul.** Assistant mode reads `~/.klyntbot/KLYNTBOT.md`; coding mode reads `~/.klyntbot/KLYNTBOT-coding.md`. Both are live-read with mtime caching. Edits to either take effect on the next message.
- **Assistant tool gating.** `feature-tasks`, `feature-finance`, `feature-notes`, `feature-productivity`, `feature-learning`, `feature-language-learning` declare `allowed_channels = "non_coding"`. The LLM in coding mode does not see them and cannot call them.

## Validation

The `kca-bench` / `kca-e2e` benchmark suite was **removed 2026-05-17** in favor of standard external evaluations (LoCoMo via mem0 + Letta's eval suite — wiring pending). Until those are wired, the enforced gates are chat-runtime only:

```bash
./scripts/run_chat_perf_gates.sh        # TTFT, stream throughput, relay cleanup, coalescer
./scripts/run_chat_proptest_soak.sh     # 10,000-case property soak (release branches)
```

See [`docs/architecture/subsystems/14-validation.md`](./docs/architecture/subsystems/14-validation.md) for status + replacement plan.
