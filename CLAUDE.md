# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Test Commands

```bash
cargo build --workspace              # Build all crates
cargo build --release                # Optimized release build (LTO, stripped)
cargo nextest run --workspace        # Run all 910+ tests (parallel, faster than cargo test)
cargo nextest run -p agent           # Test a single crate
cargo nextest run -p calendar        # Test calendar crate specifically
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

## Architecture

Klyntbot is a Rust AI agent framework — a single binary that connects to 6+ chat platforms, calls LLMs, executes tools, manages tasks/projects, syncs with Apple Calendar, and manages persistent memory.

### Workspace layout (13 crates in 8 dependency layers)

```
Layer 0: common        — Error types (KlyntbotError with 11 variants including CalendarError), MessageRole, ChannelName, ChatId, SessionKey
Layer 1: config, bus   — Config schema (camelCase JSON serde + CalendarConfig, ProjectConfig), async message bus (tokio::mpsc)
Layer 2: providers, session, scheduling, calendar, context_engine — LLM HTTP client, JSONL session persistence, cron service, CalDAV client + sync engine, token budget allocator + context assembler
Layer 3: tools         — Tool trait + 12 implementations (file I/O ×4, shell, web ×2, message, spawn, cron, todo, project, calendar)
Layer 4: channels, heartbeat — Chat platform integrations (Telegram, Discord, WhatsApp, Slack, Email, QQ)
Layer 5: agent         — Agent loop, context builder, memory store, skill manager, subagent manager, calendar handler adapter, reminder engine, orchestrator, execution engines, pipeline
Layer 6: cli           — Clap-derived CLI, REPL, command handlers (TodoCommands, ProjectCommands, CalendarCommands, UsageCommands, LearningCommands, ProviderCommands)
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

**New in v0.3.0 (Adaptive Orchestrator — `feat/adaptive-orchestrator` branch):**
- `context_engine` crate at Layer 2: token budget allocator, history compressor, context assembler
- `ProviderManager` with failover, retry, and circuit breaker logic
- `Orchestrator` module: heuristic pre-filter + LLM classifier for intent classification
- Execution engines: `DirectEngine`, `ReactPlusEngine` (with scratchpad + reflection), `PlanExecuteEngine`
- `EngineDispatch`: maps `ExecutionStrategy` → engine with automatic escalation (Direct → ToolAssisted → Autonomous)
- `AgentPipeline`: full pipeline wiring (Orchestrator → ContextEngine → EngineDispatch → ResponseValidator → CostTracker)
- `AgentLoop.process_message_v2()`: opt-in pipeline route alongside legacy `process_message()`
- `ResponseValidator` with safety checks (system prompt leak detection, length truncation, quality)
- `CostTracker` with JSONL usage recording and per-model pricing
- `StrategyLearningStore` for recording strategy effectiveness
- Multi-axis learning system: per-tool confidence, strategy tracking, behavioral signals, user profiles

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

```bash
# Chat & Agent
klyntbot chat "message"          # One-shot chat (or omit message for REPL)
klyntbot chat --session my-sess  # Resume a named session
klyntbot serve --port 8080       # Start HTTP server (default port from config)
klyntbot init                    # Interactive setup wizard (8 steps: provider, channels, tools, workspace, calendar, semantic search, daemon, review)
klyntbot status [--verbose]      # Show agent/config status

# Task Management (25 commands)
klyntbot todo add <title> [--description TEXT] [--priority N] [--due DATE] [--tags CSV]
klyntbot todo list [--status doing|todo|done] [--tag TAG] [--priority-min N] [--limit N]
klyntbot todo show ID
klyntbot todo update ID [--title TEXT] [--description TEXT] [--due DATE] [--priority N] [--status STATUS] [--tags CSV]
klyntbot todo complete ID
klyntbot todo delete ID
klyntbot todo focus [ID]         # Start focus session (auto-tracks time)
klyntbot todo unfocus ID
klyntbot todo summary
klyntbot todo tree [--project ID] [--depth N]
klyntbot todo search QUERY [--include-attachments]
klyntbot todo search-semantic QUERY [--limit N] [--threshold 0.0-1.0]
klyntbot todo search-hybrid QUERY [--limit N]
klyntbot todo add-subtask PARENT_ID <title> [--description TEXT] [--priority N] [--due DATE] [--tags CSV]
klyntbot todo move ID [--parent ID|none] [--project ID|none]
klyntbot todo attach ID --file PATH | --url URL | --note TEXT [--title TEXT]
klyntbot todo detach ID ATTACHMENT_ID
klyntbot todo log-time ID MINUTES [--note TEXT]
klyntbot todo report [--period week|month] [--project ID]
klyntbot todo depend ID [--on BLOCKER_ID] [--remove BLOCKER_ID]
klyntbot todo recur add <title> --rule RRULE [--priority N] [--tags CSV] [--project ID]
klyntbot todo recur list [--project ID]
klyntbot todo recur delete TEMPLATE_ID
klyntbot todo enrich ID          # AI-powered task enrichment (priority, duration, scheduling)

# Project Management (6 commands)
klyntbot project create <name> [--description TEXT] [--color COLOR] [--tags CSV]
klyntbot project list [--status active|paused|completed|archived] [--tag TAG] [--limit N]
klyntbot project show ID
klyntbot project update ID [--name TEXT] [--description TEXT] [--color COLOR] [--status STATUS] [--tags CSV]
klyntbot project archive ID
klyntbot project tasks ID [--tree] [--limit N]

# Monitoring & Observability (v0.3.0)
klyntbot usage report [--days 30]    # LLM cost/token report (by model, by day)
klyntbot learning status [--days 7]  # Strategy effectiveness, outcome history, tool confidence
klyntbot provider status             # Provider config, routing, circuit breaker state

# Other Commands
klyntbot channels list|login|test
klyntbot cron list|add|remove|run|enable|disable
klyntbot config show|get|set|edit|validate|reset
klyntbot skills list|info|path
```

