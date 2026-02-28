# Storage Crate

Crate path: `crates/storage/`

The `storage` crate is klyntbot's persistence layer, sitting at Layer 1.5 in the dependency hierarchy. It provides two storage backends: SQLite (via sqlx) for all relational data and LanceDB (via the lancedb crate with Arrow-based schemas) for vector embeddings. Every crate above Layer 1.5 that needs persistent state depends on this crate.

---

## Section 1: Narrative Overview

### What This Crate Does

The storage crate owns all persistent state for the klyntbot agent. It exposes:

- **StoragePool** -- a newtype around `sqlx::SqlitePool` that auto-creates the database file, enables WAL mode and foreign keys, and runs all pending SQL migrations on connect.
- **21 repository structs** -- each wrapping a `SqlitePool` clone, providing domain-specific CRUD, filtering, aggregation, and lifecycle methods.
- **VectorStore** -- a LanceDB-backed embedding store managing three tables for todo, conversation, and memory-note embeddings (384-dimensional `paraphrase-multilingual-MiniLM-L12-v2` vectors).
- **Row structs** -- `sqlx::FromRow` + `serde::Serialize` types that map 1:1 to database table columns, plus Patch and Filter structs for partial updates and query construction.

All SQLite data lives in a single file (`{data_dir}/data.db`). All vector data lives in a LanceDB directory (`{data_dir}/lancedb/`). No external database server is required.

### StoragePool Design

`StoragePool` (`crates/storage/src/pool.rs`, line 8) is a `#[derive(Clone)]` newtype around `sqlx::SqlitePool`. It provides three construction methods:

1. **`connect(data_dir: &Path)`** -- The primary constructor. Creates the directory if needed, opens (or creates) `{data_dir}/data.db` in read-write-create mode, enables WAL journal mode and foreign key enforcement via PRAGMAs, then runs all pending migrations from the embedded `./migrations` directory using `sqlx::migrate!`. This is the method used in production.

2. **`connect_in_memory()`** -- Creates an in-memory SQLite pool (`sqlite::memory:`), enables foreign keys, and runs all migrations. Used exclusively by tests. Because in-memory databases are ephemeral, each test gets a fresh, isolated schema.

3. **`from_existing(pool: SqlitePool)`** -- Wraps a pre-existing `SqlitePool` without running migrations. This is a fast path for situations where the pool has already been migrated by a prior `connect()` call. Using this on an un-migrated pool will cause runtime errors.

Additionally, `run_feature_migrations(pool, migrations)` (line 55) supports feature-owned migrations tracked in the `_feature_migrations` table. Feature crates declare `FeatureMigration` structs (from `tools-core`), and StoragePool applies any that have not yet been recorded, allowing feature crates to evolve their own schema without modifying the core migration files.

The `inner()` method exposes the underlying `sqlx::SqlitePool` for direct use by repository constructors.

### Repository Pattern

Every domain has a dedicated `*Repo` struct (e.g., `TodoRepo`, `SessionRepo`, `PlanRepo`) that holds a cloned `SqlitePool`. Because `SqlitePool` is internally `Arc`-based and therefore `Clone + Send + Sync`, repositories can be freely shared across async tasks without any `Arc<RwLock<...>>` wrapper.

Each repo provides:
- A `new(pool: SqlitePool)` constructor.
- Domain-specific async methods that execute parameterized SQL via `sqlx::query` / `sqlx::query_as`.
- `RETURNING *` clauses on INSERT/UPDATE so callers always receive the persisted row.
- Partial updates via Patch structs using SQL `COALESCE` / `CASE WHEN` patterns: only non-`None` fields in the patch are overwritten.
- Filter structs combined with `sqlx::QueryBuilder` for safe, dynamic WHERE clause construction.
- The `OptionExt` trait (`ok_or_not_found`) for ergonomic conversion of `Option<T>` into `Result<T, StorageError::NotFound>`.

### Migration Strategy

Migrations live in `crates/storage/migrations/` and are embedded at compile time via `sqlx::migrate!("./migrations")`. They execute in filename order on every `connect()` and `connect_in_memory()` call.

The migration set follows a forward-only, additive pattern. SQLite does not support `DROP COLUMN`, so dead columns from earlier schemas (like `goals.metrics` and `goals.metadata`) remain in the table but are excluded by using explicit column lists in queries rather than `SELECT *`.

The five migration files are:

| File | Purpose |
|------|---------|
| `001_initial.sql` | Baseline schema: 25+ tables covering projects, todos (with attachments, time entries, dependencies), sessions, goals, plans, learning, strategy, usage, cron, calendar, memory, finance, and the `_feature_migrations` tracker |
| `002_learning_loop.sql` | Adds `chat_id` to `strategy_records`; adds typed plan-completion columns (`plans_completed`, `plans_failed`, `avg_duration_ms`, `last_plan_at`) to `goals` |
| `003_strategy_tool_columns.sql` | Adds per-tool outcome columns (`tool_name`, `tool_success`, `tool_duration_ms`) to `strategy_records` |
| `004_intent_pipeline.sql` | Adds `visibility` and `task_id` columns to `plans`; adds `complexity_signals` and `execution_mode` to `strategy_records` |
| `005_agent_tasks.sql` | Creates `agent_tasks` table for subagent coordination and `tool_usage` analytics table |

Feature-owned migrations are tracked separately in `_feature_migrations(feature_name, version, description, applied_at)` and applied by `StoragePool::run_feature_migrations()`.

### Vector Store Design

`VectorStore` (`crates/storage/src/vector_store.rs`, line 24) wraps an `Arc<lancedb::Connection>` and manages three tables:

| Table | Schema | Purpose |
|-------|--------|---------|
| `todo_embeddings` | id (Utf8), vector (FixedSizeList\<Float32, 384\>), model (Utf8), updated_at (Utf8) | Semantic search over task titles/descriptions |
| `conv_embeddings` | id, vector(384), session_key, role, content_preview, full_content, created_at | Conversation memory similarity search |
| `memory_note_embeddings` | id, vector(384), updated_at | Memory note similarity search |

All tables share a column ordering convention: id first, then the 384-dimensional vector, then table-specific string fields, and a timestamp last.

Tables are auto-created on `VectorStore::connect()` if they do not already exist. The store uses a delete-then-insert pattern for upserts (there is no native LanceDB upsert). Similarity search uses LanceDB's approximate nearest neighbor (ANN) with cosine distance, converting to similarity via `score = 1.0 - distance`.

The embedding dimension (384) corresponds to the `paraphrase-multilingual-MiniLM-L12-v2` model used by the fastembed crate in the agent layer. The storage crate itself does not generate embeddings; it only stores and queries pre-computed vectors.

### The Repos Aggregate

`Repos` (`crates/storage/src/repos/mod.rs`, line 53) is a convenience struct that holds one instance of every repository. It is constructed from a `StoragePool` via `Repos::from_pool(&pool)`, which clones the inner pool into each repo.

The aggregate has 21 public fields (one per repo) plus a private `pool` field accessible via `Repos::pool()`. Since every field is `Clone + Send + Sync`, the entire `Repos` struct can be freely cloned and passed across task boundaries.

This pattern eliminates the need for callers to construct individual repos manually. The agent layer typically creates a single `Repos` instance at startup and passes it (or clones of it) to subsystems.

### How Tests Use Ephemeral Storage

All tests use `StoragePool::connect_in_memory()`, which provides a fresh, fully-migrated SQLite database for each test. No external database server, temp files, or cleanup is required. Example pattern from the codebase:

```rust
let pool = StoragePool::connect_in_memory().await.unwrap();
let repo = TodoRepo::new(pool.inner().clone());
// ... test operations against the repo ...
```

For `VectorStore` tests, a `tempfile::TempDir` is used to provide an ephemeral filesystem directory that is automatically cleaned up when the `TempDir` guard is dropped:

