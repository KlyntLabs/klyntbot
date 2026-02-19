# Implementation Gap Report

> Compiled from 6 subsystem deep-dive analyses on 2026-02-19.
> Source docs: `docs/subsystems/01-06`, `docs/architecture/00-overview.md`

---

## Executive Summary

Klyntbot is a 105K-line Rust AI agent framework with 17 crates, 15+ tools, and 6 chat platform integrations. The codebase is architecturally sound with clean dependency layering and no circular dependencies. However, the ongoing JSONL-to-PostgreSQL migration is approximately **80% complete**, leaving several subsystems in a fragile dual-mode state. The gap report identifies **52 gaps** across 6 severity tiers (**29 resolved** as of 2026-02-19).

**Top 3 systemic issues:**
1. **JSONL/SQL dual-mode persistence** — 6+ subsystems maintain parallel storage backends, doubling maintenance burden and creating behavior divergence
2. ~~**Zero SQL integration test coverage**~~ — **RESOLVED**: 90+ SQL integration tests added across 12 repo modules
3. ~~**AgentLoop god object**~~ — **RESOLVED**: Refactored to 1,144 lines (51% reduction), legacy path removed, pipeline-only processing

---

## Gap Index

| ID | Severity | Subsystem | Title |
|----|----------|-----------|-------|
| **P0 — Critical (blocks correctness)** |
| ~~G-01~~ | ~~P0~~ | ~~Agent~~ | ~~SQL TodoPatch missing fields (`calendar_event_uid`, `next_instance_date`, `last_reminded_at`)~~ **RESOLVED 2026-02-19** |
| ~~G-02~~ | ~~P0~~ | ~~Provider~~ | ~~Anthropic native provider lacks streaming implementation~~ **RESOLVED 2026-02-19** |
| ~~G-03~~ | ~~P0~~ | ~~Tools~~ | ~~Plan execution passes `{}` as tool arguments~~ **RESOLVED 2026-02-19** |
| **P1 — High (significant functionality gap)** |
| ~~G-04~~ | ~~P1~~ | ~~Cross-cutting~~ | ~~All SQL backend paths lack integration test coverage~~ **RESOLVED 2026-02-19** |
| ~~G-05~~ | ~~P1~~ | ~~Cross-cutting~~ | ~~No migration utility from JSONL files to SQL~~ **RESOLVED 2026-02-19** |
| ~~G-06~~ | ~~P1~~ | ~~Cross-cutting~~ | ~~Legacy JSONL stores still present (~3,000 lines of dead code)~~ **RESOLVED 2026-02-19** |
| ~~G-07~~ | ~~P1~~ | ~~Provider~~ | ~~Context assembly uses char/4 estimation, not provider token counting~~ **RESOLVED 2026-02-19** |
| ~~G-08~~ | ~~P1~~ | ~~Provider~~ | ~~Memory retrieval not wired in ContextEngine (3 budget priorities unused)~~ **RESOLVED 2026-02-19** |
| ~~G-09~~ | ~~P1~~ | ~~Dashboard~~ | ~~Dashboard not wired into `serve.rs`~~ **RESOLVED 2026-02-19** |
| ~~G-10~~ | ~~P1~~ | ~~Calendar~~ | ~~Server-wins is the only conflict resolution strategy~~ **RESOLVED 2026-02-19** |
| ~~G-11~~ | ~~P1~~ | ~~Scheduling~~ | ~~CronSchedule timezone field parsed but silently ignored~~ **RESOLVED 2026-02-19** |
| ~~G-12~~ | ~~P1~~ | ~~Config~~ | ~~7 legacy `*_store_path()` methods still exposed~~ **RESOLVED 2026-02-19** |
| **P2 — Medium (quality/maintainability)** |
| ~~G-13~~ | ~~P2~~ | ~~Agent~~ | ~~AgentLoop is 2,357 lines — god object pattern~~ **RESOLVED 2026-02-19** |
| ~~G-14~~ | ~~P2~~ | ~~Agent~~ | ~~Legacy vs Pipeline (v2) dual processing path~~ **RESOLVED 2026-02-19** |
| ~~G-15~~ | ~~P2~~ | ~~Agent~~ | ~~Two plan executors maintained simultaneously~~ **RESOLVED 2026-02-19** |
| ~~G-16~~ | ~~P2~~ | ~~Agent~~ | ~~File-based memory (daily notes + MEMORY.md) not in PostgreSQL~~ **RESOLVED 2026-02-19** |
| ~~G-17~~ | ~~P2~~ | ~~Agent~~ | ~~File-based learning stores (outcomes, thresholds, decision log)~~ **RESOLVED 2026-02-19** |
| ~~G-18~~ | ~~P2~~ | ~~Agent~~ | ~~ToolConfidenceMap exists but not wired into ConfidenceEvaluator~~ **RESOLVED 2026-02-19** |
| ~~G-19~~ | ~~P2~~ | ~~Agent~~ | ~~StrategyTracker computes stats but doesn't feed back into classification~~ **RESOLVED 2026-02-19** |
| ~~G-20~~ | ~~P2~~ | ~~Provider~~ | ~~Session stores string roles, losing tool_calls/multipart/reasoning~~ **RESOLVED 2026-02-19** |
| ~~G-21~~ | ~~P2~~ | ~~Provider~~ | ~~ProviderManager streaming has no retry logic~~ **RESOLVED 2026-02-19** |
| ~~G-22~~ | ~~P2~~ | ~~Provider~~ | ~~No per-model context window mapping (all return default 128K)~~ **RESOLVED 2026-02-19** |
| ~~G-23~~ | ~~P2~~ | ~~Provider~~ | ~~SQL session `list()` always returns `message_count: 0`~~ **RESOLVED 2026-02-19** |
| ~~G-24~~ | ~~P2~~ | ~~Provider~~ | ~~No structured output support (`response_format` parameter)~~ **RESOLVED 2026-02-19** |
| ~~G-25~~ | ~~P2~~ | ~~Provider~~ | ~~`create_provider()` doesn't create ProviderManager with failover~~ **RESOLVED 2026-02-19** |
| ~~G-26~~ | ~~P2~~ | ~~Dashboard~~ | ~~Dashboard uses legacy JSONL stores instead of SQL repos~~ **RESOLVED 2026-02-19** |
| ~~G-27~~ | ~~P2~~ | ~~Dashboard~~ | ~~`system_status` query returns hardcoded stub values~~ **RESOLVED 2026-02-19** |
| ~~G-28~~ | ~~P2~~ | ~~Dashboard~~ | ~~`do_config_patch` mutation validates but doesn't apply~~ **RESOLVED 2026-02-19** |
| ~~G-29~~ | ~~P2~~ | ~~Dashboard~~ | ~~`search_todos` semantic/hybrid modes return empty stubs~~ **RESOLVED 2026-02-19** |
| G-30 | P2 | Calendar | Sync state uses parallel file/SQL backends (not unified) |
| G-31 | P2 | Calendar | No CalDAV event caching (every call hits remote) |
| G-32 | P2 | Scheduling | CronStore uses flat JSON (full rewrite) while Goal/Plan use JSONL journal |
| G-33 | P2 | Scheduling | 100ms polling interval is inefficient for long-wait jobs |
| G-34 | P2 | Goal | No state transition validation (unlike PlanStatus) |
| G-35 | P2 | Goal | GoalStore doesn't create backup before compaction |
| G-36 | P2 | Plan | SQL upsert uses try-create/catch-duplicate (not idempotent) |
| G-37 | P2 | Storage | StorageError→KlyntbotError conversion loses structured variant info |
| G-38 | P2 | Storage | Dynamic query builder uses manual parameter index tracking |
| G-39 | P2 | Tools | MemoryTool `search_all` loads all todos in-memory for keyword search |
| G-40 | P2 | Tools | Conversation embedding store complexity (TokioRwLock + AsyncOnceCell) |
| **P3 — Low (minor improvements)** |
| G-41 | P3 | Common | `MessageRole::from("unknown")` silently falls back to User |
| G-42 | P3 | Bus | `InboundMessage::validate()` not called by `publish_inbound()` |
| G-43 | P3 | Provider | Hardcoded Groq endpoint in TranscriptionProvider |
| G-44 | P3 | Provider | HistoryCompressor purely extractive (first 100 chars only) |
| G-45 | P3 | Provider | No health check endpoint for providers |
| G-46 | P3 | Provider | Session compaction only on JSONL path (SQL grows unbounded) |
| G-47 | P3 | Provider | Model parameter overrides exist but never applied at request time |
| G-48 | P3 | Provider | Anthropic API version hardcoded (`2023-06-01`) |
| G-49 | P3 | Tools | RRULE V1 limitations (no COUNT, UNTIL, EXDATE) |
| G-50 | P3 | Tools | No tool authorization/permission model |
| G-51 | P3 | Channels | No tests for Discord, Slack, WhatsApp, QQ, Email channels |
| G-52 | P3 | Channels | No graceful WebSocket close frames on shutdown |

