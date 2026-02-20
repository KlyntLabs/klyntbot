# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build --workspace              # Build all crates
cargo build --release                # Optimized release build (LTO, stripped)
cargo nextest run --workspace        # Run all tests (parallel, faster than cargo test)
cargo nextest run -p agent           # Test a single crate
cargo nextest run -p storage         # Test storage crate (requires PostgreSQL)
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

**Database requirement**: Integration tests require a running PostgreSQL instance with pgvector. Set `DATABASE_URL` or tests will use `postgres://localhost/klyntbot_test`.

## Architecture

Klyntbot is a Rust AI agent framework — a single binary that connects to 6+ chat platforms, calls LLMs, executes tools, manages tasks/projects, syncs with Apple Calendar, and manages persistent memory. All persistent state is stored in PostgreSQL (with pgvector for embeddings).

### Workspace layout (15 crates in 9 dependency layers)

```
Layer 0: common          — Error types (KlyntbotError with 12 variants including CalendarError, Storage), MessageRole, ChannelName, ChatId, SessionKey
Layer 1: config, bus     — Config schema (camelCase JSON serde + CalendarConfig, ProjectConfig), async message bus (tokio::mpsc)
Layer 1.5: storage       — PostgreSQL connection pool (sqlx), auto-migrations, row structs, repository pattern (TodoRepo, ProjectRepo, EmbeddingRepo, etc.)
Layer 2: providers, session, scheduling, calendar, context_engine — LLM HTTP client, session persistence, cron service, CalDAV client + sync engine, token budget allocator + context assembler
Layer 3: tools           — Tool trait + 12 implementations (file I/O ×4, shell, web ×2, message, spawn, cron, todo, project, calendar)
Layer 4: channels, heartbeat — Chat platform integrations (Telegram, Discord, WhatsApp, Slack, Email, QQ)
Layer 5: agent           — Agent loop, context builder, memory store, skill manager, subagent manager, calendar handler adapter, reminder engine, orchestrator, execution engines, pipeline
Layer 6: cli             — Clap-derived CLI with 4 commands: chat, serve, init, status
Layer 7: klyntbot        — Re-export facade (src/lib.rs) + binary entry point (src/main.rs)
```

Dependencies flow strictly upward. No circular dependencies — enforced by Cargo.

**New in v0.4.0 (PostgreSQL Storage Migration):**
- `storage` crate at Layer 1.5: connection pooling (`PgPool`), auto-running migrations, row structs (`*Row`), repository pattern (`*Repo`)
- All persistent data migrated from JSONL flat files to PostgreSQL: todos, projects, embeddings, sessions, goals, plans, cron jobs, usage, strategies, outcomes
- pgvector for embedding similarity search (ANN) — replaces brute-force in-memory cosine similarity
- `Repos` aggregate struct for convenient access to all repositories
- `StoragePool::connect()` with automatic migration execution
- CLI slimmed from 40+ subcommands to 4 (chat, serve, init, status) — all task/project management now via chat
- `StorageError` variant added to `KlyntbotError` with automatic conversion

### Key patterns

- **Repository pattern**: All persistent state goes through `*Repo` structs in the `storage` crate. Repos hold a `PgPool` (which is `Clone + Send + Sync` internally via `Arc`), eliminating the need for `Arc<RwLock<Store>>` wrappers. The `Repos` aggregate provides convenient access: `Repos::from_pool(&pool)`.
- **Dependency inversion**: `SpawnHandler` and `CronHandler` traits are defined in `tools` (Layer 3) but implemented in `agent` (Layer 5). Injected as `Arc<dyn Trait>` at construction. This breaks what would otherwise be circular deps between tools and agent.
- **Re-export facade**: `src/lib.rs` re-exports all public types from workspace crates including `storage`. Integration tests and external consumers use `klyntbot::AgentLoop`, `klyntbot::Config`, `klyntbot::StoragePool`, etc.
- **Provider auto-detection**: The provider registry matches model name keywords to route to the correct LLM provider. No external routing library.
- **Config schema**: All config structs use `#[serde(rename_all = "camelCase")]`. Config file is `~/.klyntbot/config.json`. API keys are wrapped in `Secret<String>` (redacted in Debug/Display, access via `.expose()`).
- **Feature-gated email**: The `email` feature (on by default) gates IMAP/SMTP dependencies in the `channels` crate.

### Extension traits

