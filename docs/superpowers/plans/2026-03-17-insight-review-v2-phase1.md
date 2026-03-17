# Insight Review V2 — Phase 1: Core Infrastructure

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the flat `insight_review_cache` table with a versioned `insight_reviews` system in a new `feature-insights` crate, preserving all existing functionality while enabling version history.

**Architecture:** New `feature-insights` crate at L4. Defines dependency inversion traits (`CognitiveAccessor`, `FlashcardAccessor`, `InsightEmbedder`) for L5 access. `InsightService` orchestrates the pipeline (same 5-tab LLM flow as today, but stores results in versioned rows). `app-core` handlers become thin adapters delegating to the service. Schema lives in cognitive's migration file (pre-release consolidation); no `FeaturePackage` impl in Phase 1 since no agent tools are exposed — insights are UI-driven only.

**Tech Stack:** Rust (tokio, sqlx, serde_json, providers), SQLite, existing `FeaturePackage` pattern

**Spec:** `docs/superpowers/specs/2026-03-17-insight-review-v2-design.md` (Sections 2-3, 9)

**Scope:** Backend only. Frontend changes are minimal (same DTOs with optional new fields). Version history UI, Smart Merge, Scope Resolver, and Learning Progress are Phases 2-4.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `crates/feature-insights/Cargo.toml` | Crate manifest with workspace dependencies |
| `crates/feature-insights/src/lib.rs` | Module registration + re-exports (no FeaturePackage in Phase 1) |
| `crates/feature-insights/src/types.rs` | `InsightReview`, `InsightContent`, `ScopeConfig`, `ScopeType`, `ProgressSnapshot` |
| `crates/feature-insights/src/traits.rs` | `CognitiveAccessor`, `FlashcardAccessor`, `InsightEmbedder` traits |
| `crates/feature-insights/src/repo.rs` | `InsightReviewRepo` — CRUD + version listing + parent lookup |
| `crates/feature-insights/src/progress_repo.rs` | `InsightProgressRepo` — snapshot CRUD + timeline query |
| `crates/feature-insights/src/service.rs` | `InsightService` — orchestrator (generate, get_latest, list_versions, get_evolution) |
| `crates/feature-insights/src/prompts.rs` | Tab prompt templates (moved from app-core) |

### Modified files

| File | Change |
|------|--------|
| `Cargo.toml` (root) | Add `feature-insights` to workspace members + dependencies |
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Replace `insight_review_cache` with `insight_reviews` + `insight_progress_snapshots` |
| `crates/cognitive/src/repos/mod.rs` | Bump migration version from 3 to 4 |
| `crates/app-core/Cargo.toml` | Add `feature-insights` dependency |
| `crates/app-core/src/state.rs` | Replace `insight_cache_repo` with `insight_service: Option<Arc<InsightService>>` |
| `crates/app-core/src/init/mod.rs` | Init `InsightService` + pass to AppCore |
| `crates/app-core/src/handlers/notes/insight.rs` | Delegate to InsightService instead of inline pipeline |
| `crates/desktop-shared/src/commands/notes.rs` | Update `InsightReviewResponse` with version fields |

---

## Chunk 1: Crate Scaffolding + Schema + Types

### Task 1: Create `feature-insights` crate with types and traits

**Files:**
- Create: `crates/feature-insights/Cargo.toml`
- Create: `crates/feature-insights/src/lib.rs`
- Create: `crates/feature-insights/src/types.rs`
- Create: `crates/feature-insights/src/traits.rs`
- Modify: `Cargo.toml` (root workspace)

- [ ] **Step 1: Create Cargo.toml**

Create `crates/feature-insights/Cargo.toml`:

```toml
[package]
name = "feature-insights"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
common.workspace = true
tools-core.workspace = true
storage.workspace = true
providers.workspace = true
feature-notes.workspace = true
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tracing.workspace = true
chrono.workspace = true
uuid.workspace = true
sqlx.workspace = true
sha2.workspace = true
futures-util.workspace = true

[dev-dependencies]
cognitive.workspace = true
storage.workspace = true
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 2: Create types.rs**

Create `crates/feature-insights/src/types.rs`:

```rust
//! Core types for the Insight Review V2 system.

use serde::{Deserialize, Serialize};

/// The 5-tab insight content stored as a JSON blob.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InsightContent {
    pub synthesis: Option<String>,
    pub gap_analysis: Option<String>,
    pub self_assessment: Option<String>,
    pub concept_map: Option<String>,
    pub perspectives: Option<String>,
}

/// Scope type for insight generation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ScopeType {
    Backlinks,
    Semantic,
    Project,
    Manual,
}

impl Default for ScopeType {
    fn default() -> Self {
        Self::Backlinks
    }
}

/// Configuration for what context to include in insight generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeConfig {
    #[serde(default)]
    pub scope_type: ScopeType,
    #[serde(default = "default_radius")]
    pub radius: f64,
    #[serde(default)]
    pub node_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub include_cognitive: bool,
    #[serde(default)]
    pub deep_dive: bool,
    #[serde(default = "default_merge_threshold")]
    pub merge_threshold: f64,
}

impl Default for ScopeConfig {
    fn default() -> Self {
        Self {
            scope_type: ScopeType::default(),
            radius: default_radius(),
            node_ids: Vec::new(),
            include_cognitive: true,
            deep_dive: false,
            merge_threshold: default_merge_threshold(),
        }
    }
}