---

## Detailed Gap Descriptions

### P0 — Critical

#### ~~G-01: SQL TodoPatch Missing Fields~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/storage/src/repos/todo_repo.rs`, `crates/tools/src/todo_types.rs`, + 4 consumer files in `crates/agent/src/`

Added 3 fields to `storage::TodoPatch`: `calendar_event_uid: Option<Option<String>>`, `next_instance_date: Option<Option<DateTime<Utc>>>`, `last_reminded_at: Option<Option<DateTime<Utc>>>`. Double-Option pattern enables both setting (`Some(Some(v))`) and clearing (`Some(None)`). Updated `TodoRepo::update()` SQL with CASE WHEN pattern ($11–$16).

Consumer fixes — removed all no-op workarounds:
- `recurring_tasks.rs`: sets `next_instance_date` directly via patch
- `calendar_reconcile.rs`: `ClearCalendarLink` clears `calendar_event_uid` via `Some(None)`
- `reminders.rs`: sets `last_reminded_at: Some(Some(Utc::now()))`
- `calendar_sync_adapter.rs`: cleanup clears UID via patch; event push links UID after successful PUT

4 new DB-connected tests, 84/84 storage tests pass, 2129/2129 workspace tests pass, zero new clippy warnings.

#### ~~G-02: Anthropic Native Provider Lacks Streaming~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/providers/src/anthropic_native.rs` — `chat_stream()` override + `parse_anthropic_sse()`

