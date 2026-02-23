# SQLite + LanceDB Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace PostgreSQL+pgvector with SQLite+LanceDB for zero-infrastructure deployment.

**Architecture:** SQLite (via sqlx) handles all 29 relational tables. LanceDB (native Rust crate) handles all 3 vector embedding tables. fastembed stays unchanged for embedding generation. The `EmbeddingHandler` trait provides the abstraction boundary.

**Tech Stack:** sqlx (SQLite driver), lancedb 0.26, arrow/arrow-array/arrow-schema 53, fastembed 5 (unchanged)

**Design doc:** `docs/plans/2026-02-23-lancedb-sqlite-migration-design.md`

---

## Phase 1: Foundation (Dependencies, Pool, Schema)

### Task 1: Update Cargo dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root, lines 62, 98-99)
- Modify: `crates/storage/Cargo.toml` (line 10, remove pgvector, add lancedb/arrow)
- Modify: `crates/tools/Cargo.toml` (line 37, remove pgvector)
- Modify: `crates/feature-todo/Cargo.toml` (line 20, remove pgvector)

**Step 1: Update workspace root Cargo.toml**

Change sqlx features from `postgres` to `sqlite`:
```toml
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "sqlite", "migrate", "uuid", "chrono", "json"] }
```

Remove pgvector:
```toml
# DELETE this line:
# pgvector = { version = "0.4", features = ["sqlx"] }
```

Add LanceDB and Arrow:
```toml
lancedb = "0.26"
arrow = { version = "53", features = ["prettyprint"] }
arrow-array = "53"
arrow-schema = "53"
```

**Step 2: Update crate-level Cargo.toml files**

`crates/storage/Cargo.toml`: Remove `pgvector.workspace = true`, add `lancedb.workspace = true`, `arrow.workspace = true`, `arrow-array.workspace = true`, `arrow-schema.workspace = true`.

`crates/tools/Cargo.toml`: Remove `pgvector.workspace = true`.

`crates/feature-todo/Cargo.toml`: Remove `pgvector.workspace = true`.

**Step 3: Verify it compiles (it won't yet — that's expected)**

Run: `cargo check --workspace 2>&1 | head -20`
Expected: Compilation errors from removed pgvector types. This confirms the dependency removal is working.

**Step 4: Commit**

```bash
git add Cargo.toml crates/*/Cargo.toml
git commit -m "build: switch sqlx to sqlite, remove pgvector, add lancedb+arrow"
```

---

### Task 2: Replace PostgreSQL migrations with SQLite schema

**Files:**
- Delete: All files in `crates/storage/migrations/` (13 .sql files)
- Create: `crates/storage/migrations/001_initial.sql`

**Step 1: Delete all existing PostgreSQL migrations**

Remove all files in `crates/storage/migrations/`:
- `20240101000000_initial.sql`
- `20240101000001_pgvector.sql`
- `20260219000000_memory_and_learning_state.sql`
- `20260219000001_decision_log.sql`
- `20260219000002_session_message_format.sql`
- `20260219000003_calendar_event_cache.sql`
- `20260219100000_finance_tables.sql`
- `20260220000000_feature_migration_tracking.sql`
- `20260222000000_hnsw_indexes.sql`
- `20260222000001_conv_embedding_full_content.sql`
- `20260222000002_history_summaries.sql`
- `20260223000001_memory_note_embeddings.sql`
- `20260223000002_drop_history_summaries.sql`

**Step 2: Create single SQLite baseline migration**

Create `crates/storage/migrations/001_initial.sql` containing all 29 tables in SQLite dialect.

Key conversions from the existing `_initial.sql`:
- `UUID PRIMARY KEY DEFAULT gen_random_uuid()` → `TEXT PRIMARY KEY` (UUID generated in Rust)
- `TIMESTAMPTZ` → `TEXT` (ISO 8601 strings)
- `SERIAL PRIMARY KEY` → `INTEGER PRIMARY KEY AUTOINCREMENT`
- `TEXT[]` → `TEXT` (JSON array string)
- `JSONB` → `TEXT` (JSON string)
- `BOOLEAN NOT NULL DEFAULT false` → `INTEGER NOT NULL DEFAULT 0`
- `DATE` → `TEXT`
- `BIGINT` → `INTEGER`
- `DOUBLE PRECISION` → `REAL`
- `NUMERIC(15,2)` → `REAL`

