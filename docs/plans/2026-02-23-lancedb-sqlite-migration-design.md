# Drop PostgreSQL: Migrate to SQLite + LanceDB

**Date:** 2026-02-23
**Status:** Approved
**Approach:** Big bang rewrite of storage crate

## Summary

Replace PostgreSQL (+ pgvector) with SQLite (relational data) + LanceDB (vector data). Eliminates all external database infrastructure. Users run `klyntbot serve` with zero setup.

## Architecture

```
BEFORE                              AFTER
------                              -----
PostgreSQL (server)                 SQLite (embedded file)
├── 29 relational tables            ├── 29 relational tables
├── 3 embedding tables (pgvector)   └── _feature_migrations
└── _feature_migrations
                                    LanceDB (embedded files)
fastembed (in-process)              ├── todo_embeddings
                                    ├── conv_embeddings
                                    └── memory_note_embeddings

                                    fastembed (in-process, unchanged)
```

### Data directory layout

```
~/.klyntbot/
├── config.json
├── data.db              ← SQLite (all relational data)
├── lance/               ← LanceDB directory
│   ├── todo_embeddings.lance/
│   ├── conv_embeddings.lance/
│   └── memory_note_embeddings.lance/
└── skills/
```

## Storage Crate Changes

### StoragePool

```rust
// BEFORE
pub struct StoragePool(sqlx::PgPool);
impl StoragePool {
    pub async fn connect(database_url: &str) -> Result<Self, StorageError>;
    pub fn connect_lazy(database_url: &str) -> Result<Self, StorageError>;
    pub fn inner(&self) -> &sqlx::PgPool;
}

// AFTER
pub struct StoragePool(sqlx::SqlitePool);
impl StoragePool {
    pub async fn connect(data_dir: &Path) -> Result<Self, StorageError>;
    pub fn inner(&self) -> &sqlx::SqlitePool;
}
```

- Input changes from `database_url` to `data_dir` path
- SQLite file created at `{data_dir}/data.db`
- `connect_lazy` removed (SQLite doesn't need deferred connections)
- Migrations still use `sqlx::migrate!()`
- Connection enables: `PRAGMA journal_mode=WAL`, `PRAGMA foreign_keys=ON`

### Repos struct

Embedding repos removed from `Repos` (move to LanceDB):

```rust
pub struct Repos {
    pool: sqlx::SqlitePool,           // was PgPool
    pub todos: TodoRepo,              // same public API
    pub projects: ProjectRepo,
    pub sessions: SessionRepo,
    // ... all 20 relational repos stay

    // REMOVED:
    // pub embeddings: EmbeddingRepo
    // pub conv_embeddings: ConvEmbeddingRepo
    // pub memory_note_embeddings: MemoryNoteEmbeddingRepo
}
```

### New VectorStore (LanceDB)

```rust
pub struct VectorStore {
    db: lancedb::Connection,
}

impl VectorStore {
    pub async fn connect(data_dir: &Path) -> Result<Self>;
    pub async fn upsert_embedding(table: &str, id: &str, vector: &[f32]) -> Result<()>;
    pub async fn search_similar(table: &str, query: &[f32], limit: usize, threshold: f64) -> Result<Vec<(String, f64)>>;
    pub async fn delete(table: &str, id: &str) -> Result<()>;
}
```

Lives in storage crate. LanceDB tables created programmatically with Arrow schemas (no SQL migrations).

## SQL Dialect Changes

| PostgreSQL | SQLite equivalent |
|------------|-------------------|
| `$1, $2, $3` bind params | `?1, ?2, ?3` |
| `UUID` type | `TEXT` |
| `TIMESTAMPTZ` | `TEXT` (ISO 8601) |
| `JSONB` columns | `TEXT` + `json()` functions |
| `gen_random_uuid()` | UUID generated in Rust |
| `INTERVAL '30 days'` | `datetime('now', '-30 days')` |
| `EXTRACT(EPOCH FROM ...)` | `unixepoch(...)` |
| `QueryBuilder<sqlx::Postgres>` | `QueryBuilder<sqlx::Sqlite>` |
| Recursive CTEs | Same syntax (supported) |
| `ON CONFLICT DO UPDATE` | Same syntax (supported) |
| `RETURNING *` | Supported (SQLite 3.35+) |

## Dependency Changes

### Cargo.toml

```toml
# REMOVED
pgvector = { version = "0.4", features = ["sqlx"] }

# CHANGED
sqlx = { version = "0.8", features = ["runtime-tokio", "tls-rustls", "sqlite", "migrate", "uuid", "chrono", "json"] }

# ADDED
lancedb = "0.26"
arrow = { version = "53", features = ["prettyprint"] }
arrow-array = "53"
arrow-schema = "53"
```

### Config schema

```json
// BEFORE
{ "databaseUrl": "postgres://localhost/klyntbot" }

// AFTER
{ "dataDir": "~/.klyntbot" }   // optional, defaults to ~/.klyntbot
```

- `KLYNTBOT_DATABASE_URL` env var removed
- `KLYNTBOT_DATA_DIR` env var as override

## Files Deleted

| File/Dir | Reason |
|----------|--------|
| `crates/storage/migrations/*.sql` (all PG migrations) | Replaced by SQLite schema |
| `crates/storage/src/repos/embedding.rs` | Replaced by LanceDB VectorStore |
| `crates/storage/src/repos/conv_embedding.rs` | Replaced by LanceDB VectorStore |
| `crates/storage/src/repos/memory_note_embedding.rs` | Replaced by LanceDB VectorStore |
| `crates/storage/src/rows/embedding.rs` | Arrow types instead |
| `crates/feature-todo/src/storage/repo.rs` | Consolidated into storage::TodoRepo |

## Migration Schema

Single SQLite baseline migration replaces all PG migrations:

**`crates/storage/migrations/001_initial.sql`** — all 29 tables with SQLite types.

**LanceDB tables** created programmatically via Arrow schemas:

```rust
// todo_embeddings
Schema::new(vec![
    Field::new("id", DataType::Utf8, false),
    Field::new("vector", DataType::FixedSizeList(
        Box::new(Field::new("item", DataType::Float32, true)), 384
    ), false),
    Field::new("model", DataType::Utf8, false),
    Field::new("updated_at", DataType::Utf8, false),
])
```

## Test Strategy

- Repo unit tests: `SqlitePool::connect("sqlite::memory:")`
- Integration tests: temp file `sqlite:/tmp/test_{uuid}.db`
- Embedding tests: LanceDB temp dir
- Mock tests: `EmbeddingHandler` trait mocks unchanged
- Zero setup required to run tests

## Consolidation

- Duplicate `TodoRepo` in `feature-todo` deleted; `estimated_minutes` added to storage crate's `TodoPatch`
- All hardcoded `postgres://localhost/klyntbot_test` references removed
- `StoragePool::connect_lazy()` removed

## Risks & Mitigations

1. **SQLite concurrent writes** — WAL mode. Single-user agent scale is fine.
2. **Weaker JSON querying** — Most JSONB is opaque blobs; few queries need `json_extract()` rewrite.
3. **LanceDB crate maturity (v0.26)** — Isolated behind `VectorStore` abstraction. Swappable.
4. **Large PR** — Structured as ordered commits (schema, repos, vector store, config, cleanup).
5. **Duplicate TodoRepo drift** — Consolidated during migration.

## Out of Scope

- PostgreSQL as optional backend (add later via feature flags if demanded)
- Data migration tooling (pre-production, no data to migrate)
- LanceDB cloud storage (Lance format supports S3/GCS natively, can add later)