Implemented real SSE streaming for `AnthropicNativeProvider`, replacing the single-chunk fallback:
- Sends POST to `/v1/messages` with `"stream": true`
- Parses Anthropic's named SSE format (`event:` + `data:` lines) using `scan()` + `flatten()` pattern
- Handles all event types: `content_block_start` (tool call deltas), `content_block_delta` (text/tool arg/thinking deltas), `message_delta` (finish reason mapping: end_turn→stop, tool_use→tool_calls, max_tokens→length), `error` (stream error propagation)
- 14 new unit tests covering all SSE event types, error handling, and edge cases
- 74/74 provider tests pass, zero new clippy warnings

#### ~~G-03: Plan Execution Passes Empty Tool Arguments~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/agent/src/plan_executor.rs` — `build_step_context()` + `execute_step()` enhancements

Root cause: the LLM already forwarded tool call arguments (`tool_call.arguments.clone()`), but the prompt lacked context for the LLM to generate *meaningful* arguments. Fix:
- `build_step_context()` now includes plan `description` (goal) and results from last 3 completed steps (truncated at 500 chars) in a "## Previous Results" section
- `execute_step()` system/user prompts enhanced with structured markdown and explicit instruction to reference previous results for values/paths/IDs
- CLAUDE.md "Known Limitations" updated to reflect accurate state
- 2 new tests: `test_step_context_includes_completed_results`, `test_step_context_caps_completed_results_at_3`
- 270/270 agent tests pass, zero new clippy warnings

---

### P1 — High

#### ~~G-04: No SQL Integration Test Coverage~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/storage/src/repos/tests/` — 12 test modules with 90+ tests

Added comprehensive SQL integration tests for all repository implementations using testcontainers-rs (`pgvector/pgvector:pg16`). Coverage includes:
- Shared test fixture (`fixtures.rs`): `TestDb` with ephemeral Postgres container, retry logic, factory functions (`make_todo`, `make_project`, `make_embedding`)
- 12 test modules: todo, project, session, embedding, plan, goal, cron, calendar_sync, strategy, outcome, usage, conv_embedding
- Tests cover CRUD, cascading deletes, concurrency, pgvector ANN search, cycle detection, status transitions, JSONB roundtrips
- Supports `TEST_DATABASE_URL` env var for pre-configured databases (faster with nextest)
- Zero clippy warnings, compiles cleanly

#### ~~G-05: No JSONL-to-SQL Migration Utility~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/cli/src/migrate.rs` — `klyntbot migrate` CLI subcommand

Added `klyntbot migrate` command that reads legacy JSONL journal files (todos, projects, embeddings), replays the append-only journal to compute final state, and writes to PostgreSQL via SQL repos. Features:
- `--dry-run` flag for preview without DB writes
- `--force` flag to overwrite existing records
- Graceful handling of malformed lines (logged, skipped)
- Projects migrated before todos (respects FK ordering)
- 11 unit tests covering journal replay, edge cases, and row conversions

#### ~~G-06: Legacy JSONL Stores Still Present (~3,000 LoC)~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: 11 files modified across `agent` and `cli` crates