Tables to include (all from initial + later migrations, consolidated):
1. `projects`
2. `todos`
3. `todo_attachments`
4. `todo_time_entries`
5. `todo_dependencies`
6. `sessions`
7. `session_messages` (include `tool_calls TEXT`, `metadata TEXT` columns from `_session_message_format.sql`)
8. `goals`
9. `goal_project_links`
10. `plans`
11. `plan_steps`
12. `learning_outcomes`
13. `strategy_records`
14. `enrichment_feedback`
15. `usage_records`
16. `cron_jobs`
17. `calendar_sync_state`
18. `memory_notes` (from `_memory_and_learning_state.sql`)
19. `learning_state` (from `_memory_and_learning_state.sql`)
20. `decision_log` (from `_decision_log.sql`)
21. `calendar_event_cache` (from `_calendar_event_cache.sql`)
22. `finance_accounts` (from `_finance_tables.sql`)
23. `finance_transactions`
24. `finance_budgets`
25. `finance_portfolios`
26. `finance_investments`
27. `finance_investment_transactions`
28. `finance_goals`
29. `finance_liabilities`
30. `_feature_migrations` (from `_feature_migration_tracking.sql`)

**Important:** Read each existing PG migration file to faithfully translate every column, constraint, index, and default. Don't miss columns added by later ALTER TABLE migrations.

Enable foreign keys at start of migration:
```sql
PRAGMA foreign_keys = ON;
```

**Step 3: Commit**

```bash
git add crates/storage/migrations/
git commit -m "schema: replace PG migrations with single SQLite baseline"
```

---

### Task 3: Rewrite StoragePool for SQLite

**Files:**
- Modify: `crates/storage/src/pool.rs` (L1-L86)
- Modify: `crates/storage/src/error.rs` (if StorageError references PG-specific types)

**Step 1: Rewrite StoragePool**

```rust
use std::path::Path;
use crate::error::StorageError;

#[derive(Clone)]
pub struct StoragePool(sqlx::SqlitePool);

impl StoragePool {
    pub async fn connect(data_dir: &Path) -> Result<Self, StorageError> {
        let db_path = data_dir.join("data.db");
        // Create parent dirs if needed
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                StorageError::Migration(format!("Failed to create data dir: {}", e))
            })?;
        }
        let url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = sqlx::SqlitePool::connect(&url).await?;
        // Enable WAL mode and foreign keys
        sqlx::query("PRAGMA journal_mode=WAL;").execute(&pool).await?;
        sqlx::query("PRAGMA foreign_keys=ON;").execute(&pool).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self(pool))
    }

    pub fn from_existing(pool: sqlx::SqlitePool) -> Self {
        Self(pool)
    }

    pub fn inner(&self) -> &sqlx::SqlitePool {
        &self.0
    }

    pub async fn run_feature_migrations(
        pool: &sqlx::SqlitePool,
        migrations: &[tools_core::FeatureMigration],
    ) -> Result<(), StorageError> {
        for m in migrations {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM _feature_migrations WHERE feature_name = ?1 AND version = ?2)",
            )
            .bind(&m.feature_name)
            .bind(m.version)
            .fetch_one(pool)
            .await?;

            if !exists {
                tracing::info!(feature = %m.feature_name, version = m.version, "Running feature migration");
                sqlx::query(&m.sql).execute(pool).await?;
                sqlx::query(
                    "INSERT INTO _feature_migrations (feature_name, version, description) VALUES (?1, ?2, ?3)",
                )
                .bind(&m.feature_name)
                .bind(m.version)
                .bind(&m.description)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }
}
```

**Step 2: Update StorageError if needed**

Check `crates/storage/src/error.rs` — if `StorageError::Sqlx` wraps `sqlx::Error` it should be generic enough. Verify no PG-specific error types.

**Step 3: Commit**

```bash
git commit -m "refactor(storage): rewrite StoragePool for SQLite"
```

---

### Task 4: Create VectorStore (LanceDB wrapper)

**Files:**
- Create: `crates/storage/src/vector_store.rs`
- Modify: `crates/storage/src/lib.rs` (add module + re-export)

**Step 1: Create VectorStore**

Create `crates/storage/src/vector_store.rs` with:

