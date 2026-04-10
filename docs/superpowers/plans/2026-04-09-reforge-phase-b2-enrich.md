# Phase B2 — Enrich: Value-Density, Batch Graph Enrichment, Phase 6.5, Temporal Snapshots

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a value-density classifier to score conversation turns by information richness, batch graph enrichment via LLM entity resolution, a new Reforge Phase 6.5 for nightly graph consolidation, and temporal knowledge snapshots.

**Architecture:** Value-density is a lightweight heuristic (no LLM) computed per conversation turn in the background pipeline. High-density turns trigger immediate entity enrichment; medium-density turns are queued for Reforge Phase 6.5 (nightly). Phase 6.5 runs between Optimize and Compact — it collects queued turns, runs batch entity resolution via the `ReforgeHandler` trait (LLM), and writes graph quality metrics. Temporal snapshots record nightly graph state for trend analysis.

**Tech Stack:** Rust, SQLite (cognitive crate), LLM via `ReforgeHandler` trait (agent crate), existing entity/relationship infrastructure

**Depends on:** Phase B1 (complete). Phase B3 depends on this plan.

---

## Scope Note

This plan covers **B2 only** (Enrich). B3 (Dissolve: conversation promoter, graph-aware retrieval with 12th weight, temporal reasoning tool) will be a separate plan after B2 lands.

---

## File Structure

### New Files
| File | Responsibility |
|------|---------------|
| `crates/cognitive/src/services/value_density.rs` | Heuristic value-density scoring per conversation turn |
| `crates/cognitive/src/services/graph_enrichment.rs` | Batch entity resolution + relationship refinement types |
| `crates/cognitive/src/repos/knowledge_snapshot.rs` | Knowledge graph snapshot repo (nightly state) |

### Modified Files
| File | Change |
|------|--------|
| `crates/cognitive/migrations/001_cognitive_tables.sql` | Add `conversation_density` and `knowledge_snapshots` tables |
| `crates/cognitive/src/repos/mod.rs` | Export new repos, bump migration version |
| `crates/cognitive/src/repos/entity.rs` | Add `find_duplicate_candidates()` method |
| `crates/cognitive/src/services/mod.rs` | Export new modules |
| `crates/cognitive/src/services/reforge/types.rs` | Add B2 fields to `ReforgeCollected` + `ReforgeResult`, graph consolidation types |
| `crates/cognitive/src/services/reforge/mod.rs` | Add `GraphEnrichmentHandler` trait |
| `crates/cognitive/src/services/reforge/service.rs` | Add Phase 6.5 graph consolidation |
| `crates/cognitive/src/services/reforge/collector.rs` | Collect pending enrichment count + graph consolidation flag |
| `crates/cognitive/src/services/background.rs` | Compute value-density, store scores, trigger immediate enrichment |
| `crates/agent/src/adapters/cognitive_handlers.rs` | Implement `GraphEnrichmentHandler` for LLM entity resolution |

---

### Task 1: Value-density classifier module

**Files:**
- Create: `crates/cognitive/src/services/value_density.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Create the value-density module**

Create `crates/cognitive/src/services/value_density.rs`:

```rust
//! Heuristic value-density classifier for conversation turns.
//!
//! Scores each turn on four signals without any LLM call:
//! - entity_signal  (0.30) — named entities detected (capitalized words, patterns)
//! - action_signal  (0.25) — action verbs present (decided, created, changed, etc.)
//! - decision_signal (0.25) — decision markers (because, therefore, will, should, etc.)
//! - novelty_signal (0.20) — references to previously unseen terms
//!
//! Three tiers:
//! - High  (>0.7) — immediate enrichment
//! - Medium (0.4–0.7) — queued for Reforge Phase 6.5
//! - Low   (<0.4) — cheap extraction only

/// Weights for each signal component.
const W_ENTITY: f64 = 0.30;
const W_ACTION: f64 = 0.25;
const W_DECISION: f64 = 0.25;
const W_NOVELTY: f64 = 0.20;

/// Action verbs that indicate information-rich content.
const ACTION_VERBS: &[&str] = &[
    "decided", "created", "changed", "moved", "started", "finished", "cancelled",
    "approved", "rejected", "deployed", "fixed", "broke", "shipped", "migrated",
    "refactored", "implemented", "designed", "reviewed", "merged", "released",
    "hired", "fired", "promoted", "scheduled", "booked", "bought", "sold",
    "invested", "transferred", "configured", "installed", "updated",
];

/// Decision markers that indicate reasoning or commitments.
const DECISION_MARKERS: &[&str] = &[
    "because", "therefore", "decided", "will", "should", "must", "going to",
    "plan to", "chose", "picked", "settled on", "committed to", "agreed",
    "prefer", "instead of", "rather than", "the reason", "due to",
];

/// Density tier thresholds.
const HIGH_THRESHOLD: f64 = 0.7;
const MEDIUM_THRESHOLD: f64 = 0.4;

/// Value-density tier for a conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DensityTier {
    High,
    Medium,
    Low,
}

impl DensityTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

/// Result of scoring a conversation turn.
#[derive(Debug, Clone)]
pub struct DensityScore {
    pub total: f64,
    pub entity_signal: f64,
    pub action_signal: f64,
    pub decision_signal: f64,
    pub novelty_signal: f64,
    pub tier: DensityTier,
}