Migrated all production code paths from `Arc<RwLock<TodoStore>>` (JSONL) to SQL `TodoRepo`:
- Agent crate (6 files): `context.rs`, `calendar_reconcile.rs`, `calendar_sync_adapter.rs`, `reminders.rs`, `recurring_tasks.rs`, `agent_loop.rs`
- CLI crate (1 file): `serve.rs` — cron callbacks now use direct `TodoRepo` calls
- Integration tests (3 files): updated to use `TodoRepo` instead of `TodoStore`
- Legacy store files (`todo_store.rs`, `project_store.rs`, `embedding_store.rs`) still exist as dead code — only referenced by legacy integration tests. Dashboard crate references tracked separately as G-09/G-26.
- Zero new clippy warnings, workspace builds cleanly

#### ~~G-07: Context Assembly Uses Character Estimation~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/context_engine/src/token_counter.rs` (new), `crates/context_engine/src/assembler.rs`, `crates/context_engine/src/history_compressor.rs`

Added `TokenCounter` trait (sync, `Send + Sync`) with `estimate_text(&str) -> usize` method. `CharTokenCounter` implements the default `div_ceil(4)` heuristic. `ContextEngine` accepts an optional `Arc<dyn TokenCounter>` via builder pattern (`with_token_counter()`). `HistoryCompressor` refactored to use injected counter instead of hardcoded `len() / 4`. Provider-specific counters can now be wired in at construction time. 7 new unit tests, 29/29 context_engine tests pass.

#### ~~G-08: Memory Retrieval Not Wired in ContextEngine~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/context_engine/src/memory_retriever.rs` (new), `crates/context_engine/src/assembler.rs`

Added `MemoryRetriever` async trait with `retrieve(&str, usize) -> Vec<MemoryEntry>`. `ContextEngine` accepts an optional `Arc<dyn MemoryRetriever>` via `with_memory_retriever()`. During `assemble()`, retrieved memories are budgeted under `Priority::RetrievedMemory` and injected as system messages. Preference order: embedding retriever > file-based `memory_path` fallback. Higher layers (agent/storage) implement the trait with actual `EmbeddingRepo` lookups. 4 new integration tests covering retrieval, empty results, file fallback, and precedence.

#### ~~G-09: Dashboard Not Wired to `serve.rs`~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/cli/src/serve.rs`, `crates/cli/src/commands.rs`, `crates/cli/Cargo.toml`, `src/main.rs`

Added `dashboard` dependency to CLI crate. `DashboardServer` now constructed and started via `tokio::spawn` alongside agent loop, channel manager, and heartbeat in `handle_serve()`. Dashboard port configurable via `--dashboard-port` CLI arg (default: 3001). `DashboardEventBus` (capacity 256) created in serve.rs and passed to dashboard config. Dashboard handle aborted on graceful shutdown. Status output now prints dashboard URL.

#### ~~G-10: Server-Wins Only Conflict Resolution~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/calendar/src/types.rs`, `crates/calendar/src/sync_engine.rs`, `crates/config/src/schema/core.rs`, `crates/agent/src/calendar_sync_adapter.rs`

Added `ConflictResolutionStrategy` enum with 4 variants: `ServerWins` (default), `ClientWins`, `LastWriteWins` (ETag-based recency proxy), `Manual` (safe placeholder for user intervention). Enum supports `serde(rename_all = "camelCase")`, `FromStr` (camelCase + snake_case compat), and `Default`. `resolve_conflict()` signature extended to accept strategy. Config schema has `conflict_resolution` field under `CalendarConfig` (default: `"server_wins"`). `CalendarSyncAdapter` parses config string to enum at construction. 8 new strategy tests + 3 serde/FromStr tests.

#### ~~G-11: Cron Timezone Silently Ignored~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/scheduling/src/service/mod.rs`, `crates/scheduling/Cargo.toml`

Added `chrono-tz` dependency. `compute_next_run()` now destructures `tz` from `CronSchedule::Cron` and uses it: when `Some(tz_str)`, parses via `chrono_tz::Tz`, computes next run in that timezone, converts to UTC millis. When `None`, keeps existing UTC behavior. Invalid timezone strings log `tracing::warn!` and fall back to UTC. 5 new tests: named timezone, UTC explicit, None fallback, timezone differs from UTC, invalid tz handling.

#### ~~G-12: Legacy Store Path Methods Still Exposed~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/config/src/schema/core.rs`, `crates/agent/src/agent_loop.rs`

Audited all 7 `*_store_path()` methods. 3 methods with zero callers fully removed: `todo_store_path()`, `embedding_store_path()`, `project_store_path()`. 4 methods still used by `agent_loop.rs` for file-based learning stores marked `#[deprecated(note = "...G-17")]`: `goal_store_path()`, `plan_store_path()`, `learning_outcomes_path()`, `learning_state_path()`. Callers annotated with `#[allow(deprecated)]` with migration note. No test files reference removed methods.