| Trait | Defined in | Purpose |
|-------|-----------|---------|
| `Tool` | `tools` | `fn name()`, `fn description()`, `fn parameters() -> Value`, `async fn execute()` |
| `LlmProvider` | `providers` | `async fn chat()`, `async fn chat_stream()`, `fn name()`, `fn default_model()`, `fn capabilities()`, `fn context_window()`, `async fn count_tokens()` |
| `Channel` | `channels` | `async fn start()`, `async fn stop()`, `async fn send()`, `fn name()`, `fn is_allowed()` |
| `SpawnHandler` | `tools` | Dependency inversion for subagent spawning |
| `CronHandler` | `tools` | Dependency inversion for cron job management |
| `CalendarHandler` | `tools` | Dependency inversion for calendar sync (NEW: `async fn sync_now()`, `async fn get_status()`, `async fn list_events()`, etc.) |
| `EnrichmentHandler` | `tools` | Dependency inversion for AI-powered task enrichment (NEW: `async fn enrich_task()`) |
| `AgentPipeline` | `agent` | Full orchestration pipeline: Orchestrator → ContextEngine → EngineDispatch → ResponseValidator → CostTracker (`async fn process_message()`) |

### Conventions

- Error handling: Use `common::Result<T>` (alias for `Result<T, KlyntbotError>`). Domain errors auto-convert via `From` impls.
- Imports: Use crate names directly (`use common::Result`, `use config::Config`), not `use crate::` for cross-crate refs.
- Tests: Unit tests as `#[cfg(test)] mod tests` inline in each crate. Integration tests in `tests/` use the facade crate. Shared mock provider in `tests/mock_provider.rs`.
- Commits: Conventional format — `feat(providers): add streaming`, `fix(channels): handle rate limits`.
- Zero clippy warnings policy.

## CLI Subcommands

The CLI has 4 commands. All task management, project management, and other operations are handled through chat (natural language via tools).

```bash
klyntbot chat "message"          # One-shot chat (or omit message for REPL)
klyntbot chat --session my-sess  # Resume a named session
klyntbot serve --port 8080       # Start gateway daemon (channels, cron, heartbeat)
klyntbot init                    # 2-phase setup wizard (core setup + pack selection)
klyntbot init --packs            # Jump directly to pack selection
klyntbot init --reset            # Reset config to defaults before running wizard
klyntbot status [--verbose]      # Show agent/config status
```

Task management, project management, calendar sync, cron jobs, skills, and all other features are accessible through natural language in `klyntbot chat` or via channel integrations.

## Environment Variables

Config can be overridden via `KLYNTBOT_` prefix with `__` as nesting separator:

```bash
KLYNTBOT_DATABASE_URL=postgres://user:pass@localhost/klyntbot
KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o
KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-...
KLYNTBOT_TOOLS__RESTRICT_TO_WORKSPACE=true
```

PostgreSQL with pgvector is required. The database URL defaults to `postgres://localhost/klyntbot` if not set.

## Enrichment Configuration

The enrichment engine auto-infers missing task fields using keyword analysis:

**Config schema** (`~/.klyntbot/config.json`):
```json
{
  "todo": {
    "enrichment": {
      "enabled": true,
      "autoApplyThreshold": 0.70
    }
  }
}
```

**Fields:**
- `enabled`: Enable/disable enrichment engine (default: `true`)
- `autoApplyThreshold`: Confidence threshold for auto-applying suggestions (0.0-1.0, default: `0.70`)

**How it works:**
1. **Priority inference**: Detects keywords like "urgent", "critical", "fix", "feature", "cleanup"
   - High priority (1): urgent, critical, blocker, hotfix, emergency, asap
   - Medium-high (2): important, bug, fix, broken, regression
   - Medium (3): feature, enhance, improvement, update, refactor
   - Low (4): nice to have, cleanup, chore, documentation, typo

2. **Duration prediction**: Estimates task duration based on keywords
   - Quick (15 min): typo, rename, tweak, bump, toggle
   - Small (30 min): fix, patch, update, adjust, lint
   - Medium (60 min): feature, implement, add, create, build
   - Large (120 min): refactor, overhaul, rewrite, redesign, architecture

3. **Due date suggestion**: Suggests deadline based on priority and keywords
   - Urgent tasks → today
   - Important tasks → this week
   - Normal tasks → no suggestion (manual assignment)

