# Storage

## Purpose

The `storage` crate (Layer 1.5) provides all persistent data access for Klyntbot. It wraps two storage engines -- SQLite for relational data and LanceDB for vector embeddings -- behind a repository pattern that the rest of the workspace consumes. Every crate from Layer 2 upward that needs to read or write persistent state depends on `storage`.

The crate is designed around a single principle: the caller should never think about connection management, locking, or migration state. `StoragePool::connect()` handles all of that, and the resulting pool (and every repo built from it) is freely cloneable and safe to share across async tasks.

## Key Types

### StoragePool

A newtype wrapper around `sqlx::SqlitePool`. Three constructors control its lifecycle:

- **`connect(data_dir)`** -- The primary constructor. Creates or opens `{data_dir}/data.db`, enables WAL journal mode and foreign keys via PRAGMAs, then runs all pending migrations. This is used at application startup.
- **`connect_in_memory()`** -- Creates an ephemeral in-memory SQLite pool with all migrations applied. Used exclusively by tests. No files are created on disk.
- **`from_existing(pool)`** -- Wraps a pre-existing `sqlx::SqlitePool` without running migrations. Only safe when the pool has already been migrated by a prior `connect()` call. Useful for handing a raw pool to subsystems that need a `StoragePool` type but should not re-run migrations.

`StoragePool` also exposes `run_feature_migrations()`, which applies plugin-owned migrations tracked in a `_feature_migrations` table. This allows feature crates to extend the schema without modifying the core migration set.

Because `sqlx::SqlitePool` is internally `Arc`-based, `StoragePool` is `Clone + Send + Sync` with no external locking required. Cloning a pool or any repo built from it is cheap (an `Arc` bump) and safe to do across thread and task boundaries.

### Repos

An aggregate struct that holds one instance of every repository, all constructed from a single pool. Created via `Repos::from_pool(&storage_pool)`, which clones the inner `SqlitePool` into each repo field.

The `Repos` struct provides direct public field access to each repository (e.g., `repos.todos`, `repos.plans`, `repos.finance.accounts`) plus a `pool()` method for cases where raw pool access is needed.

### StorageError

An error enum with five variants:

- **`Sqlx`** -- Wraps `sqlx::Error` for database-level failures.
- **`Migration`** -- Schema migration failures.
- **`NotFound`** -- Row not found. Converts to `KlyntbotError::StorageNotFound` at the boundary.
- **`Conflict`** -- Constraint violations (e.g., dependency cycles). Converts to `KlyntbotError::StorageConflict`.
- **`Vector`** -- LanceDB operation failures.

The `OptionExt` trait adds an ergonomic `.ok_or_not_found("label")` method on `Option<T>`, used heavily across repos to convert `fetch_optional` results into `StorageError::NotFound`.

### VectorStore

A LanceDB-backed vector store for embedding similarity search. Manages three tables, all using 384-dimensional float vectors (matching the paraphrase-multilingual-MiniLM-L12-v2 model):

- **`todo_embeddings`** -- Task embeddings for semantic search. Fields: id, vector, model, updated_at.
- **`conv_embeddings`** -- Conversation message embeddings for memory recall. Fields: id, vector, session_key, role, content_preview, full_content, created_at.
- **`memory_note_embeddings`** -- Memory note embeddings. Fields: id, vector, updated_at.

Key operations:
- `connect(data_dir)` opens or creates the LanceDB directory at `{data_dir}/lance/` and ensures all three tables exist.
- `upsert_embedding()` performs a delete-then-insert to update a vector by ID.
- `search_similar()` runs approximate nearest neighbor (ANN) search and returns `(id, score)` pairs where score is `1.0 - distance`.
- `search_conv_embeddings()` returns full row data from the conversation embeddings table.
- `delete()` and `delete_where()` remove rows by ID or predicate.
- `count()` returns the number of rows in a table.

### FinanceStorage

A sub-aggregate that groups the six finance-related repositories into a single struct. Constructed via `FinanceStorage::from_pool(&pool)` and embedded inside `Repos` as the `finance` field.

### crud_repo! Macro

A declarative macro that generates the common boilerplate for repository structs: the struct definition with a `pool` field, `new()` constructor, `get()` (returns `Option`), `get_or_err()` (returns `NotFound`), and optionally `delete()`. Used by the finance repos to avoid repeating identical patterns. More complex repos (like `TodoRepo` and `ProjectRepo`) are hand-written.

## Repositories

The storage crate contains 21 repository structs organized into four groups.

### Core Domain Repositories