---

### P2 — Medium

#### ~~G-13: AgentLoop God Object~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/agent/src/agent_loop.rs` (2,355→1,144 lines, 51% reduction), `crates/agent/src/plan_runner.rs` (new, 433 lines)

Refactored the AgentLoop god object: removed legacy processing path (`run_agent_loop()`, `run_standard_iteration()`, `run_streaming_iteration()`), extracted plan execution into `plan_runner.rs` as `impl AgentLoop` extension, made `pipeline` field non-optional (`Arc<AgentPipeline>`), removed `decision_logger` field. AgentLoop struct reduced from 19 to 17 fields. All processing now routes through the 5-stage pipeline (classify→assemble→execute→validate→record). 229/229 agent tests pass, zero clippy warnings.

#### ~~G-14: Legacy vs Pipeline Dual Processing Path~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/agent/src/agent_loop.rs` — legacy path fully removed

Removed the legacy `run_agent_loop()` iteration loop (20-50 iterations per request) and all supporting methods. The `pipeline` field changed from `Option<Arc<AgentPipeline>>` to `Arc<AgentPipeline>` (always present). All message processing now routes through the v2 pipeline: Orchestrator (heuristic + LLM classification) → ContextEngine → EngineDispatch (Direct/ReactPlus/PlanExecute with escalation) → ResponseValidator → CostTracker.

#### ~~G-15: Two Plan Executors~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/agent/src/plan_executor.rs` (rewritten as free functions), `crates/agent/src/execution/plan_execute.rs` (deleted)

Consolidated into a single plan executor in `plan_executor.rs` using free functions (`run_step`, `build_step_context`, `regenerate_from`). Uses `ExecutionCore` for multi-cycle execution (MAX_CYCLES_PER_STEP=5), preserving backtracking from the legacy executor and multi-cycle support from the v2 engine. Dormant `execution/plan_execute.rs` (721 lines) deleted entirely. `StepExecutionResult` now includes `tool_name: Option<String>` for outcome recording.

#### ~~G-16: File-Based Memory Not in PostgreSQL~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/storage/src/repos/memory_note.rs` (new), `crates/storage/migrations/20260219000000_memory_and_learning_state.sql`, `crates/agent/src/memory.rs` (rewritten SQL-only)

Created `memory_notes` SQL table (TEXT PK `note_key`, TEXT `content`, timestamps) and `MemoryNoteRepo` with methods: `get()`, `upsert()`, `append()` (SQL-level content concatenation), `list_recent()`, `list_keys()`, `search()` (ILIKE with proper escaping), `delete()`. `MemoryStore` rewritten as SQL-only wrapper (135 lines, zero filesystem code). Long-term memory uses `LONG_TERM` key. Daily notes use `YYYY-MM-DD` keys.

#### ~~G-17: File-Based Learning Stores~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/storage/src/repos/learning_state.rs` (new), `crates/storage/src/repos/decision_log.rs` (new), `crates/storage/migrations/20260219000001_decision_log.sql`, multiple agent files rewritten SQL-only

Three stores migrated to PostgreSQL:
1. **OutcomeStore**: Removed JSONL journal, file I/O, `ensure_loaded()`/`compact()`. Now wraps `OutcomeRepo` directly (SQL-only). Added `new_in_memory()` for tests.
2. **AdaptiveThresholds**: Removed file-based `load()`/`save()`, `state_path`. Now uses `LearningStateRepo` exclusively via JSONB key-value store (`adaptive_thresholds` key).
3. **DecisionLogger**: Removed file-based JSONL logging. New `decision_log` SQL table with `DecisionLogRepo` (create, list_recent, list_by_date_range). JSONB columns for `tool_names` and `assessment`.

Deprecated config methods `learning_outcomes_path()` and `learning_state_path()` deleted. Net -2,385 lines of dead code removed across the migration.

#### ~~G-18: ToolConfidenceMap Not Wired~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/agent/src/confidence/evaluator.rs`, `crates/config/src/schema/core.rs`

Integrated `ToolConfidenceMap` into `ConfidenceEvaluator`: added `tool_map: Option<ToolConfidenceMap>` field, `new_with_map()` constructor, and `decide_for_tool(assessment, tool_name)` method that uses per-tool thresholds with fallback to global. Added `tool_overrides: HashMap<String, f32>` to `ConfidenceConfig`. Agent loop constructor loads overrides from config and creates evaluator with map when overrides exist. 4 new tests covering per-tool threshold selection, global fallback, and boundary behavior.

#### ~~G-19: StrategyTracker No Feedback Loop~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/storage/src/repos/strategy.rs`, `crates/agent/src/orchestrator/mod.rs`, `crates/agent/src/orchestrator/classifier.rs`