```rust
use std::path::Path;
use std::sync::Arc;
use arrow_array::{RecordBatch, RecordBatchIterator, StringArray, Float32Array, FixedSizeListArray, ArrayRef};
use arrow_schema::{Schema, Field, DataType};
use lancedb::query::{ExecutableQuery, QueryBase};
use crate::error::StorageError;

pub struct VectorStore {
    db: lancedb::Connection,
}
```

Implement methods:
- `connect(data_dir: &Path)` — opens `{data_dir}/lance/`
- `ensure_table(name: &str, schema: Schema)` — creates table if not exists
- `upsert_embedding(table: &str, id: &str, vector: &[f32], metadata: &[(&str, &str)])` — upsert by ID
- `search_similar(table: &str, query: &[f32], limit: usize, threshold: f64)` → `Vec<(String, f64)>`
- `delete(table: &str, id: &str)` — delete by ID
- `count(table: &str)` → `usize`

Three table schemas:
- `todo_embeddings`: id (Utf8), vector (FixedSizeList<Float32, 384>), model (Utf8), updated_at (Utf8)
- `conv_embeddings`: id (Utf8), session_key (Utf8), vector (FixedSizeList<Float32, 384>), role (Utf8), content_preview (Utf8), full_content (Utf8), created_at (Utf8)
- `memory_note_embeddings`: memory_note_id (Utf8), vector (FixedSizeList<Float32, 384>), updated_at (Utf8)

**Step 2: Write tests for VectorStore**

Test in `vector_store.rs` `#[cfg(test)] mod tests`:
- `test_connect_creates_directory`
- `test_upsert_and_search`
- `test_delete_removes_entry`
- `test_search_threshold_filters`

Use `tempdir` for test isolation.

**Step 3: Add module to lib.rs**

In `crates/storage/src/lib.rs`, add:
```rust
pub mod vector_store;
pub use vector_store::VectorStore;
```

**Step 4: Run tests**

Run: `cargo nextest run -p storage --lib`

**Step 5: Commit**

```bash
git commit -m "feat(storage): add VectorStore backed by LanceDB"
```

---

## Phase 2: Repo Rewrites (SQLite dialect)

### Task 5: Update Repos aggregate and row structs

**Files:**
- Modify: `crates/storage/src/repos/mod.rs` (L54-L122)
- Modify: `crates/storage/src/rows/mod.rs`
- Delete: `crates/storage/src/rows/embedding.rs`
- Modify: All row struct files to remove `pgvector::Vector` usage

**Step 1: Update Repos struct**

Change pool type from `sqlx::PgPool` to `sqlx::SqlitePool`. Remove the 3 embedding repos:

```rust
pub struct Repos {
    pool: sqlx::SqlitePool,
    pub todos: TodoRepo,
    pub projects: ProjectRepo,
    pub sessions: SessionRepo,
    pub goals: GoalRepo,
    pub plans: PlanRepo,
    pub outcomes: OutcomeRepo,
    pub strategies: StrategyRepo,
    pub usage: UsageRepo,
    pub cron: CronRepo,
    pub calendar_sync: CalendarSyncRepo,
    pub calendar_event_cache: CalendarEventCacheRepo,
    pub memory_notes: MemoryNoteRepo,
    pub learning_state: LearningStateRepo,
    pub decision_log: DecisionLogRepo,
    pub finance_accounts: FinanceAccountRepo,
    pub finance_transactions: FinanceTransactionRepo,
    pub finance_budgets: FinanceBudgetRepo,
    pub finance_investments: FinanceInvestmentRepo,
    pub finance_goals: FinanceGoalRepo,
    pub finance_liabilities: FinanceLiabilityRepo,
    // REMOVED: embeddings, conv_embeddings, memory_note_embeddings
}
```

Update `from_pool()` and `pool()` to use `SqlitePool`.

**Step 2: Delete embedding row structs**

Delete `crates/storage/src/rows/embedding.rs`. Remove its `pub mod embedding;` from `crates/storage/src/rows/mod.rs`.

**Step 3: Update remaining row structs**

All row structs that derive `sqlx::FromRow` should work unchanged since we're swapping the driver — sqlx handles TEXT↔String, TEXT↔DateTime<Utc> mappings. But check each file in `rows/` to ensure no PG-specific types remain.