| Repository | Table(s) | Purpose |
|---|---|---|
| **TodoRepo** | `todos`, `todo_attachments`, `todo_time_entries`, `todo_dependencies` | Full task management: CRUD, filtering, keyword search, focus slots, parent-child hierarchy (recursive CTE), dependency graph with cycle detection, attachments, time tracking, recurring templates, aggregation (summary, overdue, context string for LLM injection). Uses `TodoPatch` for partial updates and `TodoFilter` for listing criteria. |
| **ProjectRepo** | `projects` | Project CRUD, filtering by status/tags, archiving, and aggregated task counts per project (`ProjectWithStats`). Uses `ProjectPatch` for partial updates and `ProjectFilter` for listing. |
| **SessionRepo** | `sessions`, `session_messages` | Session and message persistence. Upserts sessions by key, adds messages (single and batch insert with chunking for SQLite bind limits), retrieves message history (all or recent N), compacts old messages, deletes stale sessions by TTL, and updates tool call metadata on the last assistant message. |
| **PlanRepo** | `plans`, `plan_steps` | Multi-step plan lifecycle management. Creates, upserts, updates, and deletes plans. Manages plan steps (add, update, upsert). Lists plans with visibility filtering (transparent/on_failure/silent). Finds the active plan for a session. Handles status transitions with timestamp bookkeeping. Cleans up stale plans by visibility and age. |
| **GoalRepo** | `goals`, `goal_project_links` | Goal persistence with plan performance tracking (plans_completed, plans_failed, rolling avg_duration_ms). Links goals to projects. |
| **CronRepo** | `cron_jobs` | Cron job scheduling state. Upserts jobs, lists active/all, toggles enabled state, updates run state after execution, supports one-shot jobs (delete_after_run). |
| **AgentTaskRepo** | `agent_tasks` | Subagent coordination task board. Creates tasks with dependency tracking (blocked_by as JSON array of task IDs), supports claim semantics (atomic pending-to-claimed transition), status updates, lists available tasks (pending + unblocked). |

### Calendar Repositories

| Repository | Table(s) | Purpose |
|---|---|---|
| **CalendarSyncRepo** | `calendar_sync_state` | Persists per-provider CalDAV sync tokens and last-sync timestamps. |
| **CalendarEventCacheRepo** | `calendar_event_cache` | Caches calendar events locally. Upserts by (uid, provider_id) composite key, lists upcoming events, queries by provider, tracks cache freshness via cached_at. |

### Learning & Intelligence Repositories

| Repository | Table(s) | Purpose |
|---|---|---|
| **OutcomeRepo** | `learning_outcomes`, `enrichment_feedback` | Records learning outcomes (tool success/failure, confidence scores, execution mode) and enrichment feedback (whether auto-inferred task fields were accepted). |
| **StrategyRepo** | `strategy_records` | Tracks intent classification accuracy. Records predicted vs. actual execution strategy, escalation counts, response times, user satisfaction. Provides aggregate stats (overall accuracy, per-tool stats, per-strategy summaries). |
| **DecisionLogRepo** | `decision_log` | Logs confidence assessment decisions per iteration (tool names, assessment JSON, outcome). Used for debugging the intent pipeline. |
| **LearningStateRepo** | `learning_state` | Key-value store for adaptive thresholds and learned parameters. Stores arbitrary JSON values by string key. |
| **MemoryNoteRepo** | `memory_notes` | Daily notes and long-term memory. Keyed by date string (e.g., "2026-03-01") or the constant `LONG_TERM`. Supports upsert, append (concatenates with double newline), keyword search, and recent listing. |
| **UsageRepo** | `usage_records` | LLM API usage tracking. Records per-request token counts (prompt, completion, cache read/write), estimated cost, model, provider, channel, and strategy. Provides aggregation by model and by day. |

### Finance Repositories

All six finance repos are grouped under `FinanceStorage` and accessed via `repos.finance.*`.

| Repository | Table(s) | Purpose |
|---|---|---|
| **FinanceAccountRepo** | `finance_accounts` | Bank/investment account CRUD (name, type, currency, balance, institution). |
| **FinanceTransactionRepo** | `finance_transactions` | Transaction CRUD with filtering by date range, account, category. Supports recurring transactions and transfer linking. |
| **FinanceBudgetRepo** | `finance_budgets` | Budget CRUD with usage queries (joins transactions to compute spent amounts per budget period). |
| **FinanceGoalRepo** | `finance_goals` | Savings/financial goal tracking (target amount, current amount, monthly contributions, expected returns). |
| **FinanceInvestmentRepo** | `finance_portfolios`, `finance_investments`, `finance_investment_transactions` | Portfolio management: portfolios, individual investments, and investment transaction history. Provides portfolio summary aggregation. |
| **FinanceLiabilityRepo** | `finance_liabilities` | Debt/liability tracking (principal, remaining, interest rate, monthly payment). |

## Row Types

Row types live in `storage::rows::*` and derive `sqlx::FromRow` for automatic deserialization from query results plus `serde::Serialize` (with `rename_all = "camelCase"`) for JSON output. Each row struct maps 1:1 to a SQLite table. Key row modules:

- `rows::todo` -- `TodoRow`, `TodoAttachmentRow`, `TodoTimeEntryRow`, `TodoDependencyRow`
- `rows::project` -- `ProjectRow`
- `rows::session` -- `SessionRow`, `SessionMessageRow`, `SessionListRow` (aggregated)
- `rows::plan` -- `PlanRow`, `PlanStepRow`
- `rows::goal` -- `GoalRow`, `GoalProjectLinkRow`
- `rows::cron` -- `CronJobRow`
- `rows::agent_task` -- `AgentTaskRow`
- `rows::calendar` -- `CalendarSyncStateRow`, `CalendarEventCacheRow`
- `rows::learning` -- `LearningStateRow`, `OutcomeRow`, `StrategyRecordRow`, `StrategySummaryRow`, `DecisionLogRow`, `EnrichmentFeedbackRow`
- `rows::memory` -- `MemoryNoteRow`
- `rows::usage` -- `UsageRecordRow`
- `rows::finance` -- Account, transaction, budget, goal, investment, liability, and portfolio row types with associated Patch and Filter structs

## How It Works

### Connection and Migration

At startup, the application calls `StoragePool::connect(data_dir)`. This:

1. Creates the data directory if it does not exist.
2. Opens (or creates) `{data_dir}/data.db` with `mode=rwc`.
3. Enables WAL journal mode (`PRAGMA journal_mode=WAL`) for concurrent read/write performance.
4. Enables foreign key enforcement (`PRAGMA foreign_keys=ON`).
5. Runs all pending migrations via `sqlx::migrate!("./migrations")`.

Migrations are standard SQL files in `crates/storage/migrations/`, named with numeric prefixes for ordering (e.g., `001_initial.sql`, `002_learning_loop.sql`, `003_strategy_tool_columns.sql`, `004_intent_pipeline.sql`, `005_agent_tasks.sql`). The `sqlx::migrate!` macro embeds them at compile time.

Feature-owned migrations (from plugins) are tracked separately in a `_feature_migrations` table, applied via `StoragePool::run_feature_migrations()`. This keeps the core migration sequence stable while allowing extensibility.

### Repository Pattern

Each repo struct holds a cloned `SqlitePool` and exposes async methods that run SQL queries via `sqlx::query_as` (for typed results) or `sqlx::query` (for execute-only). Common patterns:

- **CRUD with RETURNING** -- INSERT/UPDATE queries use `RETURNING *` to get the resulting row in one round-trip.
- **Partial updates via Patch structs** -- `TodoPatch` and `ProjectPatch` use `COALESCE` and `CASE WHEN` in SQL to only overwrite non-None fields.
- **Dynamic filtering via QueryBuilder** -- `TodoRepo::list()` and `ProjectRepo::list()` construct WHERE clauses dynamically based on filter criteria using `sqlx::QueryBuilder`.
- **Cycle detection via recursive CTEs** -- Both `TodoRepo::add_dependency()` and `TodoRepo::move_todo()` use `WITH RECURSIVE` to detect cycles before mutating the graph.
- **Batch operations** -- `SessionRepo::batch_add_messages()` chunks messages into groups of 124 (to stay under SQLite's 999 bind parameter limit) using `QueryBuilder::push_values`.
- **Upsert via ON CONFLICT** -- Sessions, cron jobs, memory notes, calendar events, and learning state all use `INSERT ... ON CONFLICT ... DO UPDATE` for idempotent writes.

### SqlitePool Concurrency

`sqlx::SqlitePool` uses `Arc` internally, making it `Clone + Send + Sync` without external synchronization. This means:

- Every repo struct is `Clone + Send + Sync` because its only field is `SqlitePool`.
- The `Repos` aggregate is `Clone + Send + Sync`.
- No `Arc<RwLock<Store>>` wrapper is needed anywhere in the codebase.
- Multiple async tasks can hold cloned repos and issue concurrent queries safely. Connection pooling is handled internally by sqlx.

### Vector Store

`VectorStore` operates independently from `StoragePool`. It connects to a LanceDB directory (`{data_dir}/lance/`) and manages three Arrow-based tables with 384-dimensional float vectors. The upsert pattern is delete-then-insert (LanceDB does not have native upsert). Similarity search uses LanceDB's built-in ANN index, returning scores computed as `1.0 - distance`.

## Connections

### Depends on

- **common** (Layer 0) -- `KlyntbotError` for error conversion.
- **tools-core** -- `FeatureMigration` type for plugin migration support.

### Depended on by

- **session** (Layer 3) -- Uses `SessionRepo` for persistence.
- **scheduling** (Layer 3) -- Uses `CronRepo`.
- **calendar** (Layer 3) -- Uses `CalendarSyncRepo` and `CalendarEventCacheRepo`.
- **domain** (Layer 3) -- Uses various repos for domain logic.
- **feature-todo** -- Uses `TodoRepo`, `ProjectRepo`.
- **feature-finance** -- Uses all finance repos via `FinanceStorage`.
- **tools** (Layer 4) -- Uses repos for tool implementations.
- **plugin-runtime** -- Uses pool for plugin migrations.
- **agent** (Layer 5) -- Uses `Repos` aggregate for the full persistence surface.
- **cli** (Layer 6) -- Uses `StoragePool` for setup and status commands.
- **klyntbot** (Layer 7) -- Re-exports storage types via the facade.
