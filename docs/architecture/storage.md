# Storage Architecture

## Overview

Klyntbot uses a dual-storage architecture:

- **SQLite** (`{data_dir}/data.db`) -- All relational data: sessions, messages, tasks, projects, areas, OKR, finance, learning analytics, cognitive memory, scheduling, and user profiling.
- **LanceDB** (`{data_dir}/lance/`) -- All vector embeddings: todo, conversation, cognitive fact, activity, and work context embeddings used for semantic similarity search.

The default data directory is `~/.klyntbot`. Both stores are initialized on startup via `StoragePool::connect()` and `VectorStore::connect()` respectively.

```
~/.klyntbot/
  data.db           # SQLite (WAL mode)
  lance/            # LanceDB vector tables
    todo_embeddings/
    conv_embeddings/
    cognitive_fact_embeddings/
    activity_embeddings/
    work_context_embeddings/
```

## StoragePool

`StoragePool` is a newtype wrapper around `sqlx::SqlitePool`:

```rust
#[derive(Clone)]
pub struct StoragePool(sqlx::SqlitePool);
```

Because `sqlx::SqlitePool` is internally `Arc`-based, `StoragePool` is `Clone + Send + Sync` without requiring an outer `Arc<RwLock<...>>`. It can be passed by value across threads and tasks.

### Connection methods

| Method | Migrations | WAL mode | Use case |
|---|---|---|---|
| `connect(data_dir)` | Yes (`sqlx::migrate!`) | Yes | Production startup |
| `connect_in_memory()` | Yes (`sqlx::migrate!`) | No (in-memory) | Tests, fallback |
| `from_existing(pool)` | **No** | Caller's responsibility | Wrapping a pre-migrated pool |

All three methods return `Result<StoragePool, StorageError>` (except `from_existing` which is infallible).

On `connect()`:
1. Creates `{data_dir}/data.db` (and parent directories) if missing.
2. Enables WAL journal mode (`PRAGMA journal_mode=WAL`).
3. Enables foreign key enforcement (`PRAGMA foreign_keys=ON`).
4. Runs all pending core migrations via `sqlx::migrate!("./migrations")`.

### Accessing the inner pool

```rust
pool.inner() -> &sqlx::SqlitePool
```

Used by `Repos::from_pool()` and feature migration runners.

## Database Schema

### Core: Sessions & Messages

| Table | Key columns | Notes |
|---|---|---|
| `sessions` | `key` (PK), `metadata`, `created_at`, `updated_at`, `project_id`, `conversation_type`, `pinned` | One row per conversation session |
| `session_messages` | `id` (PK), `session_key` (FK->sessions), `role`, `content`, `timestamp`, `request_id`, `tool_calls`, `metadata` | Indexed on `(session_key, timestamp)` |
| `session_context` | `session_key` (PK, FK->sessions), `context_type`, `entity_kind`, `entity_id`, `area_id`, `project_id`, `is_ephemeral`, `is_pinned` | PARA-aware session categorization |

### PARA: Areas, Projects, Resources, Archive

| Table | Key columns | Notes |
|---|---|---|
| `areas` | `id` (PK), `name` (UNIQUE), `description`, `color`, `icon`, `position`, `status` | Top-level organizational containers |
| `projects` | `id` (PK), `area_id` (FK->areas), `name`, `description`, `color`, `tags`, `status`, `instructions`, `ai_personality`, `user_role`, `start_date`, `target_end_date`, `settings`, `workflow_id` | Belong to areas; support AI context customization |
| `project_sources` | `id` (PK), `project_id` (FK->projects), `source_type`, `title`, `content`, `url`, `file_path`, `embedding_id`, `metadata`, `tags` | Per-project AI context sources |
| `resources` | `id` (PK), `area_id` (FK->areas), `title`, `resource_type`, `content`, `url`, `tags` | Schema-only, no tool support yet |
| `archive_items` | `id` (PK), `source_type`, `source_id`, `title`, `snapshot`, `archived_at`, `archived_reason` | Schema-only, no tool support yet |