**Step 4: Update lib.rs re-exports**

Remove re-exports for `EmbeddingRepo`, `ConvEmbeddingRepo`, `MemoryNoteEmbeddingRepo`, `EmbeddingRow`, `ConvEmbeddingRow`. Add `VectorStore` export.

**Step 5: Commit**

```bash
git commit -m "refactor(storage): update Repos aggregate for SQLite, remove embedding repos"
```

---

### Task 6: Rewrite simple repos (8 repos)

These repos have straightforward SQL that mainly needs bind param changes (`$N` → `?N`) and timestamp function changes.

**Files (8 repos):**
- Modify: `crates/storage/src/repos/usage.rs`
- Modify: `crates/storage/src/repos/cron.rs`
- Modify: `crates/storage/src/repos/calendar_sync.rs`
- Modify: `crates/storage/src/repos/calendar_event_cache.rs`
- Modify: `crates/storage/src/repos/strategy.rs`
- Modify: `crates/storage/src/repos/outcome.rs`
- Modify: `crates/storage/src/repos/learning_state.rs`
- Modify: `crates/storage/src/repos/decision_log.rs`

**Step 1: For each repo, apply these mechanical changes:**

1. Change `PgPool` → `SqlitePool` in struct definition and `new()`
2. Change all `$1, $2, $3` → `?1, ?2, ?3` in SQL strings
3. Change `now()` or `CURRENT_TIMESTAMP` → `datetime('now')` in SQL
4. Change `gen_random_uuid()` → generate UUID in Rust before INSERT, bind as `?N`
5. Change `EXTRACT(EPOCH FROM ...)` → compute in Rust or use `unixepoch()`
6. Change any `INTERVAL` → `datetime('now', '-N days')`
7. Change `ILIKE` → `LIKE` (SQLite LIKE is case-insensitive for ASCII by default)
8. Change `QueryBuilder::<sqlx::Postgres>` → `QueryBuilder::<sqlx::Sqlite>`

**Step 2: Run cargo check after each repo**

Run: `cargo check -p storage 2>&1 | head -20`

**Step 3: Commit after all 8 are done**

```bash
git commit -m "refactor(storage): rewrite 8 simple repos for SQLite dialect"
```

---

### Task 7: Rewrite medium repos (SessionRepo, GoalRepo, PlanRepo, MemoryNoteRepo)

These have JSONB columns that need attention.

**Files:**
- Modify: `crates/storage/src/repos/session.rs`
- Modify: `crates/storage/src/repos/goal.rs`
- Modify: `crates/storage/src/repos/plan.rs`
- Modify: `crates/storage/src/repos/memory_note.rs`

**Step 1: Apply same mechanical changes as Task 6**

**Step 2: Handle JSONB columns**

JSONB columns (`metadata`, `metrics`, `tool_calls`, `assessment`, `schedule`, `payload`, `backtrack_history`) become `TEXT` in SQLite. sqlx's `Json<T>` wrapper handles serialization automatically. If any queries use PostgreSQL JSON operators (`->`, `->>`, `@>`), rewrite using `json_extract()`:

```sql
-- PostgreSQL: metadata->>'key'
-- SQLite: json_extract(metadata, '$.key')
```

Most JSONB is stored/retrieved as opaque blobs (serialize in Rust, bind as text). Verify each repo.

**Step 3: Handle session CTE**

`SessionRepo::add_message()` uses `WITH touch AS (UPDATE ... RETURNING ...) INSERT INTO ...`. SQLite supports CTEs with INSERT/UPDATE, but verify the RETURNING syntax works (SQLite 3.35+).

**Step 4: Commit**

```bash
git commit -m "refactor(storage): rewrite session/goal/plan/memory repos for SQLite"
```

---

### Task 8: Rewrite complex repos (TodoRepo, ProjectRepo)

**Files:**
- Modify: `crates/storage/src/repos/todo_repo.rs` (874 lines, most complex)
- Modify: `crates/storage/src/repos/project_repo.rs`

**Step 1: TodoRepo — apply mechanical changes**

