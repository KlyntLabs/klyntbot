# Phase 1: BM25 Full-Text Search + Annotation System

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add FTS5-powered BM25 search across all cognitive tables and a new annotation system for persistent notes attached to any entity.

**Architecture:** Two independent subsystems that share the FTS5 infrastructure. BM25 adds a third retrieval signal alongside vector search and FSRS decay. Annotations are a new table + repo + tool + context source. Both integrate into the existing cognitive pipeline.

**Tech Stack:** SQLite FTS5 (porter tokenizer), sqlx, async-trait, serde, uuid, chrono

---

## File Structure

### BM25 (Upgrade 1)
| File | Action | Responsibility |
|------|--------|---------------|
| `crates/cognitive/migrations/005_fts5_tables.sql` | Create | FTS5 virtual tables + sync triggers for semantic_facts, episodic_memories, procedural_rules |
| `crates/cognitive/src/search/mod.rs` | Create | Module declaration |
| `crates/cognitive/src/search/bm25.rs` | Create | BM25 query functions against FTS5 tables |
| `crates/cognitive/src/repos/semantic_fact.rs` | Modify | Add `search_fts()` method |
| `crates/cognitive/src/repos/episodic_memory.rs` | Modify | Add `search_fts()` method |
| `crates/cognitive/src/repos/procedural_rule.rs` | Modify | Add `search_fts()` method |
| `crates/cognitive/src/retrieval.rs` | Modify | Add BM25 as third signal via `search_fts`, triple RRF merge |
| `crates/tools-core/src/search.rs` | Modify | Add `rrf_merge_triple()` for 3-signal fusion |
| `crates/cognitive/src/lib.rs` | Modify | Add `pub mod search;` |

### Annotations (Upgrade 2)
| File | Action | Responsibility |
|------|--------|---------------|
| `crates/cognitive/migrations/006_annotations.sql` | Create | Annotations table + indexes |
| `crates/cognitive/src/repos/annotation.rs` | Create | AnnotationRepo CRUD + FTS search |
| `crates/cognitive/src/repos/mod.rs` | Modify | Add annotation module + re-export |
| `crates/cognitive/src/types.rs` | Modify | Add Annotation struct |
| `crates/tools/src/annotate.rs` | Create | Annotate tool (create/get/list/delete/search) |
| `crates/tools/src/lib.rs` | Modify | Add annotate module |
| `crates/agent/src/context_sources/annotation.rs` | Create | AnnotationContextSource |
| `crates/agent/src/context_sources/mod.rs` | Modify | Add annotation module |
| `crates/app-core/src/lib.rs` | Modify | Wire AnnotationRepo init + register AnnotationContextSource |

---

## Chunk 1: BM25 Infrastructure

### Task 1: FTS5 Migration

**Files:**
- Create: `crates/cognitive/migrations/005_fts5_tables.sql`

- [ ] **Step 1: Write the FTS5 migration SQL**

```sql
-- crates/cognitive/migrations/005_fts5_tables.sql

-- Full-text index for semantic facts
CREATE VIRTUAL TABLE IF NOT EXISTS semantic_facts_fts USING fts5(
    id UNINDEXED,
    domain,
    subject,
    predicate,
    object,
    memory_type,
    content='semantic_facts',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

-- Keep FTS in sync with semantic_facts
CREATE TRIGGER IF NOT EXISTS semantic_facts_ai AFTER INSERT ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(rowid, id, domain, subject, predicate, object, memory_type)
    VALUES (new.rowid, new.id, new.domain, new.subject, new.predicate, new.object, new.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS semantic_facts_ad AFTER DELETE ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(semantic_facts_fts, rowid, id, domain, subject, predicate, object, memory_type)
    VALUES ('delete', old.rowid, old.id, old.domain, old.subject, old.predicate, old.object, old.memory_type);
END;

CREATE TRIGGER IF NOT EXISTS semantic_facts_au AFTER UPDATE ON semantic_facts BEGIN
    INSERT INTO semantic_facts_fts(semantic_facts_fts, rowid, id, domain, subject, predicate, object, memory_type)
    VALUES ('delete', old.rowid, old.id, old.domain, old.subject, old.predicate, old.object, old.memory_type);
    INSERT INTO semantic_facts_fts(rowid, id, domain, subject, predicate, object, memory_type)
    VALUES (new.rowid, new.id, new.domain, new.subject, new.predicate, new.object, new.memory_type);
END;

-- Full-text index for episodic memories
CREATE VIRTUAL TABLE IF NOT EXISTS episodic_memories_fts USING fts5(
    id UNINDEXED,
    domain,
    content,
    summary,
    content='episodic_memories',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS episodic_memories_ai AFTER INSERT ON episodic_memories BEGIN
    INSERT INTO episodic_memories_fts(rowid, id, domain, content, summary)
    VALUES (new.rowid, new.id, new.domain, new.content, new.summary);
END;

CREATE TRIGGER IF NOT EXISTS episodic_memories_ad AFTER DELETE ON episodic_memories BEGIN
    INSERT INTO episodic_memories_fts(episodic_memories_fts, rowid, id, domain, content, summary)
    VALUES ('delete', old.rowid, old.id, old.domain, old.content, old.summary);
END;

CREATE TRIGGER IF NOT EXISTS episodic_memories_au AFTER UPDATE ON episodic_memories BEGIN
    INSERT INTO episodic_memories_fts(episodic_memories_fts, rowid, id, domain, content, summary)
    VALUES ('delete', old.rowid, old.id, old.domain, old.content, old.summary);
    INSERT INTO episodic_memories_fts(rowid, id, domain, content, summary)
    VALUES (new.rowid, new.id, new.domain, new.content, new.summary);
END;

-- Full-text index for procedural rules
CREATE VIRTUAL TABLE IF NOT EXISTS procedural_rules_fts USING fts5(
    id UNINDEXED,
    domain,
    rule_text,
    content='procedural_rules',
    content_rowid='rowid',
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS procedural_rules_ai AFTER INSERT ON procedural_rules BEGIN
    INSERT INTO procedural_rules_fts(rowid, id, domain, rule_text)
    VALUES (new.rowid, new.id, new.domain, new.rule_text);
END;

CREATE TRIGGER IF NOT EXISTS procedural_rules_ad AFTER DELETE ON procedural_rules BEGIN
    INSERT INTO procedural_rules_fts(procedural_rules_fts, rowid, id, domain, rule_text)
    VALUES ('delete', old.rowid, old.id, old.domain, old.rule_text);
END;

CREATE TRIGGER IF NOT EXISTS procedural_rules_au AFTER UPDATE ON procedural_rules BEGIN
    INSERT INTO procedural_rules_fts(procedural_rules_fts, rowid, id, domain, rule_text)
    VALUES ('delete', old.rowid, old.id, old.domain, old.rule_text);
    INSERT INTO procedural_rules_fts(rowid, id, domain, rule_text)
    VALUES (new.rowid, new.id, new.domain, new.rule_text);
END;
```

