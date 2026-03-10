# Phase 2: Work Context Engine Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an automatic work-context inference engine that clusters activity events into meaningful "Work Contexts" using semantic embeddings, temporal proximity, and resource overlap.

**Architecture:** New types, repos, and inference engine inside the existing `activity-log` crate. Two new LanceDB tables in `storage::VectorStore`. A `ContextSource` impl feeds active contexts into LLM system prompts. A background loop runs inference every 5 minutes. Config via `config::WorkContextConfig`.

**Tech Stack:** Rust, SQLite (sqlx), LanceDB (lancedb), chrono, serde, async-trait, tokio

---

## Spec Corrections (vs. original Phase 2 doc)

These divergences from `docs/plans/2026-03-10-phase2-claude-code-prompt.md` were identified during codebase review:

1. **ContextSource trait** — actual signature is `provide(&self, &SourceContext) -> Option<String>` with `priority() -> u8`, NOT `ContextRequest`/`ContextBlock`/`u32`
2. **Tool pattern** — codebase uses manual `impl Tool for X`, NOT `#[derive(Tool)]`/`#[tool_actions]`
3. **TextEmbedder** — confirmed exact match: `async fn embed(&self, text: &str) -> common::Result<Vec<f32>>`
4. **VectorStore** — uses generic `upsert_embedding(table, id, &vec, &[extra_fields])` and `search_similar(table, &query, limit, threshold)` — no table-specific methods needed. Note: `search_similar` returns `(id, score)` pairs only — no raw vector retrieval. Context centroids must be cached in-memory.
5. **TextEmbedder impl** — `agent::cognitive_embedder::TextEmbedderImpl` wraps `tools::EmbeddingEngine`. NOT `SemanticFactEmbedderImpl` (which implements `SemanticFactEmbedder`, a different trait).
6. **Context source registration** — happens in `agent/src/agent_loop/builder.rs` (the `sources` vec), NOT in `app-core/init.rs`.
7. **`work_context_id` column** — already exists in Phase 1 migration (`unified_activity_log.work_context_id TEXT`), indexed. No migration change needed for `process_recent_events` to query `WHERE work_context_id IS NULL`.
8. **`shutdown_token`** — exists in `init.rs` line 223 as `let shutdown_token = CancellationToken::new()`. Use `shutdown_token.child_token()` for background loops.

### Deliberate deferrals (from spec, not in this plan):
- `resume_context` tool action (spec §7) — deferred, can be added later
- `check_merge_candidates` method — deferred, merge detection is a follow-up
- `embedding_batch_size` config field — deferred, batch processing not needed yet
- Daily `archive_dormant_contexts` cron — deferred, can be added via cron callback later

---

## File Structure

### New files to create:
| File | Responsibility |
|------|---------------|
| `crates/activity-log/migrations/002_work_contexts.sql` | 5 tables: work_contexts, work_resources, resource_edges, work_context_resources, work_context_actions |
| `crates/activity-log/src/work_context_repo.rs` | CRUD + stats + archive + merge for work_contexts |
| `crates/activity-log/src/work_resource_repo.rs` | Upsert + lookup for work_resources |
| `crates/activity-log/src/resource_edge_repo.rs` | Co-occurrence graph edges |
| `crates/activity-log/src/context_resource_repo.rs` | Context-to-resource membership |
| `crates/activity-log/src/context_action_repo.rs` | Context-to-action links |
| `crates/activity-log/src/inference.rs` | Core 3-factor scoring engine |
| `crates/activity-log/src/context_source.rs` | `ContextSource` trait impl |
| `crates/activity-log/src/inference_loop.rs` | Background tokio task |
| `crates/tools/src/work_context_tool.rs` | Agent tool (manual `impl Tool`) |
| `crates/config/src/schema/work_context.rs` | WorkContextConfig |

### Files to modify:
| File | Change |
|------|--------|
| `crates/activity-log/src/types.rs` | Add WorkContext, WorkContextStatus, WorkContextType, WorkResource, ResourceEdge, ContextAssignment |
| `crates/activity-log/src/lib.rs` | Add modules, exports, migration v2 |
| `crates/activity-log/Cargo.toml` | Add `cognitive` dependency (for TextEmbedder trait) |
| `crates/storage/src/vector_store.rs` | Add 2 schema fns + ensure_table calls in `connect()` |
| `crates/config/src/schema/mod.rs` | Add `work_context` module |
| `crates/config/src/schema/core.rs` | Add `work_context: WorkContextConfig` field to `Config` |
| `crates/app-core/src/init.rs` | Wire migration, inference engine, background loop, context source |
| `crates/app-core/src/state.rs` | (Optional) Add inference engine ref if needed for shutdown |
| `crates/tools/src/lib.rs` or `mod.rs` | Register work_context_tool module |
| `crates/agent/src/agent_loop/builder.rs` | Register WorkContextTool + wire ActivityIngestionService for Gap 1/2 |
| `crates/agent/src/agent_loop/mod.rs` | Add chat message ingestion calls (Gap 1) |

---

## Chunk 1: Migration + Types + Config

### Task 1: Create migration SQL

**Files:**
- Create: `crates/activity-log/migrations/002_work_contexts.sql`

- [ ] **Step 1: Write migration file**