Same as Task 6 changes, plus:
- `TEXT[]` columns (tags): Change `@>` array containment to JSON `LIKE '%"tag"%'` or `json_each()` + subquery
- `ANY($1)` array binding in `get_by_ids`: Change to `WHERE id IN (...)` using `QueryBuilder`
- `EXTRACT(EPOCH FROM (now() - started_at))::BIGINT` in `close_time_entry()`: Compute in Rust or use `(unixepoch('now') - unixepoch(started_at))`
- `WITH RECURSIVE` CTEs: Same syntax in SQLite, should work
- `QueryBuilder::<sqlx::Postgres>` → `QueryBuilder::<sqlx::Sqlite>`
- `ILIKE` → `LIKE`

**Step 2: Add `estimated_minutes` to TodoPatch**

Consolidate from the feature-todo duplicate:
```rust
pub struct TodoPatch {
    // ... existing fields ...
    pub estimated_minutes: Option<Option<i32>>,  // ADD THIS
}
```

Update the `update()` SQL to include the new field.

**Step 3: ProjectRepo — apply same changes**

`QueryBuilder` and standard SQL changes.

**Step 4: Commit**

```bash
git commit -m "refactor(storage): rewrite TodoRepo and ProjectRepo for SQLite"
```

---

### Task 9: Rewrite finance repos (6 repos)

**Files:**
- Modify: `crates/storage/src/repos/finance_account_repo.rs`
- Modify: `crates/storage/src/repos/finance_transaction_repo.rs`
- Modify: `crates/storage/src/repos/finance_budget_repo.rs`
- Modify: `crates/storage/src/repos/finance_investment_repo.rs`
- Modify: `crates/storage/src/repos/finance_goal_repo.rs`
- Modify: `crates/storage/src/repos/finance_liability_repo.rs`

**Step 1: Apply mechanical changes (same as Task 6)**

**Step 2: Handle finance-specific PostgreSQL functions**

- `DATE_TRUNC('month', date)` → `strftime('%Y-%m-01', date)`
- `DATE_TRUNC('year', date)` → `strftime('%Y-01-01', date)`
- `INTERVAL '30 days'` → `datetime('now', '-30 days')`
- `NUMERIC(15,2)` → `REAL` (already changed in schema)
- `CURRENT_DATE` → `date('now')`

**Step 3: Commit**

```bash
git commit -m "refactor(storage): rewrite 6 finance repos for SQLite"
```

---

### Task 10: Delete embedding repos and duplicate TodoRepo

**Files:**
- Delete: `crates/storage/src/repos/embedding.rs`
- Delete: `crates/storage/src/repos/conv_embedding.rs`
- Delete: `crates/storage/src/repos/memory_note_embedding.rs`
- Delete: `crates/feature-todo/src/storage/repo.rs` (duplicate TodoRepo)
- Modify: `crates/feature-todo/src/storage/mod.rs` (remove repo module)
- Modify: `crates/storage/src/repos/mod.rs` (remove embedding modules)

**Step 1: Delete the 3 embedding repo files**

These are replaced by `VectorStore`.

**Step 2: Delete feature-todo duplicate TodoRepo**

Remove `crates/feature-todo/src/storage/repo.rs`. Update `feature-todo` to use `storage::TodoRepo` instead.

**Step 3: Update mod.rs files**

Remove `pub mod embedding;`, `pub mod conv_embedding;`, `pub mod memory_note_embedding;` from `repos/mod.rs`.

**Step 4: Commit**

```bash
git commit -m "refactor: delete pgvector embedding repos and duplicate TodoRepo"
```

---

## Phase 3: Consumer Rewrites

### Task 11: Rewire embedding pipeline to LanceDB

**Files:**
- Modify: `crates/tools/src/embedding_engine.rs` (L194-L279 — `EmbeddingEngineImpl`)
- Modify: `crates/tools/src/embedding_store.rs` (remove pgvector::Vector usage)
- Modify: `crates/tools/src/conversation_embedding.rs` (remove pgvector::Vector usage)
- Modify: `crates/agent/src/todo_embedding_handler.rs`
- Modify: `crates/agent/src/conversation_embedding_handler.rs`
- Modify: `crates/agent/src/memory_maintenance_service.rs`

**Step 1: Update EmbeddingEngineImpl**

Change from `repo: storage::EmbeddingRepo` to `store: storage::VectorStore`:

```rust
pub struct EmbeddingEngineImpl {
    engine: Arc<EmbeddingEngine>,
    store: VectorStore,
}

impl EmbeddingEngineImpl {
    pub fn new(engine: Arc<EmbeddingEngine>, store: VectorStore) -> Self {
        Self { engine, store }
    }
}
```