- [ ] **Step 2: Verify the migration applies cleanly**

Run: `cargo nextest run -p cognitive -E 'test(upsert_and_get)'`
Expected: PASS — existing tests still work (migration runs via `cognitive_test_pool()`)

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/migrations/005_fts5_tables.sql
git commit -m "feat(cognitive): add FTS5 virtual tables and sync triggers for BM25 search"
```

---

### Task 2: BM25 Search Module — Tests First

**Files:**
- Create: `crates/cognitive/src/search/mod.rs`
- Create: `crates/cognitive/src/search/bm25.rs`
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Create search module declaration**

```rust
// crates/cognitive/src/search/mod.rs
pub mod bm25;
```

- [ ] **Step 2: Add module to lib.rs**

Add `pub mod search;` to `crates/cognitive/src/lib.rs` alongside existing module declarations.

- [ ] **Step 3: Write failing tests for BM25 search**

```rust
// crates/cognitive/src/search/bm25.rs

//! BM25 full-text search across cognitive memory tables via SQLite FTS5.

use sqlx::SqlitePool;

/// A BM25 search result with its FTS5 rank score.
#[derive(Debug, Clone)]
pub struct Bm25Result {
    pub id: String,
    /// FTS5 rank (negative BM25 — lower = better match). Negated here so higher = better.
    pub score: f64,
    /// Which table this result came from.
    pub source_table: &'static str,
}

/// Search semantic_facts via FTS5.
pub async fn search_semantic_facts(
    pool: &SqlitePool,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<Bm25Result>, sqlx::Error> {
    let sql = r#"
        SELECT fts.id, -fts.rank AS score
        FROM semantic_facts_fts fts
        INNER JOIN semantic_facts f ON f.id = fts.id
        WHERE semantic_facts_fts MATCH ?1
        AND (?2 IS NULL OR f.domain = ?2)
        AND f.superseded_at IS NULL
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as::<_, (String, f64)>(sql)
        .bind(query)
        .bind(domain)
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(id, score)| Bm25Result {
                    id,
                    score,
                    source_table: "semantic_facts",
                })
                .collect()
        })
}

/// Search episodic_memories via FTS5.
pub async fn search_episodic_memories(
    pool: &SqlitePool,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<Bm25Result>, sqlx::Error> {
    let sql = r#"
        SELECT fts.id, -fts.rank AS score
        FROM episodic_memories_fts fts
        INNER JOIN episodic_memories e ON e.id = fts.id
        WHERE episodic_memories_fts MATCH ?1
        AND (?2 IS NULL OR e.domain = ?2)
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as::<_, (String, f64)>(sql)
        .bind(query)
        .bind(domain)
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(id, score)| Bm25Result {
                    id,
                    score,
                    source_table: "episodic_memories",
                })
                .collect()
        })
}

/// Search procedural_rules via FTS5.
pub async fn search_procedural_rules(
    pool: &SqlitePool,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<Bm25Result>, sqlx::Error> {
    let sql = r#"
        SELECT fts.id, -fts.rank AS score
        FROM procedural_rules_fts fts
        INNER JOIN procedural_rules r ON r.id = fts.id
        WHERE procedural_rules_fts MATCH ?1
        AND (?2 IS NULL OR r.domain = ?2)
        AND r.active = 1
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as::<_, (String, f64)>(sql)
        .bind(query)
        .bind(domain)
        .bind(limit as i64)
        .fetch_all(pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|(id, score)| Bm25Result {
                    id,
                    score,
                    source_table: "procedural_rules",
                })
                .collect()
        })
}