## Environment Variables

Config can be overridden via `KLYNTBOT_` prefix with `__` as nesting separator:

```bash
KLYNTBOT_AGENTS__DEFAULTS__MODEL=gpt-4o
KLYNTBOT_PROVIDERS__ANTHROPIC__API_KEY=sk-...
KLYNTBOT_TOOLS__RESTRICT_TO_WORKSPACE=true
```

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

**CLI usage:**
```bash
# Manual enrichment (shows suggestions without applying)
klyntbot todo enrich <task-id>

# Enrichment runs automatically on task creation if enabled
klyntbot todo add "URGENT: Fix production auth bug"
# → Priority auto-set to 1, duration estimated at 30 mins
```

**Troubleshooting:**
- **Enrichment not working?** Check `config.todo.enrichment.enabled` is `true`
- **Suggestions not applied?** Lower `autoApplyThreshold` or manually approve via `todo enrich <id>`
- **Wrong suggestions?** Adjust task title keywords or set fields manually

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

**Search modes:**
- `search` — keyword matching (title + description substring)
- `search-semantic` — cosine similarity on embeddings only
- `search-hybrid` — merges keyword + semantic via Reciprocal Rank Fusion (RRF)

**CLI usage:**
```bash
klyntbot todo search-semantic "fix login issues" --threshold 0.6 --limit 5
klyntbot todo search-hybrid "authentication bug"
```

**How it works:**
1. Embeddings auto-generate when tasks are created or updated (best-effort, non-blocking)
2. Embeddings persist in `~/.klyntbot/data/embeddings.jsonl` (append-only journal, auto-compacts)
3. Semantic search embeds the query, then computes cosine similarity against all stored embeddings
4. Hybrid search runs both keyword and semantic, merges via RRF

**Troubleshooting:**
- **First search is slow?** Model downloads ~420MB on first use. Subsequent searches are fast.
- **No results?** Lower the threshold (e.g., `--threshold 0.3`) or use `search-hybrid` for broader matching.
- **Missing embeddings?** New tasks auto-embed. For existing tasks, run `todo backfill-embeddings`.
- **Semantic search unavailable?** Falls back with a clear error suggesting keyword search.

## Skills

Built-in skills live in `skills/` as `SKILL.md` files (summarize, skill-creator, github, tmux, weather, cron). The `SkillManager` in the agent crate discovers and loads them at runtime.

## Gotchas & Common Pitfalls

- **CalDAV sync is async**: Calendar sync runs in background. Use `CalendarTool::sync_now()` for immediate sync, or wait for next scheduled interval.
- **JSONL corruption recovery**: If `todos.jsonl` or `projects.jsonl` gets corrupted, backup files are in `~/.klyntbot/data/*.jsonl.bak`. Restore manually if needed.
- **Config changes require restart**: Modifying `~/.klyntbot/config.json` requires restarting `klyntbot serve` for changes to take effect.
- **Dependency inversion gotcha**: When adding new tools that need agent context (spawn/cron handlers), inject via `Arc<dyn Trait>` at construction to avoid circular deps.
- **Calendar conflicts are informational**: Detected conflicts are logged to `calendar_conflicts.jsonl` but don't block sync. Review and resolve manually.
- **Race condition warning**: Concurrent access (CLI + `serve` running simultaneously) can corrupt JSONL data files (`todos.jsonl`, `projects.jsonl`, `plans.jsonl`). Avoid running CLI commands while `klyntbot serve` is active, or ensure only one process accesses data files at a time. Future enhancement: file locking.

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

```bash
# Create a plan (starts in Draft)
klyntbot plan create --title "My Plan" --description "What to accomplish"

# Approve the plan (Draft → Approved)
klyntbot plan approve <plan-id>

# Execute the plan (Approved → Executing → Completed/Failed)
klyntbot plan execute <plan-id>

# Check status
klyntbot plan status
klyntbot plan show <plan-id>
```

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

- **No LLM parameter generation**: `execute_step()` passes `{}` as tool arguments. Tools must work without explicit parameters, or parameter generation needs Phase 5 enhancement.
- **Iteration limit enforcement**: `iteration_limit` field persists but is checked in `run_plan_execution()` — exceeded limits mark the plan as `Failed`.
- **No real-time progress**: Plan progress is only visible via `klyntbot plan show <id>` between executions; there's no streaming progress update.
- **JSONL race conditions**: See race condition warning above — applies equally to `plans.jsonl`.
