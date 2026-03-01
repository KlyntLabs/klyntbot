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

## Architecture

Klyntbot is a Rust personal AI agent — a single binary that connects to 6+ chat platforms, calls LLMs, manages tasks/projects, syncs with Apple Calendar, and manages persistent memory. It is **not** a code execution platform — users have dedicated tools (Claude Code, Cursor, Codex) for that. All persistent state is stored in SQLite (relational data) + LanceDB (vector embeddings).

### Workspace layout (19 crates in 9 dependency layers)

```
Layer 0: common            — Error types (KlyntbotError with 12 variants), MessageRole, ChannelName, ChatId, SessionKey
Layer 1: config, bus       — Config schema (camelCase JSON serde), async message bus (tokio::mpsc)
Layer 1.5: storage         — SQLite pool (sqlx + SqlitePool), auto-migrations, repository pattern (*Repo structs)
Layer 2: providers, session, scheduling, calendar, context_engine, domain
                           — LLM HTTP client, session persistence, cron service, CalDAV sync, token budget allocator, plan/goal domain types
Layer 2.5: tools-core, tools-core-macros
                           — Tool trait, FeaturePackage trait, ToolRegistry, derive macros (#[derive(Tool)], #[derive(ToolParams)])
Layer 3: tools             — ~15 tool implementations (filesystem ×4, web ×2, message, spawn, cron, calendar, plan, project, goal, memory, learning, browser, ask_user, agent_task)
Layer 3.5: feature-todo, feature-finance
                           — Self-contained feature packages (own tools, migrations, config, handler traits)
Layer 4: channels          — Chat platform integrations (Telegram, Discord, WhatsApp, Slack, Email, QQ)
Layer 4.5: plugin-runtime  — WASM plugin sandbox (loads .wasm from ~/.klyntbot/plugins/)
Layer 5: agent             — Agent loop, context builder, memory store, skill manager, subagent manager, intent pipeline, execution core
Layer 6: cli               — Clap-derived CLI with 4 commands: serve, init, status, plugin
Layer 7: klyntbot          — Re-export facade (src/lib.rs) + binary entry point (src/main.rs)
```

Dependencies flow strictly upward. No circular dependencies — enforced by Cargo.

**Storage stack (SQLite + LanceDB):**
- `storage` crate at Layer 1.5: `StoragePool` wraps `SqlitePool`, auto-runs migrations, exposes repository pattern (`*Repo` structs)
- All relational data in SQLite (`{data_dir}/data.db`): todos, projects, sessions, goals, plans, cron jobs, usage, strategies, outcomes
- Vector embeddings in LanceDB (`{data_dir}/lancedb/`): todo embeddings, conversation embeddings — replaces pgvector
- `Repos` aggregate struct for convenient access: `Repos::from_pool(&pool)`
- `StoragePool::connect(data_dir)` creates/opens `data.db`, enables WAL + foreign keys, runs migrations
- `StoragePool::connect_in_memory()` for tests — runs migrations on an in-memory SQLite pool
- Data directory defaults to `~/.klyntbot`, configurable via `data_dir` in config

### Key patterns

- **Repository pattern**: All persistent state goes through `*Repo` structs in the `storage` crate. Repos hold a `SqlitePool` (which is `Clone + Send + Sync` internally via `Arc`), eliminating the need for `Arc<RwLock<Store>>` wrappers. The `Repos` aggregate provides convenient access: `Repos::from_pool(&pool)`.
- **Derive-based tools**: Tools are defined via `#[derive(tools_core::Tool)]` and `#[derive(ToolParams)]` macros from `tools-core-macros`. These generate `Tool` trait impls, parameter extraction, and JSON schema. New tools should use this pattern — see `crates/tools/src/filesystem.rs` for examples.
- **Feature packages**: Self-contained features (`feature-todo`, `feature-finance`) implement the `FeaturePackage` trait (in `tools-core`), which bundles tools, migrations, config validation, and health checks. Registered at agent startup. Add new features by creating a `feature-*` crate implementing `FeaturePackage`.
- **Dependency inversion**: Handler traits (`SpawnHandler`, `CronHandler` in `tools`; `CalendarHandler` in `tools`; `EnrichmentHandler`, `EmbeddingHandler` in `feature-todo`; `FinanceHandler` in `feature-finance`) are defined in lower layers but implemented in `agent` (Layer 5). Injected as `Arc<dyn Trait>` at construction.
- **Re-export facade**: `src/lib.rs` re-exports all public types from workspace crates. Integration tests and external consumers use `klyntbot::AgentLoop`, `klyntbot::Config`, `klyntbot::StoragePool`, etc.
- **Provider auto-detection**: The provider registry matches model name keywords to route to the correct LLM provider. No external routing library.
- **Config schema**: All config structs use `#[serde(rename_all = "camelCase")]`. Config file is `~/.klyntbot/config.json`. API keys are wrapped in `Secret<String>` (redacted in Debug/Display, access via `.expose()`).
- **Feature-gated email**: The `email` feature (on by default) gates IMAP/SMTP dependencies in the `channels` crate.

### Extension traits

