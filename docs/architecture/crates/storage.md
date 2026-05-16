# Crate: `storage`

> **Status:** 🟢 Stable
> **Subsystem:** [02 — Storage & Persistence](../subsystems/02-storage.md)
> **Status last verified:** 2026-05-16
> **One-liner:** SQLite + LanceDB persistence; the only crate with a `sqlx` dependency outside test code

---

## TL;DR

`storage` is the canonical persistence layer. Every workspace crate that needs to read or write data does so through this crate. Surface area: `StoragePool` (a `Clone + Send + Sync` newtype wrapping `sqlx::SqlitePool` with auto-migration on connect), a `Repos` aggregate holding 30+ per-domain repositories, a `VectorStore` for LanceDB-backed semantic search, and `FinanceStorage` (a facade combining 9 finance repos for atomic multi-table operations).

If you're touching this crate, you are touching the data plane. Get the migration story right, prefer `connect_in_memory()` for tests, and don't reach for `sqlx` outside this crate.

---

## Module map

```
crates/storage/src/
├── lib.rs                  ← Public re-exports (~80 items)
├── pool.rs                 ← StoragePool, DataVersionWatcherHandle, run_feature_migrations
├── error.rs                ← StorageError, OptionExt (Option → Result<_, NotFound>)
├── macros.rs               ← #[macro_use]-imported helpers for repo definitions
├── sqlite_types.rs         ← SqlDate, SqlTs (jiff::Timestamp ↔ TEXT/INTEGER newtypes)
├── circuit_breaker.rs      ← Per-repo circuit breaker (degrades writes after N failures)
├── finance_storage.rs      ← FinanceStorage facade (9 finance repos + atomic helpers)
├── messages/
│   ├── mod.rs              ← Re-exports
│   └── parts.rs            ← MessagePart enum, MessagePartsRow, parts↔content mirroring
├── repos/
│   ├── mod.rs              ← Repos aggregate struct
│   └── <50+ *_repo.rs>     ← One module per repo
├── rows/
│   ├── mod.rs
│   └── <30+ row modules>   ← *Row structs with #[derive(sqlx::FromRow)]
├── vector_store/
│   ├── mod.rs              ← VectorStore + LanceDB integration
│   └── …                   ← CognitiveFactParams query helpers, sanitize_predicate_value
├── test_util.rs            ← #[cfg(test)] only
└── migrations/             ← Built-in SQL migrations (lexicographic order)
    └── NNN_<name>.sql      ← Run automatically on StoragePool::connect
```

---

## Public API surface

### `StoragePool` — the pool primitive

```rust
#[derive(Clone)]
pub struct StoragePool(sqlx::SqlitePool);

impl StoragePool {
    /// Production: connect to {data_dir}/data.db, WAL on, FK on, run all migrations.
    pub async fn connect(data_dir: &Path) -> Result<Self, StorageError>;

    /// Wrap an already-migrated pool (skips migrations).
    pub fn from_existing(pool: sqlx::SqlitePool) -> Self;

    /// Create an in-memory SQLite pool with all migrations applied.
    /// Use this in tests; nextest runs them in parallel.
    pub async fn connect_in_memory() -> Result<Self, StorageError>;

    /// Run feature-owned migrations not in `storage/migrations/`.
    /// Each (feature_name, version) wrapped in a transaction.
    /// Duplicate (feature_name, version) panics with descriptive message.
    pub async fn run_feature_migrations(
        pool: &sqlx::SqlitePool,
        migrations: &[tools_core::FeatureMigration],
    ) -> Result<(), StorageError>;

    /// Access the inner sqlx pool (for repos to use).
    pub fn inner(&self) -> &sqlx::SqlitePool;

    /// Run PRAGMA optimize. Best called on graceful shutdown.
    pub async fn optimize(&self) -> Result<(), StorageError>;
}
```

### `DataVersionWatcherHandle`