/// Unified BM25 search across all cognitive tables.
/// Returns results sorted by score descending, limited to `limit` total.
pub async fn bm25_search_all(
    pool: &SqlitePool,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<Bm25Result>, sqlx::Error> {
    // Query all three tables with per-table limit, then merge
    let per_table_limit = limit * 2; // over-fetch to get good candidates
    let (facts, episodes, rules) = tokio::try_join!(
        search_semantic_facts(pool, query, domain, per_table_limit),
        search_episodic_memories(pool, query, domain, per_table_limit),
        search_procedural_rules(pool, query, domain, per_table_limit),
    )?;

    let mut all: Vec<Bm25Result> = Vec::with_capacity(facts.len() + episodes.len() + rules.len());
    all.extend(facts);
    all.extend(episodes);
    all.extend(rules);

    // Sort by score descending
    all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    all.truncate(limit);

    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::SemanticFactRepo;
    use crate::repos::EpisodicMemoryRepo;
    use crate::repos::ProceduralRuleRepo;
    use crate::types::{SemanticFact, EpisodicMemory, ProceduralRule};

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_fact(id: &str, predicate: &str, object: &str) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: "productivity".into(),
            subject: "user".into(),
            predicate: predicate.into(),
            object: object.into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            memory_type: "fact".to_string(),
        }
    }

    fn test_memory(id: &str, content: &str) -> EpisodicMemory {
        EpisodicMemory {
            id: id.into(),
            domain: "productivity".into(),
            content: content.into(),
            summary: None,
            importance: 0.5,
            occurred_at: "2026-03-06T10:00:00".into(),
            recorded_at: "2026-03-06T10:00:00".into(),
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
        }
    }

    fn test_rule(id: &str, rule_text: &str) -> ProceduralRule {
        ProceduralRule {
            id: id.into(),
            domain: "productivity".into(),
            rule_text: rule_text.into(),
            confidence: 0.8,
            source: "reflection".into(),
            signal_count: 1,
            created_at: "2026-03-06".into(),
            updated_at: "2026-03-06".into(),
            active: true,
            project_id: None,
        }
    }

    #[tokio::test]
    async fn test_fts5_semantic_facts_basic_search() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        repo.upsert(&test_fact("f1", "peak_hours", "morning 9-11am")).await.unwrap();
        repo.upsert(&test_fact("f2", "break_pattern", "every 90 minutes")).await.unwrap();
        repo.upsert(&test_fact("f3", "coffee_preference", "black espresso")).await.unwrap();

        let results = search_semantic_facts(&pool, "morning hours", None, 10).await.unwrap();
        assert!(!results.is_empty(), "Should find facts matching 'morning hours'");
        assert_eq!(results[0].source_table, "semantic_facts");
    }

    #[tokio::test]
    async fn test_fts5_semantic_facts_domain_filter() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        let mut f1 = test_fact("f1", "peak_hours", "morning");
        f1.domain = "productivity".into();
        let mut f2 = test_fact("f2", "budget", "morning routine costs");
        f2.domain = "finance".into();

        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let results = search_semantic_facts(&pool, "morning", Some("productivity"), 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "f1");
    }

    #[tokio::test]
    async fn test_fts5_excludes_superseded_facts() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        repo.upsert(&test_fact("f1", "peak_hours", "morning 9am")).await.unwrap();
        repo.upsert(&test_fact("f2", "peak_hours", "morning 10am")).await.unwrap();
        repo.supersede("f1", "f2").await.unwrap();

        let results = search_semantic_facts(&pool, "morning peak", None, 10).await.unwrap();
        // f1 is superseded, only f2 should appear
        let ids: Vec<&str> = results.iter().map(|r| r.id.as_str()).collect();
        assert!(!ids.contains(&"f1"), "Superseded fact should be excluded");
    }

    #[tokio::test]
    async fn test_fts5_episodic_memories_search() {
        let pool = setup().await;
        let repo = EpisodicMemoryRepo::new(pool.clone());

        repo.upsert(&test_memory("e1", "Had a productive morning coding session on the Rust project")).await.unwrap();
        repo.upsert(&test_memory("e2", "Went for a walk in the afternoon park")).await.unwrap();

        let results = search_episodic_memories(&pool, "coding Rust", None, 10).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "e1");
    }

    #[tokio::test]
    async fn test_fts5_procedural_rules_search() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool.clone());

        repo.upsert(&test_rule("r1", "When user mentions deadlines, check task priorities first")).await.unwrap();
        repo.upsert(&test_rule("r2", "Always confirm before deleting any data")).await.unwrap();

        let results = search_procedural_rules(&pool, "deadline priorities", None, 10).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "r1");
    }

    #[tokio::test]
    async fn test_fts5_excludes_inactive_rules() {
        let pool = setup().await;
        let repo = ProceduralRuleRepo::new(pool.clone());

        let mut rule = test_rule("r1", "Some old inactive rule about deadlines");
        rule.active = false;
        repo.upsert(&rule).await.unwrap();

        let results = search_procedural_rules(&pool, "deadlines", None, 10).await.unwrap();
        assert!(results.is_empty(), "Inactive rules should be excluded");
    }

    #[tokio::test]
    async fn test_bm25_search_all_merges_tables() {
        let pool = setup().await;
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let episode_repo = EpisodicMemoryRepo::new(pool.clone());
        let rule_repo = ProceduralRuleRepo::new(pool.clone());

        fact_repo.upsert(&test_fact("f1", "morning_routine", "coffee then code")).await.unwrap();
        episode_repo.upsert(&test_memory("e1", "Great morning coding with coffee")).await.unwrap();
        rule_repo.upsert(&test_rule("r1", "Morning coding sessions are most productive")).await.unwrap();

        let results = bm25_search_all(&pool, "morning coding", None, 10).await.unwrap();
        assert!(results.len() >= 2, "Should find results across multiple tables");

        let tables: Vec<&str> = results.iter().map(|r| r.source_table).collect();
        // Should have results from at least 2 different tables
        let unique_tables: std::collections::HashSet<&str> = tables.into_iter().collect();
        assert!(unique_tables.len() >= 2, "Results should come from multiple tables");
    }

    #[tokio::test]
    async fn test_bm25_search_respects_limit() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        for i in 0..10 {
            repo.upsert(&test_fact(&format!("f{i}"), "coding", &format!("project {i}"))).await.unwrap();
        }

        let results = bm25_search_all(&pool, "coding project", None, 3).await.unwrap();
        assert!(results.len() <= 3);
    }

    #[tokio::test]
    async fn test_bm25_empty_query_returns_empty() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());
        repo.upsert(&test_fact("f1", "peak_hours", "morning")).await.unwrap();

        // FTS5 MATCH with empty string should error or return empty
        let results = search_semantic_facts(&pool, "", None, 10).await;
        // Either returns empty or errors — both acceptable
        assert!(results.is_err() || results.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_fts5_porter_stemming() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        repo.upsert(&test_fact("f1", "running", "daily morning runs")).await.unwrap();

        // "run" should match "running" and "runs" via porter stemmer
        let results = search_semantic_facts(&pool, "run", None, 10).await.unwrap();
        assert!(!results.is_empty(), "Porter stemmer should match 'run' to 'running'/'runs'");
    }
}
```

- [ ] **Step 4: Run tests to verify they compile and exercise FTS5**

Run: `cargo nextest run -p cognitive -E 'test(fts5) + test(bm25)'`
Expected: All new tests PASS (FTS5 tables created by migration, triggers keep them in sync)

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/search/ crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): add BM25 search module with FTS5 queries and tests"
```