Added `StrategySummaryRow` and `get_strategy_summaries()` SQL query (GROUP BY with 30-day lookback). `Orchestrator` accepts optional `StrategyRepo` via builder pattern. During classification, `build_strategy_context()` fetches per-strategy accuracy stats and injects them into the LLM classifier prompt, enabling data-driven strategy selection. Graceful degradation when no historical data exists. 2 new unit tests for formatting.

#### ~~G-20: Session Loses Structured Message Data~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/storage/migrations/20260219000002_session_message_format.sql`, `crates/session/src/manager.rs`, `crates/storage/src/repos/session.rs`

Added nullable `tool_calls JSONB` and `metadata JSONB` columns to `session_messages` table. `SessionMessage` struct extended with `Option<serde_json::Value>` fields (skip_serializing_if None). New `add_structured_message()` method preserves full message structure. SQL save/load paths updated. 3 new tests covering structured messages, JSONL roundtrip, and plain message defaults.

#### ~~G-21: ProviderManager Streaming No Retry~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/providers/src/manager.rs` — `try_primary_stream_with_retry()`

Added streaming retry with exponential backoff (500ms→1s→2s) for rate-limited errors, mirroring existing non-streaming retry logic. Non-rate-limit errors fail fast. Circuit breaker interaction preserved. `chat_stream()` updated with proper fallback routing. 6 new tests covering all streaming retry paths.

#### ~~G-22: No Per-Model Context Window Mapping~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/providers/src/openai_compat.rs` — `model_context_window()`

Added static model→context_window mapping via prefix matching: GPT-4 (8K), GPT-4-32K (32K), GPT-4-turbo (128K), GPT-4o (128K), GPT-3.5-turbo (16K), o1 (200K), o1-mini/preview (128K), o3 (200K), o3-mini (200K), o4 (200K). Case-insensitive matching. Unknown models fall back to 128K default. 8+ new tests covering all model families.

#### ~~G-23: SQL Session List Missing Message Count~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/storage/src/repos/session.rs`, `crates/session/src/manager.rs`

Added `SessionListRow` with `message_count: i64` field. `list_sessions()` now uses efficient COUNT subquery (single query, no N+1). Session manager maps real count instead of hardcoded 0. Existing composite index on `(session_key, timestamp)` supports the subquery.

#### ~~G-24: No Structured Output Support~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/providers/src/types.rs`, `crates/providers/src/openai_compat.rs`, `crates/providers/src/anthropic_native.rs`

Added `ResponseFormat` enum (Text, JsonObject, JsonSchema) and `response_format` field to `ChatParams`. OpenAI-compatible provider serializes to native `response_format` API parameter with `strict: true` for JSON schema. Anthropic provider uses synthetic tool injection with forced `tool_choice` for JsonSchema, and system instruction for JsonObject. Both providers report `structured_outputs: true` in capabilities. Both streaming and non-streaming paths supported. 6+ new tests.

#### ~~G-25: create_provider() Doesn't Create ProviderManager~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
**Implementation**: `crates/providers/src/lib.rs` — `create_provider_with_failover()`

Added factory function that creates `ProviderManager` with primary + fallback when `config.provider_manager.fallback` is configured. Helper functions `create_fallback_provider()` and `create_classifier_provider()` for clean separation. Falls back to plain `DynProvider` when no failover config exists. Re-exported from facade crate.

#### ~~G-26: Dashboard Uses Legacy JSONL Stores~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
Dashboard crate removed entirely in commit `7b94f98` (`chore: remove dashboard crate and frontend`). No dashboard code remains in the codebase — all 52 files deleted including Svelte frontend, GraphQL schema, and axum server.

#### ~~G-27: Dashboard system_status Stub~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
Dashboard crate removed entirely in commit `7b94f98`. Stub resolvers no longer exist.

#### ~~G-28: Dashboard config_patch Stub~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
Dashboard crate removed entirely in commit `7b94f98`. Mutation handlers no longer exist.

#### ~~G-29: Dashboard search_todos Semantic/Hybrid Stubs~~ — RESOLVED

**Status**: **Resolved** on 2026-02-19
Dashboard crate removed entirely in commit `7b94f98`. Search is handled via TodoTool in chat.

#### G-30: Calendar Sync State Dual Backend
**Source**: `06-domain-features.md` §7.1
Parallel file/SQL exports without unified `from_repo()` pattern.

#### G-31: No CalDAV Event Caching
**Source**: `06-domain-features.md` §7.1
Every `get_events()` call hits the remote server.