### OKR: Objectives & Key Results

| Table | Key columns | Notes |
|---|---|---|
| `objectives` | `id` (PK), `project_id` (FK->projects), `title`, `description`, `status`, `priority`, `due_date`, `progress`, `completed_at` | Belong to projects |
| `key_results` | `id` (PK), `objective_id` (FK->objectives), `title`, `tracking_mode`, `target_value`, `current_value`, `unit`, `progress`, `due_date`, `completed_at` | Belong to objectives; support manual or action-based tracking |

### Actions (Tasks)

| Table | Key columns | Notes |
|---|---|---|
| `actions` | `id` (PK), `title`, `area_id` (FK->areas), `project_id` (FK->projects), `key_result_id` (FK->key_results), `parent_id` (self-FK), `priority`, `due_date`, `tags`, `status`, `status_label_id` (FK->status_labels), `position`, `group_id` (FK->task_groups), `focused_at`, `focus_deadline`, `completed_at`, `total_tracked_secs`, `estimated_minutes`, `recurrence_rule`, `is_template` | Central task table with subtasks, focus mode, recurrence, time tracking |
| `action_attachments` | `id` (PK), `action_id` (FK->actions), `attachment_type`, `value`, `title`, `tags` | Links, notes, files attached to actions |
| `action_time_entries` | `id` (PK), `action_id` (FK->actions), `source`, `started_at`, `ended_at`, `duration_secs`, `note` | Focus sessions and manual time entries |
| `action_dependencies` | `action_id` + `blocker_id` (composite PK, both FK->actions) | Blocking relationships between actions |
| `task_groups` | `id` (PK), `project_id` (FK->projects), `name`, `color`, `position` | Collapsible sections within a project view |

### Status Workflows

| Table | Key columns | Notes |
|---|---|---|
| `status_workflows` | `id` (PK), `name`, `is_template`, `is_global_default` | Customizable kanban column sets |
| `status_labels` | `id` (PK), `workflow_id` (FK->status_workflows), `name`, `color`, `status_group` (check: not_started/active/done/stuck), `position` | Individual columns within a workflow |

Seeded with a global default workflow (`wf_default`) plus templates: Simple, Software Dev, Content Creation.

### Custom Columns

| Table | Key columns | Notes |
|---|---|---|
| `custom_columns` | `id` (PK), `project_id` (FK->projects), `name`, `column_type` (check: text/number/date/dropdown/...), `options_json`, `position`, `width` | Per-project custom field definitions |
| `custom_column_values` | `task_id` + `column_id` (composite PK), `value_json` | Per-task values for custom columns |

### Finance

| Table | Key columns | Notes |
|---|---|---|
| `finance_accounts` | `id` (PK), `name`, `account_type`, `currency`, `balance`, `institution`, `is_archived` | Bank accounts, wallets, etc. |
| `finance_transactions` | `id` (PK), `account_id` (FK->finance_accounts), `tx_type`, `amount`, `currency`, `category`, `subcategory`, `counterparty`, `tx_date`, `transfer_id`, `is_recurring`, `recurring_rule` | Income, expense, transfer records |
| `finance_budgets` | `id` (PK), `name`, `amount`, `currency`, `period`, `category`, `method`, `jar_type`, `start_date`, `alert_threshold` | Budget envelopes with alert thresholds |
| `finance_portfolios` | `id` (PK), `name`, `description`, `currency` | Investment portfolio containers |
| `finance_investments` | `id` (PK), `portfolio_id` (FK->finance_portfolios), `asset_type`, `symbol`, `name`, `quantity`, `cost_basis`, `currency`, `current_price`, `current_value` | Individual holdings |
| `finance_investment_transactions` | `id` (PK), `investment_id` (FK->finance_investments), `tx_type`, `quantity`, `price_per_unit`, `total_amount`, `fees`, `tx_date` | Buy/sell/dividend records |
| `finance_goals` | `id` (PK), `name`, `goal_type`, `target_amount`, `current_amount`, `currency`, `status`, `deadline`, `monthly_contribution` | Savings and financial goals |
| `finance_liabilities` | `id` (PK), `name`, `liability_type`, `principal`, `remaining`, `currency`, `interest_rate`, `monthly_payment`, `due_date` | Debts and loans |