Update `embed_todo()` to call `store.upsert_embedding("todo_embeddings", ...)` instead of `repo.upsert_vec()`.

Update `embed_query()` — no storage change needed (just returns Vec<f32>).

**Step 2: Update embedding_store.rs and conversation_embedding.rs**

Remove all `pgvector::Vector` imports and usage. These files should use `VectorStore` or `&[f32]` directly.

**Step 3: Update agent-side handlers**

`todo_embedding_handler.rs`, `conversation_embedding_handler.rs`, `memory_maintenance_service.rs` — update construction to pass `VectorStore` instead of embedding repos.

**Step 4: Ensure EmbeddingHandler trait is unchanged**

The trait at `embedding_engine.rs:L178-L188` should NOT change — consumers of the trait (tools, search) are unaffected.

**Step 5: Commit**

```bash
git commit -m "refactor: rewire embedding pipeline to LanceDB VectorStore"
```

---

### Task 12: Update agent_loop builder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs` (L53-L747)
- Modify: `crates/agent/src/agent_loop/mod.rs`
- Modify: `crates/agent/src/lib.rs`

**Step 1: Change pool type**

`AgentLoopBuilder`:
- `pool: Option<sqlx::PgPool>` → `pool: Option<sqlx::SqlitePool>`
- `with_pool(pool: sqlx::PgPool)` → `with_pool(pool: sqlx::SqlitePool)`
- Remove lazy fallback at L139-L142 (`postgres://localhost/klyntbot`)

**Step 2: Add VectorStore to builder**

```rust
vector_store: Option<VectorStore>,
pub fn with_vector_store(mut self, store: VectorStore) -> Self {
    self.vector_store = Some(store);
    self
}
```

**Step 3: Update build() method**

Where it currently creates `Repos::from_pool()`, ensure it passes `SqlitePool`. Where it constructs embedding handlers, pass `VectorStore` instead of embedding repos.

**Step 4: Commit**

```bash
git commit -m "refactor(agent): update AgentLoopBuilder for SQLite + VectorStore"
```

---

### Task 13: Update config schema

**Files:**
- Modify: `crates/config/src/schema/core.rs` (L123-L125)
- Modify: `crates/config/src/schema/mod.rs` (if needed)

**Step 1: Replace database_url with data_dir**

```rust
// BEFORE
pub database_url: Option<String>,

// AFTER
pub data_dir: Option<String>,
```

Default value: `~/.klyntbot` (expanded at runtime).

**Step 2: Remove KLYNTBOT_DATABASE_URL env var handling if custom**

Check if there's special env var parsing. The standard `KLYNTBOT_` prefix with `__` separator should auto-map `KLYNTBOT_DATA_DIR`.

**Step 3: Commit**

```bash
git commit -m "refactor(config): replace database_url with data_dir"
```

---

### Task 14: Update CLI (chat, serve, init, status commands)

**Files:**
- Modify: `crates/cli/src/chat.rs`
- Modify: `crates/cli/src/serve.rs`
- Modify: `crates/cli/src/status.rs`
- Modify: `crates/cli/src/wizard/` (init wizard — remove database setup)

**Step 1: Update chat.rs and serve.rs**

Replace:
```rust
let pool = StoragePool::connect(&config.database_url.unwrap_or(...)).await?;
```
With:
```rust
let data_dir = config.data_dir_path(); // resolve ~, default to ~/.klyntbot
let pool = StoragePool::connect(&data_dir).await?;
let vector_store = VectorStore::connect(&data_dir).await?;
```

**Step 2: Update status command**

Replace database connection check with data dir existence check:
```rust
// Check ~/.klyntbot/data.db exists
// Check ~/.klyntbot/lance/ exists
```

**Step 3: Update init wizard**

Remove database setup step entirely. Remove any prompts for `database_url`. The wizard should create `~/.klyntbot/` directory if it doesn't exist.

**Step 4: Commit**

```bash
git commit -m "refactor(cli): update commands for SQLite + LanceDB"
```

---

### Task 15: Update feature-todo crate