```rust
let dir = TempDir::new().unwrap();
let store = VectorStore::connect(dir.path()).await.unwrap();
```

---

## Section 2: API Reference

### StorageError

**File:** `crates/storage/src/error.rs`, line 7

```rust
pub enum StorageError {
    Sqlx(sqlx::Error),
    Migration(String),
    NotFound(String),
    Conflict(String),
    Vector(String),
}
```

Implements `From<sqlx::Error>`, `From<sqlx::migrate::MigrateError>`, and `From<StorageError> for common::KlyntbotError`.

### OptionExt Trait

**File:** `crates/storage/src/error.rs`, line 35

```rust
pub trait OptionExt<T> {
    fn ok_or_not_found(self, label: &str) -> Result<T, StorageError>;
}
```

Converts `None` into `StorageError::NotFound(label)`. Used throughout repos for fetch_optional results.

### StoragePool

**File:** `crates/storage/src/pool.rs`, line 8

| Method | Signature | Description |
|--------|-----------|-------------|
| `connect` | `async fn connect(data_dir: &Path) -> Result<Self, StorageError>` | Open/create `{data_dir}/data.db`, enable WAL + FK, run migrations |
| `connect_in_memory` | `async fn connect_in_memory() -> Result<Self, StorageError>` | In-memory pool with all migrations applied |
| `from_existing` | `fn from_existing(pool: SqlitePool) -> Self` | Wrap a pre-migrated pool (skips migrations) |
| `inner` | `fn inner(&self) -> &SqlitePool` | Access the underlying sqlx pool |
| `run_feature_migrations` | `async fn run_feature_migrations(pool: &SqlitePool, migrations: &[FeatureMigration]) -> Result<(), StorageError>` | Apply feature-owned migrations not yet in `_feature_migrations` |

### Repos Aggregate

**File:** `crates/storage/src/repos/mod.rs`, line 53

| Field | Type |
|-------|------|
| `agent_tasks` | `AgentTaskRepo` |
| `todos` | `TodoRepo` |
| `projects` | `ProjectRepo` |
| `sessions` | `SessionRepo` |
| `goals` | `GoalRepo` |
| `plans` | `PlanRepo` |
| `outcomes` | `OutcomeRepo` |
| `strategies` | `StrategyRepo` |
| `usage` | `UsageRepo` |
| `cron` | `CronRepo` |
| `calendar_sync` | `CalendarSyncRepo` |
| `calendar_event_cache` | `CalendarEventCacheRepo` |
| `memory_notes` | `MemoryNoteRepo` |
| `learning_state` | `LearningStateRepo` |
| `decision_log` | `DecisionLogRepo` |
| `finance_accounts` | `FinanceAccountRepo` |
| `finance_transactions` | `FinanceTransactionRepo` |
| `finance_budgets` | `FinanceBudgetRepo` |
| `finance_investments` | `FinanceInvestmentRepo` |
| `finance_goals` | `FinanceGoalRepo` |
| `finance_liabilities` | `FinanceLiabilityRepo` |

| Method | Signature | Description |
|--------|-----------|-------------|
| `from_pool` | `fn from_pool(pool: &StoragePool) -> Self` | Construct all repos from a single pool |
| `pool` | `fn pool(&self) -> &SqlitePool` | Access the underlying pool directly |

---

### TodoRepo

**File:** `crates/storage/src/repos/todo_repo.rs`, line 34

#### CRUD

| Method | Signature | Description |
|--------|-----------|-------------|
| `add` | `async fn add(&self, row: &TodoRow) -> Result<TodoRow, StorageError>` | Insert a new todo, returns inserted row |
| `get` | `async fn get(&self, id: &str) -> Result<Option<TodoRow>, StorageError>` | Get by ID, returns None if missing |
| `get_or_err` | `async fn get_or_err(&self, id: &str) -> Result<TodoRow, StorageError>` | Get by ID, returns NotFound error if missing |
| `get_by_ids` | `async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<TodoRow>, StorageError>` | Batch get by IDs, skips missing |
| `update` | `async fn update(&self, patch: &TodoPatch) -> Result<TodoRow, StorageError>` | Partial update via TodoPatch |
| `delete` | `async fn delete(&self, id: &str) -> Result<bool, StorageError>` | Delete with cascade |

#### Listing / Filtering

| Method | Signature | Description |
|--------|-----------|-------------|
| `list` | `async fn list(&self, filter: &TodoFilter) -> Result<Vec<TodoRow>, StorageError>` | Filter by status, tags, project, priority, template flag |
| `list_templates` | `async fn list_templates() -> Result<Vec<TodoRow>, StorageError>` | List all recurring templates |
| `search_by_keyword` | `async fn search_by_keyword(&self, query: &str, limit: Option<i64>) -> Result<Vec<TodoRow>, StorageError>` | Case-insensitive LIKE search on title/description |

#### Focus Slots

| Method | Signature | Description |
|--------|-----------|-------------|
| `focus` | `async fn focus(&self, id: &str, max_slots: i64, deadline: Option<DateTime<Utc>>) -> Result<bool, StorageError>` | Atomically focus a todo if under slot limit |
| `unfocus` | `async fn unfocus(&self, id: &str) -> Result<bool, StorageError>` | Clear focus state |
| `list_focused` | `async fn list_focused() -> Result<Vec<TodoRow>, StorageError>` | List currently focused todos |

#### Dependencies

| Method | Signature | Description |
|--------|-----------|-------------|
| `add_dependency` | `async fn add_dependency(&self, task_id: &str, blocker_id: &str) -> Result<(), StorageError>` | Add dependency edge with cycle detection |
| `remove_dependency` | `async fn remove_dependency(&self, task_id: &str, blocker_id: &str) -> Result<bool, StorageError>` | Remove dependency edge |
| `get_blockers` | `async fn get_blockers(&self, task_id: &str) -> Result<Vec<TodoRow>, StorageError>` | All blockers for a task |
| `incomplete_blockers` | `async fn incomplete_blockers(&self, task_id: &str) -> Result<Vec<TodoRow>, StorageError>` | Only non-done blockers |
| `get_blocking` | `async fn get_blocking(&self, blocker_id: &str) -> Result<Vec<TodoRow>, StorageError>` | Tasks blocked by this task |
| `get_dependencies` | `async fn get_dependencies(&self, task_id: &str) -> Result<Vec<TodoDependencyRow>, StorageError>` | Raw dependency edges |

#### Attachments

| Method | Signature | Description |
|--------|-----------|-------------|
| `add_attachment` | `async fn add_attachment(&self, todo_id: &str, attachment_type: &str, value: &str, title: Option<&str>, tags: &[String]) -> Result<TodoAttachmentRow, StorageError>` | Add attachment |
| `remove_attachment` | `async fn remove_attachment(&self, todo_id: &str, attachment_id: Uuid) -> Result<bool, StorageError>` | Remove by UUID |
| `list_attachments` | `async fn list_attachments(&self, todo_id: &str) -> Result<Vec<TodoAttachmentRow>, StorageError>` | List for a todo |

#### Time Entries

| Method | Signature | Description |
|--------|-----------|-------------|
| `add_time_entry` | `async fn add_time_entry(&self, todo_id: &str, source: &str, started_at: DateTime<Utc>, duration_secs: Option<i64>, note: Option<&str>) -> Result<TodoTimeEntryRow, StorageError>` | Add entry, auto-updates total_tracked_secs |
| `close_time_entry` | `async fn close_time_entry(&self, todo_id: &str, entry_id: Uuid) -> Result<TodoTimeEntryRow, StorageError>` | Close open entry, compute duration |
| `list_time_entries` | `async fn list_time_entries(&self, todo_id: &str) -> Result<Vec<TodoTimeEntryRow>, StorageError>` | List entries for a todo |

#### Hierarchy