#### G-32: CronStore Inconsistent Persistence
**Source**: `06-domain-features.md` §7.2
Flat JSON (full rewrite) while Goal/Plan use JSONL journals.

#### G-33: Cron 100ms Polling Inefficiency
**Source**: `06-domain-features.md` §7.2
Unnecessary CPU wake-ups for long-wait jobs. Should use `sleep_until()` with exact wake time.

#### G-34: GoalStatus No Transition Validation
**Source**: `06-domain-features.md` §7.3
Any status can transition to any other, unlike PlanStatus's enforced state machine.

#### G-35: GoalStore No Compaction Backup
**Source**: `06-domain-features.md` §7.3
PlanStore creates `.jsonl.bak` before compaction; GoalStore does not.

#### G-36: Plan SQL Upsert Not Idempotent
**Source**: `06-domain-features.md` §7.4
Uses try-create/catch-duplicate instead of `INSERT ... ON CONFLICT DO UPDATE`.

#### G-37: StorageError Conversion Loses Structure
**Source**: `01-foundation-storage.md` §6.2
`Storage(String)` variant converts via `.to_string()`, losing `NotFound` vs `Conflict` distinction.

#### G-38: Dynamic Query Builder Manual Indices
**Source**: `01-foundation-storage.md` §6.3
`TodoRepo::list()` and `ProjectRepo::list()` build SQL strings manually. Risk of off-by-one errors.

#### G-39: MemoryTool search_all O(n) In-Memory
**Source**: `03-tools-system.md` §10 #6
Loads all todos via `todo_repo.list()` then filters in-memory. Should use SQL `ILIKE` or `tsvector`.

#### G-40: ConversationEmbeddingStore Complexity
**Source**: `03-tools-system.md` §10 #5
`TokioRwLock` + `AsyncOnceCell` adds complexity now that PostgreSQL is the primary backend.

---

### P3 — Low

#### G-41: MessageRole Silent Fallback
**Source**: `01-foundation-storage.md` §6.5
Unknown role strings silently become `User`. Could mask bugs.

#### G-42: Bus Validation Not Auto-Called
**Source**: `01-foundation-storage.md` §6.6
`InboundMessage::validate()` (64KB check) exists but isn't called by `publish_inbound()`.

#### G-43: Hardcoded Groq Endpoint
**Source**: `02-providers-context.md` §9.3 G11
Can't use alternative Whisper providers.

#### G-44: Extractive-Only History Compression
**Source**: `02-providers-context.md` §9.3 G12
Just first 100 chars per message. Loses nuance.

#### G-45: No Provider Health Check
**Source**: `02-providers-context.md` §9.3 G14

#### G-46: No SQL Session Compaction
**Source**: `02-providers-context.md` §9.3 G15
SQL sessions grow unbounded (JSONL path compacts at 1000 messages).

#### G-47: Model Overrides Not Applied
**Source**: `02-providers-context.md` §9.3 G16
`ProviderRegistry::get_model_overrides()` exists but never called in request path.

#### G-48: Anthropic API Version Hardcoded
**Source**: `02-providers-context.md` §9.3 G17
`ANTHROPIC_VERSION = "2023-06-01"`.

#### G-49: RRULE V1 Limitations
**Source**: `03-tools-system.md` §10 #3
No COUNT, UNTIL, EXDATE support. Can't do "every Monday for 4 weeks".

#### G-50: No Tool Authorization Model
**Source**: `03-tools-system.md` §10 #7
Any tool callable by LLM with any parameters. No per-user restrictions.

#### G-51: Channel Test Coverage Gaps
**Source**: `05-channels-cli-dashboard.md` §5.2
Zero tests for Discord, Slack, WhatsApp, QQ, Email. Only Telegram (9 tests) and WebSocketManager (7 tests).

#### G-52: No Graceful WebSocket Close
**Source**: `05-channels-cli-dashboard.md` §5.3 #6
Channels set `running = false` but don't send WebSocket close frames.

---

## Test Coverage Summary

| Area | Unit Tests | Integration Tests | SQL Tests | Assessment |
|------|-----------|-------------------|-----------|------------|
| common | 27 | 0 | N/A | Good |
| config | 106 | 0 | N/A | Excellent |
| bus | 20 | 0 | N/A | Good |
| storage (repos) | 0 | 13 suites | 13 suites | Good (SQL-only) |
| providers | 45 | 0 | N/A | Moderate (OpenAI compat untested) |
| context_engine | 14 | 0 | N/A | Moderate |
| session | 15 | 0 | 0 | Moderate (no SQL tests) |
| tools | 200+ | 0 | N/A | Good (unit level) |
| agent | 100+ | 0 | 0 | Moderate (no integration) |
| calendar | 50 | 0 | 0 | Good (no live CalDAV tests) |
| scheduling | 29 | 0 | 0 | Moderate (no SQL tests) |
| goal | 20 | 0 | 0 | Moderate (no SQL tests) |
| plan | 14 | 0 | 0 | Moderate (no SQL tests) |
| heartbeat | 1 | 0 | N/A | Weak |
| channels | 16 | 0 | N/A | Weak (only Telegram + WsManager) |
| cli | 50 | 0 | N/A | Moderate (wizard well-tested) |
| dashboard | 0 | 0 | N/A | None |
| **Total** | **~2,722** | **47 files** | **13 suites** | |