```sql
-- Work contexts: inferred units of work
CREATE TABLE IF NOT EXISTS work_contexts (
    id                  TEXT PRIMARY KEY,
    title               TEXT NOT NULL,
    description         TEXT,
    status              TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'paused', 'completed', 'archived')),
    context_type        TEXT NOT NULL DEFAULT 'general'
                        CHECK (context_type IN ('coding', 'research', 'communication',
                        'planning', 'review', 'meeting', 'learning', 'general')),
    embedding_id        TEXT,
    linked_project_id   TEXT,
    color               TEXT,
    tags                TEXT DEFAULT '[]',
    confidence          REAL NOT NULL DEFAULT 0.5,
    first_seen_at       TEXT NOT NULL,
    last_active_at      TEXT NOT NULL,
    total_duration_secs INTEGER NOT NULL DEFAULT 0,
    event_count         INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_wc_status ON work_contexts(status);
CREATE INDEX IF NOT EXISTS idx_wc_last_active ON work_contexts(last_active_at);
CREATE INDEX IF NOT EXISTS idx_wc_project ON work_contexts(linked_project_id);

-- Resource registry
CREATE TABLE IF NOT EXISTS work_resources (
    id              TEXT PRIMARY KEY,
    resource_type   TEXT NOT NULL CHECK (resource_type IN
                    ('file', 'url', 'repo', 'note', 'conversation', 'app', 'command')),
    resource_name   TEXT NOT NULL,
    resource_path   TEXT,
    resource_uri    TEXT,
    first_seen_at   TEXT NOT NULL,
    last_seen_at    TEXT NOT NULL,
    access_count    INTEGER NOT NULL DEFAULT 0,
    embedding_id    TEXT
);

CREATE INDEX IF NOT EXISTS idx_wr_type ON work_resources(resource_type);
CREATE INDEX IF NOT EXISTS idx_wr_name ON work_resources(resource_name);

-- Resource co-occurrence graph edges
CREATE TABLE IF NOT EXISTS resource_edges (
    source_id       TEXT NOT NULL,
    target_id       TEXT NOT NULL,
    edge_type       TEXT NOT NULL CHECK (edge_type IN
                    ('co_access', 'references', 'derived_from', 'related')),
    weight          REAL NOT NULL DEFAULT 1.0,
    first_seen_at   TEXT NOT NULL,
    last_seen_at    TEXT NOT NULL,
    PRIMARY KEY (source_id, target_id, edge_type)
);

CREATE INDEX IF NOT EXISTS idx_re_source ON resource_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_re_target ON resource_edges(target_id);

-- Context-to-resource membership
CREATE TABLE IF NOT EXISTS work_context_resources (
    context_id          TEXT NOT NULL,
    resource_id         TEXT NOT NULL,
    relevance_score     REAL NOT NULL DEFAULT 0.5,
    first_associated_at TEXT NOT NULL,
    last_associated_at  TEXT NOT NULL,
    PRIMARY KEY (context_id, resource_id)
);

-- Context-to-action links
CREATE TABLE IF NOT EXISTS work_context_actions (
    context_id  TEXT NOT NULL,
    action_id   TEXT NOT NULL,
    linked_at   TEXT NOT NULL,
    PRIMARY KEY (context_id, action_id)
);
```

- [ ] **Step 2: Verify file exists**

Run: `cat crates/activity-log/migrations/002_work_contexts.sql | head -5`
Expected: First 5 lines of the SQL file.

---

### Task 2: Add types to activity-log

**Files:**
- Modify: `crates/activity-log/src/types.rs`

- [ ] **Step 1: Write tests for new type serialization**