| Method | Signature | Description |
|--------|-----------|-------------|
| `get_children` | `async fn get_children(&self, parent_id: &str) -> Result<Vec<TodoRow>, StorageError>` | Immediate children |
| `count_children` | `async fn count_children(&self, parent_id: &str) -> Result<i64, StorageError>` | Count without loading rows |
| `get_subtree` | `async fn get_subtree(&self, root_id: &str) -> Result<Vec<TodoRow>, StorageError>` | Full subtree via recursive CTE |
| `move_todo` | `async fn move_todo(&self, id: &str, new_parent_id: Option<&str>, new_project_id: Option<&str>) -> Result<TodoRow, StorageError>` | Re-parent with cycle check |
| `cascade_complete` | `async fn cascade_complete(&self, root_id: &str) -> Result<u64, StorageError>` | Mark subtree as done |

#### Aggregation

| Method | Signature | Description |
|--------|-----------|-------------|
| `summary` | `async fn summary() -> Result<TodoSummary, StorageError>` | Count by status |
| `overdue` | `async fn overdue() -> Result<Vec<TodoRow>, StorageError>` | Todos past due date |
| `to_context_string` | `async fn to_context_string() -> Result<String, StorageError>` | Active tasks formatted for LLM context injection |

#### Recurring Templates

| Method | Signature | Description |
|--------|-----------|-------------|
| `add_template` | `async fn add_template(&self, row: &TodoRow) -> Result<TodoRow, StorageError>` | Insert a template (delegates to `add`) |
| `delete_template` | `async fn delete_template(&self, id: &str) -> Result<bool, StorageError>` | Delete where `is_template = TRUE` |

#### Supporting Types

**TodoFilter** (`crates/storage/src/repos/todo_repo.rs`, line 13):

| Field | Type | Description |
|-------|------|-------------|
| `status` | `Option<String>` | Filter by status value |
| `tags` | `Option<Vec<String>>` | All tags must match (AND) |
| `project_id` | `Option<String>` | Filter by project |
| `priority_min` | `Option<i16>` | Minimum priority |
| `limit` | `Option<i64>` | Row limit |
| `templates_only` | `bool` | When true, list templates; when false, exclude them |

**TodoPatch** (`crates/storage/src/repos/todo_repo.rs`, line 878):

| Field | Type |
|-------|------|
| `id` | `String` |
| `title` | `Option<String>` |
| `description` | `Option<Option<String>>` |
| `priority` | `Option<Option<i16>>` |
| `due_date` | `Option<Option<DateTime<Utc>>>` |
| `tags` | `Option<Vec<String>>` |
| `status` | `Option<String>` |
| `calendar_event_uid` | `Option<Option<String>>` |
| `next_instance_date` | `Option<Option<DateTime<Utc>>>` |
| `last_reminded_at` | `Option<Option<DateTime<Utc>>>` |
| `estimated_minutes` | `Option<Option<i32>>` |
| `recurrence_rule` | `Option<Option<String>>` |

**TodoSummary** (`crates/storage/src/repos/todo_repo.rs`, line 24):

| Field | Type |
|-------|------|
| `todo` | `i64` |
| `doing` | `i64` |
| `done` | `i64` |
| `total` | `i64` |

---

### ProjectRepo

**File:** `crates/storage/src/repos/project_repo.rs`, line 30

| Method | Signature | Description |
|--------|-----------|-------------|
| `create` | `async fn create(&self, row: &ProjectRow) -> Result<ProjectRow, StorageError>` | Insert a new project |
| `get` | `async fn get(&self, id: &str) -> Result<Option<ProjectRow>, StorageError>` | Get by ID |
| `get_or_err` | `async fn get_or_err(&self, id: &str) -> Result<ProjectRow, StorageError>` | Get or NotFound |
| `update` | `async fn update(&self, patch: &ProjectPatch) -> Result<ProjectRow, StorageError>` | Partial update |
| `delete` | `async fn delete(&self, id: &str) -> Result<bool, StorageError>` | Delete (todos get project_id set NULL) |
| `archive` | `async fn archive(&self, id: &str) -> Result<ProjectRow, StorageError>` | Set status to archived |
| `list` | `async fn list(&self, filter: &ProjectFilter) -> Result<Vec<ProjectRow>, StorageError>` | Filter by status, tags, limit |
| `all` | `async fn all() -> Result<Vec<ProjectRow>, StorageError>` | List all (no filter) |
| `count_tasks_by_status` | `async fn count_tasks_by_status(&self, project_id: &str) -> Result<Vec<(String, i64)>, StorageError>` | Task counts per status |
| `get_with_stats` | `async fn get_with_stats(&self, id: &str) -> Result<Option<ProjectWithStats>, StorageError>` | Project with aggregated task statistics |

---

### SessionRepo

**File:** `crates/storage/src/repos/session.rs`, line 11

| Method | Signature | Description |
|--------|-----------|-------------|
| `upsert_session` | `async fn upsert_session(&self, key: &str, metadata: &Value) -> Result<SessionRow, StorageError>` | Insert or touch updated_at |
| `get_session` | `async fn get_session(&self, key: &str) -> Result<SessionRow, StorageError>` | Get or NotFound |
| `list_sessions` | `async fn list_sessions() -> Result<Vec<SessionListRow>, StorageError>` | All sessions with message counts |
| `count_sessions` | `async fn count_sessions() -> Result<i64, StorageError>` | Total session count |
| `add_message` | `async fn add_message(&self, session_key, id, role, content, request_id, tool_calls, metadata) -> Result<SessionMessageRow, StorageError>` | Add message + touch session in one round-trip (CTE) |
| `batch_add_messages` | `async fn batch_add_messages(&self, session_key, ids, roles, ...) -> Result<u64, StorageError>` | Bulk insert with 124-row chunks (SQLite bind limit) |
| `get_messages` | `async fn get_messages(&self, session_key: &str) -> Result<Vec<SessionMessageRow>, StorageError>` | All messages ordered by timestamp |
| `get_recent_messages` | `async fn get_recent_messages(&self, session_key: &str, limit: i64) -> Result<Vec<SessionMessageRow>, StorageError>` | Most recent N messages |
| `count_messages` | `async fn count_messages(&self, session_key: &str) -> Result<i64, StorageError>` | Message count |
| `compact_session` | `async fn compact_session(&self, session_key: &str, keep_count: i64) -> Result<u64, StorageError>` | Delete oldest messages, keep N |
| `delete_session` | `async fn delete_session(&self, key: &str) -> Result<bool, StorageError>` | Delete session + cascade messages |
| `update_last_assistant_metadata` | `async fn update_last_assistant_metadata(&self, session_key, tool_calls, metadata) -> Result<bool, StorageError>` | Update tool_calls/metadata on most recent assistant message |
| `delete_stale_sessions` | `async fn delete_stale_sessions(&self, ttl_days: u32) -> Result<u64, StorageError>` | Delete sessions not updated within TTL |

---

### PlanRepo

**File:** `crates/storage/src/repos/plan.rs`, line 14