---

### Task 3: Add `search_fts()` to Cognitive Repos

**Files:**
- Modify: `crates/cognitive/src/repos/semantic_fact.rs`
- Modify: `crates/cognitive/src/repos/episodic_memory.rs`
- Modify: `crates/cognitive/src/repos/procedural_rule.rs`

- [ ] **Step 1: Write failing test for `SemanticFactRepo::search_fts()`**

Add to `crates/cognitive/src/repos/semantic_fact.rs` tests module:

```rust
#[tokio::test]
async fn test_search_fts_basic() {
    let pool = setup().await;
    let repo = SemanticFactRepo::new(pool.clone());

    let f1 = test_fact("f1", "productivity", "peak_hours", "morning routine at 9am");
    let f2 = test_fact("f2", "productivity", "break_pattern", "every 90 minutes");
    repo.upsert(&f1).await.unwrap();
    repo.upsert(&f2).await.unwrap();

    let results = repo.search_fts("morning routine", None, 10).await.unwrap();
    assert!(!results.is_empty());
    assert_eq!(results[0].id, "f1");
}

#[tokio::test]
async fn test_search_fts_with_domain_filter() {
    let pool = setup().await;
    let repo = SemanticFactRepo::new(pool.clone());

    let f1 = test_fact("f1", "productivity", "peak_hours", "morning");
    let f2 = test_fact("f2", "finance", "budget", "morning expenses");
    repo.upsert(&f1).await.unwrap();
    repo.upsert(&f2).await.unwrap();

    let results = repo.search_fts("morning", Some("productivity"), 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "f1");
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(search_fts)'`
Expected: FAIL — `search_fts` method doesn't exist yet

- [ ] **Step 3: Implement `search_fts()` on SemanticFactRepo**

Add to `crates/cognitive/src/repos/semantic_fact.rs`:

```rust
/// Full-text search via FTS5 with BM25 ranking.
pub async fn search_fts(
    &self,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<SemanticFact>, sqlx::Error> {
    let sql = r#"
        SELECT f.* FROM semantic_facts f
        INNER JOIN semantic_facts_fts fts ON f.id = fts.id
        WHERE semantic_facts_fts MATCH ?1
        AND (?2 IS NULL OR f.domain = ?2)
        AND f.superseded_at IS NULL
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as::<_, SemanticFact>(sql)
        .bind(query)
        .bind(domain)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
}
```

- [ ] **Step 4: Implement `search_fts()` on EpisodicMemoryRepo**

Add to `crates/cognitive/src/repos/episodic_memory.rs`:

```rust
/// Full-text search via FTS5.
pub async fn search_fts(
    &self,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<EpisodicMemory>, sqlx::Error> {
    let sql = r#"
        SELECT e.* FROM episodic_memories e
        INNER JOIN episodic_memories_fts fts ON e.id = fts.id
        WHERE episodic_memories_fts MATCH ?1
        AND (?2 IS NULL OR e.domain = ?2)
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as::<_, EpisodicMemory>(sql)
        .bind(query)
        .bind(domain)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
}
```

- [ ] **Step 5: Implement `search_fts()` on ProceduralRuleRepo**

Add to `crates/cognitive/src/repos/procedural_rule.rs`:

```rust
/// Full-text search via FTS5.
pub async fn search_fts(
    &self,
    query: &str,
    domain: Option<&str>,
    limit: usize,
) -> Result<Vec<ProceduralRule>, sqlx::Error> {
    let sql = r#"
        SELECT r.* FROM procedural_rules r
        INNER JOIN procedural_rules_fts fts ON r.id = fts.id
        WHERE procedural_rules_fts MATCH ?1
        AND (?2 IS NULL OR r.domain = ?2)
        AND r.active = 1
        ORDER BY fts.rank
        LIMIT ?3
    "#;
    sqlx::query_as::<_, ProceduralRule>(sql)
        .bind(query)
        .bind(domain)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo nextest run -p cognitive`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/repos/semantic_fact.rs crates/cognitive/src/repos/episodic_memory.rs crates/cognitive/src/repos/procedural_rule.rs