### Scheduling

| Table | Key columns | Notes |
|---|---|---|
| `cron_jobs` | `id` (PK), `name`, `enabled`, `origin`, `schedule`, `payload`, `next_run_at_ms`, `last_run_at_ms`, `last_status`, `delete_after_run` | Persistent cron job definitions |
| `calendar_sync_state` | `provider_id` (PK), `sync_token`, `last_sync_at` | CalDAV sync state |
| `calendar_event_cache` | `uid` + `provider_id` (composite PK), `summary`, `description`, `start_at`, `end_at`, `source`, `etag`, `status` | Cached calendar events |

### Learning & Analytics

| Table | Key columns | Notes |
|---|---|---|
| `learning_outcomes` | `id` (PK), `session_key`, `tool_name`, `success`, `error_category`, `duration_ms`, `confidence_score`, `confidence_dimensions`, `execution_mode` | Per-tool execution outcomes |
| `strategy_records` | `id` (PK), `timestamp`, `request_id`, `predicted_strategy`, `actual_strategy`, `escalation_count`, `iterations_used`, `success`, `chat_id`, `tool_name`, `complexity_signals` | Strategy prediction accuracy tracking |
| `enrichment_feedback` | `id` (autoincrement), `task_id`, `field`, `suggested_value`, `actual_value`, `accepted`, `confidence` | Task enrichment suggestion feedback |
| `tool_usage` | `id` (PK), `tool_name`, `action`, `session_key`, `channel`, `intent_category`, `success`, `duration_ms`, `error_message` | Tool usage analytics |
| `usage_records` | `id` (PK), `timestamp`, `request_id`, `model`, `provider`, `prompt_tokens`, `completion_tokens`, `cache_read_tokens`, `cache_write_tokens`, `estimated_cost_usd`, `channel`, `strategy` | LLM cost tracking |
| `learning_state` | `key` (PK), `value` (JSON), `updated_at` | Key-value store for learning system state |
| `decision_log` | `id` (PK), `session_key`, `iteration`, `tool_names`, `user_message_preview`, `assessment`, `outcome` | ReAct loop decision audit trail |

### User Profiling

| Table | Key columns | Notes |
|---|---|---|
| `user_profile` | `id` (autoincrement), `category` + `key` (UNIQUE), `value`, `source`, `confidence`, `agent_name`, `last_confirmed` | Explicit user facts |
| `behavioral_patterns` | `id` (autoincrement), `pattern_type` + `pattern_key` (UNIQUE), `pattern_value`, `sample_count` | Observed interaction patterns |
| `agent_adaptations` | `id` (autoincrement), `agent_name` + `preference_key` (UNIQUE), `preference_value`, `source`, `confidence` | Per-agent user preference adaptations |
| `interaction_log` | `id` (autoincrement), `timestamp`, `agent_name`, `tool_names`, `channel`, `duration_ms` | Raw interaction data for pattern analysis |

### Agent Tasks

| Table | Key columns | Notes |
|---|---|---|
| `agent_tasks` | `id` (PK), `session_key`, `description`, `status` (check: pending/claimed/running/completed/failed), `owner_agent_id`, `parent_task_id` (self-FK), `result`, `error`, `blocked_by` | Subagent task coordination |

### Cross-Feature

| Table | Key columns | Notes |
|---|---|---|
| `entity_links` | `id` (PK), `source_kind`, `source_id`, `target_kind`, `target_id`, `link_type`, `metadata` | Generic cross-feature entity linking (UNIQUE on source+target+type) |
| `_feature_migrations` | `feature_name` + `version` (composite PK), `description`, `applied_at` | Tracks which feature migrations have been applied |