| Method | Signature | Description |
|--------|-----------|-------------|
| `create` | `async fn create(&self, row: &PlanRow) -> Result<PlanRow, StorageError>` | Insert a new plan |
| `upsert` | `async fn upsert(&self, row: &PlanRow) -> Result<PlanRow, StorageError>` | Insert or update on conflict |
| `get` | `async fn get(&self, id: Uuid) -> Result<PlanRow, StorageError>` | Get or NotFound |
| `list` | `async fn list(&self, status, session_key, goal_id, visibility) -> Result<Vec<PlanRow>, StorageError>` | Filter plans; `visibility=None` excludes silent, `Some("all")` shows all |
| `update` | `async fn update(&self, row: &PlanRow) -> Result<PlanRow, StorageError>` | Full replace of mutable fields |
| `delete` | `async fn delete(&self, id: Uuid) -> Result<bool, StorageError>` | Delete with cascade to steps |
| `update_status` | `async fn update_status(&self, id: Uuid, status: &str) -> Result<(), StorageError>` | Update status with completed_at bookkeeping |
| `delete_stale_plans` | `async fn delete_stale_plans(&self, silent_age_hours: i64, on_failure_age_hours: i64) -> Result<u64, StorageError>` | Clean up stale silent/on_failure plans |
| `get_active` | `async fn get_active(&self, session_key: &str) -> Result<Option<PlanRow>, StorageError>` | Most recent draft/approved/executing plan for session |
| `add_step` | `async fn add_step(&self, step: &PlanStepRow) -> Result<PlanStepRow, StorageError>` | Add a plan step |
| `update_step` | `async fn update_step(&self, step: &PlanStepRow) -> Result<PlanStepRow, StorageError>` | Update step status/result |
| `upsert_step` | `async fn upsert_step(&self, step: &PlanStepRow) -> Result<PlanStepRow, StorageError>` | Insert or update step on conflict |
| `get_steps` | `async fn get_steps(&self, plan_id: Uuid) -> Result<Vec<PlanStepRow>, StorageError>` | All steps ordered by step_index |

---

### GoalRepo

**File:** `crates/storage/src/repos/goal.rs`, line 21

| Method | Signature | Description |
|--------|-----------|-------------|
| `create` | `async fn create(&self, row: &GoalRow) -> Result<GoalRow, StorageError>` | Insert a new goal |
| `get` | `async fn get(&self, id: Uuid) -> Result<GoalRow, StorageError>` | Get or NotFound |
| `list` | `async fn list(&self, status: Option<&str>) -> Result<Vec<GoalRow>, StorageError>` | Optional status filter |
| `update` | `async fn update(&self, row: &GoalRow) -> Result<GoalRow, StorageError>` | Full replace of mutable fields |
| `delete` | `async fn delete(&self, id: Uuid) -> Result<bool, StorageError>` | Delete goal |
| `increment_completed` | `async fn increment_completed(&self, id: Uuid, plan_duration_ms: i64) -> Result<(), StorageError>` | Increment plans_completed, rolling avg_duration_ms |
| `increment_failed` | `async fn increment_failed(&self, id: Uuid) -> Result<(), StorageError>` | Increment plans_failed |
| `link_project` | `async fn link_project(&self, goal_id: Uuid, project_id: &str) -> Result<(), StorageError>` | Add goal-project link (ON CONFLICT DO NOTHING) |
| `unlink_project` | `async fn unlink_project(&self, goal_id: Uuid, project_id: &str) -> Result<bool, StorageError>` | Remove goal-project link |
| `get_project_links` | `async fn get_project_links(&self, goal_id: Uuid) -> Result<Vec<GoalProjectLinkRow>, StorageError>` | All project links for a goal |

---

### CronRepo

**File:** `crates/storage/src/repos/cron.rs`, line 10

| Method | Signature | Description |
|--------|-----------|-------------|
| `upsert` | `async fn upsert(&self, row: &CronJobRow) -> Result<CronJobRow, StorageError>` | Insert or update on conflict |
| `get` | `async fn get(&self, id: &str) -> Result<CronJobRow, StorageError>` | Get or NotFound |
| `list` | `async fn list() -> Result<Vec<CronJobRow>, StorageError>` | All cron jobs |
| `list_active` | `async fn list_active() -> Result<Vec<CronJobRow>, StorageError>` | Only enabled jobs |
| `set_enabled` | `async fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), StorageError>` | Toggle enabled |
| `update_run_state` | `async fn update_run_state(&self, id, last_run_at_ms, next_run_at_ms, last_status, last_error, updated_at_ms) -> Result<(), StorageError>` | Update after execution |
| `delete` | `async fn delete(&self, id: &str) -> Result<bool, StorageError>` | Delete cron job |

---

### UsageRepo

**File:** `crates/storage/src/repos/usage.rs`, line 11

| Method | Signature | Description |
|--------|-----------|-------------|
| `create` | `async fn create(&self, row: &UsageRecordRow) -> Result<UsageRecordRow, StorageError>` | Append a usage record |
| `aggregate_by_model` | `async fn aggregate_by_model(&self, since: DateTime<Utc>) -> Result<Vec<(String, i64, f64)>, StorageError>` | (model, total_tokens, total_cost) |
| `aggregate_by_day` | `async fn aggregate_by_day(&self, since: DateTime<Utc>) -> Result<Vec<(String, f64)>, StorageError>` | (date_string, total_cost) |
| `totals_since` | `async fn totals_since(&self, since: DateTime<Utc>) -> Result<(i64, f64), StorageError>` | (total_requests, total_cost) |

---

### CalendarSyncRepo

**File:** `crates/storage/src/repos/calendar_sync.rs`, line 11

| Method | Signature | Description |
|--------|-----------|-------------|
| `get` | `async fn get(&self, provider_id: &str) -> Result<CalendarSyncStateRow, StorageError>` | Get sync state or NotFound |
| `upsert` | `async fn upsert(&self, provider_id, sync_token, last_sync_at) -> Result<CalendarSyncStateRow, StorageError>` | Insert or update sync state |
| `list` | `async fn list() -> Result<Vec<CalendarSyncStateRow>, StorageError>` | All sync states |
| `delete` | `async fn delete(&self, provider_id: &str) -> Result<bool, StorageError>` | Delete sync state |

---

### CalendarEventCacheRepo

**File:** `crates/storage/src/repos/calendar_event_cache.rs`, line 11

| Method | Signature | Description |
|--------|-----------|-------------|
| `get_by_uid` | `async fn get_by_uid(&self, uid: &str) -> Result<CalendarEventCacheRow, StorageError>` | Get event by UID or NotFound |
| `list_by_provider` | `async fn list_by_provider(&self, provider_id: &str) -> Result<Vec<CalendarEventCacheRow>, StorageError>` | Events for a provider |
| `list_upcoming` | `async fn list_upcoming(&self, limit: i64) -> Result<Vec<CalendarEventCacheRow>, StorageError>` | Future events across providers |
| `upsert` | `async fn upsert(&self, uid, provider_id, summary, description, start_at, end_at, source, etag, status) -> Result<CalendarEventCacheRow, StorageError>` | Insert or update on (uid, provider_id) |
| `delete_by_provider` | `async fn delete_by_provider(&self, provider_id: &str) -> Result<u64, StorageError>` | Delete all for provider |
| `delete` | `async fn delete(&self, uid: &str, provider_id: &str) -> Result<bool, StorageError>` | Delete specific event |
| `last_cached_at` | `async fn last_cached_at(&self, provider_id: &str) -> Result<Option<DateTime<Utc>>, StorageError>` | Most recent cache timestamp |

---

### StrategyRepo

**File:** `crates/storage/src/repos/strategy.rs`, line 13

| Method | Signature | Description |
|--------|-----------|-------------|
| `create` | `async fn create(&self, row: &StrategyRecordRow) -> Result<StrategyRecordRow, StorageError>` | Insert strategy record |
| `get` | `async fn get(&self, id: Uuid) -> Result<StrategyRecordRow, StorageError>` | Get or NotFound |
| `list_by_strategy` | `async fn list_by_strategy(&self, strategy: &str, since: DateTime<Utc>) -> Result<Vec<StrategyRecordRow>, StorageError>` | Filter by predicted strategy |
| `get_accuracy` | `async fn get_accuracy(&self, strategy: &str, since: DateTime<Utc>) -> Result<Option<f32>, StorageError>` | Fraction where predicted == actual |
| `get_strategy_summaries` | `async fn get_strategy_summaries(&self, since: DateTime<Utc>) -> Result<Vec<StrategySummaryRow>, StorageError>` | Per-strategy accuracy, sample count, avg escalations |
| `list_by_date_range` | `async fn list_by_date_range(&self, from, to) -> Result<Vec<StrategyRecordRow>, StorageError>` | Records in date range |
| `set_satisfaction_for_chat` | `async fn set_satisfaction_for_chat(&self, chat_id: &str, since: DateTime<Utc>, satisfaction: f32) -> Result<bool, StorageError>` | Update satisfaction on most recent record for a chat |
| `count_all` | `async fn count_all() -> Result<i64, StorageError>` | Total record count |
| `get_overall_stats` | `async fn get_overall_stats() -> Result<OverallStats, StorageError>` | Accuracy, avg response time, avg satisfaction |
| `get_tool_stats` | `async fn get_tool_stats() -> Result<Vec<ToolStatsRow>, StorageError>` | Per-tool call count, success count, avg duration |