```rust
/// Optional background watcher that polls data.db's data_version PRAGMA.
/// Notifies subscribers when another process writes (typically the MCP child).
/// Dropping the handle cancels the watcher (via embedded CancellationToken).
pub struct DataVersionWatcherHandle(CancellationToken);

impl Drop for DataVersionWatcherHandle {
    fn drop(&mut self) { self.0.cancel(); }
}
```

### `Repos` aggregate

```rust
pub struct Repos {
    // Sessions / context (4)
    pub session: SessionRepo,
    pub session_context: SessionContextRepo,
    pub session_memory: SessionMemoryRepo,
    pub subagent_instance: SubagentInstanceRepo,

    // Tasks / projects / OKR (12)
    pub task: TaskRepo,
    pub task_group: TaskGroupRepo,
    pub task_recurrence: TaskRecurrenceRepo,
    pub task_alarms: TaskAlarmsRepo,
    pub project: ProjectRepo,
    pub project_source: ProjectSourceRepo,
    pub area: AreaRepo,
    pub objective: ObjectiveRepo,
    pub key_result: KeyResultRepo,
    pub entity_link: EntityLinkRepo,
    pub custom_column: CustomColumnRepo,
    pub status_workflow: StatusWorkflowRepo,

    // Finance (9 + facade)
    pub finance_account: FinanceAccountRepo,
    pub finance_transaction: FinanceTransactionRepo,
    pub finance_budget: FinanceBudgetRepo,
    pub finance_goal: FinanceGoalRepo,
    pub finance_liability: FinanceLiabilityRepo,
    pub finance_investment: FinanceInvestmentRepo,
    pub finance_allocation: FinanceAllocationRepo,
    pub finance_snapshot: FinanceSnapshotRepo,
    pub finance_exchange_rate: FinanceExchangeRateRepo,

    // Productivity (2)
    pub dnd_override: DndOverrideRepo,
    pub bash_job: BashJobRepo,

    // Cognitive / learning (10)
    pub strategy: StrategyRepo,
    pub learning_state: LearningStateRepo,
    pub outcome: OutcomeRepo,
    pub decision_log: DecisionLogRepo,
    pub interaction_log: InteractionLogRepo,
    pub retrieval_feedback: RetrievalFeedbackRepo,
    pub reforge_suggestion: ReforgeSuggestionRepo,
    pub reforge_state: ReforgeStateRepo,
    pub skill_version: SkillVersionRepo,
    pub brain_signal: BrainSignalRepo,

    // Coaching / coding (7)
    pub coaching_strategy: CoachingStrategyRepo,
    pub coaching_intervention_log: CoachingInterventionLogRepo,
    pub coding_approval_history: CodingApprovalHistoryRepo,
    pub coding_todo: CodingTodoRepo,
    pub coding_reviews: CodingReviewsRepo,
    pub coding_background_jobs: CodingBackgroundJobsRepo,
    pub approval_pattern_history: ApprovalPatternHistoryRepo,

    // Tools / observability (3)
    pub tool_usage: ToolUsageRepo,
    pub response_warning: ResponseWarningRepo,
    pub usage: UsageRepo,

    // Scheduling / alerts (4)
    pub cron: CronRepo,
    pub scheduled_fires: ScheduledFiresRepo,
    pub held_notifications: HeldNotificationsRepo,
    pub notification_log: NotificationLogRepo,

    // Autotuner (1)
    pub trial: TrialRepo,

    // Agent tasks (1)
    pub agent_task: AgentTaskRepo,
}

impl Repos {
    pub fn from_pool(pool: &StoragePool) -> Self;
}
```

**~52 repos.** Construction is cheap — just clones the `StoragePool` (which is itself a clone of `Arc<sqlx::SqlitePool>`). Safe to construct per-handler or per-request.

### `VectorStore` (LanceDB)