**Files:**
- Modify: `crates/feature-todo/src/lib.rs`
- Modify: `crates/feature-todo/src/embedding.rs`
- Modify: `crates/feature-todo/src/tool/actions/search.rs`
- Modify: `crates/feature-todo/src/tool/actions/add.rs`
- Modify: `crates/feature-todo/src/tool/actions/update.rs`
- Modify: `crates/feature-todo/src/tool/actions/delete.rs`
- Modify: `crates/feature-todo/src/tool/mod.rs`
- Delete: `crates/feature-todo/src/storage/` (duplicate repo directory)

**Step 1: Remove duplicate storage layer**

Delete the entire `crates/feature-todo/src/storage/` directory. Update `feature-todo` to import from `storage::TodoRepo` instead.

**Step 2: Update embedding integration**

Replace any `EmbeddingRepo` or `pgvector` references with `VectorStore` or `EmbeddingHandler` trait calls.

**Step 3: Update search action**

The semantic search in `search.rs` should use `VectorStore::search_similar()` instead of `EmbeddingRepo::search_similar_vec()`.

**Step 4: Commit**

```bash
git commit -m "refactor(feature-todo): use storage crate repos, remove duplicate"
```

---

## Phase 4: Tests & Cleanup

### Task 16: Rewrite test infrastructure

**Files:**
- Modify: `tests/common/mod.rs` (remove PG pool setup)
- Modify: `tests/test_utils/mod.rs`
- Modify: `tests/test_utils/embedding.rs` (remove pgvector types)
- Modify: All integration tests in `tests/`
- Delete: Any PG-specific test fixtures

**Step 1: Update test helpers**

Replace PG pool creation with SQLite in-memory:
```rust
pub async fn test_pool() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./crates/storage/migrations").run(&pool).await.unwrap();
    sqlx::query("PRAGMA foreign_keys=ON;").execute(&pool).await.unwrap();
    pool
}

pub async fn test_vector_store() -> VectorStore {
    let dir = tempfile::tempdir().unwrap();
    VectorStore::connect(dir.path()).await.unwrap()
}
```

**Step 2: Remove all hardcoded postgres:// URLs**

Search and remove from:
- `crates/agent/src/agent_loop/builder.rs` (~L139)
- `tests/common/mod.rs`
- Any other files with `postgres://localhost/klyntbot`

**Step 3: Update mock embedding handlers**

`tests/mock_embedding_handler.rs` and `tests/mock_conversation_embedding_handler.rs` — these mock the trait, not PG directly. Should need minimal changes.

**Step 4: Run full test suite**

Run: `cargo nextest run --workspace`

**Step 5: Commit**

```bash
git commit -m "test: rewrite test infra for SQLite + LanceDB"
```

---

### Task 17: Update documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md` (if it exists and has PG references)

**Step 1: Update CLAUDE.md**

- Remove "PostgreSQL with pgvector is required" from Gotchas
- Remove `DATABASE_URL` from Environment Variables
- Add `DATA_DIR` to Environment Variables
- Update "Database requirement" note in Build & Test section
- Remove pgvector references from Architecture section
- Update config schema example

**Step 2: Commit**

```bash
git commit -m "docs: update CLAUDE.md for SQLite + LanceDB"
```

---

### Task 18: Final cleanup and verification

**Files:**
- All workspace files

**Step 1: Search for any remaining PostgreSQL references**

```bash
rg -i "postgres\|pgvector\|PgPool\|TIMESTAMPTZ\|JSONB\|DATABASE_URL" --type rust
```

Fix any stragglers.

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 3: Run fmt**

Run: `cargo fmt --all --check`

**Step 4: Run full test suite**

Run: `cargo nextest run --workspace`

**Step 5: Run doctests**

Run: `cargo test --workspace --doc`

**Step 6: Final commit**

```bash
git commit -m "chore: final cleanup — remove all PostgreSQL remnants"
```

---

## Summary

| Phase | Tasks | Focus |
|-------|-------|-------|
| 1: Foundation | Tasks 1-4 | Dependencies, schema, pool, VectorStore |
| 2: Repos | Tasks 5-10 | Rewrite all 23 repos for SQLite, delete PG embedding repos |
| 3: Consumers | Tasks 11-15 | Agent builder, config, CLI, feature crates |
| 4: Cleanup | Tasks 16-18 | Tests, docs, final verification |

**Total: 18 tasks across 4 phases.**