### Cognitive Memory (Feature Migrations)

These tables are created by the `cognitive` feature migration system, not the core `sqlx::migrate!` pipeline.

| Table | Key columns | Notes |
|---|---|---|
| `semantic_facts` | `id` (PK), `domain`, `subject`, `predicate`, `object`, `confidence`, `source`, `valid_from`, `valid_until`, `recorded_at`, `superseded_at`, `superseded_by`, `stability`, `last_accessed`, `access_count`, `project_id`, `memory_type` | Bi-temporal semantic knowledge (SPO triples); FSRS-based stability decay |
| `semantic_facts_archive` | Same columns as `semantic_facts` + `archived_at` | Cold storage for superseded facts older than N days |
| `episodic_memories` | `id` (PK), `domain`, `content`, `summary`, `importance`, `occurred_at`, `recorded_at`, `stability`, `last_accessed`, `access_count`, `project_id` | Event-based memories with importance scoring |
| `procedural_rules` | `id` (PK), `domain`, `rule_text`, `confidence`, `source`, `signal_count`, `active`, `project_id` | Learned behavioral rules; activated/deactivated per domain |
| `coaching_strategies` | `id` (PK), `strategy_type` + `domain` (UNIQUE), `times_used`, `times_accepted`, `times_led_to_improvement`, `avg_improvement_magnitude`, `confidence` | Coaching strategy effectiveness tracking |
| `domain_event_log` | `id` (PK), `event_type`, `domain`, `salience`, `payload`, `timestamp` | Persisted broadcast domain events |
| `pipeline_event_log` | `id` (PK), `event_kind` (extraction/consolidation), `observation`, `facts_extracted`, `operation`, `fact_triple`, `timestamp` | Extraction and consolidation audit trail |
| `accumulated_observations` | `id` (PK), `event_type_key`, `domain`, `content`, `importance`, `source_event`, `observed_at`, `day_key` | Buffered low-salience events awaiting promotion threshold |
| `annotations` | `id` (PK), `target_type`, `target_id`, `content`, `tags`, `author`, `priority`, `expires_at`, `access_count` | Contextual annotations on any entity (UNIQUE on target+content) |

### FTS5 Virtual Tables (Cognitive)

| Virtual table | Indexed columns | Source table |
|---|---|---|
| `semantic_facts_fts` | `domain`, `subject`, `predicate`, `object`, `memory_type` | `semantic_facts` |
| `episodic_memories_fts` | `domain`, `content`, `summary` | `episodic_memories` |
| `procedural_rules_fts` | `domain`, `rule_text` | `procedural_rules` |
| `annotations_fts` | `target_type`, `target_id`, `content`, `tags` | `annotations` |

All FTS5 tables use `porter unicode61` tokenization and are kept in sync via `AFTER INSERT/UPDATE/DELETE` triggers.

## Migration System

### Core migrations (sqlx)

Core migrations live in `crates/storage/migrations/` and are applied automatically by `StoragePool::connect()` and `connect_in_memory()` via `sqlx::migrate!("./migrations")`. SQLx tracks applied migrations in its own internal `_sqlx_migrations` table.

Current core migration history:

| File | Description |
|---|---|
| `001_initial.sql` | All baseline tables: PARA, OKR, actions, sessions, finance, scheduling, analytics, entity links |
| `002_session_context.sql` | Session context categorization table |
| `003_learning_system.sql` | User profile, behavioral patterns, agent adaptations, interaction log |
| `004_status_workflows.sql` | Customizable status workflows and labels; seed data |
| `005_task_groups.sql` | Task groups table; adds `group_id` column to actions |
| `006_custom_columns.sql` | Custom column definitions and values per project/task |
| `007_drop_memory_notes.sql` | Drops `memory_notes` table (replaced by cognitive memory) |