**Usage via chat:**
Enrichment runs automatically on task creation if enabled. Ask the agent to "enrich task <id>" for manual enrichment. For example, creating a task titled "URGENT: Fix production auth bug" auto-sets priority to 1 and estimates 30 min duration.

**Troubleshooting:**
- **Enrichment not working?** Check `config.todo.enrichment.enabled` is `true`
- **Suggestions not applied?** Lower `autoApplyThreshold` (default 0.70)
- **Wrong suggestions?** Adjust task title keywords or set fields manually

## Task Creation Mode

Controls how klyntbot handles task creation — whether it asks for details first or auto-fills.

**Config schema** (`~/.klyntbot/config.json`):
```json
{
  "todo": {
    "creationMode": "ask-first"
  }
}
```

**Values:**
- `"ask-first"` (default): Uses `ask_user` to gather details before creating vague tasks. The `confirmed` parameter must be set on `todo add` calls with optional fields.
- `"yolo"`: Auto-enriches from conversation context, presents suggestions for approval before applying.
- `"party"`: Interactive brainstorming — asks targeted questions one at a time to build up the task.

**How it works:**
1. User says "create task: buy"
2. In `ask-first` mode: agent calls `ask_user` to clarify title, priority, due date
3. User responds with details
4. Agent calls `todo add` with gathered details + `confirmed: true`
5. Enrichment engine adds any remaining fields (estimated time, etc.)

## Semantic Search

Semantic search uses local embeddings (fastembed, paraphrase-multilingual-MiniLM-L12-v2, 384 dimensions) for meaning-based task retrieval. Finds related concepts, not just keywords (e.g., "login bug" finds "authentication issue").

**Setup** (`klyntbot init` Step 6):
- Option 1: Enable & download model now (~420MB, ~60 sec with progress bar)
- Option 2: Disable semantic search (saves disk space)

**Config schema** (`~/.klyntbot/config.json`):
```json
{
  "todo": {
    "search": {
      "enabled": true,
      "semanticThreshold": 0.5,
      "embeddingModel": "paraphrase-multilingual-MiniLM-L12-v2",
      "rrfK": 60
    }
  }
}
```

**Fields:**
- `enabled`: Enable/disable semantic search (default: `true`)
- `semanticThreshold`: Minimum cosine similarity for results (0.0-1.0, default: `0.5`)
- `embeddingModel`: Model name stored in embedding records (default: `paraphrase-multilingual-MiniLM-L12-v2`)
- `rrfK`: Reciprocal Rank Fusion k parameter for hybrid search (default: `60`)

**Search modes** (available via TodoTool in chat):
- `search` — keyword matching (title + description substring via SQL)
- `search-semantic` — pgvector ANN cosine similarity on embeddings
- `search-hybrid` — merges keyword + semantic via Reciprocal Rank Fusion (RRF)

**How it works:**
1. Embeddings auto-generate when tasks are created or updated (best-effort, non-blocking)
2. Embeddings persist in PostgreSQL `todo_embeddings` table (pgvector `vector(384)` column)
3. Semantic search uses pgvector approximate nearest neighbor (ANN) for fast cosine similarity
4. Hybrid search runs both keyword and semantic, merges via RRF

**Troubleshooting:**
- **First search is slow?** Model downloads ~420MB on first use. Subsequent searches are fast.
- **No results?** Lower the threshold (e.g., 0.3) or use hybrid search for broader matching.
- **pgvector not installed?** Run `CREATE EXTENSION vector;` in your PostgreSQL instance.
- **Semantic search unavailable?** Falls back with a clear error suggesting keyword search.

## Feature Packs

Feature packs bundle related config settings and skills into selectable groups. Users choose packs during `klyntbot init` (Phase 2: Pack Selection).

**Pack tiers:**
- **Core** (always enabled): task-management
- **Recommended** (pre-checked): productivity, ai-intelligence, developer-tools
- **Optional** (unchecked by default): finance, weather, skill-creator

**Config schema** (`~/.klyntbot/config.json`):
```json
{
  "packs": {
    "enabled": ["task-management", "productivity", "ai-intelligence", "developer-tools"],
    "enabledSkills": ["todo", "todo-party", "todo-yolo", "daily-planning", "cron", "summarize", "github", "tmux"]
  }
}
```