---

### OutcomeRepo

**File:** `crates/storage/src/repos/outcome.rs`, line 11

| Method | Signature | Description |
|--------|-----------|-------------|
| `create` | `async fn create(&self, row: &OutcomeRow) -> Result<OutcomeRow, StorageError>` | Insert learning outcome |
| `list_by_date_range` | `async fn list_by_date_range(&self, from, to) -> Result<Vec<OutcomeRow>, StorageError>` | Outcomes in date range |
| `list_by_tool` | `async fn list_by_tool(&self, tool_name: &str) -> Result<Vec<OutcomeRow>, StorageError>` | Outcomes for a tool |
| `count_stats` | `async fn count_stats(&self, since: DateTime<Utc>) -> Result<(i64, i64), StorageError>` | (total, success_count) |
| `create_enrichment_feedback` | `async fn create_enrichment_feedback(&self, task_id, field, suggested_value, actual_value, accepted, confidence) -> Result<EnrichmentFeedbackRow, StorageError>` | Insert enrichment feedback |
| `list_enrichment_feedback` | `async fn list_enrichment_feedback() -> Result<Vec<EnrichmentFeedbackRow>, StorageError>` | All enrichment feedback |

---

### LearningStateRepo

**File:** `crates/storage/src/repos/learning_state.rs`, line 10

| Method | Signature | Description |
|--------|-----------|-------------|
| `get` | `async fn get(&self, key: &str) -> Result<Option<LearningStateRow>, StorageError>` | Get full row by key |
| `get_value` | `async fn get_value(&self, key: &str) -> Result<Option<Value>, StorageError>` | Get just the JSON value |
| `set` | `async fn set(&self, key: &str, value: &Value) -> Result<LearningStateRow, StorageError>` | Upsert key-value pair |
| `get_all` | `async fn get_all() -> Result<Vec<LearningStateRow>, StorageError>` | All entries |
| `delete` | `async fn delete(&self, key: &str) -> Result<bool, StorageError>` | Delete by key |

---

### DecisionLogRepo

**File:** `crates/storage/src/repos/decision_log.rs`, line 11

| Method | Signature | Description |
|--------|-----------|-------------|
| `create` | `async fn create(&self, row: &DecisionLogRow) -> Result<DecisionLogRow, StorageError>` | Insert decision log entry |
| `list_recent` | `async fn list_recent(&self, limit: i64) -> Result<Vec<DecisionLogRow>, StorageError>` | Most recent N entries |
| `list_by_date_range` | `async fn list_by_date_range(&self, from, to) -> Result<Vec<DecisionLogRow>, StorageError>` | Entries in date range |

---

### MemoryNoteRepo

**File:** `crates/storage/src/repos/memory_note.rs`, line 10

| Method | Signature | Description |
|--------|-----------|-------------|
| `get` | `async fn get(&self, note_key: &str) -> Result<Option<MemoryNoteRow>, StorageError>` | Get by key (date string or `LONG_TERM`) |
| `upsert` | `async fn upsert(&self, note_key: &str, content: &str) -> Result<MemoryNoteRow, StorageError>` | Insert or replace content |
| `append` | `async fn append(&self, note_key: &str, content: &str) -> Result<MemoryNoteRow, StorageError>` | Append to existing (double-newline separator) |
| `list_recent` | `async fn list_recent(&self, limit: i64) -> Result<Vec<MemoryNoteRow>, StorageError>` | Recent daily notes (excludes LONG_TERM) |
| `list_keys` | `async fn list_keys() -> Result<Vec<String>, StorageError>` | All note keys |
| `search` | `async fn search(&self, query: &str) -> Result<Vec<MemoryNoteRow>, StorageError>` | Case-insensitive LIKE search on content |
| `delete` | `async fn delete(&self, note_key: &str) -> Result<bool, StorageError>` | Delete by key |

Constant: `LONG_TERM_KEY = "LONG_TERM"` (line 15)

---

### AgentTaskRepo

**File:** `crates/storage/src/repos/agent_task.rs`, line 11

| Method | Signature | Description |
|--------|-----------|-------------|
| `create` | `async fn create(&self, session_key: &str, description: &str, blocked_by: &[String]) -> Result<AgentTaskRow, StorageError>` | Create with auto-generated UUID |
| `claim` | `async fn claim(&self, task_id: &str, agent_id: &str) -> Result<AgentTaskRow, StorageError>` | Atomically claim (pending + unclaimed only) |
| `update_status` | `async fn update_status(&self, task_id, status, result, error) -> Result<AgentTaskRow, StorageError>` | Update status/result/error |
| `list_by_session` | `async fn list_by_session(&self, session_key: &str) -> Result<Vec<AgentTaskRow>, StorageError>` | All tasks for session |
| `list_available` | `async fn list_available(&self, session_key: &str) -> Result<Vec<AgentTaskRow>, StorageError>` | Pending + unclaimed + all blockers completed |
| `delete_by_session` | `async fn delete_by_session(&self, session_key: &str) -> Result<u64, StorageError>` | Delete all tasks for session |
| `get` | `async fn get(&self, task_id: &str) -> Result<AgentTaskRow, StorageError>` | Get or NotFound |

---

### Finance Repos

#### FinanceAccountRepo

**File:** `crates/storage/src/repos/finance_account_repo.rs`, line 10

| Method | Signature | Description |
|--------|-----------|-------------|
| `add` | `async fn add(&self, row: &FinanceAccountRow) -> Result<FinanceAccountRow, StorageError>` | Insert account |
| `get` | `async fn get(&self, id: &str) -> Result<Option<FinanceAccountRow>, StorageError>` | Get by ID |
| `get_or_err` | `async fn get_or_err(&self, id: &str) -> Result<FinanceAccountRow, StorageError>` | Get or NotFound |
| `update` | `async fn update(&self, patch: &FinanceAccountPatch) -> Result<FinanceAccountRow, StorageError>` | Partial update |
| `delete` | `async fn delete(&self, id: &str) -> Result<bool, StorageError>` | Delete (cascades transactions) |
| `list` | `async fn list(&self, include_archived: bool) -> Result<Vec<FinanceAccountRow>, StorageError>` | All accounts, optionally including archived |
| `list_by_currency` | `async fn list_by_currency(&self, currency: &str) -> Result<Vec<FinanceAccountRow>, StorageError>` | Non-archived by currency |
| `total_balance_by_currency` | `async fn total_balance_by_currency() -> Result<Vec<(String, i64)>, StorageError>` | Sum balances grouped by currency |
| `adjust_balance` | `async fn adjust_balance(&self, id: &str, delta: i64) -> Result<FinanceAccountRow, StorageError>` | Atomic balance += delta |

#### FinanceTransactionRepo

**File:** `crates/storage/src/repos/finance_transaction_repo.rs`, line 13