Add to the bottom of `types.rs`, inside a new `#[cfg(test)] mod tests` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_work_context_status_roundtrip() {
        let s = WorkContextStatus::Active;
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(json, "\"active\"");
        let parsed: WorkContextStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, WorkContextStatus::Active);
    }

    #[test]
    fn test_work_context_type_roundtrip() {
        let t = WorkContextType::Coding;
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"coding\"");
        let parsed: WorkContextType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, WorkContextType::Coding);
    }

    #[test]
    fn test_work_context_status_as_str() {
        assert_eq!(WorkContextStatus::Active.as_str(), "active");
        assert_eq!(WorkContextStatus::Archived.as_str(), "archived");
    }

    #[test]
    fn test_work_context_status_parse() {
        assert_eq!(WorkContextStatus::parse("active"), Some(WorkContextStatus::Active));
        assert_eq!(WorkContextStatus::parse("invalid"), None);
    }

    #[test]
    fn test_work_context_type_as_str() {
        assert_eq!(WorkContextType::Coding.as_str(), "coding");
        assert_eq!(WorkContextType::General.as_str(), "general");
    }

    #[test]
    fn test_work_context_type_parse() {
        assert_eq!(WorkContextType::parse("coding"), Some(WorkContextType::Coding));
        assert_eq!(WorkContextType::parse("invalid"), None);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p activity-log -E 'test(work_context)'`
Expected: FAIL — types don't exist yet.

- [ ] **Step 3: Add types**

Add after `ActivityLogEntry` and its impls in `types.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkContext {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: WorkContextStatus,
    pub context_type: WorkContextType,
    pub embedding_id: Option<String>,
    pub linked_project_id: Option<String>,
    pub color: Option<String>,
    pub tags: Vec<String>,
    pub confidence: f64,
    pub first_seen_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    pub total_duration_secs: i64,
    pub event_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkContextStatus {
    Active,
    Paused,
    Completed,
    Archived,
}

impl WorkContextStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "active" => Some(Self::Active),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkContextType {
    Coding,
    Research,
    Communication,
    Planning,
    Review,
    Meeting,
    Learning,
    General,
}

impl WorkContextType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Coding => "coding",
            Self::Research => "research",
            Self::Communication => "communication",
            Self::Planning => "planning",
            Self::Review => "review",
            Self::Meeting => "meeting",
            Self::Learning => "learning",
            Self::General => "general",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "coding" => Some(Self::Coding),
            "research" => Some(Self::Research),
            "communication" => Some(Self::Communication),
            "planning" => Some(Self::Planning),
            "review" => Some(Self::Review),
            "meeting" => Some(Self::Meeting),
            "learning" => Some(Self::Learning),
            "general" => Some(Self::General),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkResource {
    pub id: String,
    pub resource_type: String,
    pub resource_name: String,
    pub resource_path: Option<String>,
    pub resource_uri: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub access_count: i64,
    pub embedding_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceEdge {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub weight: f64,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextAssignment {
    pub event_id: String,
    pub context_id: String,
    pub is_new_context: bool,
    pub similarity_score: f64,
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p activity-log -E 'test(work_context)'`
Expected: All 6 tests PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/activity-log/migrations/002_work_contexts.sql crates/activity-log/src/types.rs
git commit -m "feat(activity-log): add work context migration and types"
```

---

### Task 3: Add WorkContextConfig

**Files:**
- Create: `crates/config/src/schema/work_context.rs`
- Modify: `crates/config/src/schema/mod.rs`
- Modify: `crates/config/src/schema/core.rs`

- [ ] **Step 1: Create config file**

```rust
use serde::{Deserialize, Serialize};

/// Configuration for the Work Context inference engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkContextConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_inference_interval")]
    pub inference_interval_mins: u64,

    #[serde(default = "default_assignment_threshold")]
    pub assignment_threshold: f64,

    #[serde(default = "default_merge_threshold")]
    pub merge_threshold: f64,

    #[serde(default = "default_max_dormancy_days")]
    pub max_dormancy_days: f64,

    #[serde(default = "default_max_active_contexts")]
    pub max_active_contexts: usize,

    #[serde(default = "default_semantic_weight")]
    pub semantic_weight: f64,

    #[serde(default = "default_temporal_weight")]
    pub temporal_weight: f64,

    #[serde(default = "default_resource_weight")]
    pub resource_weight: f64,
}

impl Default for WorkContextConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            inference_interval_mins: default_inference_interval(),
            assignment_threshold: default_assignment_threshold(),
            merge_threshold: default_merge_threshold(),
            max_dormancy_days: default_max_dormancy_days(),
            max_active_contexts: default_max_active_contexts(),
            semantic_weight: default_semantic_weight(),
            temporal_weight: default_temporal_weight(),
            resource_weight: default_resource_weight(),
        }
    }
}

fn default_true() -> bool { true }
fn default_inference_interval() -> u64 { 5 }
fn default_assignment_threshold() -> f64 { 0.55 }
fn default_merge_threshold() -> f64 { 0.85 }
fn default_max_dormancy_days() -> f64 { 7.0 }
fn default_max_active_contexts() -> usize { 50 }
fn default_semantic_weight() -> f64 { 0.50 }
fn default_temporal_weight() -> f64 { 0.25 }
fn default_resource_weight() -> f64 { 0.25 }
```

- [ ] **Step 2: Add module to schema/mod.rs**

Add `mod work_context;` and `pub use self::work_context::*;` following the existing pattern.

- [ ] **Step 3: Add field to Config in core.rs**

Add to the `Config` struct (after `cognitive`):

```rust
    /// Work context inference engine configuration.
    #[serde(default)]
    pub work_context: WorkContextConfig,
```

Add the import: `use super::work_context::WorkContextConfig;`

- [ ] **Step 4: Run existing config tests**

Run: `cargo nextest run -p config`
Expected: All tests PASS (new field defaults correctly).

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/work_context.rs crates/config/src/schema/mod.rs crates/config/src/schema/core.rs
git commit -m "feat(config): add WorkContextConfig for inference engine"
```

---

## Chunk 2: Repository Layer (5 repos)

### Task 4: WorkContextRepo

**Files:**
- Create: `crates/activity-log/src/work_context_repo.rs`
- Modify: `crates/activity-log/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Create `work_context_repo.rs` with the test module first. Tests need the migration helper from service.rs tests. The setup function runs both migrations (001 + 002).

```rust
use chrono::{DateTime, Utc};
use storage::{StorageError, StoragePool};

use crate::types::{WorkContext, WorkContextStatus, WorkContextType};

pub struct WorkContextRepo;

// ... (methods will be added in Step 3)

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalizers::new_ulid;

    async fn setup() -> StoragePool {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &crate::ActivityLog::migrations_static(),
        )
        .await
        .unwrap();
        pool
    }

    fn make_context(title: &str) -> WorkContext {
        let now = Utc::now();
        WorkContext {
            id: new_ulid(),
            title: title.to_string(),
            description: None,
            status: WorkContextStatus::Active,
            context_type: WorkContextType::Coding,
            embedding_id: None,
            linked_project_id: None,
            color: None,
            tags: vec![],
            confidence: 0.7,
            first_seen_at: now,
            last_active_at: now,
            total_duration_secs: 0,
            event_count: 0,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let pool = setup().await;
        let ctx = make_context("Test Context");
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        let loaded = WorkContextRepo::get(&pool, &ctx.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().title, "Test Context");
    }

    #[tokio::test]
    async fn test_list_active() {
        let pool = setup().await;
        let c1 = make_context("Active 1");
        let mut c2 = make_context("Archived");
        c2.status = WorkContextStatus::Archived;
        WorkContextRepo::insert(&pool, &c1).await.unwrap();
        WorkContextRepo::insert(&pool, &c2).await.unwrap();
        let active = WorkContextRepo::list_active(&pool).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].title, "Active 1");
    }

    #[tokio::test]
    async fn test_update_stats() {
        let pool = setup().await;
        let ctx = make_context("Stats Test");
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        WorkContextRepo::update_stats(&pool, &ctx.id, Utc::now(), 300, 5).await.unwrap();
        let loaded = WorkContextRepo::get(&pool, &ctx.id).await.unwrap().unwrap();
        assert_eq!(loaded.total_duration_secs, 300);
        assert_eq!(loaded.event_count, 5);
    }

    #[tokio::test]
    async fn test_archive_dormant() {
        let pool = setup().await;
        let mut ctx = make_context("Old Context");
        ctx.last_active_at = Utc::now() - chrono::Duration::days(30);
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        let archived = WorkContextRepo::archive_dormant(&pool, 7).await.unwrap();
        assert_eq!(archived, 1);
        let loaded = WorkContextRepo::get(&pool, &ctx.id).await.unwrap().unwrap();
        assert_eq!(loaded.status, WorkContextStatus::Archived);
    }

    #[tokio::test]
    async fn test_search_by_title() {
        let pool = setup().await;
        let ctx = make_context("Coding: activity-log crate");
        WorkContextRepo::insert(&pool, &ctx).await.unwrap();
        let results = WorkContextRepo::search_by_title(&pool, "activity").await.unwrap();
        assert_eq!(results.len(), 1);
    }
}
```

- [ ] **Step 2: Update lib.rs — add module + migration**

Add to `lib.rs`:
```rust
pub mod work_context_repo;
```

Update `migrations_static()` to include v2:
```rust
pub fn migrations_static() -> Vec<FeatureMigration> {
    vec![
        FeatureMigration {
            feature_name: "activity_log".to_string(),
            version: 1,
            description: "Create unified activity log table".to_string(),
            sql: include_str!("../migrations/001_unified_activity_log.sql").to_string(),
        },
        FeatureMigration {
            feature_name: "activity_log".to_string(),
            version: 2,
            description: "Create work context tables".to_string(),
            sql: include_str!("../migrations/002_work_contexts.sql").to_string(),
        },
    ]
}
```

Re-export: `pub use work_context_repo::WorkContextRepo;`

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p activity-log -E 'test(work_context_repo)'`
Expected: FAIL — methods not implemented.

- [ ] **Step 4: Implement WorkContextRepo methods**

Full implementation with: `insert`, `get`, `update`, `list_active`, `list_by_status`, `list_by_project`, `update_stats`, `update_embedding`, `archive_dormant`, `merge`, `search_by_title`.

Key patterns (matching existing `ActivityLogRepo`):
- Use `StorageError::from` for error mapping
- Use `sqlx::query_as::<_, RawRow>` with `From<RawRow>` conversion
- Tags stored as JSON string, parsed with `serde_json`
- Timestamps as RFC3339 strings

```rust
impl WorkContextRepo {
    pub async fn insert(pool: &StoragePool, ctx: &WorkContext) -> common::Result<()> {
        let tags_json = serde_json::to_string(&ctx.tags).unwrap_or_else(|_| "[]".to_string());
        sqlx::query(
            "INSERT INTO work_contexts (id, title, description, status, context_type, \
             embedding_id, linked_project_id, color, tags, confidence, first_seen_at, \
             last_active_at, total_duration_secs, event_count, created_at, updated_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)"
        )
        .bind(&ctx.id)
        .bind(&ctx.title)
        .bind(&ctx.description)
        .bind(ctx.status.as_str())
        .bind(ctx.context_type.as_str())
        .bind(&ctx.embedding_id)
        .bind(&ctx.linked_project_id)
        .bind(&ctx.color)
        .bind(&tags_json)
        .bind(ctx.confidence)
        .bind(ctx.first_seen_at.to_rfc3339())
        .bind(ctx.last_active_at.to_rfc3339())
        .bind(ctx.total_duration_secs)
        .bind(ctx.event_count)
        .bind(ctx.created_at.to_rfc3339())
        .bind(ctx.updated_at.to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn get(pool: &StoragePool, id: &str) -> common::Result<Option<WorkContext>> {
        let row = sqlx::query_as::<_, WcRawRow>(
            "SELECT id, title, description, status, context_type, embedding_id, \
             linked_project_id, color, tags, confidence, first_seen_at, last_active_at, \
             total_duration_secs, event_count, created_at, updated_at \
             FROM work_contexts WHERE id = ?1"
        )
        .bind(id)
        .fetch_optional(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(row.map(Into::into))
    }

    pub async fn update(pool: &StoragePool, ctx: &WorkContext) -> common::Result<()> {
        let tags_json = serde_json::to_string(&ctx.tags).unwrap_or_else(|_| "[]".to_string());
        sqlx::query(
            "UPDATE work_contexts SET title=?2, description=?3, status=?4, context_type=?5, \
             embedding_id=?6, linked_project_id=?7, color=?8, tags=?9, confidence=?10, \
             last_active_at=?11, total_duration_secs=?12, event_count=?13, updated_at=?14 \
             WHERE id=?1"
        )
        .bind(&ctx.id)
        .bind(&ctx.title)
        .bind(&ctx.description)
        .bind(ctx.status.as_str())
        .bind(ctx.context_type.as_str())
        .bind(&ctx.embedding_id)
        .bind(&ctx.linked_project_id)
        .bind(&ctx.color)
        .bind(&tags_json)
        .bind(ctx.confidence)
        .bind(ctx.last_active_at.to_rfc3339())
        .bind(ctx.total_duration_secs)
        .bind(ctx.event_count)
        .bind(Utc::now().to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn list_active(pool: &StoragePool) -> common::Result<Vec<WorkContext>> {
        let rows = sqlx::query_as::<_, WcRawRow>(
            &format!("SELECT {WC_COLS} FROM work_contexts WHERE status = 'active' \
                      ORDER BY last_active_at DESC")
        )
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_by_status(pool: &StoragePool, status: WorkContextStatus, limit: i64) -> common::Result<Vec<WorkContext>> {
        let rows = sqlx::query_as::<_, WcRawRow>(
            &format!("SELECT {WC_COLS} FROM work_contexts WHERE status = ?1 \
                      ORDER BY last_active_at DESC LIMIT ?2")
        )
        .bind(status.as_str())
        .bind(limit)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn list_by_project(pool: &StoragePool, project_id: &str) -> common::Result<Vec<WorkContext>> {
        let rows = sqlx::query_as::<_, WcRawRow>(
            &format!("SELECT {WC_COLS} FROM work_contexts WHERE linked_project_id = ?1 \
                      ORDER BY last_active_at DESC")
        )
        .bind(project_id)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn update_stats(
        pool: &StoragePool,
        id: &str,
        last_active_at: DateTime<Utc>,
        duration_increment: i64,
        event_increment: i64,
    ) -> common::Result<()> {
        sqlx::query(
            "UPDATE work_contexts SET \
             last_active_at = ?2, \
             total_duration_secs = total_duration_secs + ?3, \
             event_count = event_count + ?4, \
             updated_at = ?5 \
             WHERE id = ?1"
        )
        .bind(id)
        .bind(last_active_at.to_rfc3339())
        .bind(duration_increment)
        .bind(event_increment)
        .bind(Utc::now().to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn update_embedding(pool: &StoragePool, id: &str, embedding_id: &str) -> common::Result<()> {
        sqlx::query("UPDATE work_contexts SET embedding_id = ?2, updated_at = ?3 WHERE id = ?1")
            .bind(id)
            .bind(embedding_id)
            .bind(Utc::now().to_rfc3339())
            .execute(pool.inner())
            .await
            .map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn archive_dormant(pool: &StoragePool, dormancy_days: i64) -> common::Result<u64> {
        // Called with config.work_context.max_dormancy_days as f64 — cast at call site
        let cutoff = Utc::now() - chrono::Duration::days(dormancy_days);
        let result = sqlx::query(
            "UPDATE work_contexts SET status = 'archived', updated_at = ?2 \
             WHERE status = 'active' AND last_active_at < ?1"
        )
        .bind(cutoff.to_rfc3339())
        .bind(Utc::now().to_rfc3339())
        .execute(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(result.rows_affected())
    }

    pub async fn merge(pool: &StoragePool, keep_id: &str, remove_id: &str) -> common::Result<()> {
        let mut tx = pool.inner().begin().await.map_err(StorageError::from)?;

        // Transfer resources
        sqlx::query(
            "UPDATE OR IGNORE work_context_resources SET context_id = ?1 WHERE context_id = ?2"
        )
        .bind(keep_id)
        .bind(remove_id)
        .execute(&mut *tx)
        .await
        .map_err(StorageError::from)?;

        // Transfer actions
        sqlx::query(
            "UPDATE OR IGNORE work_context_actions SET context_id = ?1 WHERE context_id = ?2"
        )
        .bind(keep_id)
        .bind(remove_id)
        .execute(&mut *tx)
        .await
        .map_err(StorageError::from)?;

        // Delete orphaned rows that conflicted
        sqlx::query("DELETE FROM work_context_resources WHERE context_id = ?1")
            .bind(remove_id)
            .execute(&mut *tx)
            .await
            .map_err(StorageError::from)?;
        sqlx::query("DELETE FROM work_context_actions WHERE context_id = ?1")
            .bind(remove_id)
            .execute(&mut *tx)
            .await
            .map_err(StorageError::from)?;

        // Delete removed context
        sqlx::query("DELETE FROM work_contexts WHERE id = ?1")
            .bind(remove_id)
            .execute(&mut *tx)
            .await
            .map_err(StorageError::from)?;

        tx.commit().await.map_err(StorageError::from)?;
        Ok(())
    }

    pub async fn search_by_title(pool: &StoragePool, query: &str) -> common::Result<Vec<WorkContext>> {
        let pattern = format!("%{query}%");
        let rows = sqlx::query_as::<_, WcRawRow>(
            &format!("SELECT {WC_COLS} FROM work_contexts WHERE title LIKE ?1 \
                      ORDER BY last_active_at DESC")
        )
        .bind(&pattern)
        .fetch_all(pool.inner())
        .await
        .map_err(StorageError::from)?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}
```

Also add `WcRawRow` struct and `From<WcRawRow> for WorkContext` conversion (same pattern as `RawRow` in `repo.rs`), and `const WC_COLS` for the SELECT column list.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p activity-log -E 'test(work_context_repo)'`
Expected: All 5 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/activity-log/src/work_context_repo.rs crates/activity-log/src/lib.rs
git commit -m "feat(activity-log): add WorkContextRepo with CRUD + archive + merge"
```

---

### Task 5: WorkResourceRepo

**Files:**
- Create: `crates/activity-log/src/work_resource_repo.rs`
- Modify: `crates/activity-log/src/lib.rs`

- [ ] **Step 1: Write tests + implementation**

Same pattern as Task 4. Methods: `upsert`, `get`, `find_by_name`, `find_by_path`, `find_by_uri`, `list_by_context` (JOIN with work_context_resources), `list_recent`.

Key: `upsert` uses `INSERT ... ON CONFLICT(id) DO UPDATE SET last_seen_at = excluded.last_seen_at, access_count = access_count + 1`.

Tests:
- `test_upsert_and_get` — insert, verify, upsert again, verify access_count incremented
- `test_find_by_path` — insert with path, find by path
- `test_list_recent` — insert 3, verify order by last_seen_at DESC

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p activity-log -E 'test(work_resource)'`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/activity-log/src/work_resource_repo.rs crates/activity-log/src/lib.rs
git commit -m "feat(activity-log): add WorkResourceRepo with upsert + lookups"
```

---

### Task 6: ResourceEdgeRepo

**Files:**
- Create: `crates/activity-log/src/resource_edge_repo.rs`
- Modify: `crates/activity-log/src/lib.rs`

- [ ] **Step 1: Write tests + implementation**

Methods: `upsert` (increment weight on conflict), `get_neighbors` (JOIN work_resources), `get_co_accessed` (filter by edge_type='co_access' and min_weight).

Tests:
- `test_upsert_increments_weight` — upsert twice, verify weight=2.0
- `test_get_neighbors` — create 2 resources + edge, verify neighbor returned with weight

- [ ] **Step 2: Run tests, commit**

Run: `cargo nextest run -p activity-log -E 'test(resource_edge)'`

```bash
git add crates/activity-log/src/resource_edge_repo.rs crates/activity-log/src/lib.rs
git commit -m "feat(activity-log): add ResourceEdgeRepo for co-occurrence graph"
```

---

### Task 7: ContextResourceRepo

**Files:**
- Create: `crates/activity-log/src/context_resource_repo.rs`
- Modify: `crates/activity-log/src/lib.rs`

- [ ] **Step 1: Write tests + implementation**

Methods: `link` (upsert), `unlink`, `list_for_context` (JOIN work_resources, returns Vec<(WorkResource, f64)>), `list_contexts_for_resource` (reverse JOIN), `update_relevance`.

Tests:
- `test_link_and_list` — link resource to context, list, verify
- `test_unlink` — link then unlink, verify empty
- `test_update_relevance` — link with 0.5, update to 0.9, verify

- [ ] **Step 2: Run tests, commit**

Run: `cargo nextest run -p activity-log -E 'test(context_resource)'`

```bash
git add crates/activity-log/src/context_resource_repo.rs crates/activity-log/src/lib.rs
git commit -m "feat(activity-log): add ContextResourceRepo for context-resource membership"
```

---

### Task 8: ContextActionRepo

**Files:**
- Create: `crates/activity-log/src/context_action_repo.rs`
- Modify: `crates/activity-log/src/lib.rs`

- [ ] **Step 1: Write tests + implementation**

Methods: `link`, `unlink`, `list_actions_for_context`, `list_contexts_for_action`.

Tests:
- `test_link_and_list_actions` — link action, list for context, verify
- `test_list_contexts_for_action` — link 2 contexts to same action, verify both returned

- [ ] **Step 2: Run tests, commit**

Run: `cargo nextest run -p activity-log -E 'test(context_action)'`

```bash
git add crates/activity-log/src/context_action_repo.rs crates/activity-log/src/lib.rs
git commit -m "feat(activity-log): add ContextActionRepo"
```

---

## Chunk 3: Vector Store + Inference Engine

### Task 9: Add LanceDB table schemas

**Files:**
- Modify: `crates/storage/src/vector_store.rs`

- [ ] **Step 1: Add schema functions**

After `cognitive_fact_schema()`, add:

```rust
fn activity_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("source", DataType::Utf8, false),       // "event" | "bucket_summary"
        Field::new("work_context_id", DataType::Utf8, false), // "" if unassigned
        Field::new("timestamp", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}

fn work_context_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}
```

- [ ] **Step 2: Register tables in connect()**

Add after the `cognitive_fact_embeddings` ensure_table call:

```rust
store.ensure_table("activity_embeddings", activity_embedding_schema()).await?;
store.ensure_table("work_context_embeddings", work_context_embedding_schema()).await?;
```

- [ ] **Step 3: Update docstring + ensure_indexes**

Update the VectorStore doc comment to list the 2 new tables. Also update `ensure_indexes()` to include the new tables in its hardcoded list (so IVF-PQ indexes are built when enough rows exist).

- [ ] **Step 4: Run storage tests**

Run: `cargo nextest run -p storage`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/vector_store.rs
git commit -m "feat(storage): add activity_embeddings and work_context_embeddings LanceDB tables"
```

---

### Task 10: Inference Engine

**Files:**
- Create: `crates/activity-log/src/inference.rs`
- Modify: `crates/activity-log/Cargo.toml` (add `cognitive` dep)
- Modify: `crates/activity-log/src/lib.rs`

This is the core intelligence. The engine scores events against active contexts using 3 factors: semantic similarity, temporal proximity, and resource overlap.

- [ ] **Step 1: Add cognitive dependency**

In `Cargo.toml`, add:
```toml
cognitive.workspace = true
```

- [ ] **Step 2: Write test scaffold with MockTextEmbedder**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalizers::new_ulid;
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    struct MockTextEmbedder;

    #[async_trait::async_trait]
    impl cognitive::TextEmbedder for MockTextEmbedder {
        async fn embed(&self, text: &str) -> common::Result<Vec<f32>> {
            let mut hasher = DefaultHasher::new();
            text.hash(&mut hasher);
            let seed = hasher.finish();
            let mut rng_state = seed;
            let vec: Vec<f32> = (0..384).map(|_| {
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((rng_state >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0
            }).collect();
            let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
            Ok(vec.into_iter().map(|x| x / norm).collect())
        }
    }

    // Tests:
    // 1. test_assign_event_creates_new_context — no existing contexts → new context
    // 2. test_assign_event_matches_existing — insert context with similar event → assigns to existing
    // 3. test_temporal_proximity_scoring — recent context scores higher than old
    // 4. test_resource_overlap_scoring — shared resources boost score
    // 5. test_centroid_ema_update — centroid moves toward new event
}
```

**Important:** The inference engine does NOT directly use VectorStore for the core scoring loop. It:
- Uses `TextEmbedder` to generate embeddings
- Stores/retrieves centroids via `VectorStore` (for persistence)
- But the actual scoring uses in-memory cosine similarity between event embedding and context centroid

For tests without LanceDB, the engine should work with an `Option<VectorStore>` — when None, centroids are not persisted (test-only path). When Some, centroids are persisted to `work_context_embeddings`.

- [ ] **Step 3: Implement ContextInferenceConfig + ContextInferenceEngine**

```rust
use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use storage::{StoragePool, VectorStore};
use tracing::{debug, warn};

use crate::normalizers::new_ulid;
use crate::types::*;
use crate::work_context_repo::WorkContextRepo;
use crate::work_resource_repo::WorkResourceRepo;
use crate::context_resource_repo::ContextResourceRepo;
use crate::resource_edge_repo::ResourceEdgeRepo;

pub struct ContextInferenceConfig {
    pub assignment_threshold: f64,
    pub merge_threshold: f64,
    pub coherence_threshold: f64,
    pub temporal_gap_hours: f64,
    pub max_dormancy_days: f64,
    pub centroid_learning_rate: f64,
    pub semantic_weight: f64,
    pub temporal_weight: f64,
    pub resource_weight: f64,
    pub temporal_decay_lambda: f64,
    pub max_active_contexts: usize,
}

impl Default for ContextInferenceConfig { /* use spec defaults */ }

pub struct ContextInferenceEngine {
    pool: StoragePool,
    embedder: Arc<dyn cognitive::TextEmbedder>,
    vector_store: Option<VectorStore>,
    config: ContextInferenceConfig,
    /// In-memory centroid cache: context_id → 384-dim embedding vector.
    /// VectorStore's search_similar only returns (id, score), not raw vectors,
    /// so we must cache centroids here. Persisted to LanceDB on update for durability.
    centroids: tokio::sync::RwLock<std::collections::HashMap<String, Vec<f32>>>,
}
```

Core method: `assign_event(&self, event: &ActivityLogEntry) -> Result<ContextAssignment>`

Algorithm:
1. Build embedding text from event fields
2. `self.embedder.embed(&text)` → `event_vec`
3. Get active contexts via `WorkContextRepo::list_active`
4. For each context, compute score:
   - **Semantic**: cosine_similarity(event_vec, context_centroid) — get centroid from `vector_store.search_similar("work_context_embeddings", ...)`  or keep in-memory cache
   - **Temporal**: `exp(-λ * hours_since_last_active)`
   - **Resource**: Jaccard overlap between event resource IDs and context resource IDs
   - **Combined**: `α*semantic + β*temporal + γ*resource`
5. If best score ≥ threshold → assign, else create new context
6. Update stats, centroid (EMA), link resources

Helper: `fn cosine_similarity(a: &[f32], b: &[f32]) -> f64`

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p activity-log -E 'test(inference)'`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/activity-log/src/inference.rs crates/activity-log/Cargo.toml crates/activity-log/src/lib.rs
git commit -m "feat(activity-log): add ContextInferenceEngine with 3-factor scoring"
```

---

## Chunk 4: Context Source + Tool + Background Loop + Wiring

### Task 11: WorkContextSource (ContextSource impl)

**Files:**
- Create: `crates/activity-log/src/context_source.rs`
- Modify: `crates/activity-log/Cargo.toml` (add `context_engine` dep)
- Modify: `crates/activity-log/src/lib.rs`

- [ ] **Step 1: Implement**

```rust
use async_trait::async_trait;
use context_engine::source::{ContextSource, SourceContext};
use storage::StoragePool;

use crate::work_context_repo::WorkContextRepo;
use crate::context_resource_repo::ContextResourceRepo;

pub struct WorkContextSource {
    pool: StoragePool,
}

impl WorkContextSource {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContextSource for WorkContextSource {
    fn name(&self) -> &str { "work_context" }

    fn priority(&self) -> u8 { 55 }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let contexts = WorkContextRepo::list_active(&self.pool).await.ok()?;
        if contexts.is_empty() {
            return None;
        }

        let mut output = String::from("[Current Work Contexts]\n");
        for (i, ctx) in contexts.iter().take(3).enumerate() {
            let age = Utc::now() - ctx.last_active_at;
            let age_str = if age.num_minutes() < 60 {
                format!("{}min ago", age.num_minutes())
            } else {
                format!("{}h ago", age.num_hours())
            };
            let duration_str = format!("{:.1}h today", ctx.total_duration_secs as f64 / 3600.0);

            let label = if i == 0 { "Active" } else { "Recent" };
            output.push_str(&format!(
                "{label}: \"{}\" ({}, {duration_str})\n",
                ctx.title, age_str
            ));

            // Get key resources
            if let Ok(resources) = ContextResourceRepo::list_for_context(&self.pool, &ctx.id).await {
                if !resources.is_empty() {
                    let names: Vec<&str> = resources.iter()
                        .take(3)
                        .map(|(r, _)| r.resource_name.as_str())
                        .collect();
                    output.push_str(&format!("  Key resources: {}\n", names.join(", ")));
                }
            }
        }

        Some(output)
    }
}
```

- [ ] **Step 2: Add deps and module to lib.rs**

Add `context_engine.workspace = true` to Cargo.toml.
Add `pub mod context_source;` and `pub use context_source::WorkContextSource;` to lib.rs.

- [ ] **Step 3: Run build**

Run: `cargo build -p activity-log`
Expected: SUCCESS.

- [ ] **Step 4: Commit**

```bash
git add crates/activity-log/src/context_source.rs crates/activity-log/Cargo.toml crates/activity-log/src/lib.rs
git commit -m "feat(activity-log): add WorkContextSource for system prompt assembly"
```

---

### Task 12: Background Inference Loop

**Files:**
- Create: `crates/activity-log/src/inference_loop.rs`
- Modify: `crates/activity-log/src/lib.rs`

- [ ] **Step 1: Implement**

```rust
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::inference::ContextInferenceEngine;

pub struct ContextInferenceLoop;

impl ContextInferenceLoop {
    pub fn start(
        engine: Arc<ContextInferenceEngine>,
        interval_mins: u64,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_mins * 60));
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => {
                        debug!("ContextInferenceLoop shutting down");
                        break;
                    }
                    _ = interval.tick() => {
                        // +1 min overlap for safety; process_recent_events queries
                        // WHERE work_context_id IS NULL, so already-assigned events are skipped (idempotent).
                        let since = Utc::now() - chrono::Duration::minutes((interval_mins as i64) + 1);
                        match engine.process_recent_events(since).await {
                            Ok(assignments) => {
                                if !assignments.is_empty() {
                                    debug!("Assigned {} events to work contexts", assignments.len());
                                }
                            }
                            Err(e) => {
                                warn!("Context inference error: {e}");
                            }
                        }
                    }
                }
            }
        })
    }
}
```

- [ ] **Step 2: Add module, commit**

```bash
git add crates/activity-log/src/inference_loop.rs crates/activity-log/src/lib.rs
git commit -m "feat(activity-log): add background inference loop"
```

---

### Task 13: WorkContextTool (Agent tool)

**Files:**
- Create: `crates/tools/src/work_context_tool.rs`
- Modify: `crates/tools/src/lib.rs` (or `mod.rs`)

- [ ] **Step 1: Implement following AreaTool pattern**

```rust
use async_trait::async_trait;
use serde_json::Value;
use storage::StoragePool;

use super::{RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::{Result, ToolError};

pub struct WorkContextTool {
    pool: StoragePool,
}

impl WorkContextTool {
    pub fn new(pool: StoragePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Tool for WorkContextTool {
    fn name(&self) -> &str { "work_context" }

    fn description(&self) -> &str {
        "Manage work contexts (inferred units of work). Actions: list, show, rename, link_project, search."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "show", "rename", "link_project", "search"],
                    "description": "Action to perform"
                },
                "id": { "type": "string", "description": "Context ID (for show/rename/link_project)" },
                "title": { "type": "string", "description": "New title (for rename)" },
                "project_id": { "type": "string", "description": "Project ID (for link_project)" },
                "query": { "type": "string", "description": "Search query (for search)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "list" => {
                let contexts = activity_log::WorkContextRepo::list_active(&self.pool).await?;
                if contexts.is_empty() {
                    return Ok("No active work contexts.".to_string());
                }
                let mut out = format!("Active work contexts ({}):\n\n", contexts.len());
                for ctx in &contexts {
                    out.push_str(&format!(
                        "• {} [{}] ({} events, {:.1}h) ID: {}\n",
                        ctx.title,
                        ctx.context_type.as_str(),
                        ctx.event_count,
                        ctx.total_duration_secs as f64 / 3600.0,
                        ctx.id
                    ));
                }
                Ok(out)
            }
            "show" => {
                let id = p.required_str("id")?;
                let ctx = activity_log::WorkContextRepo::get(&self.pool, id)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Context not found".into()))?;

                let resources = activity_log::ContextResourceRepo::list_for_context(&self.pool, id).await?;
                let mut out = format!("Work Context: {}\n", ctx.title);
                out.push_str(&format!("Type: {}\nStatus: {}\n", ctx.context_type.as_str(), ctx.status.as_str()));
                out.push_str(&format!("Events: {}\nDuration: {:.1}h\n", ctx.event_count, ctx.total_duration_secs as f64 / 3600.0));
                if !resources.is_empty() {
                    out.push_str("\nResources:\n");
                    for (r, score) in &resources {
                        out.push_str(&format!("  • {} ({:.0}%)\n", r.resource_name, score * 100.0));
                    }
                }
                Ok(out)
            }
            "rename" => {
                let id = p.required_str("id")?;
                let title = p.required_str("title")?;
                let mut ctx = activity_log::WorkContextRepo::get(&self.pool, id)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Context not found".into()))?;
                ctx.title = title.to_string();
                activity_log::WorkContextRepo::update(&self.pool, &ctx).await?;
                Ok(format!("Context renamed to: {}", title))
            }
            "link_project" => {
                let id = p.required_str("id")?;
                let project_id = p.required_str("project_id")?;
                let mut ctx = activity_log::WorkContextRepo::get(&self.pool, id)
                    .await?
                    .ok_or_else(|| ToolError::InvalidParams("Context not found".into()))?;
                ctx.linked_project_id = Some(project_id.to_string());
                activity_log::WorkContextRepo::update(&self.pool, &ctx).await?;
                Ok(format!("Context linked to project {}", project_id))
            }
            "search" => {
                let query = p.required_str("query")?;
                let results = activity_log::WorkContextRepo::search_by_title(&self.pool, query).await?;
                if results.is_empty() {
                    return Ok("No matching contexts.".to_string());
                }
                let mut out = format!("Search results ({}):\n\n", results.len());
                for ctx in &results {
                    out.push_str(&format!("• {} [{}] ID: {}\n", ctx.title, ctx.status.as_str(), ctx.id));
                }
                Ok(out)
            }
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {action}")).into()),
        }
    }
}
```

- [ ] **Step 2: Register module**

Add `pub mod work_context_tool;` to tools `lib.rs`/`mod.rs`.

- [ ] **Step 3: Build**

Run: `cargo build -p tools`
Expected: SUCCESS.

- [ ] **Step 4: Commit**

```bash
git add crates/tools/src/work_context_tool.rs crates/tools/src/lib.rs
git commit -m "feat(tools): add WorkContextTool for agent work context management"
```

---

### Task 14: Wire into app-core + agent loop (Pre-task gaps + Phase 2 wiring)

**Files:**
- Modify: `crates/app-core/src/init.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Wire in init.rs**

After the existing activity-log migration block (~line 164-173), and BEFORE the agent builder (~line 187), add:

```rust
// Initialize work context inference engine (requires embedder + vector store).
// TextEmbedderImpl wraps EmbeddingEngine — same pattern as conversation recall.
// Note: EmbeddingEngine is created inside the agent builder. To avoid coupling,
// create a second EmbeddingEngine instance here if needed, or pass it through.
// The simplest approach: create EmbeddingEngine in init.rs and pass to builder.
let inference_engine = if config.work_context.enabled {
    if let Some(ref vs) = vector_store {
        // EmbeddingEngine uses fastembed (local model), no LLM provider needed.
        match tools::EmbeddingEngine::new() {
            Ok(ee) => {
                let engine_arc = Arc::new(ee);
                let embedder: Arc<dyn cognitive::TextEmbedder> = Arc::new(
                    agent::cognitive_embedder::TextEmbedderImpl::new(engine_arc),
                );
                let inference_config = activity_log::inference::ContextInferenceConfig::from_work_context_config(&config.work_context);
                let engine = Arc::new(activity_log::inference::ContextInferenceEngine::new(
                    storage_pool.clone(),
                    embedder,
                    Some(vs.clone()),
                    inference_config,
                ));

                // Start background inference loop (shutdown_token is at line 223)
                let _inference_handle = activity_log::inference_loop::ContextInferenceLoop::start(
                    Arc::clone(&engine),
                    config.work_context.inference_interval_mins,
                    shutdown_token.child_token(),
                );

                Some(engine)
            }
            Err(e) => {
                warn!("EmbeddingEngine init failed — work context disabled: {e}");
                None
            }
        }
    } else { None }
} else { None };
```

**Note:** `shutdown_token` is created at line 223 of init.rs. The inference engine init must be placed AFTER line 223 but BEFORE the AppCore struct construction.

Register `WorkContextSource` in the agent builder (NOT init.rs — sources are built in `builder.rs`). Pass the pool through to the builder:
```rust
if config.work_context.enabled {
    builder = builder.with_work_context(storage_pool.clone());
}
```

This builder method (see Step 2) both registers the `WorkContextSource` in the `sources` vec and the `WorkContextTool` in the tool registry.

- [ ] **Step 2: Wire in builder.rs (context source + tool)**

Add field to `AgentLoopBuilder`:
```rust
work_context_pool: Option<StoragePool>,
```

Add builder method:
```rust
pub fn with_work_context(mut self, pool: StoragePool) -> Self {
    self.work_context_pool = Some(pool);
    self
}
```

In `build()`, after other context sources are pushed to `sources` vec (~line 216):
```rust
if let Some(ref wc_pool) = self.work_context_pool {
    sources.push(Box::new(activity_log::WorkContextSource::new(wc_pool.clone())));
}
```

In `build()`, after other tool registrations:
```rust
if let Some(ref wc_pool) = self.work_context_pool {
    registry.register(Box::new(tools::work_context_tool::WorkContextTool::new(wc_pool.clone())));
}
```

- [ ] **Step 3: Wire chat message ingestion (Pre-task Gap 1)**

In `agent_loop/mod.rs`, after user message is added to session (~line 334) and after assistant response is saved (~line 353), add fire-and-forget ingestion calls. This requires passing `activity_ingestion_service: Option<Arc<ActivityIngestionService>>` into the AgentLoop struct and builder.

**Note:** Check the exact builder pattern for how to add this field. Follow the same pattern as `conversation_recall_handler` which is also `Option<Arc<dyn ...>>`.

Add field to `AgentLoop` struct:
```rust
pub(crate) activity_svc: Option<Arc<activity_log::ActivityIngestionService>>,
```

Add builder method:
```rust
pub fn with_activity_service(mut self, svc: Arc<activity_log::ActivityIngestionService>) -> Self {
    self.activity_svc = Some(svc);
    self
}
```

In the message processing, after user message is added to session (~line 334), and after assistant response is saved (~line 353):

```rust
// After user message:
if let Some(ref svc) = self.activity_svc {
    let normalizer = activity_log::ChatMessageNormalizer;
    let input = activity_log::ChatMessageInput {
        session_key: session_key.clone(),
        role: "user".to_string(),
        content: user_message_text.clone(),
    };
    // normalize returns Option<ActivityLogEntry>
    if let Some(entry) = normalizer.normalize(&input as &dyn std::any::Any) {
        svc.ingest_fire_and_forget(entry);
        // ingest_fire_and_forget(&self: Arc<Self>, entry: ActivityLogEntry)
        // takes one arg: the ActivityLogEntry (not normalizer+input)
    }
}

// After assistant response:
if let Some(ref svc) = self.activity_svc {
    let normalizer = activity_log::ChatMessageNormalizer;
    let input = activity_log::ChatMessageInput {
        session_key: session_key.clone(),
        role: "assistant".to_string(),
        content: response_content.clone(),
    };
    if let Some(entry) = normalizer.normalize(&input as &dyn std::any::Any) {
        svc.ingest_fire_and_forget(entry);
    }
}
```

- [ ] **Step 4: Full workspace build**

Run: `cargo build --workspace`
Expected: SUCCESS.

- [ ] **Step 5: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All tests PASS.

- [ ] **Step 6: Clippy + fmt**

Run: `cargo clippy --workspace --all-targets --all-features && cargo fmt --all --check`
Expected: 0 warnings, format clean.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(app-core): wire work context engine, inference loop, and chat ingestion"
```

---

## Verification Checklist

After all tasks complete:

- [ ] `cargo build --workspace` — clean
- [ ] `cargo nextest run --workspace` — all pass
- [ ] `cargo clippy --workspace --all-targets --all-features` — 0 warnings
- [ ] `cargo fmt --all --check` — clean
- [ ] New activity-log tests cover: repo CRUD, inference scoring, context source output
- [ ] Migration 002 creates all 5 tables
- [ ] Config deserializes with/without workContext section
- [ ] VectorStore creates 2 new tables on connect