git commit -m "feat(cognitive): add search_fts() to all cognitive repos"
```

---

### Task 4: Triple RRF Merge

**Files:**
- Modify: `crates/tools-core/src/search.rs`

- [ ] **Step 1: Write failing test for `rrf_merge_triple()`**

Add to `crates/tools-core/src/search.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Clone, Debug)]
    struct TestItem { id: String }
    impl Searchable for TestItem {
        fn search_id(&self) -> &str { &self.id }
    }

    #[test]
    fn test_rrf_merge_triple_combines_three_signals() {
        let kw = vec![TestItem { id: "a".into() }, TestItem { id: "b".into() }];
        let sem = vec![("b".to_string(), 0.9), ("c".to_string(), 0.8)];
        let bm25 = vec![("a".to_string(), 5.0), ("c".to_string(), 3.0)];

        let mut lookup: HashMap<String, TestItem> = HashMap::new();
        lookup.insert("a".into(), TestItem { id: "a".into() });
        lookup.insert("b".into(), TestItem { id: "b".into() });
        lookup.insert("c".into(), TestItem { id: "c".into() });

        let results = rrf_merge_triple(&kw, &sem, &bm25, 60, &lookup);
        assert_eq!(results.len(), 3, "All three items should appear");

        // Items appearing in more signals should rank higher
        let ids: Vec<&str> = results.iter().map(|(item, _, _)| item.search_id()).collect();
        // "a" appears in kw + bm25, "b" in kw + sem, "c" in sem + bm25
        // All appear in 2 signals — order depends on rank positions
        assert!(ids.contains(&"a"));
        assert!(ids.contains(&"b"));
        assert!(ids.contains(&"c"));
    }

    #[test]
    fn test_rrf_merge_triple_item_in_all_three_ranks_highest() {
        let kw = vec![TestItem { id: "x".into() }, TestItem { id: "y".into() }];
        let sem = vec![("x".to_string(), 0.9), ("z".to_string(), 0.5)];
        let bm25 = vec![("x".to_string(), 5.0), ("w".to_string(), 2.0)];

        let mut lookup: HashMap<String, TestItem> = HashMap::new();
        for id in ["x", "y", "z", "w"] {
            lookup.insert(id.into(), TestItem { id: id.into() });
        }

        let results = rrf_merge_triple(&kw, &sem, &bm25, 60, &lookup);
        // "x" appears in all 3 signals — should be rank 1
        assert_eq!(results[0].0.search_id(), "x");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo nextest run -p tools-core -E 'test(rrf_merge_triple)'`
Expected: FAIL — function doesn't exist

- [ ] **Step 3: Implement `rrf_merge_triple()`**

Add to `crates/tools-core/src/search.rs`:

```rust
/// Triple-source Reciprocal Rank Fusion: keyword + semantic + BM25.
///
/// Like `rrf_merge` but with a third BM25 signal. BM25 results are
/// `(id, score)` pairs where score is the negated FTS5 rank.
pub fn rrf_merge_triple<T: Searchable + Clone>(
    keyword_results: &[T],
    semantic_results: &[(String, f64)],
    bm25_results: &[(String, f64)],
    k: u32,
    items_by_id: &HashMap<String, T>,
) -> Vec<(T, f64, &'static str)> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut sources: HashMap<String, u8> = HashMap::new(); // bitmask: 1=keyword, 2=semantic, 4=bm25

    for (rank, result) in keyword_results.iter().enumerate() {
        let id = result.search_id().to_string();
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
        *sources.entry(id).or_insert(0) |= 1;
    }

    for (rank, (id, _sim)) in semantic_results.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
        *sources.entry(id.clone()).or_insert(0) |= 2;
    }

    for (rank, (id, _score)) in bm25_results.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
        *sources.entry(id.clone()).or_insert(0) |= 4;
    }

    let mut ranked: Vec<_> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    ranked
        .into_iter()
        .filter_map(|(id, score)| {
            let item = keyword_results
                .iter()
                .find(|r| r.search_id() == id)
                .cloned()
                .or_else(|| items_by_id.get(&id).cloned());

            let source_bits = sources.get(&id).copied().unwrap_or(0);
            let source = match source_bits {
                7 => "all",
                6 => "semantic+bm25",
                5 => "keyword+bm25",
                4 => "bm25",
                3 => "both",     // keyword+semantic (backward compat)
                2 => "semantic",
                1 => "keyword",
                _ => "unknown",
            };

            item.map(|i| (i, score, source))
        })
        .collect()
}
```

- [ ] **Step 4: Update re-export in lib.rs**

In `crates/tools-core/src/lib.rs`, update the existing re-export line from:
```rust
pub use search::{rrf_merge, Searchable};
```
To:
```rust
pub use search::{rrf_merge, rrf_merge_triple, Searchable};
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p tools-core`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/tools-core/src/search.rs crates/tools-core/src/lib.rs
git commit -m "feat(tools-core): add rrf_merge_triple for 3-signal reciprocal rank fusion"
```

---

### Task 5: Integrate BM25 into Retrieval Pipeline

**Files:**
- Modify: `crates/cognitive/src/retrieval.rs`

This task integrates `search_fts()` as a third signal in `retrieve_relevant_facts()`. The existing function uses vector + fallback. We add BM25 results as an additional ranked list that feeds into scoring.

- [ ] **Step 1: Write test for BM25 integration in retrieval**

Add to `crates/cognitive/src/retrieval.rs` tests:

```rust
#[tokio::test]
async fn test_retrieval_uses_bm25_when_query_non_empty() {
    let pool = setup().await;
    let repo = SemanticFactRepo::new(pool);

    // Insert facts with distinctive text for BM25 matching.
    // Use space-separated words (not underscores) so FTS5 porter tokenizer
    // can match individual terms.
    let mut f1 = test_fact("f1", "morning routine", 1.0, 0);
    f1.object = "wakes up early for deep work".to_string();
    repo.upsert(&f1).await.unwrap();
    let mut f2 = test_fact("f2", "afternoon break", 1.0, 0);
    f2.object = "take a walk after lunch".to_string();
    repo.upsert(&f2).await.unwrap();

    // Without vector search, BM25 should still surface relevant facts
    let results = retrieve_relevant_facts(
        &repo,
        None,  // no embedder
        "morning routine",
        &["productivity"],
        &default_params(10),
    ).await.unwrap();

    assert!(!results.is_empty());
    // f1 should rank higher because "morning routine" matches query
    assert_eq!(results[0].fact.id, "f1");
}
```

- [ ] **Step 2: Run to see baseline behavior**

Run: `cargo nextest run -p cognitive -E 'test(retrieval_uses_bm25)'`
Expected: PASS or FAIL depending on fallback behavior — this establishes baseline

- [ ] **Step 3: Add BM25 signal to `retrieve_relevant_facts()`**

Modify `crates/cognitive/src/retrieval.rs` to query FTS5 when query is non-empty and boost scores of matching facts:

```rust
// At the top of retrieve_relevant_facts(), after the vector/fallback path:
// Add BM25 boost for non-empty queries
if !query.is_empty() {
    if let Ok(bm25_hits) = repo.search_fts(query, None, params.limit * 2).await {
        let bm25_ids: std::collections::HashMap<String, usize> = bm25_hits
            .iter()
            .enumerate()
            .map(|(rank, f)| (f.id.clone(), rank))
            .collect();

        for result in &mut scored {
            if let Some(&rank) = bm25_ids.get(&result.fact.id) {
                // BM25 boost: add RRF-style score contribution
                let bm25_boost = 1.0 / (60.0 + rank as f64 + 1.0);
                result.score += bm25_boost;
            }
        }
    }
}
```

- [ ] **Step 4: Run all retrieval tests**

Run: `cargo nextest run -p cognitive -E 'test(retriev)'`
Expected: All PASS

- [ ] **Step 5: Run full cognitive test suite**

Run: `cargo nextest run -p cognitive`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/retrieval.rs
git commit -m "feat(cognitive): integrate BM25 as third retrieval signal via FTS5"
```

---

## Chunk 2: Annotation System

### Task 6: Annotation Table Migration

**Files:**
- Create: `crates/cognitive/migrations/006_annotations.sql`

- [ ] **Step 1: Write the annotations migration**

```sql
-- crates/cognitive/migrations/006_annotations.sql

CREATE TABLE IF NOT EXISTS annotations (
    id TEXT PRIMARY KEY,
    target_type TEXT NOT NULL,
    target_id TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT DEFAULT '',
    author TEXT NOT NULL DEFAULT 'agent',
    priority INTEGER DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    expires_at TEXT,
    access_count INTEGER DEFAULT 0,
    UNIQUE(target_type, target_id, content)
);

CREATE INDEX IF NOT EXISTS idx_annotations_target ON annotations(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_annotations_tags ON annotations(tags);
CREATE INDEX IF NOT EXISTS idx_annotations_priority ON annotations(priority);

-- FTS5 for annotation search
CREATE VIRTUAL TABLE IF NOT EXISTS annotations_fts USING fts5(
    id UNINDEXED,
    target_type,
    target_id,
    content,
    tags,
    tokenize='porter unicode61'
);

CREATE TRIGGER IF NOT EXISTS annotations_ai AFTER INSERT ON annotations BEGIN
    INSERT INTO annotations_fts(rowid, id, target_type, target_id, content, tags)
    VALUES (new.rowid, new.id, new.target_type, new.target_id, new.content, new.tags);
END;

CREATE TRIGGER IF NOT EXISTS annotations_ad AFTER DELETE ON annotations BEGIN
    INSERT INTO annotations_fts(annotations_fts, rowid, id, target_type, target_id, content, tags)
    VALUES ('delete', old.rowid, old.id, old.target_type, old.target_id, old.content, old.tags);
END;

CREATE TRIGGER IF NOT EXISTS annotations_au AFTER UPDATE ON annotations BEGIN
    INSERT INTO annotations_fts(annotations_fts, rowid, id, target_type, target_id, content, tags)
    VALUES ('delete', old.rowid, old.id, old.target_type, old.target_id, old.content, old.tags);
    INSERT INTO annotations_fts(rowid, id, target_type, target_id, content, tags)
    VALUES (new.rowid, new.id, new.target_type, new.target_id, new.content, new.tags);
END;
```

- [ ] **Step 2: Verify migration applies**

Run: `cargo nextest run -p cognitive -E 'test(upsert_and_get)'`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/cognitive/migrations/006_annotations.sql
git commit -m "feat(cognitive): add annotations table with FTS5 search"
```

---

### Task 7: Annotation Type + Repo

**Files:**
- Modify: `crates/cognitive/src/types.rs`
- Create: `crates/cognitive/src/repos/annotation.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Add Annotation struct to types.rs**

Add to `crates/cognitive/src/types.rs`:

```rust
/// A persistent annotation attached to any entity (tool, fact, rule, skill, project).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Annotation {
    pub id: String,
    pub target_type: String,
    pub target_id: String,
    pub content: String,
    pub tags: String,
    pub author: String,
    pub priority: i32,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: Option<String>,
    pub access_count: i64,
}
```

- [ ] **Step 2: Write failing tests for AnnotationRepo**

```rust
// crates/cognitive/src/repos/annotation.rs

//! Repository for the `annotations` table.

use sqlx::SqlitePool;

use crate::types::Annotation;

#[derive(Debug, Clone)]
pub struct AnnotationRepo {
    pool: SqlitePool,
}