| Method | Signature | Description |
|--------|-----------|-------------|
| `add` | `async fn add(&self, row: &FinanceTransactionRow) -> Result<FinanceTransactionRow, StorageError>` | Insert transaction |
| `get` | `async fn get(&self, id: &str) -> Result<Option<FinanceTransactionRow>, StorageError>` | Get by ID |
| `update` | `async fn update(&self, patch: &FinanceTransactionPatch) -> Result<FinanceTransactionRow, StorageError>` | Partial update |
| `delete` | `async fn delete(&self, id: &str) -> Result<Option<FinanceTransactionRow>, StorageError>` | Delete, returns deleted row for balance reversal |
| `list` | `async fn list(&self, filter: &FinanceTransactionFilter) -> Result<Vec<FinanceTransactionRow>, StorageError>` | QueryBuilder filtering (account, type, category, dates, amounts, text search) |
| `get_by_transfer_id` | `async fn get_by_transfer_id(&self, transfer_id: &str) -> Result<Vec<FinanceTransactionRow>, StorageError>` | Both sides of a transfer |
| `sum_by_category` | `async fn sum_by_category(&self, date_from, date_to, tx_type) -> Result<Vec<(String, i64)>, StorageError>` | Category breakdown |
| `sum_by_period` | `async fn sum_by_period(&self, tx_type, n_periods, period_type) -> Result<Vec<(String, i64)>, StorageError>` | Monthly totals |
| `category_history` | `async fn category_history(&self, limit: i64) -> Result<Vec<(String, String, i64)>, StorageError>` | Counterparty-to-category pairings for auto-categorization |

#### FinanceBudgetRepo

**File:** `crates/storage/src/repos/finance_budget_repo.rs`, line 10

| Method | Signature | Description |
|--------|-----------|-------------|
| `add` | `async fn add(&self, row: &FinanceBudgetRow) -> Result<FinanceBudgetRow, StorageError>` | Insert budget |
| `get` | `async fn get(&self, id: &str) -> Result<Option<FinanceBudgetRow>, StorageError>` | Get by ID |
| `update` | `async fn update(&self, patch: &FinanceBudgetPatch) -> Result<FinanceBudgetRow, StorageError>` | Partial update |
| `delete` | `async fn delete(&self, id: &str) -> Result<bool, StorageError>` | Delete budget |
| `list_active` | `async fn list_active() -> Result<Vec<FinanceBudgetRow>, StorageError>` | Active budgets |
| `get_by_category` | `async fn get_by_category(&self, category: &str) -> Result<Option<FinanceBudgetRow>, StorageError>` | Active budget for category |
| `budget_usage` | `async fn budget_usage(&self, budget_id: &str) -> Result<BudgetUsageRow, StorageError>` | Budget + spent amount (SQL JOIN with period-aware date boundaries) |
| `all_budget_usage` | `async fn all_budget_usage() -> Result<Vec<BudgetUsageRow>, StorageError>` | Budget usage for all active budgets |

#### FinanceInvestmentRepo

**File:** `crates/storage/src/repos/finance_investment_repo.rs`, line 14

| Method | Signature | Description |
|--------|-----------|-------------|
| `add_portfolio` | `async fn add_portfolio(&self, row: &FinancePortfolioRow) -> Result<FinancePortfolioRow, StorageError>` | Insert portfolio |
| `get_portfolio` | `async fn get_portfolio(&self, id: &str) -> Result<Option<FinancePortfolioRow>, StorageError>` | Get portfolio |
| `list_portfolios` | `async fn list_portfolios() -> Result<Vec<FinancePortfolioRow>, StorageError>` | All portfolios |
| `delete_portfolio` | `async fn delete_portfolio(&self, id: &str) -> Result<bool, StorageError>` | Delete with cascade |
| `add_investment` | `async fn add_investment(&self, row: &FinanceInvestmentRow) -> Result<FinanceInvestmentRow, StorageError>` | Insert investment |
| `get_investment` | `async fn get_investment(&self, id: &str) -> Result<Option<FinanceInvestmentRow>, StorageError>` | Get investment |
| `update_investment` | `async fn update_investment(&self, patch: &FinanceInvestmentPatch) -> Result<FinanceInvestmentRow, StorageError>` | Partial update |
| `update_price` | `async fn update_price(&self, id: &str, current_price: i64, current_value: i64) -> Result<FinanceInvestmentRow, StorageError>` | Quick price refresh |
| `list_investments` | `async fn list_investments(&self, filter: &FinanceInvestmentFilter) -> Result<Vec<FinanceInvestmentRow>, StorageError>` | Filter by portfolio, asset_type, has_symbol |
| `list_with_symbols` | `async fn list_with_symbols() -> Result<Vec<FinanceInvestmentRow>, StorageError>` | Investments with symbols (for batch price refresh) |
| `delete_investment` | `async fn delete_investment(&self, id: &str) -> Result<bool, StorageError>` | Delete with cascade |
| `add_investment_tx` | `async fn add_investment_tx(&self, row: &FinanceInvestmentTxRow) -> Result<FinanceInvestmentTxRow, StorageError>` | Insert investment transaction |
| `list_investment_txs` | `async fn list_investment_txs(&self, investment_id: &str) -> Result<Vec<FinanceInvestmentTxRow>, StorageError>` | Transactions for an investment |
| `portfolio_summary` | `async fn portfolio_summary(&self, portfolio_id: &str) -> Result<PortfolioSummaryRow, StorageError>` | Aggregate cost/value/count |
| `total_value_by_currency` | `async fn total_value_by_currency() -> Result<Vec<(String, i64)>, StorageError>` | Sum current_value by currency |

#### FinanceGoalRepo

**File:** `crates/storage/src/repos/finance_goal_repo.rs`, line 10

| Method | Signature | Description |
|--------|-----------|-------------|
| `add` | `async fn add(&self, row: &FinanceGoalRow) -> Result<FinanceGoalRow, StorageError>` | Insert goal |
| `get` | `async fn get(&self, id: &str) -> Result<Option<FinanceGoalRow>, StorageError>` | Get by ID |
| `update` | `async fn update(&self, patch: &FinanceGoalPatch) -> Result<FinanceGoalRow, StorageError>` | Partial update |
| `delete` | `async fn delete(&self, id: &str) -> Result<bool, StorageError>` | Delete goal |
| `list_active` | `async fn list_active() -> Result<Vec<FinanceGoalRow>, StorageError>` | Active goals |
| `update_progress` | `async fn update_progress(&self, id: &str, current_amount: i64) -> Result<FinanceGoalRow, StorageError>` | Set current_amount directly |

#### FinanceLiabilityRepo

**File:** `crates/storage/src/repos/finance_liability_repo.rs`, line 10

| Method | Signature | Description |
|--------|-----------|-------------|
| `add` | `async fn add(&self, row: &FinanceLiabilityRow) -> Result<FinanceLiabilityRow, StorageError>` | Insert liability |
| `get` | `async fn get(&self, id: &str) -> Result<Option<FinanceLiabilityRow>, StorageError>` | Get by ID |
| `update` | `async fn update(&self, patch: &FinanceLiabilityPatch) -> Result<FinanceLiabilityRow, StorageError>` | Partial update |
| `delete` | `async fn delete(&self, id: &str) -> Result<bool, StorageError>` | Delete liability |
| `list_all` | `async fn list_all() -> Result<Vec<FinanceLiabilityRow>, StorageError>` | All liabilities |
| `total_remaining_by_currency` | `async fn total_remaining_by_currency() -> Result<Vec<(String, i64)>, StorageError>` | Sum remaining by currency |

---

### Row Structs

#### TodoRow

**File:** `crates/storage/src/rows/todo.rs`, line 11