### Feature migrations (FeatureMigration trait)

Feature crates own their own migrations, tracked in the `_feature_migrations` table (created by core migration 001). This avoids coupling feature-specific schema to the core migration pipeline.

The `FeatureMigration` struct:

```rust
pub struct FeatureMigration {
    pub feature_name: String,  // e.g. "cognitive", "todo"
    pub version: i64,          // sequential within the feature
    pub description: String,
    pub sql: String,           // raw SQL to execute
}
```

`StoragePool::run_feature_migrations()` iterates over a slice of `FeatureMigration`s. For each one, it checks `_feature_migrations` for a matching `(feature_name, version)` row. If absent, it executes the SQL and records the migration.

**Active feature migration sets:**

- **`cognitive`** (6 versions): Semantic facts, episodic memories, procedural rules, coaching strategies, archive tables, event logs, accumulated observations, FTS5 virtual tables, annotations.
- **`todo`** (1 version): Actions, attachments, time entries, dependencies (uses `IF NOT EXISTS` since core migration 001 already creates these tables).

### Adding a new feature migration

1. Create a SQL file in your feature crate's `migrations/` directory.
2. Add a `FeatureMigration` entry in your `FeaturePackage::migrations()` implementation (or equivalent function).
3. SQL is loaded at compile time via `include_str!`.
4. Use `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` for idempotency when tables may already exist from core migrations.

## Repository Pattern

### Repos aggregate

`Repos` is the central access point for all 24 repository handles. It is constructed from a `StoragePool`:

```rust
let repos = Repos::from_pool(&pool);
```

Each repository wraps a cloned `SqlitePool` and is `Clone + Send + Sync`. The `Repos` struct itself is also `Clone`.

### Complete repo listing

| Field | Type | Domain |
|---|---|---|
| `actions` | `ActionRepo` | Tasks/actions CRUD, filtering, summaries |
| `agent_tasks` | `AgentTaskRepo` | Subagent task coordination |
| `areas` | `AreaRepo` | PARA areas |
| `projects` | `ProjectRepo` | PARA projects with stats |
| `sessions` | `SessionRepo` | Session CRUD, message persistence |
| `session_context` | `SessionContextRepo` | Session PARA categorization |
| `objectives` | `ObjectiveRepo` | OKR objectives |
| `key_results` | `KeyResultRepo` | OKR key results |
| `outcomes` | `OutcomeRepo` | Learning outcomes |
| `strategies` | `StrategyRepo` | Strategy records, tool stats |
| `usage` | `UsageRepo` | LLM cost/usage tracking |
| `cron` | `CronRepo` | Scheduled job persistence |
| `learning_state` | `LearningStateRepo` | Key-value learning state |
| `decision_log` | `DecisionLogRepo` | ReAct decision audit log |
| `user_profile` | `UserProfileRepo` | Explicit user facts |
| `behavioral_patterns` | `BehavioralPatternRepo` | Observed patterns |
| `agent_adaptations` | `AgentAdaptationRepo` | Per-agent preferences |
| `interaction_log` | `InteractionLogRepo` | Raw interaction data |
| `status_workflows` | `StatusWorkflowRepo` | Kanban workflow management |
| `task_groups` | `TaskGroupRepo` | Task group sections |
| `custom_columns` | `CustomColumnRepo` | Per-project custom fields |
| `entity_links` | `EntityLinkRepo` | Cross-feature entity links |
| `project_sources` | `ProjectSourceRepo` | Per-project AI context sources |
| `finance` | `FinanceStorage` | Sub-aggregate of 6 finance repos (accounts, transactions, budgets, investments, goals, liabilities) |

### How individual repos work

Each repo follows the same pattern:

1. Wraps a `SqlitePool` (stored as a field, not behind `Arc`).
2. Constructor: `fn new(pool: SqlitePool) -> Self`.
3. Methods use `sqlx::query` / `sqlx::query_as` with `&self.pool`.
4. Row structs derive `sqlx::FromRow` (defined in `crates/storage/src/rows/`).

## Vector Storage (LanceDB)

### VectorStore

`VectorStore` wraps an `Arc<lancedb::Connection>` and manages 5 embedding tables:

| Table | Extra columns (beyond `id` and `vector`) | Timestamp column |
|---|---|---|
| `todo_embeddings` | `model` | `updated_at` |
| `conv_embeddings` | `session_key`, `role`, `content_preview`, `full_content` | `created_at` |
| `cognitive_fact_embeddings` | `domain`, `text`, `importance`, `stability`, `confidence` | `updated_at` |
| `activity_embeddings` | `source`, `work_context_id`, `timestamp` | `updated_at` |
| `work_context_embeddings` | (none) | `updated_at` |

All tables share the same convention: `id` (Utf8), `vector` (FixedSizeList<Float32, 384>), extra string columns, then a timestamp column.

### Embeddings

- **Dimension:** 384 (matching `fastembed` / `paraphrase-multilingual-MiniLM-L12-v2`).
- **Distance metric:** Cosine similarity (`1.0 - distance`).
- **Search scores:** Returned as `score = 1.0 - distance`, filtered by a caller-specified threshold.

### Insert-then-delete upsert pattern

`upsert_embedding()` uses a crash-safe two-phase approach:

1. **Insert** the new row first. If a crash occurs here, no data is lost (no change visible).
2. **Delete** old rows matching the same `id` with an older timestamp. If a crash occurs here, a temporary duplicate exists, cleaned up on the next upsert or by `dedup_table()`.

This avoids the data-loss window of a delete-then-insert pattern.

### Deduplication

`dedup_table(table, ts_column)` performs a full table scan, groups rows by `id`, and deletes all but the newest row per ID. Intended for background maintenance.

### IVF-PQ Indexing

`ensure_indexes(min_rows)` creates IVF-PQ vector indexes on all 5 tables when they have enough rows to train the index. Safe to call repeatedly (no-op if index already exists). Uses cosine distance.

### Predicate sanitization

LanceDB does not support parameterized queries. String values are sanitized via `sanitize_predicate_value()`, which escapes single quotes and rejects semicolons, newlines, and SQL comment markers.

## Session Persistence

### SessionManager

`SessionManager` combines an in-memory cache with SQL persistence:

```
SessionManager
  sessions:   Arc<DashMap<String, Arc<TokioMutex<Session>>>>
  lru_order:  Arc<StdMutex<IndexMap<String, ()>>>
  sql_repo:   SessionRepo
```

It is `Clone + Send + Sync` without requiring external wrapping.

### Concurrency model

- Each session has its own `TokioMutex` -- concurrent access to *different* sessions proceeds without blocking.
- `DashMap` provides lock-free concurrent reads across sessions.
- LRU order is tracked in a `std::sync::Mutex<IndexMap>` (brief synchronous lock, O(1) promote/evict).

### LRU eviction

When a session is accessed via `get_or_create()`:

1. The session key is promoted to the back of the `IndexMap` (O(1) swap-remove + re-insert).
2. If the cache exceeds `max_cache_size`, the least-recently-used sessions are evicted from the front: saved to SQL, then removed from the `DashMap`.

### Compaction

When a session is saved and its SQL message count exceeds 1000 (`COMPACTION_THRESHOLD`):

1. A system-role compaction marker message is inserted: `"[Session compacted: N older messages removed]"`.
2. `compact_session()` deletes the oldest messages, keeping the most recent 500 (`COMPACTION_KEEP`).

### Batch INSERT

`save()` uses `batch_add_messages()` to persist all session messages in a single SQL round-trip with `ON CONFLICT DO NOTHING` for idempotency.

## Cognitive Memory Storage

The cognitive memory system (in `crates/cognitive/`) uses its own feature migrations applied via `StoragePool::run_feature_migrations()`.