```rust
pub struct VectorStore { /* opaque LanceDB connection */ }

impl VectorStore {
    pub async fn open(data_dir: &Path) -> Result<Self, StorageError>;

    pub async fn create_table_if_missing(
        &self, name: &str, schema: Arc<Schema>
    ) -> Result<(), StorageError>;

    pub async fn insert(
        &self, table: &str, rows: Vec<RecordBatch>
    ) -> Result<(), StorageError>;

    pub async fn search(
        &self, table: &str, embedding: Vec<f32>, top_k: usize,
        predicate: Option<&str>,
    ) -> Result<Vec<SearchResult>, StorageError>;

    pub async fn delete(
        &self, table: &str, predicate: &str
    ) -> Result<u64, StorageError>;

    // Tables created on demand by callers; common ones:
    //   episodic_memory, semantic_memory, notes_embeddings
}

pub fn sanitize_predicate_value(s: &str) -> String;

pub struct CognitiveFactParams { /* typed query surface for cognitive facts */ }
```

**Schema migrations are informal** — LanceDB doesn't have versioned migrations. Schema mismatch triggers an in-place rebuild path (`vector_store/mod.rs:112` handles the legacy `full_content` column case).

### `FinanceStorage` facade

```rust
pub struct FinanceStorage {
    pool: StoragePool,
    // wraps 9 finance repos
}

impl FinanceStorage {
    pub fn new(pool: StoragePool) -> Self;

    /// Atomic: insert transaction + update account.balance + bump budget.spent
    pub async fn record_transaction(
        &self, tx: NewFinanceTransaction
    ) -> Result<FinanceTransaction, StorageError>;

    /// Atomic: take a net-worth snapshot across all accounts + investments
    pub async fn record_net_worth_snapshot(&self) -> Result<NetWorthSnapshot, StorageError>;

    /// + ~30 other multi-table operations
}
```

**Why a facade?** Finance operations frequently touch multiple tables atomically. Doing that with separate repos would require passing `&mut Transaction<'_, Sqlite>` through every method signature. The facade absorbs that.

### `StorageError`

```rust
#[derive(thiserror::Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]      Database(#[from] sqlx::Error),
    #[error("Migration error: {0}")]     Migration(String),
    #[error("Not found: {0}")]           NotFound(String),
    #[error("Conflict: {0}")]            Conflict(String),
    #[error("LanceDB error: {0}")]       Lance(String),
    #[error("Serialization error: {0}")] Serde(#[from] serde_json::Error),
    #[error("I/O error: {0}")]           Io(#[from] std::io::Error),
}

pub trait OptionExt<T> {
    fn require(self, msg: &str) -> Result<T, StorageError>;
}
```

`OptionExt::require` is the canonical "this should exist; surface as `NotFound`" helper.

### `SqlDate` / `SqlTs`

```rust
/// Wraps jiff::civil::Date for SQLite TEXT storage.
pub struct SqlDate(pub jiff::civil::Date);

/// Wraps jiff::Timestamp for SQLite INTEGER (epoch-ms) storage.
pub struct SqlTs(pub jiff::Timestamp);
```

Both implement `sqlx::Type<Sqlite>`, `Encode`, and `Decode`. Use these in `*Row` structs instead of raw `String` or `i64`.

### Representative repo: `SessionRepo` (selected methods)

```rust
pub struct SessionRepo { pool: StoragePool }

impl SessionRepo {
    pub fn new(pool: StoragePool) -> Self;

    pub async fn create_session(
        &self, session_key: &SessionKey, mode: SessionMode,
    ) -> Result<SessionRow, StorageError>;

    pub async fn get(&self, session_key: &str) -> Result<Option<SessionRow>, StorageError>;

    pub async fn list_sessions(
        &self, limit: u32, before: Option<Timestamp>,
    ) -> Result<Vec<SessionListRow>, StorageError>;

    pub async fn add_message(
        &self, session_key: &str, msg_id: &str, role: &str,
        content: &str, parts: Option<&str>,
        tool_call_id: Option<&str>, tool_name: Option<&str>,
        usage: Option<&Usage>,
    ) -> Result<(), StorageError>;

    pub async fn replace_message_parts(
        &self, msg_id: &str, parts: &[MessagePart],
    ) -> Result<(), StorageError>;

    pub async fn detect_zombie_sessions(
        &self, threshold_ms: i64,
    ) -> Result<Vec<String>, StorageError>;

    pub async fn touch_last_event(&self, session_key: &str) -> Result<(), StorageError>;

    pub async fn delete_session(&self, session_key: &str) -> Result<(), StorageError>;
}
```