impl AnnotationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_annotation(id: &str, target_type: &str, target_id: &str, content: &str) -> Annotation {
        Annotation {
            id: id.into(),
            target_type: target_type.into(),
            target_id: target_id.into(),
            content: content.into(),
            tags: "".into(),
            author: "agent".into(),
            priority: 0,
            created_at: "2026-03-10T10:00:00Z".into(),
            updated_at: "2026-03-10T10:00:00Z".into(),
            expires_at: None,
            access_count: 0,
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get_for_target() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let ann = test_annotation("a1", "tool", "search", "Use BM25 for keyword queries");
        repo.upsert(&ann).await.unwrap();

        let results = repo.get_for_target("tool", "search").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Use BM25 for keyword queries");
    }

    #[tokio::test]
    async fn test_upsert_deduplicates() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let ann = test_annotation("a1", "tool", "search", "Same content");
        repo.upsert(&ann).await.unwrap();
        // Second upsert with same target_type + target_id + content should update, not duplicate
        repo.upsert(&ann).await.unwrap();

        let results = repo.get_for_target("tool", "search").await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_delete() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let ann = test_annotation("a1", "tool", "search", "Temp note");
        repo.upsert(&ann).await.unwrap();

        let deleted = repo.delete("a1").await.unwrap();
        assert!(deleted);

        let results = repo.get_for_target("tool", "search").await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let deleted = repo.delete("nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_search_fts() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        repo.upsert(&test_annotation("a1", "tool", "search", "BM25 ranking algorithm")).await.unwrap();
        repo.upsert(&test_annotation("a2", "api", "stripe", "Webhook requires raw body")).await.unwrap();

        let results = repo.search("BM25 ranking", 10).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "a1");
    }

    #[tokio::test]
    async fn test_list_all() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        repo.upsert(&test_annotation("a1", "tool", "search", "Note 1")).await.unwrap();
        repo.upsert(&test_annotation("a2", "api", "stripe", "Note 2")).await.unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_increment_access() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        repo.upsert(&test_annotation("a1", "tool", "search", "Note")).await.unwrap();
        repo.increment_access("a1").await.unwrap();
        repo.increment_access("a1").await.unwrap();

        let results = repo.get_for_target("tool", "search").await.unwrap();
        assert_eq!(results[0].access_count, 2);
    }

    #[tokio::test]
    async fn test_delete_expired() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let mut ann = test_annotation("a1", "tool", "search", "Expiring note");
        ann.expires_at = Some("2026-03-01T00:00:00Z".into()); // already expired
        repo.upsert(&ann).await.unwrap();

        let count = repo.delete_expired().await.unwrap();
        assert_eq!(count, 1);

        let all = repo.list_all().await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_get_by_priority() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let mut critical = test_annotation("a1", "tool", "search", "Critical: API change");
        critical.priority = 2;
        repo.upsert(&critical).await.unwrap();

        let mut normal = test_annotation("a2", "tool", "other", "Normal note");
        normal.priority = 0;
        repo.upsert(&normal).await.unwrap();

        let results = repo.get_by_min_priority(2).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a1");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(annotation)'`
Expected: FAIL — methods not implemented

- [ ] **Step 4: Implement AnnotationRepo methods**

Complete the `AnnotationRepo` implementation:

```rust
impl AnnotationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, annotation: &Annotation) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO annotations (id, target_type, target_id, content, tags, author,
                priority, created_at, updated_at, expires_at, access_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT (target_type, target_id, content) DO UPDATE SET
                tags = excluded.tags,
                author = excluded.author,
                priority = excluded.priority,
                updated_at = excluded.updated_at,
                expires_at = excluded.expires_at
            "#,
        )
        .bind(&annotation.id)
        .bind(&annotation.target_type)
        .bind(&annotation.target_id)
        .bind(&annotation.content)
        .bind(&annotation.tags)
        .bind(&annotation.author)
        .bind(annotation.priority)
        .bind(&annotation.created_at)
        .bind(&annotation.updated_at)
        .bind(&annotation.expires_at)
        .bind(annotation.access_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_for_target(
        &self,
        target_type: &str,
        target_id: &str,
    ) -> Result<Vec<Annotation>, sqlx::Error> {
        sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE target_type = ?1 AND target_id = ?2 ORDER BY priority DESC, updated_at DESC",
        )
        .bind(target_type)
        .bind(target_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<Annotation>, sqlx::Error> {
        let sql = r#"
            SELECT a.* FROM annotations a
            INNER JOIN annotations_fts fts ON a.id = fts.id
            WHERE annotations_fts MATCH ?1
            ORDER BY fts.rank
            LIMIT ?2
        "#;
        sqlx::query_as::<_, Annotation>(sql)
            .bind(query)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
    }

    pub async fn list_all(&self) -> Result<Vec<Annotation>, sqlx::Error> {
        sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations ORDER BY priority DESC, updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM annotations WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM annotations WHERE expires_at IS NOT NULL AND expires_at < datetime('now')",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn increment_access(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE annotations SET access_count = access_count + 1 WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_by_min_priority(&self, min_priority: i32) -> Result<Vec<Annotation>, sqlx::Error> {
        sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE priority >= ?1 ORDER BY priority DESC, updated_at DESC",
        )
        .bind(min_priority)
        .fetch_all(&self.pool)
        .await
    }
}
```

- [ ] **Step 5: Add module to repos/mod.rs**

Add to `crates/cognitive/src/repos/mod.rs`:
```rust
pub mod annotation;
pub use annotation::AnnotationRepo;
```

- [ ] **Step 6: Run all tests**