**How it works:**
1. `PackRegistry` in `crates/cli/src/wizard/packs/registry.rs` defines 7 packs with tier, skills, and descriptions
2. `pack_selection::apply_pack_config()` maps pack IDs to config section mutations (e.g., ai-intelligence enables `conversation.embedding`, `learning`)
3. `config.packs.enabled_skills` is computed by the wizard and saved to config
4. At agent startup, `SkillManager::filter_by_skills()` restricts built-in skills to those from enabled packs
5. Workspace-loaded skills (from `~/.klyntbot/skills/`) are always kept regardless of pack selection

**Adding a new pack:** Add a `Pack` entry to `PACKS` in `registry.rs`, then add config mutations to `apply_pack_config()` in `pack_selection.rs`.

## Skills

Built-in skills live in `skills/` as `SKILL.md` files (summarize, skill-creator, github, tmux, weather, cron). The `SkillManager` in the agent crate discovers and loads them at runtime. Skills are filtered by enabled packs — only skills from selected packs are available.

## Gotchas & Common Pitfalls

- **PostgreSQL required**: A running PostgreSQL instance with the `pgvector` extension is required. Migrations run automatically on first connect via `StoragePool::connect()`.
- **Database URL**: Defaults to `postgres://localhost/klyntbot`. Set `database_url` in config or `KLYNTBOT_DATABASE_URL` env var.
- **CalDAV sync is async**: Calendar sync runs in background. Use `CalendarTool::sync_now()` for immediate sync, or wait for next scheduled interval.
- **Config changes require restart**: Modifying `~/.klyntbot/config.json` requires restarting `klyntbot serve` for changes to take effect.
- **Dependency inversion gotcha**: When adding new tools that need agent context (spawn/cron handlers), inject via `Arc<dyn Trait>` at construction to avoid circular deps.
- **PgPool is Clone+Send+Sync**: Unlike the old `Arc<RwLock<Store>>` pattern, `PgPool` (and therefore all `*Repo` structs) can be freely cloned and shared across tasks without locking. Connection pooling is handled internally by sqlx.

## Planning Engine (Phase 4 — v0.3.0)

### Plan Lifecycle

Plans follow this state machine (enforced by `PlanStatus::validate_transition`):

```
Draft → Approved → Executing → Completed
                ↘               ↘
             Abandoned         Failed
```

Any state can transition to `Abandoned`. From `Completed`, `Failed`, or `Abandoned` — no further transitions are allowed.

### Creating and Executing Plans

Plans are managed through chat via the `PlanTool`. Ask the agent to create, approve, execute, or check plan status in natural language.

### How Plan Execution Works

1. `PlanTool` action `"execute"` calls `PlanHandler::execute_plan()` → transitions to `Executing`
2. `AgentLoop::run_plan_execution()` in `agent/agent_loop.rs` drives the step-by-step loop
3. For each step: `PlanExecutor::execute_step()` in `agent/plan_executor.rs`:
   - Builds a system prompt from the plan context window (current + next 3 steps)
   - Calls the LLM provider to generate tool calls
   - Executes tool calls via `ToolRegistry` (Arc<dyn Tool> cloned before lock release)
   - Falls back to LLM text response when no tool calls are generated
4. Step state (status, timestamps, results, attempt_count) is updated by `run_plan_execution()` after each `execute_step()` call
5. On step failure: up to `MAX_BACKTRACK_ATTEMPTS` (3) backtracking events via `PlanExecutor::regenerate_from()`
6. After all steps complete: plan transitions to `Completed` with `completed_at` timestamp

### Backtracking

When a step exceeds `max_attempts` (default: 3 retries per step):
1. A `BacktrackEntry` is recorded in `plan.backtrack_history`
2. `regenerate_from()` prompts the LLM for replacement steps from the failure point
3. If LLM returns invalid JSON, a single "Retry: <step>" fallback step is inserted
4. After `MAX_BACKTRACK_ATTEMPTS` (3) full backtrack events, the plan is marked `Failed`

### Known Limitations

- **Single-cycle execution**: `execute_step()` makes one LLM call per step. The LLM generates tool calls with arguments from step context (description, previous results, plan goal). A multi-cycle ReAct loop is available via `PlanExecuteEngine`.
- **Iteration limit enforcement**: `iteration_limit` field persists but is checked in `run_plan_execution()` — exceeded limits mark the plan as `Failed`.
- **No real-time progress**: Plan progress is only visible between executions; there's no streaming progress update.
