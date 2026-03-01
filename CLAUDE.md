# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build --workspace              # Build all crates
cargo build --release                # Optimized release build (LTO, stripped)
cargo nextest run --workspace        # Run all tests (parallel, faster than cargo test)
cargo nextest run -p agent           # Test a single crate
cargo nextest run -p storage         # Test storage crate (uses ephemeral SQLite)
cargo nextest run --test integration_tests  # Run a specific test file
cargo nextest run -E 'test(session_persistence)'  # Run tests matching pattern
cargo nextest run --nocapture        # Show test stdout (nextest captures by default)
cargo nextest run --failure-output immediate  # Show failures as they happen
cargo test --workspace --doc         # Run doctests (nextest doesn't support doctests, use cargo test)
cargo clippy --workspace --all-targets --all-features  # Lint (must be 0 warnings)
cargo fmt --all --check              # Check formatting
cargo build --no-default-features    # Build without email channel
```

**Why nextest?** Parallel test execution (faster), better output formatting, test retries, partition support for CI. Falls back to `cargo test --doc` for doctests only.

**No external database required**: All tests use ephemeral SQLite pools (`StoragePool::connect(tempdir)` or `StoragePool::connect_in_memory()`).

## Desktop UI (desktop-ui/)

```bash
cd desktop-ui && bun run dev            # Start Vite dev server (port 1420)
cd desktop-ui && bun run build          # Production build
cd desktop-ui && bun install            # Install dependencies
```

**Always use `bun` (not `npm`) for the desktop-ui frontend.**

**Tailwind v4 (CSS-driven, no config file):** Theme tokens defined in `src/styles/theme.css` via CSS variables + `@theme inline`. No `tailwind.config.js` — all customization is in CSS.

**Color token system:** Flat, dark-mode-only tokens in `:root`. Surface staircase (`surface-lowest` through `surface-highest`), text hierarchy (`text-primary/secondary/muted/dim`), brand (`brand`, `brand-hover`), semantic (`success`, `destructive`, `info`, `purple`). Never hardcode hex/rgba in components — always use the token utilities (e.g., `bg-surface-base`, `text-muted`, `border-border`).

## Architecture

Klyntbot is a Rust personal AI agent — a single binary that connects to 6+ chat platforms, calls LLMs, manages tasks/projects, syncs with Apple Calendar, and manages persistent memory. It is **not** a code execution platform — users have dedicated tools (Claude Code, Cursor, Codex) for that. All persistent state is stored in SQLite (relational data) + LanceDB (vector embeddings).

### Workspace layout (19 crates in 8 dependency layers)

```
Layer 0: common              — Error types (KlyntbotError, 15 variants), MessageRole, ChannelName, ChatId, SessionKey
Layer 1: config, bus, tools-core, tools-core-macros
                             — Config schema (camelCase JSON serde), async message bus (tokio::mpsc),
                               Tool trait, FeaturePackage trait, ToolRegistry, derive macros (#[derive(Tool)], #[derive(ToolParams)])
Layer 2: storage, domain     — SQLite pool (sqlx + SqlitePool), auto-migrations, repository pattern (*Repo structs),
                               plan/goal domain types
Layer 3: providers, session, scheduling, calendar, context_engine
                             — LLM HTTP clients, session persistence, cron service, CalDAV sync, token budget allocator
Layer 4: tools, feature-todo, feature-finance, plugin-runtime
                             — 20+ tool implementations (filesystem ×4, web ×2, grep, glob, message, spawn, cron, calendar,
                               plan, project, goal, memory, learning, browser, ask_user, agent_task),
                               self-contained feature packages (own tools, migrations, config, handler traits),
                               WASM plugin sandbox
Layer 5: channels, agent     — Chat platform integrations (Telegram, Discord, WhatsApp, Slack, Email, QQ),
                               agent loop, intent pipeline, execution core, memory store, skill manager, subagent manager
Layer 6: cli                 — Clap-derived CLI with 4 commands: serve, init, status, plugin
Layer 7: klyntbot            — Re-export facade (src/lib.rs) + binary entry point (src/main.rs)
```

One additional crate (`plugin-sdk`) is excluded from the workspace.

Dependencies flow strictly upward. No circular dependencies — enforced by Cargo.

**Storage stack (SQLite + LanceDB):**
- `storage` crate (Layer 2): `StoragePool` wraps `SqlitePool`, auto-runs migrations, exposes repository pattern (`*Repo` structs)
- All relational data in SQLite (`{data_dir}/data.db`): todos, projects, sessions, goals, plans, cron jobs, usage, strategies, outcomes, learning state, memory notes, calendar cache, finance data (accounts, transactions, budgets, investments), agent tasks
- Vector embeddings in LanceDB (`{data_dir}/lancedb/`): todo embeddings, conversation embeddings — replaces pgvector
- `Repos` aggregate struct for convenient access: `Repos::from_pool(&pool)`
- `StoragePool::connect(data_dir)` creates/opens `data.db`, enables WAL + foreign keys, runs migrations. Feature crates register additional migrations via `FeatureMigration`
- `StoragePool::connect_in_memory()` for tests — runs migrations on an in-memory SQLite pool
- Data directory defaults to `~/.klyntbot`, configurable via `data_dir` in config

### Key patterns

- **Repository pattern**: All persistent state goes through `*Repo` structs in the `storage` crate. Repos hold a `SqlitePool` (which is `Clone + Send + Sync` internally via `Arc`), eliminating the need for `Arc<RwLock<Store>>` wrappers. The `Repos` aggregate provides convenient access: `Repos::from_pool(&pool)`.
- **Derive-based tools**: Tools are defined via `#[derive(tools_core::Tool)]` and `#[derive(ToolParams)]` macros from `tools-core-macros`. These generate `Tool` trait impls, parameter extraction, and JSON schema. Multi-action tools use `#[tool_actions]` attribute macro with `#[derive(ActionParams)]` per-action params. New tools should use this pattern — see `crates/tools/src/filesystem.rs` for examples.
- **Feature packages**: Self-contained features (`feature-todo`, `feature-finance`) implement the `FeaturePackage` trait (in `tools-core`), which bundles tools, migrations, config validation, and health checks. Registered at agent startup. Add new features by creating a `feature-*` crate implementing `FeaturePackage`.
- **Dependency inversion**: Handler traits (`SpawnHandler`, `CronHandler`, `CalendarHandler` in `tools`; `EnrichmentHandler`, `EmbeddingHandler` in `feature-todo`; `FinanceHandler` in `feature-finance`; `GoalHandler`, `PlanHandler`, `LearningHandler` in `tools`) are defined in lower layers but implemented in `agent` (Layer 5). Injected as `Arc<dyn Trait>` at construction.
- **Re-export facade**: `src/lib.rs` re-exports all public types from workspace crates. Integration tests and external consumers use `klyntbot::AgentLoop`, `klyntbot::Config`, `klyntbot::StoragePool`, etc.
- **Provider auto-detection**: The provider registry matches model name keywords to route to the correct LLM provider. No external routing library.
- **Config schema**: All config structs use `#[serde(rename_all = "camelCase")]`. Config file is `~/.klyntbot/config.json`. API keys are wrapped in `Secret<String>` (redacted in Debug/Display, access via `.expose()`).
- **Feature-gated email**: The `email` feature (on by default) gates IMAP/SMTP dependencies in the `channels` crate.

### Extension traits

| Trait | Defined in | Purpose |
|-------|-----------|---------|
| `Tool`, `ToolExecute`, `ToolParams` | `tools-core` | Core tool framework — usually derived via `#[derive(Tool)]` / `#[derive(ToolParams)]` |
| `FeaturePackage` | `tools-core` | Self-contained feature registration (tools, migrations, config, health) |
| `InteractionChannel` | `tools-core` | Platform-native UI (Telegram buttons, Discord selects) — avoids circular deps with channels |
| `LlmProvider` | `providers` | `async fn chat()`, `async fn chat_stream()`, `fn name()`, `fn default_model()`, etc. |
| `Channel` | `channels` | `async fn start()`, `async fn stop()`, `async fn send()`, `fn name()`, `fn is_allowed()` |
| `SpawnHandler` | `tools` | Dependency inversion for subagent spawning |
| `CronHandler` | `tools` | Dependency inversion for cron job management |
| `CalendarHandler` | `tools` | Dependency inversion for calendar sync |
| `GoalHandler` | `tools` | Dependency inversion for goal management + LLM plan generation |
| `PlanHandler` | `tools` | Dependency inversion for plan management + LLM step generation |
| `LearningHandler` | `tools` | Dependency inversion for adaptive threshold management |
| `EnrichmentHandler` | `feature-todo` | Dependency inversion for AI-powered task enrichment |
| `EmbeddingHandler` | `feature-todo` | Dependency inversion for todo embedding generation |
| `FinanceHandler` | `feature-finance` | Dependency inversion for finance price lookups |
| `IntentPipeline` | `agent` | Full pipeline: IntentAnalyzer -> ContextEngine -> ExecutionRouter -> ResponseValidator -> CostTracker |
| `ExecutionEngine` | `agent` | Unified async trait for Direct, Reactive, and Planned engines |

### Conventions

- Error handling: Use `common::Result<T>` (alias for `Result<T, KlyntbotError>`). Domain errors auto-convert via `From` impls.
- Imports: Use crate names directly (`use common::Result`, `use config::Config`), not `use crate::` for cross-crate refs.
- Tests: Unit tests as `#[cfg(test)] mod tests` inline in each crate. Integration tests in `tests/` use the facade crate. Shared mock provider in `tests/mock_provider.rs`.
- Commits: Conventional format — `feat(providers): add streaming`, `fix(channels): handle rate limits`.
- Zero clippy warnings policy.

## CLI Subcommands

```bash
klyntbot serve --port 8080       # Start gateway daemon (channels, cron, heartbeat)
klyntbot init                    # 2-phase setup wizard (core setup + pack selection)
klyntbot init --packs            # Jump directly to pack selection
klyntbot init --reset            # Reset config to defaults before running wizard
klyntbot status [--verbose]      # Show agent/config status
```

Task management, project management, calendar sync, cron jobs, skills, and all other features are accessible through channel integrations (Telegram, Discord, etc.) or the dashboard.

## Environment Variables

Config can be overridden via `KLYNTBOT_` prefix with `__` as nesting separator:

```bash
KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o
KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-...
KLYNTBOT_TOOLS__RESTRICT_TO_WORKSPACE=true
```

No external database required. Data is stored in `~/.klyntbot/data.db` (SQLite) and `~/.klyntbot/lancedb/` (vectors) by default. Override with `data_dir` in config.

## Enrichment & Semantic Search

**Enrichment** (`feature-todo` crate): Auto-infers priority, duration, and due dates from task title keywords. Optionally enhanced with LLM-based inference when `use_llm` is enabled. Config: `todo.enrichment.enabled` (default: `true`), `todo.enrichment.autoApplyThreshold` (default: `0.70`). Implementation in `crates/feature-todo/src/enrichment.rs`.

**Semantic search** (`feature-todo` crate): Uses fastembed (paraphrase-multilingual-MiniLM-L12-v2, 384d) + LanceDB for ANN similarity search. Three modes: `search` (keyword SQL), `search-semantic` (cosine similarity), `search-hybrid` (RRF merge). Config: `todo.search.enabled`, `todo.search.semanticThreshold` (default: `0.5`). Embeddings stored in `{data_dir}/lancedb/`.

## Feature Packs

Feature packs bundle config + skills into selectable groups chosen during `klyntbot init`.

- **Core** (always): task-management. **Recommended**: productivity, ai-intelligence. **Optional**: finance, weather, skill-creator.
- Config: `packs.enabled` and `packs.enabledSkills` arrays in config.json.
- Registry: `crates/cli/src/wizard/packs/registry.rs`. Config mutations: `pack_selection.rs`.
- At startup, `SkillManager::filter_by_skills()` restricts built-in skills to enabled packs. Workspace skills (`~/.klyntbot/skills/`) are always kept.

## Skills

Built-in skills live in `skills/` as `SKILL.md` files (browser, cron, daily-planning, finance, skill-creator, summarize, todo, weather, weekly-report). Nine skills are bundled at compile time via `include_str!`. Workspace skills loaded from `~/.klyntbot/skills/*/SKILL.md` override built-in skills with the same name. YAML frontmatter provides metadata: `description`, `version`, `always` (always load full content), `triggers` (activation keywords), `requires_bins` and `requires_env` (prerequisite checks). Skills are filtered by enabled packs.

## Gotchas & Common Pitfalls

- **No external DB required**: SQLite file is created automatically at `{data_dir}/data.db` on first run. LanceDB directory is created at `{data_dir}/lancedb/` when semantic search is first used.
- **Data directory**: Defaults to `~/.klyntbot`. Override with `data_dir` in `~/.klyntbot/config.json`.
- **`StoragePool::from_existing()` skips migrations**: Only use for pools already migrated by `StoragePool::connect()`. For in-memory test pools, always use `StoragePool::connect_in_memory()`.
- **CalDAV sync is async**: Calendar sync runs in background. Use `CalendarTool::sync_now()` for immediate sync, or wait for next scheduled interval.
- **Config changes require restart**: Modifying `~/.klyntbot/config.json` requires restarting `klyntbot serve` for changes to take effect.
- **Dependency inversion gotcha**: When adding new tools that need agent context (spawn/cron handlers), inject via `Arc<dyn Trait>` at construction to avoid circular deps.
- **SqlitePool is Clone+Send+Sync**: Unlike the old `Arc<RwLock<Store>>` pattern, `SqlitePool` (and therefore all `*Repo` structs) can be freely cloned and shared across tasks without locking. Connection pooling is handled internally by sqlx.

## Intent Pipeline

### Architecture

The IntentPipeline replaces the former Orchestrator + EngineDispatch + AgentPipeline with a unified flow:

```
IntentAnalyzer → ContextEngine → ExecutionRouter → ResponseValidator → CostTracker
```

**Modules** in `crates/agent/src/intent_pipeline/`:

| Module | Purpose |
|--------|---------|
| `types.rs` | `ExecutionMode` (Direct/Reactive/Planned), `ComplexitySignals`, `IntentAnalysis`, `ToolGroup` |
| `analysis.rs` | `IntentAnalyzer` (two-stage: heuristic keywords -> LLM `IntentClassifier`). Strategy history from `StrategyRepo` feeds classifier context. |
| `engines/` | `ExecutionEngine` trait + `DirectEngine`, `ReactiveEngine`, `PlannedEngine` |
| `router.rs` | `ExecutionRouter` — maps mode to engine, handles escalation chain (Direct -> Reactive -> Planned) with `EscalationContext` |
| `pipeline.rs` | `IntentPipeline` struct — wires everything into `process_message()` (classify -> context -> filter tools -> route -> validate -> record) |
| `visibility.rs` | `PlanCleanupService` — background cleanup for stale silent/on_failure plans |

### Execution Modes

- **Direct**: Single LLM call, no tools. For greetings, simple questions, acknowledgments.
- **Reactive { max_iterations }**: ReAct loop with tool calls. For task CRUD, search, calendar ops. Escalates at 80% of max_iterations.
- **Planned { visibility, max_steps }**: Multi-step plan generation and execution. For complex multi-tool workflows. Falls back to ReactiveEngine(50) if plan generation fails.

### Escalation Chain

When an engine signals it cannot handle a request (`EngineResult::Escalate`), the router automatically escalates: Direct -> Reactive -> Planned. `EscalationContext` carries messages + completed tool work across transitions. Max escalations are configurable via `config.orchestrator.max_escalations` (default: 3).

### ExecutionCore

Shared by all engines. `run_cycle()` performs one LLM-tool round: call `provider.chat()`, execute tool calls in parallel via `join_all` with per-tool timeout, detect fabricated responses (LLM faking tool results in text), and track duplicate tool calls via `HashSet<String>`.

### Configuration

```json
{
  "orchestrator": {
    "maxEscalations": 3,
    "heuristicConfidenceThreshold": 0.9,
    "llmClassifierTimeout": 5000
  }
}
```

### Task Complexity Bridge

`feature-todo` crate provides `TaskComplexitySignals` to evaluate whether a task warrants plan-based execution. The `execute` action on `TodoTool` checks complexity (dependencies, subtasks, duration, priority) and either starts the task directly or suggests creating a plan.

## Planning Engine

Plan types live in `domain` crate (`crates/domain/src/plan.rs`). Execution logic in `agent` crate.

**Lifecycle** (enforced by `PlanStatus::validate_transition`): `Draft → Approved → Executing → Completed|Failed`. Any state → `Abandoned`. Terminal states are final.

**Visibility** (`PlanVisibility`): `transparent` (default, always shown), `on_failure` (shown only on failure, auto-cleaned after 7 days), `silent` (never shown, auto-cleaned after 24h). `PlanCleanupService` runs hourly.

**Execution flow**: `PlanTool` -> `PlanHandler::execute_plan()` -> `AgentLoop::run_plan_execution()` -> per-step `plan_executor::run_step()` (up to 5 LLM-tool cycles per step). On step failure: up to 3 retries, then backtracking via `plan_executor::regenerate_from()`. Max 3 backtrack events before plan fails. `PlannedEngine` synthesizes a human-readable summary from step outputs after completion.

**Key files**: `agent/plan_executor.rs`, `agent/plan_handler.rs`, `agent/plan_step_generator.rs`, `domain/src/plan.rs`.