fn default_radius() -> f64 {
    0.72
}
fn default_true() -> bool {
    true
}
fn default_merge_threshold() -> f64 {
    0.60
}

/// A single insight review version (row from `insight_reviews` table).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InsightReviewRow {
    pub id: String,
    pub note_id: String,
    pub version: i64,
    pub generated_at: String,
    pub content: String,
    pub input_hash: String,
    pub scope_config: String,
    pub persona_ids: String,
    pub parent_insight_id: Option<String>,
    pub token_cost_usd: Option<f64>,
    pub superseded_at: Option<String>,
}

/// A progress snapshot for a specific insight version.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProgressSnapshotRow {
    pub id: String,
    pub insight_review_id: String,
    pub version: i64,
    pub flashcard_success: f64,
    pub semantic_drift: f64,
    pub gap_closure: f64,
    pub quiz_score: f64,
    pub overall_progress: f64,
    pub computed_at: String,
}

/// Progress weights for the composite score (configurable).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressWeights {
    pub flashcard: f64,
    pub drift: f64,
    pub gap: f64,
    pub quiz: f64,
}

impl Default for ProgressWeights {
    fn default() -> Self {
        Self {
            flashcard: 0.40,
            drift: 0.25,
            gap: 0.20,
            quiz: 0.15,
        }
    }
}
```

- [ ] **Step 3: Create traits.rs**

Create `crates/feature-insights/src/traits.rs`:

```rust
//! Dependency inversion traits for L5 access from L4.
//!
//! These traits are defined here in `feature-insights` (L4) and implemented
//! in `app-core` (L7) or `agent` (L5) where cognitive repos are available.
//! Injected into `InsightService` as `Arc<dyn Trait>` during AppCore init.

use async_trait::async_trait;

/// Provides cognitive memory data for insight context injection.
#[async_trait]
pub trait CognitiveAccessor: Send + Sync {
    /// Search semantic facts by query text, optionally filtered by domain.
    async fn search_facts(&self, query: &str, domain: Option<&str>, limit: usize) -> Vec<String>;
    /// Get recent episodic memories mentioning a note.
    async fn recent_memories(&self, note_id: &str, limit: usize) -> Vec<String>;
    /// Get active procedural rules for a domain.
    async fn domain_rules(&self, domain: &str) -> Vec<String>;
    /// Get user model summary for a domain (deep dive only).
    async fn user_model_summary(&self, domain: &str) -> Option<String>;
    /// Get entity graph neighborhood as text (deep dive only).
    /// The implementation resolves note_id → entity IDs internally
    /// (via EntityRepo::get_entities_for_note), then calls get_neighborhood.
    async fn entity_neighborhood(&self, note_id: &str, depth: u8) -> Vec<String>;
    /// Get temporal fact history (deep dive only).
    async fn fact_history(&self, subject: &str) -> Vec<String>;
}

/// Provides flashcard review data for learning progress computation.
#[async_trait]
pub trait FlashcardAccessor: Send + Sync {
    /// Get average review success rate for an insight (0.0-1.0).
    async fn review_success_rate(&self, insight_review_id: &str, days: i64) -> f64;
}

/// Provides embedding operations for insight content.
#[async_trait]
pub trait InsightEmbedder: Send + Sync {
    /// Embed insight content and store in vector DB.
    async fn embed_and_store(&self, insight_id: &str, content: &str) -> Result<(), String>;
    /// Get cosine similarity between two insight embeddings (None if either missing).
    async fn similarity(&self, id_a: &str, id_b: &str) -> Option<f64>;
}

/// No-op implementations for when cognitive features are unavailable.
pub struct NoopCognitiveAccessor;

#[async_trait]
impl CognitiveAccessor for NoopCognitiveAccessor {
    async fn search_facts(&self, _: &str, _: Option<&str>, _: usize) -> Vec<String> {
        Vec::new()
    }
    async fn recent_memories(&self, _: &str, _: usize) -> Vec<String> {
        Vec::new()
    }
    async fn domain_rules(&self, _: &str) -> Vec<String> {
        Vec::new()
    }
    async fn user_model_summary(&self, _: &str) -> Option<String> {
        None
    }
    async fn entity_neighborhood(&self, _: &str, _: u8) -> Vec<String> {
        Vec::new()
    }
    async fn fact_history(&self, _: &str) -> Vec<String> {
        Vec::new()
    }
}

pub struct NoopFlashcardAccessor;

#[async_trait]
impl FlashcardAccessor for NoopFlashcardAccessor {
    async fn review_success_rate(&self, _: &str, _: i64) -> f64 {
        0.0
    }
}

pub struct NoopInsightEmbedder;

#[async_trait]
impl InsightEmbedder for NoopInsightEmbedder {
    async fn embed_and_store(&self, _: &str, _: &str) -> Result<(), String> {
        Ok(())
    }
    async fn similarity(&self, _: &str, _: &str) -> Option<f64> {
        None
    }
}
```

- [ ] **Step 4: Create lib.rs stub**

Create `crates/feature-insights/src/lib.rs`:

```rust
//! Feature crate for versioned Insight Reviews with learning progress tracking.

pub mod traits;
pub mod types;