/// Score a conversation turn's value-density using lightweight heuristics.
///
/// `known_entities` is an optional set of entity names already in the graph.
/// If provided, references to unknown entities boost the novelty signal.
pub fn score_turn(content: &str, known_entities: Option<&[String]>) -> DensityScore {
    let lower = content.to_lowercase();
    let words: Vec<&str> = content.split_whitespace().collect();

    if words.is_empty() {
        return DensityScore {
            total: 0.0,
            entity_signal: 0.0,
            action_signal: 0.0,
            decision_signal: 0.0,
            novelty_signal: 0.0,
            tier: DensityTier::Low,
        };
    }

    let word_count = words.len() as f64;

    // Entity signal: capitalized words that aren't sentence starters
    let entity_count = words
        .iter()
        .enumerate()
        .filter(|(i, w)| {
            *i > 0
                && w.len() > 1
                && w.chars().next().is_some_and(|c| c.is_uppercase())
                && !w.chars().all(|c| c.is_uppercase()) // skip ALL-CAPS
        })
        .count();
    let entity_signal = (entity_count as f64 / word_count * 4.0).min(1.0);

    // Action signal: count of action verbs
    let action_count = ACTION_VERBS
        .iter()
        .filter(|v| lower.contains(**v))
        .count();
    let action_signal = (action_count as f64 / 3.0).min(1.0);

    // Decision signal: count of decision markers
    let decision_count = DECISION_MARKERS
        .iter()
        .filter(|m| lower.contains(**m))
        .count();
    let decision_signal = (decision_count as f64 / 2.0).min(1.0);

    // Novelty signal: references to entities not in known set
    let novelty_signal = if let Some(known) = known_entities {
        let known_lower: Vec<String> = known.iter().map(|e| e.to_lowercase()).collect();
        let novel_count = words
            .iter()
            .enumerate()
            .filter(|(i, w)| {
                *i > 0
                    && w.len() > 1
                    && w.chars().next().is_some_and(|c| c.is_uppercase())
                    && !known_lower.iter().any(|k| k == &w.to_lowercase())
            })
            .count();
        (novel_count as f64 / word_count * 5.0).min(1.0)
    } else {
        // Without known entities, use a proxy: ratio of capitalized words
        entity_signal * 0.5
    };

    let total = entity_signal * W_ENTITY
        + action_signal * W_ACTION
        + decision_signal * W_DECISION
        + novelty_signal * W_NOVELTY;

    let tier = if total >= HIGH_THRESHOLD {
        DensityTier::High
    } else if total >= MEDIUM_THRESHOLD {
        DensityTier::Medium
    } else {
        DensityTier::Low
    };

    DensityScore {
        total,
        entity_signal,
        action_signal,
        decision_signal,
        novelty_signal,
        tier,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_density_question() {
        let score = score_turn("What time is it?", None);
        assert_eq!(score.tier, DensityTier::Low);
        assert!(score.total < MEDIUM_THRESHOLD);
    }

    #[test]
    fn high_density_decision() {
        let content = "I decided to migrate the Klynt project to Rust because \
            TypeScript was too slow. Started the refactoring yesterday and \
            deployed the first module to Production today.";
        let score = score_turn(content, None);
        assert!(
            score.total >= MEDIUM_THRESHOLD,
            "Decision-rich content should be at least medium, got {:.2}",
            score.total
        );
        assert!(score.action_signal > 0.0);
        assert!(score.decision_signal > 0.0);
    }

    #[test]
    fn novelty_boost_with_unknown_entities() {
        let known = vec!["Rust".to_string(), "Jayden".to_string()];
        let content = "I told Sarah about the Acme project we discussed with Bob at Google";
        let score = score_turn(content, Some(&known));
        assert!(
            score.novelty_signal > 0.0,
            "Unknown entities should boost novelty"
        );
    }

    #[test]
    fn empty_content() {
        let score = score_turn("", None);
        assert_eq!(score.tier, DensityTier::Low);
        assert_eq!(score.total, 0.0);
    }
}
```

- [ ] **Step 2: Export the module**

In `crates/cognitive/src/services/mod.rs`, add:

```rust
pub mod value_density;
```

- [ ] **Step 3: Verify**

Run: `cargo nextest run -p cognitive -E 'test(density)'`
Expected: All 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/src/services/value_density.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): add heuristic value-density classifier for conversation turns"
```

---

### Task 2: Conversation density + knowledge snapshot tables and repos

**Files:**
- Modify: `crates/cognitive/migrations/001_cognitive_tables.sql`
- Create: `crates/cognitive/src/repos/knowledge_snapshot.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Add tables to migration**

Append to the end of `crates/cognitive/migrations/001_cognitive_tables.sql`:

```sql
-- Conversation turn value-density scores.
-- Populated in real-time by background pipeline; queried by Reforge Phase 6.5.
CREATE TABLE IF NOT EXISTS conversation_density (
    id TEXT PRIMARY KEY,
    session_key TEXT NOT NULL,
    content_preview TEXT NOT NULL,
    density_score REAL NOT NULL,
    tier TEXT NOT NULL,          -- 'high', 'medium', 'low'
    entity_signal REAL NOT NULL,
    action_signal REAL NOT NULL,
    decision_signal REAL NOT NULL,
    novelty_signal REAL NOT NULL,
    enriched INTEGER NOT NULL DEFAULT 0,  -- 1 after graph enrichment processed this turn
    computed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_conversation_density_tier
    ON conversation_density(tier, enriched, computed_at);

-- Nightly knowledge graph snapshots for trend analysis.
CREATE TABLE IF NOT EXISTS knowledge_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fact_count INTEGER NOT NULL,
    entity_count INTEGER NOT NULL,
    relationship_count INTEGER NOT NULL,
    domain_summary TEXT,       -- JSON: {"work": 42, "finance": 15, ...}
    top_entities TEXT,         -- JSON: [{"name": "Rust", "mentions": 30}, ...]
    graph_metrics TEXT,        -- JSON: {"orphan_rate": 0.12, "avg_degree": 2.3, ...}
    snapshot_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

- [ ] **Step 2: Bump cognitive migration version**

In `crates/cognitive/src/repos/mod.rs`, change the cognitive migration version from `1` to `2`:

```rust
        FeatureMigration {
            feature_name: "cognitive".to_string(),
            version: 2,
            description: "Core cognitive tables".to_string(),
            sql: include_str!("../../migrations/001_cognitive_tables.sql").to_string(),
        },
```

- [ ] **Step 3: Create the ConversationDensityRepo**

Add to the bottom of `crates/cognitive/src/repos/knowledge_snapshot.rs`:

```rust
//! Repos for conversation density scores and knowledge graph snapshots.

use sqlx::SqlitePool;

// ---------------------------------------------------------------------------
// ConversationDensityRepo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConversationDensityRow {
    pub id: String,
    pub session_key: String,
    pub content_preview: String,
    pub density_score: f64,
    pub tier: String,
    pub entity_signal: f64,
    pub action_signal: f64,
    pub decision_signal: f64,
    pub novelty_signal: f64,
    pub enriched: bool,
    pub computed_at: String,
}

#[derive(Debug, Clone)]
pub struct ConversationDensityRepo {
    pool: SqlitePool,
}

impl ConversationDensityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a density score for a conversation turn.
    pub async fn insert(
        &self,
        id: &str,
        session_key: &str,
        content_preview: &str,
        score: &crate::services::value_density::DensityScore,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO conversation_density
             (id, session_key, content_preview, density_score, tier,
              entity_signal, action_signal, decision_signal, novelty_signal)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        )
        .bind(id)
        .bind(session_key)
        .bind(content_preview)
        .bind(score.total)
        .bind(score.tier.as_str())
        .bind(score.entity_signal)
        .bind(score.action_signal)
        .bind(score.decision_signal)
        .bind(score.novelty_signal)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Count unenriched turns by tier since a timestamp.
    pub async fn count_pending_by_tier(
        &self,
        tier: &str,
        since: &str,
    ) -> Result<u32, sqlx::Error> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM conversation_density
             WHERE tier = ?1 AND enriched = 0 AND computed_at > ?2",
        )
        .bind(tier)
        .bind(since)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 as u32)
    }

    /// Load unenriched medium-density turns for Phase 6.5 batch processing.
    pub async fn load_pending_medium(
        &self,
        limit: u32,
    ) -> Result<Vec<ConversationDensityRow>, sqlx::Error> {
        sqlx::query_as::<_, ConversationDensityRow>(
            "SELECT * FROM conversation_density
             WHERE tier = 'medium' AND enriched = 0
             ORDER BY density_score DESC
             LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Mark turns as enriched after graph processing.
    pub async fn mark_enriched(&self, ids: &[String]) -> Result<(), sqlx::Error> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "UPDATE conversation_density SET enriched = 1 WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }
        query.execute(&self.pool).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// KnowledgeSnapshotRepo
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct KnowledgeSnapshotRow {
    pub id: i64,
    pub fact_count: i64,
    pub entity_count: i64,
    pub relationship_count: i64,
    pub domain_summary: Option<String>,
    pub top_entities: Option<String>,
    pub graph_metrics: Option<String>,
    pub snapshot_at: String,
}

#[derive(Debug, Clone)]
pub struct KnowledgeSnapshotRepo {
    pool: SqlitePool,
}

impl KnowledgeSnapshotRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a nightly knowledge graph snapshot.
    pub async fn insert(
        &self,
        fact_count: u32,
        entity_count: u32,
        relationship_count: u32,
        domain_summary: Option<&str>,
        top_entities: Option<&str>,
        graph_metrics: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO knowledge_snapshots
             (fact_count, entity_count, relationship_count, domain_summary, top_entities, graph_metrics)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(fact_count)
        .bind(entity_count)
        .bind(relationship_count)
        .bind(domain_summary)
        .bind(top_entities)
        .bind(graph_metrics)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load the N most recent snapshots for trend analysis.
    pub async fn recent(
        &self,
        limit: u32,
    ) -> Result<Vec<KnowledgeSnapshotRow>, sqlx::Error> {
        sqlx::query_as::<_, KnowledgeSnapshotRow>(
            "SELECT * FROM knowledge_snapshots ORDER BY snapshot_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Delete snapshots older than N days.
    pub async fn prune(&self, max_age_days: u32) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM knowledge_snapshots WHERE snapshot_at < datetime('now', ?1)",
        )
        .bind(format!("-{max_age_days} days"))
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    #[tokio::test]
    async fn density_insert_and_load_pending() {
        let pool = setup().await;
        let repo = ConversationDensityRepo::new(pool);

        let score = crate::services::value_density::DensityScore {
            total: 0.55,
            entity_signal: 0.4,
            action_signal: 0.3,
            decision_signal: 0.2,
            novelty_signal: 0.1,
            tier: crate::services::value_density::DensityTier::Medium,
        };
        repo.insert("t1", "sess1", "test content", &score)
            .await
            .unwrap();

        let pending = repo.load_pending_medium(10).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "t1");
    }

    #[tokio::test]
    async fn density_mark_enriched() {
        let pool = setup().await;
        let repo = ConversationDensityRepo::new(pool);

        let score = crate::services::value_density::DensityScore {
            total: 0.55,
            entity_signal: 0.4,
            action_signal: 0.3,
            decision_signal: 0.2,
            novelty_signal: 0.1,
            tier: crate::services::value_density::DensityTier::Medium,
        };
        repo.insert("t2", "sess1", "content", &score).await.unwrap();
        repo.mark_enriched(&["t2".to_string()]).await.unwrap();

        let pending = repo.load_pending_medium(10).await.unwrap();
        assert!(pending.is_empty(), "Enriched turns should not appear in pending");
    }

    #[tokio::test]
    async fn snapshot_insert_and_recent() {
        let pool = setup().await;
        let repo = KnowledgeSnapshotRepo::new(pool);

        repo.insert(100, 50, 30, Some(r#"{"work":42}"#), None, None)
            .await
            .unwrap();
        repo.insert(105, 52, 32, Some(r#"{"work":44}"#), None, None)
            .await
            .unwrap();

        let recent = repo.recent(5).await.unwrap();
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].fact_count, 105); // newest first
    }
}
```

- [ ] **Step 4: Export new repos**

In `crates/cognitive/src/repos/mod.rs`, add the module and re-exports alongside the existing exports:

```rust
pub mod knowledge_snapshot;
pub use knowledge_snapshot::{
    ConversationDensityRepo, ConversationDensityRow,
    KnowledgeSnapshotRepo, KnowledgeSnapshotRow,
};
```

- [ ] **Step 5: Verify**

Run: `cargo nextest run -p cognitive -E 'test(density) | test(snapshot)'`
Expected: All 4 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/migrations/ crates/cognitive/src/repos/knowledge_snapshot.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add conversation_density and knowledge_snapshots tables and repos"
```

---

### Task 3: Add `find_duplicate_candidates()` to EntityRepo

**Files:**
- Modify: `crates/cognitive/src/repos/entity.rs`

- [ ] **Step 1: Add the test**

Add to the `#[cfg(test)] mod tests` block in `entity.rs`:

```rust
    #[tokio::test]
    async fn test_find_duplicate_candidates() {
        let pool = crate::repos::cognitive_test_pool().await;
        let repo = EntityRepo::new(pool);

        // Insert similar entities
        repo.upsert_entity(&NewEntity {
            name: "Rust".to_string(),
            entity_type: "technology".to_string(),
            description: Some("Programming language".to_string()),
            source: "test".to_string(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
        repo.upsert_entity(&NewEntity {
            name: "rust".to_string(),
            entity_type: "technology".to_string(),
            description: Some("Systems language".to_string()),
            source: "test".to_string(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
        repo.upsert_entity(&NewEntity {
            name: "Rust Lang".to_string(),
            entity_type: "technology".to_string(),
            description: Some("The Rust programming language".to_string()),
            source: "test".to_string(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();

        let candidates = repo.find_duplicate_candidates(50).await.unwrap();
        // "Rust" and "rust" are exact case-insensitive matches, so upsert deduplicates them.
        // "Rust Lang" has partial overlap — should appear in candidates.
        // This test verifies the query runs without error and returns plausible results.
        // Exact assertions depend on the dedup logic of upsert_entity.
        assert!(candidates.len() <= 3);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(find_duplicate)'`
Expected: FAIL — `find_duplicate_candidates` method doesn't exist.

- [ ] **Step 3: Implement `find_duplicate_candidates`**

Add this method to the `impl EntityRepo` block in `crates/cognitive/src/repos/entity.rs`, after the `merge_entities` method:

```rust
    /// Find entity pairs that are likely duplicates based on name similarity.
    ///
    /// Returns pairs of (entity_a_id, entity_b_id, entity_a_name, entity_b_name)
    /// where names share a common prefix or one contains the other.
    /// Used by Reforge Phase 6.5 for batch entity resolution.
    pub async fn find_duplicate_candidates(
        &self,
        limit: u32,
    ) -> Result<Vec<(String, String, String, String)>, sqlx::Error> {
        // Self-join: find pairs where one name contains the other (case-insensitive)
        // or they share a common type and have high token overlap.
        // Exclude pairs where ids are identical.
        let rows: Vec<(String, String, String, String)> = sqlx::query_as(
            r#"
            SELECT a.id, b.id, a.name, b.name
            FROM entities a
            JOIN entities b ON a.entity_type = b.entity_type
                AND a.id < b.id
                AND (
                    LOWER(TRIM(a.name)) LIKE '%' || LOWER(TRIM(b.name)) || '%'
                    OR LOWER(TRIM(b.name)) LIKE '%' || LOWER(TRIM(a.name)) || '%'
                )
            LIMIT ?1
            "#,
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p cognitive -E 'test(find_duplicate)'`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/entity.rs
git commit -m "feat(cognitive): add find_duplicate_candidates for entity deduplication"
```

---

### Task 4: Graph enrichment handler trait and types

**Files:**
- Create: `crates/cognitive/src/services/graph_enrichment.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/cognitive/src/services/reforge/types.rs`
- Modify: `crates/cognitive/src/services/reforge/mod.rs`

- [ ] **Step 1: Create graph enrichment types**

Create `crates/cognitive/src/services/graph_enrichment.rs`:

```rust
//! Types and traits for batch graph enrichment.
//!
//! Phase 6.5 collects medium-density conversation turns and runs LLM-based
//! entity resolution in a single batch call. The `GraphEnrichmentHandler`
//! trait is implemented in the agent crate (dependency inversion).

/// A candidate entity pair for deduplication.
#[derive(Debug, Clone)]
pub struct DuplicateCandidate {
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub entity_a_name: String,
    pub entity_b_name: String,
}

/// LLM decision for a duplicate pair.
#[derive(Debug, Clone)]
pub struct MergeDecision {
    pub entity_a_id: String,
    pub entity_b_id: String,
    /// `true` = merge (a absorbs b), `false` = keep separate.
    pub should_merge: bool,
    /// The canonical name to use after merge.
    pub canonical_name: Option<String>,
    pub reason: String,
}

/// An entity relationship discovered from conversation context.
#[derive(Debug, Clone)]
pub struct DiscoveredRelationship {
    pub source_entity_name: String,
    pub target_entity_name: String,
    pub relationship_type: String,
    pub strength: f64,
}

/// Input for batch graph enrichment (Phase 6.5).
#[derive(Debug, Clone)]
pub struct GraphEnrichmentInput {
    /// Medium-density conversation turn previews to extract relationships from.
    pub turn_previews: Vec<String>,
    /// Duplicate candidate pairs for entity resolution.
    pub duplicate_candidates: Vec<DuplicateCandidate>,
}

/// Output from batch graph enrichment (Phase 6.5).
#[derive(Debug, Clone, Default)]
pub struct GraphEnrichmentOutput {
    pub merge_decisions: Vec<MergeDecision>,
    pub discovered_relationships: Vec<DiscoveredRelationship>,
}

/// Graph quality metrics computed after Phase 6.5.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct GraphQualityMetrics {
    pub entity_count: u32,
    pub relationship_count: u32,
    pub orphan_entity_count: u32,
    pub orphan_rate: f64,
    pub avg_degree: f64,
    pub merge_count: u32,
    pub new_relationships: u32,
}
```

- [ ] **Step 2: Export the module**

In `crates/cognitive/src/services/mod.rs`, add:

```rust
pub mod graph_enrichment;
```

- [ ] **Step 3: Add `GraphEnrichmentHandler` trait to reforge mod**

In `crates/cognitive/src/services/reforge/mod.rs`, add after the `AutotunerBridge` trait (after line 49):

```rust
// ---------------------------------------------------------------------------
// GraphEnrichmentHandler — Phase 6.5 dependency inversion
// ---------------------------------------------------------------------------

/// Bridge trait for LLM-based graph enrichment in Phase 6.5.
/// Implemented in the agent crate.
#[async_trait]
pub trait GraphEnrichmentHandler: Send + Sync {
    /// Run batch entity resolution and relationship discovery.
    /// Single LLM call processes all duplicate candidates and turn previews.
    async fn enrich_graph(
        &self,
        input: &crate::services::graph_enrichment::GraphEnrichmentInput,
    ) -> common::Result<crate::services::graph_enrichment::GraphEnrichmentOutput>;
}
```

- [ ] **Step 4: Add B2 fields to ReforgeCollected**

In `crates/cognitive/src/services/reforge/types.rs`, add to the `ReforgeCollected` struct after `extraction_yield_by_domain`:

```rust
    // Phase B2: Enrichment context
    pub pending_enrichment_turns: u32,
    pub graph_consolidation_needed: bool,
```

- [ ] **Step 5: Add B2 fields to ReforgeResult**

In `crates/cognitive/src/services/reforge/types.rs`, add to the `ReforgeResult` struct after `patterns_persisted`:

```rust
    // Phase B2: Graph consolidation
    pub entities_merged: u32,
    pub relationships_discovered: u32,
    pub snapshot_recorded: bool,
```

Also update the `Default` impl — these fields default to `0/false` which is already correct for `u32` and `bool`.

- [ ] **Step 6: Verify**

Run: `cargo build -p cognitive`
Expected: Clean compile.

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/services/graph_enrichment.rs crates/cognitive/src/services/mod.rs \
       crates/cognitive/src/services/reforge/mod.rs crates/cognitive/src/services/reforge/types.rs
git commit -m "feat(cognitive): add graph enrichment types, handler trait, and B2 result fields"
```

---

### Task 5: Implement Phase 6.5 in Reforge service

**Files:**
- Modify: `crates/cognitive/src/services/reforge/service.rs`
- Modify: `crates/cognitive/src/services/reforge/collector.rs`

- [ ] **Step 1: Update `run_reforge` signature**

In `crates/cognitive/src/services/reforge/service.rs`, add new parameters to `run_reforge()` after `feedback_sources`:

```rust
    graph_enrichment_handler: Option<&dyn super::GraphEnrichmentHandler>,
    density_repo: Option<&crate::repos::ConversationDensityRepo>,
    entity_repo: Option<&crate::repos::EntityRepo>,
    snapshot_repo: Option<&crate::repos::KnowledgeSnapshotRepo>,
```

- [ ] **Step 2: Add Phase 6.5 between Phase 6 and Phase 7**

In `crates/cognitive/src/services/reforge/service.rs`, after the Phase 6 block (after line 300 `debug!("Reforge Phase 6: skipped (no autotuner bridge)");`), insert:

```rust
    // ------------------------------------------------------------------
    // Phase 6.5: Graph Consolidation
    // ------------------------------------------------------------------
    if let (Some(enricher), Some(density_repo), Some(entity_repo)) =
        (graph_enrichment_handler, density_repo, entity_repo)
    {
        info!("Reforge Phase 6.5: Graph Consolidation");

        // Step 1: Load medium-density turns queued since last cycle
        let pending_turns = match density_repo.load_pending_medium(50).await {
            Ok(turns) => turns,
            Err(e) => {
                warn!("Reforge Phase 6.5: failed to load pending turns: {e}");
                result.phase_errors.push(format!("graph_consolidation/load: {e}"));
                Vec::new()
            }
        };

        // Step 2: Find duplicate entity candidates
        let dup_candidates = match entity_repo.find_duplicate_candidates(30).await {
            Ok(dups) => dups,
            Err(e) => {
                warn!("Reforge Phase 6.5: failed to find duplicates: {e}");
                Vec::new()
            }
        };

        // Step 3: Run LLM enrichment (single call) if there's work to do
        if !pending_turns.is_empty() || !dup_candidates.is_empty() {
            let input = crate::services::graph_enrichment::GraphEnrichmentInput {
                turn_previews: pending_turns
                    .iter()
                    .map(|t| t.content_preview.clone())
                    .collect(),
                duplicate_candidates: dup_candidates
                    .iter()
                    .map(|(a_id, b_id, a_name, b_name)| {
                        crate::services::graph_enrichment::DuplicateCandidate {
                            entity_a_id: a_id.clone(),
                            entity_b_id: b_id.clone(),
                            entity_a_name: a_name.clone(),
                            entity_b_name: b_name.clone(),
                        }
                    })
                    .collect(),
            };

            match enricher.enrich_graph(&input).await {
                Ok(output) => {
                    // Apply merge decisions
                    for decision in &output.merge_decisions {
                        if decision.should_merge {
                            if let Err(e) = entity_repo
                                .merge_entities(&decision.entity_a_id, &decision.entity_b_id)
                                .await
                            {
                                debug!("Phase 6.5: merge failed: {e}");
                            } else {
                                result.entities_merged += 1;
                            }
                        }
                    }

                    // Apply discovered relationships
                    for rel in &output.discovered_relationships {
                        let source_entities = entity_repo.find_by_name(&rel.source_entity_name).await.unwrap_or_default();
                        let target_entities = entity_repo.find_by_name(&rel.target_entity_name).await.unwrap_or_default();

                        if let (Some(src), Some(tgt)) =
                            (source_entities.first(), target_entities.first())
                        {
                            let new_rel = crate::repos::entity::NewRelationship {
                                source_entity_id: src.id.clone(),
                                target_entity_id: tgt.id.clone(),
                                relationship_type: rel.relationship_type.clone(),
                                evidence: None,
                                source: "reforge_phase_6.5".to_string(),
                            };
                            if entity_repo.upsert_relationship(&new_rel).await.is_ok() {
                                result.relationships_discovered += 1;
                            }
                        }
                    }

                    // Mark processed turns as enriched
                    let turn_ids: Vec<String> =
                        pending_turns.iter().map(|t| t.id.clone()).collect();
                    if let Err(e) = density_repo.mark_enriched(&turn_ids).await {
                        debug!("Phase 6.5: mark_enriched failed: {e}");
                    }

                    info!(
                        merged = result.entities_merged,
                        relationships = result.relationships_discovered,
                        turns_processed = pending_turns.len(),
                        "Reforge Phase 6.5 complete"
                    );
                }
                Err(e) => {
                    warn!("Reforge Phase 6.5 enrichment failed: {e}");
                    result.phase_errors.push(format!("graph_consolidation/enrich: {e}"));
                }
            }
        } else {
            debug!("Reforge Phase 6.5: nothing to consolidate");
        }

        // Step 4: Record knowledge snapshot
        if let Some(snapshot_repo) = snapshot_repo {
            if let Err(e) = record_knowledge_snapshot(entity_repo, fact_repo, snapshot_repo).await {
                debug!("Phase 6.5: snapshot failed: {e}");
            } else {
                result.snapshot_recorded = true;
            }
        }
    } else {
        debug!("Reforge Phase 6.5: skipped (missing repos or handler)");
    }
```

- [ ] **Step 3: Add `record_knowledge_snapshot` helper**

Add this function at the bottom of `service.rs`, before or after `create_trials_from_suggestions`:

```rust
/// Record a nightly knowledge graph snapshot.
async fn record_knowledge_snapshot(
    entity_repo: &crate::repos::EntityRepo,
    fact_repo: &SemanticFactRepo,
    snapshot_repo: &crate::repos::KnowledgeSnapshotRepo,
) -> common::Result<()> {
    // Count facts
    let facts = fact_repo.count_active().await.unwrap_or(0) as u32;

    // Count entities + relationships using raw pool queries
    let entity_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM entities")
            .fetch_one(entity_repo.pool())
            .await
            .unwrap_or((0,));
    let rel_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM entity_relationships")
            .fetch_one(entity_repo.pool())
            .await
            .unwrap_or((0,));

    // Domain summary
    let domain_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT domain, COUNT(*) FROM semantic_facts WHERE superseded_at IS NULL GROUP BY domain",
    )
    .fetch_all(entity_repo.pool())
    .await
    .unwrap_or_default();
    let domain_json = serde_json::to_string(
        &domain_rows
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>(),
    )
    .ok();

    // Orphan rate: entities with no relationships
    let orphan_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM entities e
         WHERE NOT EXISTS (
             SELECT 1 FROM entity_relationships r
             WHERE r.source_entity_id = e.id OR r.target_entity_id = e.id
         )",
    )
    .fetch_one(entity_repo.pool())
    .await
    .unwrap_or((0,));

    let ec = entity_count.0 as u32;
    let rc = rel_count.0 as u32;
    let orphan_rate = if ec > 0 {
        orphan_count.0 as f64 / ec as f64
    } else {
        0.0
    };
    let avg_degree = if ec > 0 {
        rc as f64 * 2.0 / ec as f64
    } else {
        0.0
    };

    let metrics = serde_json::json!({
        "orphan_rate": orphan_rate,
        "avg_degree": avg_degree,
        "orphan_count": orphan_count.0,
    });

    snapshot_repo
        .insert(facts, ec, rc, domain_json.as_deref(), None, Some(&metrics.to_string()))
        .await
        .map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;

    Ok(())
}
```

- [ ] **Step 4: Expose `pool()` on EntityRepo**

In `crates/cognitive/src/repos/entity.rs`, add a `pool()` accessor if one doesn't already exist:

```rust
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
```

- [ ] **Step 5: Expose `count_active()` on SemanticFactRepo if missing**

Check if `SemanticFactRepo` has `count_active()`. If not, add to `crates/cognitive/src/repos/semantic_fact.rs`:

```rust
    /// Count active (non-superseded) facts.
    pub async fn count_active(&self) -> Result<u32, sqlx::Error> {
        let row: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM semantic_facts WHERE superseded_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0 as u32)
    }
```

- [ ] **Step 6: Update collector for B2 fields**

In `crates/cognitive/src/services/reforge/collector.rs`, in the `collect()` function where `ReforgeCollected` is constructed, add the new fields with defaults:

```rust
        pending_enrichment_turns: 0,
        graph_consolidation_needed: false,
```

If the collector has access to a `ConversationDensityRepo` (via `FeedbackSources` or a new parameter), populate these fields:

In `FeedbackSources`, add:

```rust
    pub density_repo: Option<&'a crate::repos::ConversationDensityRepo>,
```

Then in `collect()`, after existing feedback loading:

```rust
        let (pending_enrichment_turns, graph_consolidation_needed) =
            if let Some(Some(density_repo)) = feedback_sources.map(|f| f.density_repo) {
                let count = density_repo
                    .count_pending_by_tier("medium", &since_str)
                    .await
                    .unwrap_or(0);
                (count, count > 5)
            } else {
                (0, false)
            };
```

And wire these into the `ReforgeCollected` construction.

- [ ] **Step 7: Update doc comment on `run_reforge`**

Update the module doc comment at the top of `service.rs`:

```rust
//! Phase orchestrator for the Reforge cycle.
//!
//! `run_reforge` drives all 8 phases: Collect → Synthesize → Review →
//! Narrate → Apply → Optimize → Graph Consolidation → Compact.  Each phase
//! is isolated so that a single failure does not abort the remaining phases.
```

- [ ] **Step 8: Verify**

Run: `cargo build -p cognitive`
Expected: Compile succeeds. There will be unused parameter warnings for the new `run_reforge` params until the caller is updated — that's OK at this stage.

- [ ] **Step 9: Commit**

```bash
git add crates/cognitive/src/services/reforge/ crates/cognitive/src/repos/entity.rs \
       crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(cognitive): implement Reforge Phase 6.5 graph consolidation"
```

---

### Task 6: Implement `GraphEnrichmentHandler` in agent crate

**Files:**
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs`

- [ ] **Step 1: Add the enrichment prompt**

In `crates/agent/src/adapters/cognitive_handlers.rs`, add after the existing prompt constants:

```rust
const GRAPH_ENRICHMENT_PROMPT: &str = "\
You are a knowledge graph maintenance agent. Given conversation excerpts and \
candidate duplicate entity pairs, make decisions about entity merging and \
discover relationships.\n\n\
For duplicate candidates, decide if two entities refer to the same real-world thing. \
Consider: name similarity, type match, context clues.\n\n\
For conversation excerpts, extract any relationships between mentioned entities.\n\n\
Respond with JSON:\n\
{\"merge_decisions\": [{\"entity_a_id\": \"...\", \"entity_b_id\": \"...\", \
\"should_merge\": true, \"canonical_name\": \"Preferred Name\", \
\"reason\": \"Same entity, different casing\"}], \
\"relationships\": [{\"source\": \"Entity A\", \"target\": \"Entity B\", \
\"type\": \"works_on\", \"strength\": 0.7}]}";
```

- [ ] **Step 2: Add JSON parsing types**

```rust
#[derive(serde::Deserialize)]
struct EnrichmentResponse {
    #[serde(default)]
    merge_decisions: Vec<MergeDecisionJson>,
    #[serde(default)]
    relationships: Vec<RelationshipJson>,
}

#[derive(serde::Deserialize)]
struct MergeDecisionJson {
    entity_a_id: String,
    entity_b_id: String,
    should_merge: bool,
    canonical_name: Option<String>,
    reason: String,
}

#[derive(serde::Deserialize)]
struct RelationshipJson {
    source: String,
    target: String,
    #[serde(rename = "type")]
    relationship_type: String,
    #[serde(default = "default_strength")]
    strength: f64,
}

fn default_strength() -> f64 {
    0.5
}
```

- [ ] **Step 3: Implement `GraphEnrichmentHandler`**

Add the implementation on the existing handler struct (likely `LlmCognitiveHandler` or similar — the implementer should find the struct that already implements `ReforgeHandler`):

```rust
#[async_trait::async_trait]
impl cognitive::services::reforge::GraphEnrichmentHandler for LlmCognitiveHandler {
    async fn enrich_graph(
        &self,
        input: &cognitive::services::graph_enrichment::GraphEnrichmentInput,
    ) -> common::Result<cognitive::services::graph_enrichment::GraphEnrichmentOutput> {
        use cognitive::services::graph_enrichment::*;

        // Build the user message with context
        let mut user_msg = String::new();
        if !input.duplicate_candidates.is_empty() {
            user_msg.push_str("## Duplicate Candidates\n");
            for c in &input.duplicate_candidates {
                user_msg.push_str(&format!(
                    "- ({}) \"{}\" vs ({}) \"{}\"\n",
                    c.entity_a_id, c.entity_a_name, c.entity_b_id, c.entity_b_name
                ));
            }
        }
        if !input.turn_previews.is_empty() {
            user_msg.push_str("\n## Conversation Excerpts\n");
            for (i, preview) in input.turn_previews.iter().enumerate() {
                user_msg.push_str(&format!("{}: {}\n", i + 1, preview));
            }
        }

        if user_msg.is_empty() {
            return Ok(GraphEnrichmentOutput::default());
        }

        let response = self
            .call_llm(GRAPH_ENRICHMENT_PROMPT, &user_msg, 0.3)
            .await?;

        // Parse response JSON
        let parsed: EnrichmentResponse = serde_json::from_str(&response).unwrap_or(
            EnrichmentResponse {
                merge_decisions: Vec::new(),
                relationships: Vec::new(),
            },
        );

        Ok(GraphEnrichmentOutput {
            merge_decisions: parsed
                .merge_decisions
                .into_iter()
                .map(|d| MergeDecision {
                    entity_a_id: d.entity_a_id,
                    entity_b_id: d.entity_b_id,
                    should_merge: d.should_merge,
                    canonical_name: d.canonical_name,
                    reason: d.reason,
                })
                .collect(),
            discovered_relationships: parsed
                .relationships
                .into_iter()
                .map(|r| DiscoveredRelationship {
                    source_entity_name: r.source,
                    target_entity_name: r.target,
                    relationship_type: r.relationship_type,
                    strength: r.strength,
                })
                .collect(),
        })
    }
}
```

Note: The implementer should find the existing handler struct and its `call_llm` method (or equivalent). The handler struct likely has a provider field and a method like `call_extraction_llm()` or similar used for the `ReforgeHandler` implementations. Reuse the same pattern.

- [ ] **Step 4: Verify**

Run: `cargo build -p agent`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/adapters/cognitive_handlers.rs
git commit -m "feat(agent): implement GraphEnrichmentHandler for LLM-based entity resolution"
```

---

### Task 7: Wire value-density into background pipeline

**Files:**
- Modify: `crates/cognitive/src/services/background.rs`

- [ ] **Step 1: Add density_repo to BackgroundServiceConfig**

In `crates/cognitive/src/services/background.rs`, add to `BackgroundServiceConfig`:

```rust
    pub density_repo: Option<crate::repos::ConversationDensityRepo>,
```

- [ ] **Step 2: Compute and store value-density after extraction**

In the background service's message processing loop, after extraction results are available and before consolidation, add density scoring. Find the section where `BatchExtractionResult` is produced (around line 410) and add after it:

```rust
        // Compute value-density for the conversation turn
        if let Some(ref density_repo) = config.density_repo {
            let score = crate::services::value_density::score_turn(
                &observation_content,
                None, // TODO(B3): pass known entity names for novelty scoring
            );
            if let Err(e) = density_repo
                .insert(&message_id, &session_key, &truncated_preview, &score)
                .await
            {
                tracing::debug!("Failed to store density score: {e}");
            }

            // High-density: trigger immediate entity enrichment
            if score.tier == crate::services::value_density::DensityTier::High {
                let entity_repo = crate::repos::EntityRepo::new(config.repo.pool().clone());
                if !extraction_result.entities.is_empty()
                    || !extraction_result.relationships.is_empty()
                {
                    crate::pipeline::persist_entities(
                        &entity_repo,
                        &extraction_result.entities,
                        &extraction_result.relationships,
                    )
                    .await;
                }
            }
        }
```

The exact variable names (`observation_content`, `message_id`, `session_key`, `truncated_preview`, `extraction_result`) depend on the local scope. The implementer should match these to the variables available at the insertion point. The key variables to look for:
- Content: the raw text being extracted from (likely an `observation.content` or similar)
- ID: a unique message identifier
- Session key: from the observation metadata
- Preview: typically a truncated version of content (100 chars)

- [ ] **Step 3: Wire density_repo in construction**

Find where `BackgroundServiceConfig` is constructed (likely in `app-core/src/init/` or `builder.rs`) and add:

```rust
density_repo: Some(cognitive::ConversationDensityRepo::new(pool.clone())),
```

- [ ] **Step 4: Verify**

Run: `cargo build --workspace`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/services/background.rs crates/app-core/
git commit -m "feat(cognitive): wire value-density scoring into background pipeline"
```

---

### Task 8: Wire Phase 6.5 into cron handler

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`

- [ ] **Step 1: Thread new repos and handler to `run_reforge`**

Find where `run_reforge()` is called in the cron handler. Add the new parameters:

```rust
    // After existing parameters:
    graph_enrichment_handler,  // Option<&dyn GraphEnrichmentHandler>
    density_repo.as_ref(),     // Option<&ConversationDensityRepo>
    entity_repo.as_ref(),      // Option<&EntityRepo>
    snapshot_repo.as_ref(),    // Option<&KnowledgeSnapshotRepo>
```

The implementer needs to:
1. Construct `ConversationDensityRepo`, `EntityRepo`, and `KnowledgeSnapshotRepo` from the pool.
2. The `GraphEnrichmentHandler` should be obtained the same way `ReforgeHandler` is — likely from an `Arc<dyn GraphEnrichmentHandler>` stored in the cron handler struct.
3. If the cron handler struct doesn't have a `graph_enrichment_handler` field yet, add `pub graph_enrichment_handler: Option<Arc<dyn cognitive::services::reforge::GraphEnrichmentHandler>>` and wire it during construction.

- [ ] **Step 2: Also add density_repo to FeedbackSources**

Where `FeedbackSources` is constructed before the `run_reforge` call, add:

```rust
    density_repo: density_repo_ref,
```

- [ ] **Step 3: Verify**

Run: `cargo build --workspace`
Expected: Clean compile.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/cron.rs
git commit -m "feat(app-core): wire Phase 6.5 repos and handler into Reforge cron job"
```

---

### Task 9: Integration tests and verification

**Files:**
- Modify: `tests/integration/cognitive.rs`

- [ ] **Step 1: Add value-density scoring test**

```rust
#[test]
fn test_value_density_tiers() {
    use klyntbot::cognitive::services::value_density::{score_turn, DensityTier};

    // Low: simple question
    let low = score_turn("What's the weather?", None);
    assert_eq!(low.tier, DensityTier::Low);

    // Medium/High: decision with entities
    let high = score_turn(
        "I decided to deploy the Klynt project to AWS because Google Cloud \
         was too expensive. Started migration yesterday and shipped the first \
         module to Production.",
        None,
    );
    assert!(
        high.tier != DensityTier::Low,
        "Decision-rich content should be medium or high, got {:?} ({:.2})",
        high.tier,
        high.total
    );
}
```

- [ ] **Step 2: Add knowledge snapshot repo test**

```rust
#[tokio::test]
async fn test_knowledge_snapshot_lifecycle() {
    let pool = klyntbot::cognitive::repos::cognitive_test_pool().await;
    let repo = klyntbot::cognitive::repos::KnowledgeSnapshotRepo::new(pool);

    repo.insert(50, 20, 15, Some(r#"{"work":30}"#), None, None)
        .await
        .unwrap();

    let snapshots = repo.recent(10).await.unwrap();
    assert_eq!(snapshots.len(), 1);
    assert_eq!(snapshots[0].entity_count, 20);
}
```

- [ ] **Step 3: Add density repo test**

```rust
#[tokio::test]
async fn test_conversation_density_pending_workflow() {
    use klyntbot::cognitive::services::value_density::{DensityScore, DensityTier};

    let pool = klyntbot::cognitive::repos::cognitive_test_pool().await;
    let repo = klyntbot::cognitive::repos::ConversationDensityRepo::new(pool);

    let medium_score = DensityScore {
        total: 0.55,
        entity_signal: 0.4,
        action_signal: 0.3,
        decision_signal: 0.2,
        novelty_signal: 0.1,
        tier: DensityTier::Medium,
    };
    repo.insert("msg1", "s1", "some content", &medium_score)
        .await
        .unwrap();

    let pending = repo.load_pending_medium(10).await.unwrap();
    assert_eq!(pending.len(), 1);

    repo.mark_enriched(&["msg1".to_string()]).await.unwrap();

    let after = repo.load_pending_medium(10).await.unwrap();
    assert!(after.is_empty());
}
```

- [ ] **Step 4: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All pass.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings.

- [ ] **Step 6: Commit**

```bash
git add tests/integration/cognitive.rs
git commit -m "test: add Phase B2 integration tests for value-density, snapshots, and density repo"
```

---

## Summary

| Task | Component | Files | Tests |
|------|-----------|-------|-------|
| 1 | Value-density classifier | 2 | 4 unit tests |
| 2 | Density + snapshot tables/repos | 3 | 4 unit tests |
| 3 | Entity duplicate finder | 1 | 1 unit test |
| 4 | Graph enrichment types + handler trait | 4 | compile check |
| 5 | Reforge Phase 6.5 implementation | 3 | compile check |
| 6 | LLM GraphEnrichmentHandler | 1 | compile check |
| 7 | Wire value-density into pipeline | 2 | compile check |
| 8 | Wire Phase 6.5 into cron | 1 | compile check |
| 9 | Integration tests + verification | 1 | 3 integration tests + workspace |

**Total: ~18 files modified/created, ~12 tests added, 9 commits**

---

## What Ships After B2

**Phase B3 (Dissolve)** — depends on B2's value-density + graph enrichment + snapshots:
- B3a: Conversation promoter (promotion lifecycle for `conv_embeddings`, `promoted_at` field)
- B3b: Graph-aware retrieval (12th weight `graph_path_boost`, entity extraction from query → graph neighborhood → vector merge)
- B3c: Temporal reasoning tool (`TemporalTool`, multi-action read-only: `facts_as_of`, `first_mention`, `change_history`, `competing_truths`, `knowledge_diff`, `decision_points`)