All other repos follow the same shape: `new(pool)`, CRUD methods, domain-specific queries.

### Built-in migrations

```
crates/storage/migrations/
├── 001_initial.sql                ← Core tables: sessions, messages, cron_jobs, tasks, …
├── 002_…
├── …
└── NNN_<name>.sql
```

Run lexicographically by `sqlx::migrate!("./migrations")`. Tracked in `_sqlx_migrations` table. Each file runs in its own transaction.

**Pre-1.0 rule (per CLAUDE.md):** alter existing migration files in place — no user data to preserve. Post-1.0, every change becomes a new migration with `INSERT OR IGNORE` for idempotency.

---

## Internals

### Per-connection PRAGMAs (set in `after_connect`)

| PRAGMA | Value | Reason |
|---|---|---|
| `foreign_keys` | `ON` | Enforce FK constraints |
| `busy_timeout` | `5000` ms | Handle WAL contention |
| `cache_size` | `-2000` (≈2MB) | Single-user app; default 8MB is overkill |

### Pool-level PRAGMAs (run once after pool open)

| PRAGMA | Value | Reason |
|---|---|---|
| `journal_mode` | `WAL` | Required for concurrent readers |
| `wal_autocheckpoint` | `1000` pages (≈4MB) | Prevent unbounded WAL growth |

### `PoolOptions`

```rust
sqlx::pool::PoolOptions::<sqlx::Sqlite>::new()
    .max_connections(5)
    .after_connect(|conn, _| { /* PRAGMAs */ })
    .connect(&url)
    .await?
```

**Max 5 connections.** Single-user workload doesn't need more; helps cap memory. Reads are concurrent (WAL); writes serialize per connection but ~5 in flight is sufficient.

### Cross-process sharing

Both the desktop process and the MCP child connect via `StoragePool::connect(data_dir)`. WAL mode allows safe concurrent access. The `DataVersionWatcherHandle` notifies the desktop when the child writes (so UI can refresh).

### Legacy `messages.content` mirror

`SessionRepo::add_message` writes both:
- `content`: legacy String column — joined Text parts with `\n`
- `parts`: JSON column with structured `MessagePart` array