### Semantic facts (bi-temporal, FSRS decay)

`SemanticFactRepo` manages SPO (subject-predicate-object) triples with two temporal dimensions:

- **Valid time:** `valid_from` / `valid_until` -- when the fact was true in the real world.
- **Transaction time:** `recorded_at` / `superseded_at` -- when the system learned about it.

**FSRS (Free Spaced Repetition Scheduler) decay:** Each fact has a `stability` score. `record_access()` updates stability and `access_count`. `list_low_stability()` finds facts that may need reinforcement or archival.

**Supersession:** When a fact is updated, the old fact is superseded (`supersede(old_id, new_id)`) rather than deleted. Superseded facts can be archived to `semantic_facts_archive` after a configurable delay via `archive_superseded(older_than_days)`.

**Reinstatement:** Archived facts can be reinstated to the active table via `reinstate_archived(id)`, which clears `superseded_at`, `superseded_by`, and `valid_until`.

### Episodic memories

`EpisodicMemoryRepo` stores event-based memories with `importance` scoring and FSRS-based `stability` decay. Indexed by domain and occurrence time.

### Procedural rules

`ProceduralRuleRepo` stores learned behavioral rules per domain. Rules have a `confidence` score, `signal_count`, and an `active` flag for activation/deactivation.

### FTS5 full-text search

Four FTS5 virtual tables provide BM25-ranked full-text search across cognitive memory:

- `semantic_facts_fts`: Searches across `domain`, `subject`, `predicate`, `object`, `memory_type`.
- `episodic_memories_fts`: Searches across `domain`, `content`, `summary`.
- `procedural_rules_fts`: Searches across `domain`, `rule_text`.
- `annotations_fts`: Searches across `target_type`, `target_id`, `content`, `tags`.

All use `porter unicode61` tokenization and stay synchronized via SQL triggers on `INSERT`, `UPDATE`, and `DELETE`.

Usage example via `SemanticFactRepo::search_fts()`:

```rust
let results = fact_repo.search_fts("morning routine", Some("productivity"), 10).await?;
```

### Archive tables

`semantic_facts_archive` provides cold storage for superseded facts. The archive shares the same schema as `semantic_facts` plus an `archived_at` timestamp. Archived facts are searchable via `search_archived()`.

## Data Retention

`Repos::cleanup_analytics()` deletes records older than fixed retention periods from analytics tables:

| Table | Retention period | Deletion method |
|---|---|---|
| `strategy_records` | 90 days | `StrategyRepo::delete_older_than()` |
| `learning_outcomes` | 30 days | `OutcomeRepo::delete_older_than()` |
| `interaction_log` | 60 days | `InteractionLogRepo::delete_older_than()` |
| `tool_usage` | 90 days | Direct SQL (`DELETE WHERE created_at < cutoff`) |
| `enrichment_feedback` | 90 days | Direct SQL (`DELETE WHERE timestamp < cutoff`) |

The method returns the total number of rows deleted across all tables.

## Testing Patterns

### In-memory databases

All tests use `StoragePool::connect_in_memory()`, which creates an ephemeral SQLite database with all core migrations applied. No external database setup is required.

```rust
#[tokio::test]
async fn test_something() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repos = Repos::from_pool(&pool);
    // ... test using repos ...
}
```

For cognitive feature tests, a helper creates the pool and applies feature migrations:

```rust
let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
sqlx::migrate!("../storage/migrations").run(&pool).await.unwrap();
StoragePool::run_feature_migrations(&pool, &cognitive_migrations()).await.unwrap();
```

### Gotcha: `from_existing()` skips migrations

`StoragePool::from_existing(pool)` wraps a raw `sqlx::SqlitePool` **without running any migrations**. This is only safe for pools that have already been migrated. Using it on a fresh pool will cause "no such table" errors at runtime. Tests must always use `connect_in_memory()`.