Run: `cargo nextest run -p cognitive`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add crates/cognitive/src/types.rs crates/cognitive/src/repos/annotation.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add Annotation type and AnnotationRepo with CRUD + FTS search"
```

---

### Task 8: Annotate Tool

**Files:**
- Create: `crates/tools/src/annotate.rs`
- Modify: `crates/tools/src/lib.rs`

This is a **user contribution point**. The tool has multiple valid approaches for action routing.

- [ ] **Step 1: Write the annotate tool skeleton**

Create `crates/tools/src/annotate.rs` with the tool structure following existing multi-action tool patterns (see `crates/tools/src/filesystem.rs` for reference).

The tool should support actions: `create`, `get`, `list`, `delete`, `search`.

Use `#[tool_actions]` + `#[derive(ActionParams)]` pattern from tools-core-macros.

**Key design decisions for the user:**
- Should `create` auto-generate IDs or accept them? (Recommendation: auto-generate via uuid)
- Should `search` use FTS5 only or also filter by target_type? (Recommendation: support both)
- How should `get` work — by target (type+id) or by annotation id? (Recommendation: by target, since that's the common access pattern)

- [ ] **Step 2: Register the tool**

Add `pub mod annotate;` to `crates/tools/src/lib.rs` and register `AnnotateTool` in the tool builder/factory.

- [ ] **Step 3: Test tool registration**

Run: `cargo nextest run -p tools`
Expected: PASS — tool compiles and registers

- [ ] **Step 4: Commit**

```bash
git add crates/tools/src/annotate.rs crates/tools/src/lib.rs
git commit -m "feat(tools): add annotate tool for annotation CRUD"
```

---

### Task 9: Annotation Context Source

**Files:**
- Create: `crates/agent/src/context_sources/annotation.rs`
- Modify: `crates/agent/src/context_sources/mod.rs`

- [ ] **Step 1: Write the AnnotationContextSource**

```rust
// crates/agent/src/context_sources/annotation.rs

//! Injects active annotations into the system prompt.

use async_trait::async_trait;
use cognitive::repos::AnnotationRepo;
use context_engine::source::{ContextSource, SourceContext};

pub struct AnnotationContextSource {
    repo: AnnotationRepo,
}

impl AnnotationContextSource {
    pub fn new(repo: AnnotationRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ContextSource for AnnotationContextSource {
    fn name(&self) -> &str {
        "annotations"
    }

    /// Priority between RetrievedMemory (70) and CompressedHistory (30).
    fn priority(&self) -> u8 {
        50
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        // Get critical annotations (priority >= 2) unconditionally
        let critical = self.repo.get_by_min_priority(2).await.ok()?;

        if critical.is_empty() {
            return None;
        }

        let mut text = "[Active Annotations]\n".to_string();
        for ann in &critical {
            text.push_str(&format!(
                "- [{}] {}: {}\n",
                ann.target_type, ann.target_id, ann.content
            ));
            // Increment access count (fire and forget)
            let _ = self.repo.increment_access(&ann.id).await;
        }

        Some(text)
    }
}
```

- [ ] **Step 2: Register in context_sources/mod.rs**

Add `pub mod annotation;` and export `AnnotationContextSource`.

- [ ] **Step 3: Wire into agent runtime**

The `AnnotationContextSource` needs to be added to the context engine's source list in `app-core` initialization. This wiring depends on where `ContextEngine::with_sources()` is called.

- [ ] **Step 4: Run agent tests**

Run: `cargo nextest run -p agent`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/context_sources/annotation.rs crates/agent/src/context_sources/mod.rs
git commit -m "feat(agent): add AnnotationContextSource for system prompt injection"
```

---

### Task 10: Wire AnnotationRepo + AnnotationContextSource in app-core

**Files:**
- Modify: `crates/app-core/src/lib.rs` (or wherever `ContextEngine::with_sources()` is called)

- [ ] **Step 1: Read app-core initialization to find where ContextEngine sources are registered**

Read `crates/app-core/src/lib.rs` and search for `with_sources` or `ContextEngine` construction to find where context sources are assembled.

- [ ] **Step 2: Add AnnotationRepo initialization**

After the StoragePool or cognitive repos are initialized, create an `AnnotationRepo`:

```rust
let annotation_repo = cognitive::repos::AnnotationRepo::new(pool.inner().clone());
```

- [ ] **Step 3: Add AnnotationContextSource to context engine sources**

Where the context sources vector is built, add:

```rust
sources.push(Box::new(
    agent::context_sources::annotation::AnnotationContextSource::new(annotation_repo.clone())
));
```

- [ ] **Step 4: Wire AnnotationRepo to AnnotateTool**

Pass the `annotation_repo` to the `AnnotateTool` constructor when registering tools.

- [ ] **Step 5: Run workspace tests**

Run: `cargo nextest run --workspace`
Expected: All PASS

- [ ] **Step 6: Commit**

```bash
git add crates/app-core/src/lib.rs
git commit -m "feat(app-core): wire AnnotationRepo and AnnotationContextSource into initialization"
```

---

### Task 11: Final Integration + Verification

- [ ] **Step 1: Run full cognitive test suite**

Run: `cargo nextest run -p cognitive`
Expected: All PASS

- [ ] **Step 2: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All PASS

- [ ] **Step 3: Clippy check**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues

- [ ] **Step 5: Commit any fixes**

```bash
git commit -m "fix: address clippy warnings and formatting from Phase 1"
```

---

> **Deferred:** The spec (section 3.6) describes a self-improving loop where the agent auto-creates annotations after task completion via `AgentRuntime::process_message()`. This is deferred to a follow-up task after the core annotation infrastructure is validated. It requires LLM integration and behavioral heuristics that are better iterated on once manual annotation via the `annotate` tool is working.