| Trait | Defined in | Purpose |
|-------|-----------|---------|
| `Tool`, `ToolExecute`, `ToolParams` | `tools-core` | Core tool framework — usually derived via `#[derive(Tool)]` / `#[derive(ToolParams)]` |
| `FeaturePackage` | `tools-core` | Self-contained feature registration (tools, migrations, config, health) |
| `LlmProvider` | `providers` | `async fn chat()`, `async fn chat_stream()`, `fn name()`, `fn default_model()`, etc. |
| `Channel` | `channels` | `async fn start()`, `async fn stop()`, `async fn send()`, `fn name()`, `fn is_allowed()` |
| `SpawnHandler` | `tools` | Dependency inversion for subagent spawning |
| `CronHandler` | `tools` | Dependency inversion for cron job management |
| `CalendarHandler` | `tools` | Dependency inversion for calendar sync |
| `EnrichmentHandler` | `feature-todo` | Dependency inversion for AI-powered task enrichment |
| `EmbeddingHandler` | `feature-todo` | Dependency inversion for todo embedding generation |
| `FinanceHandler` | `feature-finance` | Dependency inversion for finance price lookups |

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

**Enrichment** (`feature-todo` crate): Auto-infers priority, duration, and due dates from task title keywords. Config: `todo.enrichment.enabled` (default: `true`), `todo.enrichment.autoApplyThreshold` (default: `0.70`). Implementation in `crates/feature-todo/src/enrichment.rs`.

**Semantic search** (`feature-todo` crate): Uses fastembed (paraphrase-multilingual-MiniLM-L12-v2, 384d) + LanceDB for ANN similarity search. Three modes: `search` (keyword SQL), `search-semantic` (cosine similarity), `search-hybrid` (RRF merge). Config: `todo.search.enabled`, `todo.search.semanticThreshold` (default: `0.5`). Embeddings stored in `{data_dir}/lancedb/`.

## Feature Packs

Feature packs bundle config + skills into selectable groups chosen during `klyntbot init`.

- **Core** (always): task-management. **Recommended**: productivity, ai-intelligence. **Optional**: finance, weather, skill-creator.
- Config: `packs.enabled` and `packs.enabledSkills` arrays in config.json.
- Registry: `crates/cli/src/wizard/packs/registry.rs`. Config mutations: `pack_selection.rs`.
- At startup, `SkillManager::filter_by_skills()` restricts built-in skills to enabled packs. Workspace skills (`~/.klyntbot/skills/`) are always kept.

## Skills

Built-in skills live in `skills/` as `SKILL.md` files (browser, cron, daily-planning, finance, skill-creator, summarize, todo, weather, weekly-report). The `SkillManager` in the agent crate discovers and loads them at runtime. Skills are filtered by enabled packs — only skills from selected packs are available.

## Gotchas & Common Pitfalls

- **No external DB required**: SQLite file is created automatically at `{data_dir}/data.db` on first run. LanceDB directory is created at `{data_dir}/lancedb/` when semantic search is first used.
- **Data directory**: Defaults to `~/.klyntbot`. Override with `data_dir` in `~/.klyntbot/config.json`.
- **`StoragePool::from_existing()` skips migrations**: Only use for pools already migrated by `StoragePool::connect()`. For in-memory test pools, always use `StoragePool::connect_in_memory()`.
- **CalDAV sync is async**: Calendar sync runs in background. Use `CalendarTool::sync_now()` for immediate sync, or wait for next scheduled interval.
- **Config changes require restart**: Modifying `~/.klyntbot/config.json` requires restarting `klyntbot serve` for changes to take effect.
- **Dependency inversion gotcha**: When adding new tools that need agent context (spawn/cron handlers), inject via `Arc<dyn Trait>` at construction to avoid circular deps.
- **SqlitePool is Clone+Send+Sync**: Unlike the old `Arc<RwLock<Store>>` pattern, `SqlitePool` (and therefore all `*Repo` structs) can be freely cloned and shared across tasks without locking. Connection pooling is handled internally by sqlx.

## Intent Pipeline (Phase 5 — v0.4.0)

### Architecture

The IntentPipeline replaces the former Orchestrator + EngineDispatch + AgentPipeline with a unified flow:

```
IntentAnalyzer → ContextEngine → ExecutionRouter → ResponseValidator → CostTracker
```

**Modules** in `crates/agent/src/intent_pipeline/`:

| Module | Purpose |
|--------|---------|
| `types.rs` | `ExecutionMode` (Direct/Reactive/Planned), `ComplexitySignals`, `IntentAnalysis` |
| `analysis.rs` | `IntentAnalyzer` (two-stage: heuristic keywords → LLM classifier), `IntentClassifier` |
| `engines/` | `ExecutionEngine` trait + `DirectEngine`, `ReactiveEngine`, `PlannedEngine` |
| `router.rs` | Maps mode to engine, handles escalation chain (Direct → Reactive → Planned) |
| `pipeline.rs` | `IntentPipeline` struct — wires everything into `process_message()` |
| `visibility.rs` | Background cleanup service for stale silent/on_failure plans |

### Execution Modes

- **Direct**: Single LLM call, no tools. For greetings, simple questions, acknowledgments.
- **Reactive**: ReAct loop with tool calls. For task CRUD, search, calendar ops.
- **Planned**: Multi-step plan generation and execution. For complex multi-tool workflows.

### Escalation Chain

When an engine signals it cannot handle a request (`EngineResult::Escalate`), the router automatically escalates: Direct → Reactive → Planned. Max escalations are configurable via `config.orchestrator.max_escalations` (default: 3).

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

**Execution flow**: `PlanTool` → `PlanHandler::execute_plan()` → `AgentLoop::run_plan_execution()` → per-step `PlanExecutor::execute_step()`. Each step: build context window (current + next 3), LLM call → tool execution → result capture. On step failure: up to 3 retries, then backtracking via `regenerate_from()`. Max 3 backtrack events before plan fails.

**Key files**: `agent/plan_executor.rs`, `agent/plan_handler.rs`, `agent/plan_step_generator.rs`, `domain/src/plan.rs`.