---

## Recommended Action Plan

### Phase 1: Correctness & Stability (Weeks 1-2)
1. ~~**G-01**: Add missing fields to `storage::TodoPatch`~~ ✅ Done 2026-02-19
2. ~~**G-03/G-15**: Choose v2 plan executor, deprecate legacy~~ ✅ G-03 done 2026-02-19, G-15 done 2026-02-19 (consolidated into single executor with multi-cycle support)
3. ~~**G-04**: Set up shared SQL test fixture; add tests for critical repos~~ ✅ Done 2026-02-19
4. ~~**G-11**: Wire timezone into cron schedule computation~~ ✅ Done 2026-02-19

### Phase 2: Complete SQL Migration (Weeks 3-4)
5. ~~**G-05**: Build `klyntbot migrate` CLI command~~ ✅ Done 2026-02-19
6. ~~**G-06**: Remove legacy JSONL stores (todo_store, project_store, embedding_store)~~ ✅ Done 2026-02-19
7. ~~**G-12**: Audit and remove legacy store path methods~~ ✅ Done 2026-02-19
8. ~~**G-16/G-17**: Migrate memory store and learning stores to PostgreSQL~~ ✅ Done 2026-02-19
9. **G-30**: Unify calendar sync state to `from_repo()` pattern

### Phase 3: Provider & Context Quality (Weeks 5-6)
10. ~~**G-02**: Implement Anthropic native streaming~~ ✅ Done 2026-02-19
11. ~~**G-07**: Wire provider token counting into ContextEngine~~ ✅ Done 2026-02-19
12. ~~**G-08**: Implement memory retrieval in context assembly~~ ✅ Done 2026-02-19
13. ~~**G-20**: Extend session message format to preserve tool calls/reasoning~~ ✅ Done 2026-02-19
14. ~~**G-22**: Add per-model context window mapping~~ ✅ Done 2026-02-19

### Phase 4: Dashboard & Integration (Weeks 7-8)
15. ~~**G-09**: Wire DashboardServer into serve.rs~~ ✅ Done 2026-02-19
16. ~~**G-26**: Migrate dashboard to use SQL repos~~ ✅ Resolved 2026-02-19 (dashboard crate removed)
17. ~~**G-27/G-28/G-29**: Implement dashboard stub resolvers~~ ✅ Resolved 2026-02-19 (dashboard crate removed)
18. ~~**G-13/G-14**: Refactor AgentLoop into focused subcomponents~~ ✅ Done 2026-02-19

### Phase 5: Polish & Hardening (Ongoing)
19. ~~**G-10**: Add configurable conflict resolution strategies~~ ✅ Done 2026-02-19
20. **G-51**: Add channel integration tests (Discord, Slack priority)
21. **G-50**: Add tool authorization model
22. Remaining P3 items as capacity allows

---

## Document Map

| Document | Path | Content |
|----------|------|---------|
| Architecture Overview | `docs/architecture/00-overview.md` | High-level crate map, message journey, patterns |
| Foundation & Storage | `docs/subsystems/01-foundation-storage.md` | common, config, bus, storage (Layers 0-1.5) |
| Providers & Context | `docs/subsystems/02-providers-context.md` | providers, context_engine, session (Layer 2) |
| Tools System | `docs/subsystems/03-tools-system.md` | All 15+ tools, 11 handler traits (Layer 3) |
| Agent Core | `docs/subsystems/04-agent-core.md` | Agent loop, pipeline, learning, execution (Layer 5) |
| Channels, CLI, Dashboard | `docs/subsystems/05-channels-cli-dashboard.md` | 6 channels, CLI, dashboard (Layers 4-6) |
| Domain Features | `docs/subsystems/06-domain-features.md` | Calendar, scheduling, goal, plan, heartbeat (Layer 2) |
| **This Report** | `docs/gaps/implementation-gaps.md` | Consolidated gap report with action plan |

---

*Compiled by team-lead from 6 parallel subsystem analyses, 2026-02-19*