| Field | Type | Notes |
|-------|------|-------|
| `id` | `String` | |
| `title` | `String` | |
| `description` | `Option<String>` | |
| `priority` | `Option<i16>` | 1=highest |
| `due_date` | `Option<DateTime<Utc>>` | |
| `tags` | `Vec<String>` | `#[sqlx(json)]` |
| `status` | `String` | todo, doing, done |
| `focused_at` | `Option<DateTime<Utc>>` | |
| `focus_deadline` | `Option<DateTime<Utc>>` | |
| `focus_expired_count` | `i32` | |
| `created_at` | `DateTime<Utc>` | |
| `updated_at` | `DateTime<Utc>` | |
| `completed_at` | `Option<DateTime<Utc>>` | Auto-set when status=done |
| `parent_id` | `Option<String>` | FK to todos(id) |
| `project_id` | `Option<String>` | FK to projects(id) |
| `total_tracked_secs` | `i64` | Sum of time entries |
| `estimated_minutes` | `Option<i32>` | |
| `calendar_event_uid` | `Option<String>` | Linked calendar event |
| `last_reminded_at` | `Option<DateTime<Utc>>` | |
| `recurrence_rule` | `Option<String>` | |
| `recurrence_parent_id` | `Option<String>` | |
| `is_template` | `bool` | Recurring template flag |
| `next_instance_date` | `Option<DateTime<Utc>>` | |

#### TodoAttachmentRow

**File:** `crates/storage/src/rows/todo.rs`, line 41

| Field | Type |
|-------|------|
| `id` | `Uuid` |
| `todo_id` | `String` |
| `attachment_type` | `String` |
| `value` | `String` |
| `title` | `Option<String>` |
| `tags` | `Vec<String>` (`#[sqlx(json)]`) |
| `created_at` | `DateTime<Utc>` |

#### TodoTimeEntryRow

**File:** `crates/storage/src/rows/todo.rs`, line 55

| Field | Type |
|-------|------|
| `id` | `Uuid` |
| `todo_id` | `String` |
| `source` | `String` |
| `started_at` | `DateTime<Utc>` |
| `ended_at` | `Option<DateTime<Utc>>` |
| `duration_secs` | `Option<i64>` |
| `note` | `Option<String>` |

#### TodoDependencyRow

**File:** `crates/storage/src/rows/todo.rs`, line 68

| Field | Type |
|-------|------|
| `task_id` | `String` |
| `blocker_id` | `String` |

#### ProjectRow

**File:** `crates/storage/src/rows/project.rs`, line 10

| Field | Type |
|-------|------|
| `id` | `String` |
| `name` | `String` |
| `description` | `Option<String>` |
| `color` | `String` |
| `tags` | `Vec<String>` (`#[sqlx(json)]`) |
| `status` | `String` |
| `created_at` | `DateTime<Utc>` |
| `updated_at` | `DateTime<Utc>` |

#### SessionRow

**File:** `crates/storage/src/rows/session.rs`, line 10

| Field | Type |
|-------|------|
| `key` | `String` |
| `metadata` | `serde_json::Value` |
| `created_at` | `DateTime<Utc>` |
| `updated_at` | `DateTime<Utc>` |

#### SessionMessageRow

**File:** `crates/storage/src/rows/session.rs`, line 20

| Field | Type |
|-------|------|
| `id` | `Uuid` |
| `session_key` | `String` |
| `role` | `String` |
| `content` | `String` |
| `timestamp` | `DateTime<Utc>` |
| `request_id` | `Option<String>` |
| `tool_calls` | `Option<serde_json::Value>` |
| `metadata` | `Option<serde_json::Value>` |

#### SessionListRow

**File:** `crates/storage/src/rows/session.rs`, line 34

| Field | Type |
|-------|------|
| `key` | `String` |
| `metadata` | `serde_json::Value` |
| `created_at` | `DateTime<Utc>` |
| `updated_at` | `DateTime<Utc>` |
| `message_count` | `i64` |

#### PlanRow

**File:** `crates/storage/src/rows/plan.rs`, line 10

| Field | Type |
|-------|------|
| `id` | `Uuid` |
| `session_key` | `String` |
| `goal_id` | `Option<Uuid>` |
| `title` | `String` |
| `description` | `String` |
| `status` | `String` |
| `current_step_index` | `i32` |
| `iteration_limit` | `i32` |
| `backtrack_history` | `serde_json::Value` |
| `visibility` | `String` |
| `task_id` | `Option<String>` |
| `created_at` | `DateTime<Utc>` |
| `updated_at` | `DateTime<Utc>` |
| `completed_at` | `Option<DateTime<Utc>>` |

#### PlanStepRow

**File:** `crates/storage/src/rows/plan.rs`, line 30

| Field | Type |
|-------|------|
| `id` | `Uuid` |
| `plan_id` | `Uuid` |
| `step_index` | `i32` |
| `description` | `String` |
| `reasoning` | `String` |
| `expected_tools` | `Vec<String>` (`#[sqlx(json)]`) |
| `status` | `String` |
| `attempt_count` | `i16` |
| `max_attempts` | `i16` |
| `result` | `Option<String>` |
| `started_at` | `Option<DateTime<Utc>>` |
| `completed_at` | `Option<DateTime<Utc>>` |

#### GoalRow

**File:** `crates/storage/src/rows/goal.rs`, line 14

| Field | Type |
|-------|------|
| `id` | `Uuid` |
| `title` | `String` |
| `description` | `String` |
| `status` | `String` |
| `priority` | `i16` |
| `target_date` | `Option<DateTime<Utc>>` |
| `created_at` | `DateTime<Utc>` |
| `updated_at` | `DateTime<Utc>` |
| `plans_completed` | `i32` |
| `plans_failed` | `i32` |
| `avg_duration_ms` | `Option<i64>` |
| `last_plan_at` | `Option<DateTime<Utc>>` |

#### GoalProjectLinkRow

**File:** `crates/storage/src/rows/goal.rs`, line 32

| Field | Type |
|-------|------|
| `goal_id` | `Uuid` |
| `project_id` | `String` |

#### CronJobRow

**File:** `crates/storage/src/rows/cron.rs`, line 9

| Field | Type |
|-------|------|
| `id` | `String` |
| `name` | `String` |
| `enabled` | `bool` |
| `schedule` | `serde_json::Value` |
| `payload` | `serde_json::Value` |
| `next_run_at_ms` | `Option<i64>` |
| `last_run_at_ms` | `Option<i64>` |
| `last_status` | `Option<String>` |
| `last_error` | `Option<String>` |
| `created_at_ms` | `i64` |
| `updated_at_ms` | `i64` |
| `delete_after_run` | `bool` |

#### UsageRecordRow

**File:** `crates/storage/src/rows/usage.rs`, line 10

| Field | Type |
|-------|------|
| `id` | `Uuid` |
| `timestamp` | `DateTime<Utc>` |
| `request_id` | `String` |
| `model` | `String` |
| `provider` | `String` |
| `prompt_tokens` | `i32` |
| `completion_tokens` | `i32` |
| `cache_read_tokens` | `i32` |
| `cache_write_tokens` | `i32` |
| `estimated_cost_usd` | `f64` |
| `channel` | `String` |
| `strategy` | `String` |

#### CalendarSyncStateRow

**File:** `crates/storage/src/rows/calendar.rs`, line 10

| Field | Type |
|-------|------|
| `provider_id` | `String` |
| `sync_token` | `Option<String>` |
| `last_sync_at` | `Option<DateTime<Utc>>` |

#### CalendarEventCacheRow

**File:** `crates/storage/src/rows/calendar.rs`, line 19

| Field | Type |
|-------|------|
| `uid` | `String` |
| `provider_id` | `String` |
| `summary` | `String` |
| `description` | `Option<String>` |
| `start_at` | `DateTime<Utc>` |
| `end_at` | `DateTime<Utc>` |
| `source` | `String` |
| `etag` | `Option<String>` |
| `status` | `Option<String>` |
| `cached_at` | `DateTime<Utc>` |

#### OutcomeRow

**File:** `crates/storage/src/rows/learning.rs`, line 11

| Field | Type |
|-------|------|
| `id` | `String` |
| `session_key` | `String` |
| `tool_name` | `String` |
| `success` | `bool` |
| `error_category` | `Option<String>` |
| `duration_ms` | `i64` |
| `confidence_score` | `Option<f32>` |
| `confidence_dimensions` | `Option<serde_json::Value>` |
| `execution_mode` | `serde_json::Value` |
| `created_at` | `DateTime<Utc>` |