// Re-exports
pub use traits::*;
pub use types::*;
```

- [ ] **Step 5: Register in workspace**

In `Cargo.toml` (root), add to the `[workspace]` members list:
```toml
    "crates/feature-insights",
```

And in `[workspace.dependencies]`:
```toml
feature-insights = { path = "crates/feature-insights" }
```

- [ ] **Step 6: Verify build**

Run: `cargo build -p feature-insights`
Expected: compiles.

- [ ] **Step 7: Commit**

```bash
git add crates/feature-insights/ Cargo.toml
git commit -m "feat(feature-insights): create crate with types, traits, and FeaturePackage stub"
```

---

### Task 2: Schema Migration — Replace insight_review_cache

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Replace the insight_review_cache DDL**

In `crates/cognitive/migrations/001_cognitive_tables.sql`, find the `insight_review_cache` CREATE TABLE block (around lines 428-445) and replace it with:

```sql
-- ── Insight Reviews (versioned, replaces old insight_review_cache) ──────────

DROP TABLE IF EXISTS insight_review_cache;

CREATE TABLE IF NOT EXISTS insight_reviews (
    id                  TEXT PRIMARY KEY,
    note_id             TEXT NOT NULL,
    version             INTEGER NOT NULL DEFAULT 1,
    generated_at        TEXT NOT NULL,
    content             TEXT NOT NULL,
    input_hash          TEXT NOT NULL,
    scope_config        TEXT NOT NULL DEFAULT '{"scopeType":"backlinks","radius":0.72,"nodeIds":[],"includeCognitive":true,"deepDive":false,"mergeThreshold":0.6}',
    persona_ids         TEXT NOT NULL DEFAULT '[]',
    parent_insight_id   TEXT REFERENCES insight_reviews(id),
    token_cost_usd      REAL,
    superseded_at       TEXT,
    UNIQUE(note_id, version)
);

CREATE INDEX IF NOT EXISTS idx_insight_reviews_note ON insight_reviews(note_id, version);
CREATE INDEX IF NOT EXISTS idx_insight_reviews_hash ON insight_reviews(input_hash);
CREATE INDEX IF NOT EXISTS idx_insight_reviews_parent ON insight_reviews(parent_insight_id);

CREATE TABLE IF NOT EXISTS insight_progress_snapshots (
    id                  TEXT PRIMARY KEY,
    insight_review_id   TEXT NOT NULL REFERENCES insight_reviews(id) ON DELETE CASCADE,
    version             INTEGER NOT NULL,
    flashcard_success   REAL NOT NULL DEFAULT 0.0,
    semantic_drift      REAL NOT NULL DEFAULT 0.0,
    gap_closure         REAL NOT NULL DEFAULT 0.0,
    quiz_score          REAL NOT NULL DEFAULT 0.0,
    overall_progress    REAL NOT NULL DEFAULT 0.0,
    computed_at         TEXT NOT NULL,
    UNIQUE(insight_review_id, version)
);

CREATE INDEX IF NOT EXISTS idx_progress_insight ON insight_progress_snapshots(insight_review_id, version);
```

- [ ] **Step 2: Bump migration version**

In `crates/cognitive/src/repos/mod.rs`, change the migration version from 3 to 4:

```rust
pub fn cognitive_migrations() -> Vec<FeatureMigration> {
    vec![FeatureMigration {
        feature_name: "cognitive".to_string(),
        version: 4,
        description: "Cognitive memory system tables + versioned insight reviews".to_string(),
        sql: include_str!("../../migrations/001_cognitive_tables.sql").to_string(),
    }]
}
```

- [ ] **Step 3: Build + test**

Run: `cargo nextest run -p cognitive`
Expected: all cognitive tests pass (the test pool helper creates tables from scratch).

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/migrations/001_cognitive_tables.sql crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): replace insight_review_cache with versioned insight_reviews schema"
```

---

### Task 3: InsightReviewRepo with tests

**Files:**
- Create: `crates/feature-insights/src/repo.rs`
- Modify: `crates/feature-insights/src/lib.rs`

- [ ] **Step 1: Create InsightReviewRepo**

Create `crates/feature-insights/src/repo.rs`:

```rust
//! Repository for the `insight_reviews` table — versioned insight storage.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::types::{InsightReviewRow, ScopeConfig};

#[derive(Debug, Clone)]
pub struct InsightReviewRepo {
    pool: SqlitePool,
}

impl InsightReviewRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new insight review version. Automatically sets version = max + 1 for the note.
    pub async fn insert(
        &self,
        note_id: &str,
        content: &str,
        input_hash: &str,
        scope_config: &ScopeConfig,
        persona_ids: &[String],
        parent_insight_id: Option<&str>,
    ) -> Result<InsightReviewRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        // Get next version number for this note
        let max_version: Option<i64> = sqlx::query_scalar(
            "SELECT MAX(version) FROM insight_reviews WHERE note_id = ?1",
        )
        .bind(note_id)
        .fetch_one(&self.pool)
        .await?;
        let version = max_version.unwrap_or(0) + 1;

        let scope_json =
            serde_json::to_string(scope_config).unwrap_or_else(|_| "{}".to_string());
        let persona_json =
            serde_json::to_string(persona_ids).unwrap_or_else(|_| "[]".to_string());

        sqlx::query(
            r#"
            INSERT INTO insight_reviews
                (id, note_id, version, generated_at, content, input_hash,
                 scope_config, persona_ids, parent_insight_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&id)
        .bind(note_id)
        .bind(version)
        .bind(&now)
        .bind(content)
        .bind(input_hash)
        .bind(&scope_json)
        .bind(&persona_json)
        .bind(parent_insight_id)
        .execute(&self.pool)
        .await?;

        self.get(&id).await.map(|opt| opt.expect("just inserted"))
    }

    /// Get a single insight review by ID.
    pub async fn get(&self, id: &str) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        sqlx::query_as::<_, InsightReviewRow>(
            "SELECT * FROM insight_reviews WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Get the latest (highest version) insight for a note.
    pub async fn get_latest(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        sqlx::query_as::<_, InsightReviewRow>(
            "SELECT * FROM insight_reviews WHERE note_id = ?1 AND superseded_at IS NULL ORDER BY version DESC LIMIT 1",
        )
        .bind(note_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Get insight by note_id and exact input_hash (cache hit check).
    pub async fn get_by_hash(
        &self,
        note_id: &str,
        input_hash: &str,
    ) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        sqlx::query_as::<_, InsightReviewRow>(
            "SELECT * FROM insight_reviews WHERE note_id = ?1 AND input_hash = ?2 AND superseded_at IS NULL ORDER BY version DESC LIMIT 1",
        )
        .bind(note_id)
        .bind(input_hash)
        .fetch_optional(&self.pool)
        .await
    }

    /// List all versions for a note, newest first.
    pub async fn list_versions(
        &self,
        note_id: &str,
    ) -> Result<Vec<InsightReviewRow>, sqlx::Error> {
        sqlx::query_as::<_, InsightReviewRow>(
            "SELECT * FROM insight_reviews WHERE note_id = ?1 ORDER BY version DESC",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Soft-archive an insight version (mark as superseded).
    pub async fn supersede(&self, id: &str) -> Result<(), sqlx::Error> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE insight_reviews SET superseded_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Update content for a specific tab (used by regenerate_tab).
    pub async fn update_content(
        &self,
        id: &str,
        content: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE insight_reviews SET content = ?1 WHERE id = ?2")
            .bind(content)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        // Run storage migrations first
        sqlx::migrate!("../storage/migrations")
            .run(&pool)
            .await
            .unwrap();
        // Run cognitive migrations (which include insight_reviews)
        let migrations = cognitive::cognitive_migrations();
        storage::StoragePool::run_feature_migrations(&pool, &migrations)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn test_insert_and_get() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);

        let scope = ScopeConfig::default();
        let row = repo
            .insert("note-1", r#"{"synthesis":"hello"}"#, "hash-abc", &scope, &[], None)
            .await
            .unwrap();

        assert_eq!(row.note_id, "note-1");
        assert_eq!(row.version, 1);
        assert_eq!(row.input_hash, "hash-abc");
        assert!(row.parent_insight_id.is_none());

        let fetched = repo.get(&row.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, row.id);
    }

    #[tokio::test]
    async fn test_version_auto_increment() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);
        let scope = ScopeConfig::default();

        let v1 = repo
            .insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();
        assert_eq!(v1.version, 1);

        let v2 = repo
            .insert("note-1", r#"{"synthesis":"v2"}"#, "hash-2", &scope, &[], None)
            .await
            .unwrap();
        assert_eq!(v2.version, 2);

        // Different note starts at 1
        let other = repo
            .insert("note-2", r#"{"synthesis":"v1"}"#, "hash-3", &scope, &[], None)
            .await
            .unwrap();
        assert_eq!(other.version, 1);
    }

    #[tokio::test]
    async fn test_get_latest_and_list_versions() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);
        let scope = ScopeConfig::default();

        repo.insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();
        repo.insert("note-1", r#"{"synthesis":"v2"}"#, "hash-2", &scope, &[], None)
            .await
            .unwrap();

        let latest = repo.get_latest("note-1").await.unwrap().unwrap();
        assert_eq!(latest.version, 2);

        let versions = repo.list_versions("note-1").await.unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version, 2); // newest first
        assert_eq!(versions[1].version, 1);
    }

    #[tokio::test]
    async fn test_get_by_hash() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);
        let scope = ScopeConfig::default();

        repo.insert("note-1", r#"{"synthesis":"v1"}"#, "hash-abc", &scope, &[], None)
            .await
            .unwrap();

        let found = repo.get_by_hash("note-1", "hash-abc").await.unwrap();
        assert!(found.is_some());

        let not_found = repo.get_by_hash("note-1", "wrong-hash").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_supersede_hides_from_latest() {
        let pool = setup().await;
        let repo = InsightReviewRepo::new(pool);
        let scope = ScopeConfig::default();

        let v1 = repo
            .insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();

        repo.supersede(&v1.id).await.unwrap();

        // get_latest should not find superseded insights
        let latest = repo.get_latest("note-1").await.unwrap();
        assert!(latest.is_none());

        // but list_versions still includes them
        let versions = repo.list_versions("note-1").await.unwrap();
        assert_eq!(versions.len(), 1);
    }
}
```

- [ ] **Step 2: Register module in lib.rs**

Update `crates/feature-insights/src/lib.rs`:

```rust
//! Feature crate for versioned Insight Reviews with learning progress tracking.

pub mod repo;
pub mod traits;
pub mod types;

// Re-exports
pub use repo::InsightReviewRepo;
pub use traits::*;
pub use types::*;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-insights`
Expected: all 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/feature-insights/src/repo.rs crates/feature-insights/src/lib.rs
git commit -m "feat(feature-insights): add InsightReviewRepo with version auto-increment and tests"
```

---

### Task 4: InsightProgressRepo with tests

**Files:**
- Create: `crates/feature-insights/src/progress_repo.rs`
- Modify: `crates/feature-insights/src/lib.rs`

- [ ] **Step 1: Create InsightProgressRepo**

Create `crates/feature-insights/src/progress_repo.rs`:

```rust
//! Repository for the `insight_progress_snapshots` table.

use chrono::Utc;
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::types::{ProgressSnapshotRow, ProgressWeights};

#[derive(Debug, Clone)]
pub struct InsightProgressRepo {
    pool: SqlitePool,
}

impl InsightProgressRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or update a progress snapshot for a specific insight version.
    pub async fn upsert(
        &self,
        insight_review_id: &str,
        version: i64,
        flashcard_success: f64,
        semantic_drift: f64,
        gap_closure: f64,
        quiz_score: f64,
        weights: &ProgressWeights,
    ) -> Result<ProgressSnapshotRow, sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        let overall = weights.flashcard * flashcard_success
            + weights.drift * (1.0 - semantic_drift)
            + weights.gap * gap_closure
            + weights.quiz * quiz_score;

        sqlx::query(
            r#"
            INSERT INTO insight_progress_snapshots
                (id, insight_review_id, version, flashcard_success, semantic_drift,
                 gap_closure, quiz_score, overall_progress, computed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(insight_review_id, version) DO UPDATE SET
                flashcard_success = excluded.flashcard_success,
                semantic_drift = excluded.semantic_drift,
                gap_closure = excluded.gap_closure,
                quiz_score = excluded.quiz_score,
                overall_progress = excluded.overall_progress,
                computed_at = excluded.computed_at
            "#,
        )
        .bind(&id)
        .bind(insight_review_id)
        .bind(version)
        .bind(flashcard_success)
        .bind(semantic_drift)
        .bind(gap_closure)
        .bind(quiz_score)
        .bind(overall)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let row = sqlx::query_as::<_, ProgressSnapshotRow>(
            "SELECT * FROM insight_progress_snapshots WHERE insight_review_id = ?1 AND version = ?2",
        )
        .bind(insight_review_id)
        .bind(version)
        .fetch_one(&self.pool)
        .await?;

        Ok(row)
    }

    /// Get the progress timeline for a note (all versions, ordered by version).
    pub async fn get_timeline(
        &self,
        note_id: &str,
    ) -> Result<Vec<ProgressSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, ProgressSnapshotRow>(
            r#"
            SELECT p.* FROM insight_progress_snapshots p
            INNER JOIN insight_reviews r ON p.insight_review_id = r.id
            WHERE r.note_id = ?1
            ORDER BY p.version ASC
            "#,
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await
    }

    /// Get the latest progress snapshot for a specific insight.
    pub async fn get_latest(
        &self,
        insight_review_id: &str,
    ) -> Result<Option<ProgressSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, ProgressSnapshotRow>(
            "SELECT * FROM insight_progress_snapshots WHERE insight_review_id = ?1 ORDER BY version DESC LIMIT 1",
        )
        .bind(insight_review_id)
        .fetch_optional(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::InsightReviewRepo;
    use crate::types::ScopeConfig;

    async fn setup() -> SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::migrate!("../storage/migrations")
            .run(&pool)
            .await
            .unwrap();
        let migrations = cognitive::cognitive_migrations();
        storage::StoragePool::run_feature_migrations(&pool, &migrations)
            .await
            .unwrap();
        pool
    }

    #[tokio::test]
    async fn test_upsert_and_get() {
        let pool = setup().await;
        let insight_repo = InsightReviewRepo::new(pool.clone());
        let progress_repo = InsightProgressRepo::new(pool);
        let scope = ScopeConfig::default();
        let weights = ProgressWeights::default();

        let insight = insight_repo
            .insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();

        let snapshot = progress_repo
            .upsert(&insight.id, 1, 0.8, 0.1, 0.5, 0.7, &weights)
            .await
            .unwrap();

        assert_eq!(snapshot.insight_review_id, insight.id);
        assert!((snapshot.flashcard_success - 0.8).abs() < f64::EPSILON);
        // overall = 0.40*0.8 + 0.25*(1-0.1) + 0.20*0.5 + 0.15*0.7
        //         = 0.32 + 0.225 + 0.10 + 0.105 = 0.75
        assert!((snapshot.overall_progress - 0.75).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_timeline() {
        let pool = setup().await;
        let insight_repo = InsightReviewRepo::new(pool.clone());
        let progress_repo = InsightProgressRepo::new(pool);
        let scope = ScopeConfig::default();
        let weights = ProgressWeights::default();

        let v1 = insight_repo
            .insert("note-1", r#"{"synthesis":"v1"}"#, "hash-1", &scope, &[], None)
            .await
            .unwrap();
        let v2 = insight_repo
            .insert("note-1", r#"{"synthesis":"v2"}"#, "hash-2", &scope, &[], None)
            .await
            .unwrap();

        progress_repo
            .upsert(&v1.id, 1, 0.5, 0.0, 0.0, 0.3, &weights)
            .await
            .unwrap();
        progress_repo
            .upsert(&v2.id, 2, 0.8, 0.2, 0.6, 0.7, &weights)
            .await
            .unwrap();

        let timeline = progress_repo.get_timeline("note-1").await.unwrap();
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].version, 1);
        assert_eq!(timeline[1].version, 2);
        assert!(timeline[1].overall_progress > timeline[0].overall_progress);
    }
}
```

- [ ] **Step 2: Register module**

In `crates/feature-insights/src/lib.rs`, add:
```rust
pub mod progress_repo;
pub use progress_repo::InsightProgressRepo;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p feature-insights`
Expected: all tests pass (repo + progress_repo).

- [ ] **Step 4: Commit**

```bash
git add crates/feature-insights/src/progress_repo.rs crates/feature-insights/src/lib.rs
git commit -m "feat(feature-insights): add InsightProgressRepo with timeline queries and tests"
```

---

## Chunk 2: Service + App-Core Wiring

### Task 5: Move prompts to feature-insights

**Files:**
- Create: `crates/feature-insights/src/prompts.rs`
- Modify: `crates/feature-insights/src/lib.rs`

- [ ] **Step 1: Copy prompt functions**

Copy the contents of `crates/app-core/src/handlers/notes/insight_prompts.rs` to `crates/feature-insights/src/prompts.rs`. The functions are: `synthesis_prompt`, `gap_analysis_prompt`, `self_assessment_prompt`, `concept_map_prompt`, `perspectives_prompt`, `format_persona_blocks`.

Read the source file first, then create the new file with all functions. Add `//! Prompt templates for Insight Review tabs.` at the top.

- [ ] **Step 2: Register module**

In `crates/feature-insights/src/lib.rs`, add:
```rust
pub mod prompts;
```

- [ ] **Step 3: Build**

Run: `cargo build -p feature-insights`

- [ ] **Step 4: Commit**

```bash
git add crates/feature-insights/src/prompts.rs crates/feature-insights/src/lib.rs
git commit -m "feat(feature-insights): move prompt templates from app-core"
```

---

### Task 6: InsightService — Basic Orchestrator

**Files:**
- Create: `crates/feature-insights/src/service.rs`
- Modify: `crates/feature-insights/src/lib.rs`

The service mirrors the current pipeline in `insight.rs` but stores results in versioned `insight_reviews` instead of the flat cache. Key differences:
- Returns `InsightReviewRow` (with version) instead of just cache data
- Uses `InsightReviewRepo::insert()` which auto-increments versions
- Hash check uses `get_by_hash()` instead of the old `get_if_fresh()`
- Emits the same Tauri events as today (synthesis-chunk, tab-done, etc.)

- [ ] **Step 1: Create service.rs**

Create `crates/feature-insights/src/service.rs`:

```rust
//! InsightService — orchestrates the insight generation pipeline.
//!
//! This is the Phase 1 version that preserves the existing 5-tab pipeline
//! but stores results in the new versioned `insight_reviews` table.
//! Smart Merge, Scope Resolution, and Learning Progress are added in Phases 2-4.

use std::sync::Arc;

use futures_util::StreamExt;
use sha2::{Digest, Sha256};

use crate::progress_repo::InsightProgressRepo;
use crate::repo::InsightReviewRepo;
use crate::traits::{FlashcardAccessor, InsightEmbedder};
use crate::types::{InsightContent, InsightReviewRow, ProgressWeights, ScopeConfig};

/// The central orchestrator for insight generation and retrieval.
///
/// Note: Event emission (streaming, tab-done) stays in `app-core`'s
/// insight handler since it depends on the transport-specific `AppEventEmitter`.
/// The service handles storage and retrieval only.
pub struct InsightService {
    pub(crate) repo: InsightReviewRepo,
    pub(crate) progress_repo: InsightProgressRepo,
    pub(crate) flashcards: Arc<dyn FlashcardAccessor>,
    pub(crate) embedder: Arc<dyn InsightEmbedder>,
    pub(crate) progress_weights: ProgressWeights,
}

impl InsightService {
    pub fn new(
        repo: InsightReviewRepo,
        progress_repo: InsightProgressRepo,
        flashcards: Arc<dyn FlashcardAccessor>,
        embedder: Arc<dyn InsightEmbedder>,
        progress_weights: ProgressWeights,
    ) -> Self {
        Self {
            repo,
            progress_repo,
            flashcards,
            embedder,
            progress_weights,
        }
    }

    /// Check if a fresh insight exists for the given hash. Returns it if found.
    pub async fn check_cache(
        &self,
        note_id: &str,
        input_hash: &str,
    ) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        self.repo.get_by_hash(note_id, input_hash).await
    }

    /// Store a completed insight (called after the background pipeline finishes).
    pub async fn store_insight(
        &self,
        note_id: &str,
        content: &InsightContent,
        input_hash: &str,
        scope_config: &ScopeConfig,
        persona_ids: &[String],
    ) -> Result<InsightReviewRow, sqlx::Error> {
        let content_json =
            serde_json::to_string(content).unwrap_or_else(|_| "{}".to_string());

        let row = self
            .repo
            .insert(note_id, &content_json, input_hash, scope_config, persona_ids, None)
            .await?;

        // Compute initial progress snapshot (all zeros — no flashcard data yet)
        let _ = self
            .progress_repo
            .upsert(&row.id, row.version, 0.0, 0.0, 0.0, 0.0, &self.progress_weights)
            .await;

        // Embed the content for future semantic drift + dedup (best effort)
        let embed_id = row.id.clone();
        let embed_content = content_json.clone();
        let embedder = Arc::clone(&self.embedder);
        tokio::spawn(async move {
            let _ = embedder.embed_and_store(&embed_id, &embed_content).await;
        });

        Ok(row)
    }

    /// Get the latest insight for a note (no LLM call).
    pub async fn get_latest(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewRow>, sqlx::Error> {
        self.repo.get_latest(note_id).await
    }

    /// List all versions for a note.
    pub async fn list_versions(
        &self,
        note_id: &str,
    ) -> Result<Vec<InsightReviewRow>, sqlx::Error> {
        self.repo.list_versions(note_id).await
    }

    /// Update a single tab in the latest insight's content.
    pub async fn update_tab(
        &self,
        insight_id: &str,
        tab_name: &str,
        tab_content: &str,
    ) -> Result<(), sqlx::Error> {
        let row = self.repo.get(insight_id).await?;
        if let Some(row) = row {
            let mut content: InsightContent =
                serde_json::from_str(&row.content).unwrap_or_default();

            match tab_name {
                "synthesis" => content.synthesis = Some(tab_content.to_string()),
                "gaps" | "gap_analysis" => content.gap_analysis = Some(tab_content.to_string()),
                "assessment" | "self_assessment" => {
                    content.self_assessment = Some(tab_content.to_string())
                }
                "concept-map" | "concept_map" => {
                    content.concept_map = Some(tab_content.to_string())
                }
                "perspectives" => content.perspectives = Some(tab_content.to_string()),
                _ => return Ok(()),
            }

            let updated_json =
                serde_json::to_string(&content).unwrap_or_else(|_| "{}".to_string());
            self.repo.update_content(insight_id, &updated_json).await?;
        }
        Ok(())
    }

    /// Compute input hash for cache check (same algorithm as before).
    pub fn compute_input_hash(note_title: &str, note_body: &str, related_ids: &[String]) -> String {
        let hash_input = format!("{}{}{}", note_title, note_body, related_ids.join(","));
        format!("{:x}", Sha256::digest(hash_input.as_bytes()))
    }
}
```

- [ ] **Step 2: Register module**

In `crates/feature-insights/src/lib.rs`, add:
```rust
pub mod service;
pub use service::InsightService;
```

- [ ] **Step 3: Build**

Run: `cargo build -p feature-insights`

- [ ] **Step 4: Commit**

```bash
git add crates/feature-insights/src/service.rs crates/feature-insights/src/lib.rs
git commit -m "feat(feature-insights): add InsightService orchestrator with versioned storage"
```

---

### Task 7: Wire into AppCore — Replace InsightCacheRepo

**Files:**
- Modify: `crates/app-core/Cargo.toml`
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/handlers/notes/insight.rs`

This is the integration task. The insight handler in `app-core` delegates to `InsightService` instead of doing everything inline. The pipeline logic (LLM calls, streaming, events) stays in `insight.rs` but uses `InsightService::store_insight()` and `InsightService::check_cache()` instead of the old `InsightCacheRepo::upsert()` and `get_if_fresh()`.

- [ ] **Step 1: Add feature-insights dependency to app-core**

In `crates/app-core/Cargo.toml`, add:
```toml
feature-insights.workspace = true
```

- [ ] **Step 2: Update AppCore state**

In `crates/app-core/src/state.rs`:

Replace `insight_cache_repo` field with:
```rust
    /// Insight service for versioned insight reviews (None when cognitive feature unavailable).
    pub insight_service: Option<Arc<feature_insights::InsightService>>,
```

Keep `flashcard_repo` and `persona_repo` as-is (they're still used directly).

Also remove the old `InsightCacheRepo` import if it was used directly.

- [ ] **Step 3: Update AppCore init**

In `crates/app-core/src/init/mod.rs`, replace the `insight_cache_repo` initialization with:

```rust
            insight_service: Some(Arc::new(feature_insights::InsightService::new(
                feature_insights::InsightReviewRepo::new(storage_pool.inner().clone()),
                feature_insights::InsightProgressRepo::new(storage_pool.inner().clone()),
                Arc::new(feature_insights::NoopFlashcardAccessor),  // Phase 3 wires real impl
                Arc::new(feature_insights::NoopInsightEmbedder),    // Phase 2 wires real impl
                feature_insights::ProgressWeights::default(),
            ))),
```

- [ ] **Step 4: Update insight.rs handlers to use InsightService**

Rewrite `crates/app-core/src/handlers/notes/insight.rs` to delegate to `InsightService`. The key changes:

1. `note_insight_review()` — use `insight_service.check_cache()` instead of `insight_cache_repo.get_if_fresh()`
2. `run_insight_pipeline()` — use `insight_service.store_insight()` instead of `cache_repo.upsert()`
3. `note_insight_cache_get()` — use `insight_service.get_latest()` and deserialize `InsightContent` from the JSON blob
4. `note_insight_regenerate_tab()` — use `insight_service.update_tab()` instead of `cache_repo.update_tab()`
5. `InsightPipelineArgs` — replace `cache_repo: Option<cognitive::InsightCacheRepo>` with `insight_service: Option<Arc<feature_insights::InsightService>>`

The LLM call logic, streaming, and event emission stay the same — only the storage layer changes.

**IMPORTANT**: The implementer should read the current `insight.rs` (which was fully rewritten in Phase 4) and adapt it. Key adapter pattern:

After the pipeline completes with individual `Option<String>` tab results, assemble into `InsightContent`:
```rust
let content = feature_insights::InsightContent {
    synthesis: synthesis,
    gap_analysis: gaps,
    self_assessment: assessment,
    concept_map: concept_map,
    perspectives: perspectives,
};
// Then call:
insight_service.store_insight(&note_id, &content, &input_hash, &scope, &persona_ids).await;
```

Replace `cache_repo.upsert(...)` with the above. Replace `cache_repo.get_if_fresh(...)` with `insight_service.check_cache(...)`. Replace `cache_repo.update_tab(...)` with `insight_service.update_tab(...)`.

- [ ] **Step 5: Update InsightReviewResponse DTO**

In `crates/desktop-shared/src/commands/notes.rs`, add `version` and `generated_at` to `InsightReviewResponse`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightReviewResponse {
    pub insight_review_id: String,
    pub note_id: String,
    pub version: i64,
    pub generated_at: String,
    pub synthesis: Option<String>,
    pub gap_analysis: Option<String>,
    pub self_assessment: Option<Vec<QuizQuestion>>,
    pub concept_map: Option<String>,
    pub perspectives: Option<String>,
    pub persona_ids: Option<Vec<String>>,
}
```

- [ ] **Step 6: Build and test**

Run: `cargo build --workspace`
Run: `cargo nextest run --workspace`

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/ crates/desktop-shared/src/commands/notes.rs
git commit -m "feat(app-core): wire InsightService into AppCore, replacing InsightCacheRepo"
```

---

### Task 8: Update Tauri Commands + Add Version Listing

**Files:**
- Modify: `crates/desktop/src/commands/notes.rs`

- [ ] **Step 1: Add list_versions and get_evolution Tauri commands**

```rust
#[tauri::command]
pub async fn note_insight_list_versions(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<Vec<InsightVersionResponse>, ApiError> {
    state.note_insight_list_versions(&note_id).await
}
```

Add corresponding `InsightVersionResponse` DTO to `desktop-shared`:

```rust
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightVersionResponse {
    pub id: String,
    pub version: i64,
    pub generated_at: String,
    pub input_hash: String,
    pub has_parent: bool,
}
```

- [ ] **Step 2: Add handler in app-core**

Add `note_insight_list_versions` method to `insight.rs` or `insight_personas.rs`:

```rust
pub async fn note_insight_list_versions(
    &self,
    note_id: &str,
) -> Result<Vec<InsightVersionResponse>, ApiError> {
    let service = self.insight_service.as_ref()
        .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Insight service not available"))?;
    let versions = service.list_versions(note_id).await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
    Ok(versions.into_iter().map(|v| InsightVersionResponse {
        id: v.id,
        version: v.version,
        generated_at: v.generated_at,
        input_hash: v.input_hash,
        has_parent: v.parent_insight_id.is_some(),
    }).collect())
}
```

- [ ] **Step 3: Register in DEV_COMMANDS + dispatch_dev + main.rs**

Follow the existing pattern for adding commands.

- [ ] **Step 4: Build + test**

Run: `cargo build --workspace && cargo nextest run --workspace`

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/ crates/desktop-shared/ crates/app-core/
git commit -m "feat(desktop): add insight version listing Tauri command"
```

---

### Task 9: Deprecate Old InsightCacheRepo

**Files:**
- Modify: `crates/cognitive/src/repos/insight_cache.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Add deprecation warning to InsightCacheRepo**

Add `#[deprecated(note = "Use feature_insights::InsightReviewRepo instead")]` to `InsightCacheRepo` and `InsightCacheRow`.

- [ ] **Step 2: Remove InsightCacheRepo from AppCore state**

If not already done in Task 7, ensure `insight_cache_repo` is removed from `AppCore` fields and init.

- [ ] **Step 3: Build and verify no references remain**

Run: `cargo build --workspace`
Check that nothing still uses `InsightCacheRepo` (except the deprecated module itself and tests).

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/ crates/app-core/
git commit -m "refactor(cognitive): deprecate InsightCacheRepo in favor of feature-insights"
```

---

### Task 10: Final Verification

- [ ] **Step 1: Full test suite**

Run: `cargo nextest run --workspace`
Expected: all tests pass.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: no new warnings.

- [ ] **Step 3: Format**

Run: `cargo fmt --all`

- [ ] **Step 4: Frontend build**

Run: `cd desktop-ui && bun run build`
Expected: compiles (frontend changes are minimal in Phase 1).

- [ ] **Step 5: Commit if needed**

```bash
git add -A && git commit -m "style: format Insight Review V2 Phase 1"
```