Both atomic in the same INSERT. Reads fall back to wrapping legacy `content` in a `Text` part when `parts IS NULL`. This is Anthropic-shape compatibility for older readers. See [`TECH_DEBT.md`](../TECH_DEBT.md#3-legacy-code-paths-in-active-use).

### Circuit breaker per repo

`circuit_breaker.rs` tracks per-repo failure counts. After N consecutive write failures, the breaker opens — subsequent writes return `StorageError` immediately without hitting SQLite. Periodic half-open probes attempt to close. Used mostly for cognitive (where a single failing memory write shouldn't block the chat path).

### `ai-core` upward dependency

`storage/Cargo.toml:7` has `ai-core.workspace = true`. Some `*Row` types materialize `ai-core` trait objects. **This is an architectural anomaly** — `ai-core` is logically higher in the stack. See [`TECH_DEBT.md`](../TECH_DEBT.md#7-architectural-anomalies).

### `FeatureMigration` system

```rust
// In tools-core
pub struct FeatureMigration {
    pub feature_name: String,
    pub version: i64,
    pub description: String,
    pub sql: String,
}

// Tracking
CREATE TABLE feature_migrations (
    feature_name TEXT NOT NULL,
    version      INTEGER NOT NULL,
    description  TEXT,
    applied_at   INTEGER NOT NULL,
    PRIMARY KEY (feature_name, version)
);
```

Each `FeatureMigration` + the tracking `INSERT` are wrapped in a transaction so a crash between SQL and tracking can't leave the DB in an inconsistent state. `INSERT OR IGNORE` makes the tracking row idempotent.

**Duplicate `(feature_name, version)` panics on startup** with a descriptive message — caught early in dev console.

---

## Workflows

### Connect + migrate at startup

```rust
let data_dir = config.data_dir();
let pool = StoragePool::connect(&data_dir).await?;
let repos = Repos::from_pool(&pool);
let vectors = VectorStore::open(&data_dir).await?;

// Apply per-feature migrations
let feature_migrations: Vec<FeatureMigration> = features.iter()
    .flat_map(|f| f.migrations())
    .collect();
StoragePool::run_feature_migrations(pool.inner(), &feature_migrations).await?;
```

### Write a message (with legacy mirror)

```rust
let msg = NewMessage {
    role: MessageRole::User,
    parts: vec![MessagePart::Text { text: user_msg.clone() }],
    ...
};

let content = parts_to_legacy_content(&msg.parts);  // join Text parts with \n
let parts_json = serde_json::to_string(&msg.parts)?;

repos.session.add_message(
    session_key.as_str(),
    &msg.id,
    role.as_str(),
    &content,
    Some(&parts_json),
    None, None, None,
).await?;
```

### Vector search

```rust
// Embed query via providers
let embedding: Vec<f32> = embedder.embed(&query).await?;

// Search LanceDB
let results = vectors.search(
    "semantic_memory",
    embedding,
    /* top_k */ 10,
    Some("decay_factor > 0.5"),
).await?;

// Hydrate full rows via storage repo
let ids: Vec<i64> = results.iter().map(|r| r.id).collect();
let facts = repos.semantic_fact.find_many(&ids).await?;
```

### In-memory test

```rust
#[tokio::test]
async fn task_crud() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);

    let task = repos.task.create(NewTask {
        title: "Test".into(), ..Default::default()
    }).await.unwrap();

    let got = repos.task.find(&task.id).await.unwrap().unwrap();
    assert_eq!(got.title, "Test");
}
```

**No fixture DBs.** Every test gets a fresh in-memory pool. `cargo nextest` runs them in parallel — each test has its own ephemeral pool, no shared state.

---

## Testing approach

### In-memory pool (default)

```rust
let pool = StoragePool::connect_in_memory().await.unwrap();
```

Runs all migrations. Mirrors production schema. Use for any test that needs persistence.

### Mocking specific repos

If you need to mock a specific repo (e.g. for failure injection), wrap it in a trait:

```rust
#[async_trait]
trait TaskRepoLike {
    async fn create(&self, t: NewTask) -> Result<TaskRow, StorageError>;
}

#[async_trait]
impl TaskRepoLike for TaskRepo {
    async fn create(&self, t: NewTask) -> Result<TaskRow, StorageError> {
        self.create(t).await
    }
}

struct AlwaysFailingTaskRepo;
#[async_trait]
impl TaskRepoLike for AlwaysFailingTaskRepo {
    async fn create(&self, _: NewTask) -> Result<TaskRow, StorageError> {
        Err(StorageError::Database(...))
    }
}
```

Most tests don't need this — the in-memory pool is fast enough.

### Vector tests

`VectorStore` doesn't have an in-memory mode. For unit-tested code paths, use a temp dir:

```rust
let tmp = tempfile::tempdir().unwrap();
let vectors = VectorStore::open(tmp.path()).await.unwrap();
```

Cleanup happens on `tmp` drop.

---

## Extension points

### Add a new repo

1. Create `crates/storage/src/rows/my_thing.rs`:
   ```rust
   #[derive(sqlx::FromRow, Debug, Clone)]
   pub struct MyThingRow {
       pub id: String,
       pub name: String,
       pub created_at: SqlTs,
   }
   ```
2. Create `crates/storage/src/repos/my_thing_repo.rs`:
   ```rust
   pub struct MyThingRepo { pool: StoragePool }
   impl MyThingRepo {
       pub fn new(pool: StoragePool) -> Self { Self { pool } }
       pub async fn insert(&self, row: MyThingRow) -> Result<(), StorageError> { ... }
       pub async fn find(&self, id: &str) -> Result<Option<MyThingRow>, StorageError> { ... }
   }
   ```
3. Add field to `Repos` struct in `crates/storage/src/repos/mod.rs` + construct in `Repos::from_pool`.
4. Re-export `MyThingRepo` + `MyThingRow` from `lib.rs`.
5. Add migration in `crates/storage/migrations/NNN_add_my_thing.sql` (built-in) OR as a `FeaturePackage::migrations()` entry (per-feature).

### Add a new built-in migration (pre-1.0)

```sql
-- crates/storage/migrations/NNN_my_change.sql
CREATE TABLE my_thing (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    created_at INTEGER NOT NULL DEFAULT (cast(strftime('%s','now') as integer))
);
CREATE INDEX my_thing_created_idx ON my_thing(created_at);
```

Filename must start with a unique 3-digit prefix higher than any existing migration. Each runs in its own transaction.

**Post-1.0:** add a new migration file; never alter an existing one. Use `IF NOT EXISTS` everywhere for idempotency.

### Add a vector-backed memory type

1. Define the schema (Arrow `Schema` with `vector` column).
2. In your service's init: `vectors.create_table_if_missing("my_memory", schema).await?`.
3. Define a typed query params struct (see `CognitiveFactParams` as template).
4. Embed via `providers` and write via `VectorStore::insert`.
5. Read via `VectorStore::search` with optional predicate.

### Add a feature migration (in a feature crate)

```rust
impl FeaturePackage for MyFeature {
    fn migrations(&self) -> Vec<FeatureMigration> {
        vec![
            FeatureMigration {
                feature_name: "my_feature".into(),
                version: 1,
                description: "Initial schema".into(),
                sql: include_str!("../migrations/001_initial.sql").into(),
            },
        ]
    }
}
```

**Watch:** duplicate `(feature_name, version)` panics on startup.

### Add a method to `FinanceStorage`

If the operation touches multiple finance tables atomically, add to `FinanceStorage` rather than a single repo. Pattern:

```rust
impl FinanceStorage {
    pub async fn my_atomic_op(&self, ...) -> Result<T, StorageError> {
        let mut tx = self.pool.inner().begin().await?;
        sqlx::query!("UPDATE foo SET ... WHERE ...").execute(&mut *tx).await?;
        sqlx::query!("INSERT INTO bar ...").execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(...)
    }
}
```

---

## Key constants

| Constant | Value | Location |
|---|---|---|
| `PoolOptions::max_connections` | `5` | `pool.rs::connect` |
| Per-connection `busy_timeout` | `5000` ms | `pool.rs::after_connect` |
| Per-connection `cache_size` | `-2000` (≈2MB) | `pool.rs::after_connect` |
| Pool `wal_autocheckpoint` | `1000` pages | `pool.rs::connect` |
| Circuit breaker failure threshold | per-repo (default 5) | `circuit_breaker.rs` |

---

## Open questions

- **`storage` depends upward on `ai-core`** — architectural anomaly. Decide: move trait to `common`? Invert via dependency-inversion? Formalize?
- **Legacy `messages.content` column mirror** — every message writes both `content` and `parts`. Deprecate once all consumers read from `parts` only.
- **`VectorStore` has no formal migration system** — schema mismatch triggers in-place rebuild. Acceptable today; will need a real story when external schemas appear.
- **`BashJobRepo` lives in storage** even though it's coding-only. Move to `coding-memory` or `feature-coding-bash` for crate hygiene.
- **No structured query API for the vector store** — every consumer hand-writes predicates. `CognitiveFactParams` is the template; could be generalized.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #3 + #7 for specifics.

---

## Cross-references

- [Subsystem 02 — Storage & Persistence](../subsystems/02-storage.md) (parent)
- [Subsystem 05 — Cognitive Memory](../subsystems/05-cognitive-memory.md) (heaviest consumer)
- [Subsystem 07 — Tools Framework](../subsystems/07-tools-framework.md) (`FeatureMigration` defined there)
- [`crates/providers.md`](./providers.md) (consumes embeddings via `VectorStore`)
- [`crates/agent.md`](./agent.md) *(planned)* (consumes `Repos`)
- [`crates/app-core.md`](./app-core.md) *(planned)* (owns the pool)