#### StrategyRecordRow

**File:** `crates/storage/src/rows/learning.rs`, line 27

| Field | Type |
|-------|------|
| `id` | `Uuid` |
| `timestamp` | `DateTime<Utc>` |
| `request_id` | `String` |
| `predicted_strategy` | `String` |
| `actual_strategy` | `String` |
| `escalation_count` | `i32` |
| `iterations_used` | `i32` |
| `max_iterations` | `i32` |
| `success` | `bool` |
| `user_satisfaction` | `Option<f32>` |
| `response_time_ms` | `i64` |
| `chat_id` | `Option<String>` |
| `tool_name` | `Option<String>` |
| `tool_success` | `Option<bool>` |
| `tool_duration_ms` | `Option<i64>` |

#### EnrichmentFeedbackRow

**File:** `crates/storage/src/rows/learning.rs`, line 48

| Field | Type |
|-------|------|
| `id` | `i32` |
| `task_id` | `String` |
| `field` | `String` |
| `suggested_value` | `String` |
| `actual_value` | `Option<String>` |
| `accepted` | `bool` |
| `confidence` | `f64` |
| `timestamp` | `DateTime<Utc>` |

#### LearningStateRow

**File:** `crates/storage/src/rows/learning.rs`, line 62

| Field | Type |
|-------|------|
| `key` | `String` |
| `value` | `serde_json::Value` |
| `updated_at` | `DateTime<Utc>` |

#### StrategySummaryRow

**File:** `crates/storage/src/rows/learning.rs`, line 71

| Field | Type |
|-------|------|
| `predicted_strategy` | `String` |
| `sample_count` | `i64` |
| `correct_count` | `i64` |
| `avg_escalations` | `f32` |

#### DecisionLogRow

**File:** `crates/storage/src/rows/learning.rs`, line 81

| Field | Type |
|-------|------|
| `id` | `String` |
| `session_key` | `String` |
| `iteration` | `i32` |
| `tool_names` | `serde_json::Value` |
| `user_message_preview` | `String` |
| `assessment` | `serde_json::Value` |
| `outcome` | `Option<String>` |
| `created_at` | `DateTime<Utc>` |

#### MemoryNoteRow

**File:** `crates/storage/src/rows/memory.rs`, line 13

| Field | Type |
|-------|------|
| `note_key` | `String` |
| `content` | `String` |
| `created_at` | `DateTime<Utc>` |
| `updated_at` | `DateTime<Utc>` |

#### AgentTaskRow

**File:** `crates/storage/src/rows/agent_task.rs`, line 10

| Field | Type | Notes |
|-------|------|-------|
| `id` | `String` | |
| `session_key` | `String` | |
| `description` | `String` | |
| `status` | `String` | pending, claimed, running, completed, failed |
| `owner_agent_id` | `Option<String>` | |
| `parent_task_id` | `Option<String>` | |
| `result` | `Option<String>` | |
| `error` | `Option<String>` | |
| `blocked_by` | `String` | JSON array of task IDs |
| `created_at` | `DateTime<Utc>` | |
| `updated_at` | `DateTime<Utc>` | |

#### Finance Row Structs

All defined in `crates/storage/src/rows/finance.rs`.

**FinanceAccountRow** (line 17): id, name, account_type, currency, balance (i64), institution, notes, is_archived (bool), created_at, updated_at

**FinanceTransactionRow** (line 33): id, account_id, tx_type, amount (i64), currency, category, subcategory, counterparty, notes, tx_date (NaiveDate), transfer_id, is_recurring (bool), recurring_rule, created_at, updated_at

**FinanceBudgetRow** (line 54): id, name, amount (i64), currency, period, category, method, jar_type, start_date (NaiveDate), end_date, is_active (bool), alert_threshold (i32), created_at, updated_at

**FinancePortfolioRow** (line 74): id, name, description, currency, created_at, updated_at

**FinanceInvestmentRow** (line 86): id, portfolio_id, asset_type, symbol, name, quantity (f64), cost_basis (i64), currency, current_price, current_value, purchase_date (NaiveDate), notes, created_at, updated_at

**FinanceInvestmentTxRow** (line 106): id, investment_id, tx_type, quantity (f64), price_per_unit, total_amount (i64), currency, fees (i64), tx_date (NaiveDate), notes, created_at

**FinanceGoalRow** (line 123): id, name, goal_type, target_amount (i64), current_amount (i64), currency, status, deadline (NaiveDate), monthly_contribution, expected_return_rate (f64), inflation_rate (f64), notes, created_at, updated_at

**FinanceLiabilityRow** (line 143): id, name, liability_type, principal (i64), remaining (i64), currency, interest_rate (f64), monthly_payment, due_date (NaiveDate), notes, created_at, updated_at

**BudgetUsageRow** (line 272): All FinanceBudgetRow fields + `spent` (i64, sum of matching expenses in current period)

**PortfolioSummaryRow** (line 294): portfolio_id, total_cost_basis (i64), total_current_value (i64), holding_count (i64)

---

### VectorStore

**File:** `crates/storage/src/vector_store.rs`, line 24

| Method | Signature | Description |
|--------|-----------|-------------|
| `connect` | `async fn connect(data_dir: &Path) -> Result<Self, StorageError>` | Open/create `{data_dir}/lancedb/`, ensure all 3 tables exist |
| `upsert_embedding` | `async fn upsert_embedding(&self, table: &str, id: &str, vector: &[f32], extra_fields: &[(&str, &str)]) -> Result<(), StorageError>` | Delete-then-insert upsert; extra_fields in schema column order |
| `search_similar` | `async fn search_similar(&self, table: &str, query: &[f32], limit: usize, threshold: f64) -> Result<Vec<(String, f64)>, StorageError>` | ANN search returning (id, score) pairs where score >= threshold |
| `delete` | `async fn delete(&self, table: &str, id: &str) -> Result<(), StorageError>` | Delete embedding by ID |
| `delete_where` | `async fn delete_where(&self, table: &str, predicate: &str) -> Result<(), StorageError>` | Delete by SQL predicate |
| `search_conv_embeddings` | `async fn search_conv_embeddings(&self, query: &[f32], limit: usize, threshold: f64) -> Result<Vec<(String, String, String, String, String, f64)>, StorageError>` | Specialized conv search returning (id, session_key, role, content_preview, full_content, score) |
| `count` | `async fn count(&self, table: &str) -> Result<usize, StorageError>` | Row count for a table |

---

### Migration Files

| File | Path | Tables Created/Modified |
|------|------|------------------------|
| `001_initial.sql` | `crates/storage/migrations/001_initial.sql` | Creates all 25+ baseline tables: projects, todos, todo_attachments, todo_time_entries, todo_dependencies, sessions, session_messages, goals, goal_project_links, plans, plan_steps, learning_outcomes, strategy_records, enrichment_feedback, usage_records, cron_jobs, calendar_sync_state, memory_notes, learning_state, decision_log, calendar_event_cache, finance_accounts, finance_transactions, finance_budgets, finance_portfolios, finance_investments, finance_investment_transactions, finance_goals, finance_liabilities, _feature_migrations |
| `002_learning_loop.sql` | `crates/storage/migrations/002_learning_loop.sql` | ALTER strategy_records (add chat_id), ALTER goals (add plans_completed, plans_failed, avg_duration_ms, last_plan_at) |
| `003_strategy_tool_columns.sql` | `crates/storage/migrations/003_strategy_tool_columns.sql` | ALTER strategy_records (add tool_name, tool_success, tool_duration_ms) |
| `004_intent_pipeline.sql` | `crates/storage/migrations/004_intent_pipeline.sql` | ALTER plans (add visibility, task_id), ALTER strategy_records (add complexity_signals, execution_mode) |
| `005_agent_tasks.sql` | `crates/storage/migrations/005_agent_tasks.sql` | CREATE agent_tasks, CREATE tool_usage |
