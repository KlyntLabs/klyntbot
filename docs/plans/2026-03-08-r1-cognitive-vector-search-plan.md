# R1: Connect Vector Search to Cognitive Retrieval — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the FSRS 5-factor relevance formula fully functional by connecting real vector search to cognitive memory retrieval, replacing the hardcoded `semantic_similarity = 0.5`.

**Architecture:** Define `SemanticFactEmbedder` trait in `cognitive` crate (L5), implement in `agent` crate using existing `EmbeddingEngine` + new LanceDB table. Wire into `CognitiveContextSource` for two-tier context injection (static identity + dynamic relevant facts). Embed facts on consolidation upsert, search by user message text.

**Tech Stack:** Rust, fastembed (384-dim MiniLM), LanceDB, SQLite, async-trait, tokio

**Important architectural note:** `build_system_prompt()` is called BEFORE intent analysis in the current pipeline (`agent_loop/mod.rs:529` → `runtime.rs:247`). The query for vector search uses `SourceContext.message` (raw user message). A follow-up task can restructure to use the condensed intent summary.

---

### Task 1: Add `SemanticFactEmbedder` trait

**Files:**
- Create: `crates/cognitive/src/embedder.rs`
- Modify: `crates/cognitive/src/lib.rs:4-25`

**Step 1: Create the trait file**

Create `crates/cognitive/src/embedder.rs`:

```rust
//! Trait for embedding semantic facts into vector storage.
//!
//! Defined in `cognitive` (L5), implemented in `agent` (L5) via
//! dependency inversion — same pattern as `EmbeddingHandler` and
//! `ConversationEmbeddingHandler`.

use async_trait::async_trait;

use crate::types::SemanticFact;

/// Embeds semantic facts into vector storage for similarity search.
///
/// Implementations handle:
/// - Generating 384-dim embeddings from SPO triple text
/// - Storing/removing vectors in LanceDB
/// - Searching by cosine similarity with domain pre-filtering
#[async_trait]
pub trait SemanticFactEmbedder: Send + Sync {
    /// Embed a semantic fact and store its vector in LanceDB.
    ///
    /// Text formula: `"{subject} {predicate} {object}"`.
    /// Called after every consolidation upsert (fire-and-forget).
    async fn embed_and_store_fact(&self, fact: &SemanticFact) -> common::Result<()>;

    /// Remove the embedding for a superseded/archived fact.
    async fn remove_embedding(&self, fact_id: &str) -> common::Result<()>;

    /// Search for facts similar to a query, pre-filtered by domains.
    ///
    /// Returns `(fact_id, cosine_similarity)` pairs sorted by similarity desc.
    /// Only results with similarity >= `min_similarity` are returned.
    async fn search_similar(
        &self,
        query: &str,
        domains: &[&str],
        top_k: usize,
        min_similarity: f64,
    ) -> common::Result<Vec<(String, f64)>>;

    /// Re-embed all provided facts (backfill/reindex).
    ///
    /// Returns the number of facts successfully embedded.
    async fn reindex_all(&self, facts: &[SemanticFact]) -> common::Result<usize>;

    /// Whether the embedding engine is loaded and available.
    fn is_available(&self) -> bool;
}
```

**Step 2: Export the module**

In `crates/cognitive/src/lib.rs`, add the module and re-export:

```rust
// Add after existing module declarations:
pub mod embedder;

// Add to re-exports:
pub use embedder::SemanticFactEmbedder;
```

**Step 3: Verify it compiles**

Run: `cargo check -p cognitive`
Expected: PASS (trait has no implementation yet, just a definition)

**Step 4: Commit**

```bash
git add crates/cognitive/src/embedder.rs crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): add SemanticFactEmbedder trait for vector search"
```

---

### Task 2: Add `get_batch` to `SemanticFactRepo`

**Files:**
- Modify: `crates/cognitive/src/repos/semantic_fact.rs:62-77`

**Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` in `crates/cognitive/src/repos/semantic_fact.rs`:

```rust
#[tokio::test]
async fn test_get_batch_returns_matching_facts() {
    let pool = super::cognitive_test_pool().await;
    let repo = SemanticFactRepo::new(pool);

    let f1 = test_fact("batch1", "pred1");
    let f2 = test_fact("batch2", "pred2");
    let f3 = test_fact("batch3", "pred3");
    repo.upsert(&f1).await.unwrap();
    repo.upsert(&f2).await.unwrap();
    repo.upsert(&f3).await.unwrap();

    let results = repo.get_batch(&["batch1", "batch3"]).await.unwrap();
    assert_eq!(results.len(), 2);
    let ids: Vec<&str> = results.iter().map(|f| f.id.as_str()).collect();
    assert!(ids.contains(&"batch1"));
    assert!(ids.contains(&"batch3"));
}

#[tokio::test]
async fn test_get_batch_empty_ids() {
    let pool = super::cognitive_test_pool().await;
    let repo = SemanticFactRepo::new(pool);
    let results = repo.get_batch(&[]).await.unwrap();
    assert!(results.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p cognitive -E 'test(get_batch)'`
Expected: FAIL — `get_batch` method does not exist

**Step 3: Implement `get_batch`**

Add to `SemanticFactRepo` impl block in `crates/cognitive/src/repos/semantic_fact.rs`, after the `get()` method (around line 67):

```rust
/// Fetch multiple facts by ID in a single query.
pub async fn get_batch(&self, ids: &[&str]) -> Result<Vec<SemanticFact>, sqlx::Error> {
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    // SQLite doesn't support array params — build IN clause with placeholders
    let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
    let sql = format!(
        "SELECT * FROM semantic_facts WHERE id IN ({})",
        placeholders.join(", ")
    );
    let mut query = sqlx::query_as::<_, SemanticFact>(&sql);
    for id in ids {
        query = query.bind(id);
    }
    query.fetch_all(&self.pool).await
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(get_batch)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/cognitive/src/repos/semantic_fact.rs
git commit -m "feat(cognitive): add get_batch() to SemanticFactRepo"
```

---

### Task 3: Add `cognitive_fact_embeddings` table to VectorStore

**Files:**
- Modify: `crates/storage/src/vector_store.rs`

**Step 1: Write the failing test**

Add to existing test module in `crates/storage/src/vector_store.rs`:

```rust
#[tokio::test]
async fn test_cognitive_table_creation() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap())
        .await
        .unwrap();
    // Table should exist after init
    let results = store
        .search_cognitive_facts(&[0.1; 384], &["identity"], 5, 0.0)
        .await
        .unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn test_cognitive_upsert_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap())
        .await
        .unwrap();

    // Insert a fact embedding
    store
        .upsert_cognitive_fact(
            "fact1",
            &[0.5; 384],
            "identity",
            "user name Jayden",
            0.9,
            1.0,
            0.85,
        )
        .await
        .unwrap();

    // Search with similar vector
    let results = store
        .search_cognitive_facts(&[0.5; 384], &["identity"], 5, 0.0)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "fact1");
    assert!(results[0].1 > 0.99); // Near-identical vector
}

#[tokio::test]
async fn test_cognitive_domain_filter() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap())
        .await
        .unwrap();

    store
        .upsert_cognitive_fact("f1", &[0.5; 384], "identity", "text1", 0.9, 1.0, 0.8)
        .await
        .unwrap();
    store
        .upsert_cognitive_fact("f2", &[0.5; 384], "finance", "text2", 0.8, 1.0, 0.7)
        .await
        .unwrap();

    // Search only identity domain
    let results = store
        .search_cognitive_facts(&[0.5; 384], &["identity"], 5, 0.0)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, "f1");
}

#[tokio::test]
async fn test_cognitive_delete() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap())
        .await
        .unwrap();

    store
        .upsert_cognitive_fact("f1", &[0.5; 384], "identity", "text", 0.9, 1.0, 0.8)
        .await
        .unwrap();
    store.delete("cognitive_fact_embeddings", "f1").await.unwrap();

    let results = store
        .search_cognitive_facts(&[0.5; 384], &["identity"], 5, 0.0)
        .await
        .unwrap();
    assert!(results.is_empty());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p storage -E 'test(cognitive)'`
Expected: FAIL — methods don't exist

**Step 3: Implement the table schema + methods**

In `crates/storage/src/vector_store.rs`:

Add the schema function alongside existing schemas (after `memory_note_schema()`):

```rust
fn cognitive_fact_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("domain", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("importance", DataType::Float32, false),
        Field::new("stability", DataType::Float32, false),
        Field::new("confidence", DataType::Float32, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}
```

Add table creation to `VectorStore::new()` (after existing `ensure_table` calls):

```rust
store
    .ensure_table("cognitive_fact_embeddings", cognitive_fact_schema())
    .await?;
```

Add the upsert method:

```rust
/// Upsert a cognitive fact embedding.
pub async fn upsert_cognitive_fact(
    &self,
    fact_id: &str,
    vector: &[f32],
    domain: &str,
    text: &str,
    importance: f32,
    stability: f32,
    confidence: f32,
) -> Result<(), StorageError> {
    let updated_at = chrono::Utc::now().to_rfc3339();
    self.upsert_embedding(
        "cognitive_fact_embeddings",
        fact_id,
        vector,
        &[
            ("domain", domain),
            ("text", text),
            ("importance", &importance.to_string()),
            ("stability", &stability.to_string()),
            ("confidence", &confidence.to_string()),
            ("updated_at", &updated_at),
        ],
    )
    .await
}
```

Add the search method:

```rust
/// Search cognitive fact embeddings by vector similarity with domain filtering.
///
/// Returns `(fact_id, similarity_score)` pairs sorted by similarity desc.
pub async fn search_cognitive_facts(
    &self,
    query_vector: &[f32],
    domains: &[&str],
    top_k: usize,
    min_similarity: f64,
) -> Result<Vec<(String, f64)>, StorageError> {
    let table = self
        .db
        .open_table("cognitive_fact_embeddings")
        .execute()
        .await
        .map_err(|e| StorageError::Vector(format!("Open cognitive table: {e}")))?;

    // Build domain filter: domain IN ('identity', 'energy', ...)
    let domain_filter = if domains.is_empty() {
        String::new()
    } else {
        let quoted: Vec<String> = domains.iter().map(|d| format!("'{d}'")).collect();
        format!("domain IN ({})", quoted.join(", "))
    };

    let mut query = table
        .vector_search(query_vector)
        .map_err(|e| StorageError::Vector(format!("Vector search setup: {e}")))?
        .limit(top_k);

    if !domain_filter.is_empty() {
        query = query
            .filter(domain_filter)
            .map_err(|e| StorageError::Vector(format!("Domain filter: {e}")))?;
    }

    let results = query
        .execute()
        .await
        .map_err(|e| StorageError::Vector(format!("Vector search: {e}")))?;

    let batches: Vec<_> = results
        .try_collect()
        .await
        .map_err(|e| StorageError::Vector(format!("Collect results: {e}")))?;

    let mut scored = Vec::new();
    for batch in &batches {
        let id_col = batch
            .column_by_name("id")
            .and_then(|c| c.as_any().downcast_ref::<StringArray>());
        let dist_col = batch
            .column_by_name("_distance")
            .and_then(|c| c.as_any().downcast_ref::<Float32Array>());

        if let (Some(ids), Some(dists)) = (id_col, dist_col) {
            for i in 0..batch.num_rows() {
                let similarity = 1.0 - dists.value(i) as f64;
                if similarity >= min_similarity {
                    scored.push((ids.value(i).to_string(), similarity));
                }
            }
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    Ok(scored)
}
```

**Note:** Check existing search methods in the file (e.g., `search_similar`, `search_conv_embeddings`) for the exact imports needed (`StringArray`, `Float32Array`, `StreamExt`/`TryStreamExt`). Mirror those imports.

**Step 4: Run tests**

Run: `cargo nextest run -p storage -E 'test(cognitive)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/storage/src/vector_store.rs
git commit -m "feat(storage): add cognitive_fact_embeddings table to VectorStore"
```

---

### Task 4: Update `ScoredFact` and `retrieve_relevant_facts`

**Files:**
- Modify: `crates/cognitive/src/retrieval.rs`

**Step 1: Write failing tests**

Replace the existing test module in `crates/cognitive/src/retrieval.rs` with updated tests. Keep all existing tests but adapt them to the new function signature, and add new tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::SemanticFactEmbedder;
    use std::sync::atomic::{AtomicBool, Ordering};

    async fn setup() -> sqlx::SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_fact(id: &str, predicate: &str, stability: f64, access_count: i64) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: "productivity".into(),
            subject: "user".into(),
            predicate: predicate.into(),
            object: "value".into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06T10:00:00".into(),
            superseded_at: None,
            superseded_by: None,
            stability,
            last_accessed: None,
            access_count,
        }
    }

    /// Mock embedder that returns configurable similarity scores.
    struct MockEmbedder {
        available: AtomicBool,
        similarities: std::sync::Mutex<Vec<(String, f64)>>,
    }

    impl MockEmbedder {
        fn new(similarities: Vec<(String, f64)>) -> Self {
            Self {
                available: AtomicBool::new(true),
                similarities: std::sync::Mutex::new(similarities),
            }
        }

        fn unavailable() -> Self {
            Self {
                available: AtomicBool::new(false),
                similarities: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait::async_trait]
    impl SemanticFactEmbedder for MockEmbedder {
        async fn embed_and_store_fact(&self, _fact: &SemanticFact) -> common::Result<()> {
            Ok(())
        }
        async fn remove_embedding(&self, _fact_id: &str) -> common::Result<()> {
            Ok(())
        }
        async fn search_similar(
            &self,
            _query: &str,
            _domains: &[&str],
            _top_k: usize,
            _min_similarity: f64,
        ) -> common::Result<Vec<(String, f64)>> {
            Ok(self.similarities.lock().unwrap().clone())
        }
        async fn reindex_all(&self, _facts: &[SemanticFact]) -> common::Result<usize> {
            Ok(0)
        }
        fn is_available(&self) -> bool {
            self.available.load(Ordering::Relaxed)
        }
    }

    #[tokio::test]
    async fn test_vector_path_uses_real_similarity() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0)).await.unwrap();
        repo.upsert(&test_fact("f2", "break_pattern", 1.0, 0)).await.unwrap();

        // f2 has higher similarity than f1
        let embedder = MockEmbedder::new(vec![
            ("f1".into(), 0.6),
            ("f2".into(), 0.9),
        ]);

        let results = retrieve_relevant_facts(
            &repo, Some(&embedder), "when do I take breaks", &["productivity"],
            10, 30, 0.5,
        ).await.unwrap();

        assert_eq!(results.len(), 2);
        // f2 should rank higher due to 0.9 similarity
        assert_eq!(results[0].fact.id, "f2");
        assert!(results[0].similarity.unwrap() > 0.8);
    }

    #[tokio::test]
    async fn test_fallback_when_embedder_is_none() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0)).await.unwrap();
        repo.upsert(&test_fact("f2", "break_pattern", 5.0, 10)).await.unwrap();

        let results = retrieve_relevant_facts(
            &repo, None, "anything", &["productivity"],
            10, 30, 0.5,
        ).await.unwrap();

        assert_eq!(results.len(), 2);
        // All should have similarity = None (fallback path)
        assert!(results.iter().all(|r| r.similarity.is_none()));
    }

    #[tokio::test]
    async fn test_fallback_when_query_is_empty() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0)).await.unwrap();

        let embedder = MockEmbedder::new(vec![]);

        let results = retrieve_relevant_facts(
            &repo, Some(&embedder), "", &["productivity"],
            10, 30, 0.5,
        ).await.unwrap();

        assert!(!results.is_empty());
        assert!(results[0].similarity.is_none());
    }

    #[tokio::test]
    async fn test_fallback_when_few_vector_results() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        // Insert 5 facts
        for i in 0..5 {
            repo.upsert(&test_fact(&format!("f{i}"), &format!("pred{i}"), 1.0, 0))
                .await.unwrap();
        }

        // Vector search returns only 2 results (below threshold of 3)
        let embedder = MockEmbedder::new(vec![
            ("f0".into(), 0.9),
            ("f1".into(), 0.8),
        ]);

        let results = retrieve_relevant_facts(
            &repo, Some(&embedder), "query", &["productivity"],
            10, 30, 0.5,
        ).await.unwrap();

        // Should have more than 2 results (fallback merged in)
        assert!(results.len() > 2);
    }

    #[tokio::test]
    async fn test_scored_fact_records_access() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0)).await.unwrap();

        retrieve_relevant_facts(
            &repo, None, "", &["productivity"],
            10, 30, 0.5,
        ).await.unwrap();

        let updated = repo.get("f1").await.unwrap().unwrap();
        assert_eq!(updated.access_count, 1);
        assert!(updated.stability > 1.0);
    }

    #[tokio::test]
    async fn test_respects_limit() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        for i in 0..10 {
            repo.upsert(&test_fact(&format!("f{i}"), &format!("pred{i}"), 1.0, 0))
                .await.unwrap();
        }

        let results = retrieve_relevant_facts(
            &repo, None, "", &["productivity"],
            3, 30, 0.5,
        ).await.unwrap();
        assert_eq!(results.len(), 3);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p cognitive -E 'test(retrieve)'`
Expected: FAIL — `retrieve_relevant_facts` and `ScoredFact` don't exist

**Step 3: Implement `ScoredFact` and `retrieve_relevant_facts`**

Rewrite `crates/cognitive/src/retrieval.rs`:

```rust
//! Memory retrieval with FSRS-scored relevance and optional vector search.
//!
//! `retrieve_relevant_facts` searches semantic facts using vector similarity
//! (when available) combined with FSRS retrievability, importance, and access
//! frequency. Falls back to importance × stability ranking when vector search
//! is unavailable or returns too few results.

use chrono::Utc;
use tracing::{debug, warn};

use crate::decay::{relevance_score, retrievability, update_stability};
use crate::embedder::SemanticFactEmbedder;
use crate::repos::SemanticFactRepo;
use crate::types::SemanticFact;

/// Minimum vector results before fallback kicks in.
const MIN_VECTOR_RESULTS: usize = 3;

/// A scored retrieval result with optional similarity from vector search.
#[derive(Debug, Clone)]
pub struct ScoredFact {
    pub fact: SemanticFact,
    pub score: f64,
    /// Cosine similarity from vector search. `None` if fallback path was used.
    pub similarity: Option<f64>,
}

/// Retrieve and rank facts using FSRS scoring with optional vector search.
///
/// **Vector path** (embedder available + non-empty query + ≥3 vector results):
///   1. `search_similar(query, domains, top_k)` → candidate fact_ids with similarity
///   2. Batch-load facts from SQL
///   3. Score with real `semantic_similarity` in the relevance formula
///
/// **Fallback path** (embedder unavailable, empty query, or <3 vector results):
///   1. Load all active facts across requested domains from SQL
///   2. Score with `semantic_similarity = 0.5` (neutral)
///   3. Secondary sort by `importance × stability`
///
/// Both paths record access events on retrieved facts (increases FSRS stability).
pub async fn retrieve_relevant_facts(
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
    query: &str,
    domains: &[&str],
    limit: usize,
    vector_top_k: usize,
    situational_boost: f64,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    let use_vector = !query.is_empty()
        && embedder
            .map(|e| e.is_available())
            .unwrap_or(false);

    let mut scored = if use_vector {
        let embedder = embedder.unwrap(); // safe: checked above
        match embedder.search_similar(query, domains, vector_top_k, 0.55).await {
            Ok(hits) if hits.len() >= MIN_VECTOR_RESULTS => {
                vector_path(repo, &hits, situational_boost).await?
            }
            Ok(hits) => {
                // Too few vector results — merge with fallback
                let mut vector_scored = vector_path(repo, &hits, situational_boost).await?;
                let vector_ids: std::collections::HashSet<String> =
                    vector_scored.iter().map(|s| s.fact.id.clone()).collect();
                let mut fallback = fallback_path(repo, domains, situational_boost).await?;
                fallback.retain(|s| !vector_ids.contains(&s.fact.id));
                vector_scored.append(&mut fallback);
                vector_scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
                vector_scored
            }
            Err(e) => {
                warn!("Vector search failed, using fallback: {e}");
                fallback_path(repo, domains, situational_boost).await?
            }
        }
    } else {
        fallback_path(repo, domains, situational_boost).await?
    };

    scored.truncate(limit);

    // Record access on retrieved facts (increases FSRS stability)
    for result in &scored {
        let new_stability = update_stability(result.fact.stability, true);
        if let Err(e) = repo.record_access(&result.fact.id, new_stability).await {
            warn!("Failed to record access for fact '{}': {e}", result.fact.id);
        }
    }

    debug!(
        "Retrieved {} facts across {} domains (top score: {:.3}, vector: {})",
        scored.len(),
        domains.len(),
        scored.first().map(|r| r.score).unwrap_or(0.0),
        use_vector,
    );

    Ok(scored)
}

/// Score facts using real vector similarity.
async fn vector_path(
    repo: &SemanticFactRepo,
    hits: &[(String, f64)],
    situational_boost: f64,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    if hits.is_empty() {
        return Ok(Vec::new());
    }

    let ids: Vec<&str> = hits.iter().map(|(id, _)| id.as_str()).collect();
    let sim_map: std::collections::HashMap<&str, f64> =
        hits.iter().map(|(id, sim)| (id.as_str(), *sim)).collect();

    let facts = repo.get_batch(&ids).await?;
    let now = Utc::now();

    Ok(facts
        .into_iter()
        .map(|fact| {
            let similarity = sim_map.get(fact.id.as_str()).copied().unwrap_or(0.5);
            let (r, freq) = compute_decay_and_freq(&fact, &now);
            let score = relevance_score(similarity, r, fact.confidence, freq, situational_boost);
            ScoredFact {
                fact,
                score,
                similarity: Some(similarity),
            }
        })
        .collect())
}

/// Score facts with hardcoded similarity (fallback when vector search unavailable).
async fn fallback_path(
    repo: &SemanticFactRepo,
    domains: &[&str],
    situational_boost: f64,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    let mut all_facts = Vec::new();
    for domain in domains {
        let mut facts = repo.list_active(domain).await?;
        all_facts.append(&mut facts);
    }

    let now = Utc::now();

    let mut scored: Vec<ScoredFact> = all_facts
        .into_iter()
        .map(|fact| {
            let (r, freq) = compute_decay_and_freq(&fact, &now);
            let score = relevance_score(0.5, r, fact.confidence, freq, situational_boost);
            ScoredFact {
                fact,
                score,
                similarity: None,
            }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(scored)
}

/// Compute FSRS retrievability and normalized access frequency for a fact.
fn compute_decay_and_freq(fact: &SemanticFact, now: &chrono::DateTime<Utc>) -> (f64, f64) {
    let elapsed_days = fact
        .last_accessed
        .as_ref()
        .and_then(|la| la.parse::<chrono::NaiveDateTime>().ok())
        .map(|la| (now.naive_utc() - la).num_seconds() as f64 / 86400.0)
        .unwrap_or_else(|| {
            fact.recorded_at
                .parse::<chrono::NaiveDateTime>()
                .ok()
                .map(|ra| (now.naive_utc() - ra).num_seconds() as f64 / 86400.0)
                .unwrap_or(30.0)
        });

    let r = retrievability(elapsed_days, fact.stability);
    let freq = 1.0 - (1.0 / (1.0 + fact.access_count as f64));
    (r, freq)
}

// Keep the multi-domain convenience function, updated to new signature
/// Retrieve facts across all domains, ranked by FSRS score.
pub async fn retrieve_all_domains(
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
    query: &str,
    domains: &[&str],
    limit_per_domain: usize,
    situational_boost: f64,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    let mut all = retrieve_relevant_facts(
        repo, embedder, query, domains,
        limit_per_domain * domains.len(), 30, situational_boost,
    ).await?;
    all.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(all)
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(retrieve)' -E 'test(scored)' -E 'test(vector)' -E 'test(fallback)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/cognitive/src/retrieval.rs
git commit -m "feat(cognitive): implement retrieve_relevant_facts with vector + fallback paths"
```

---

### Task 5: Add `CognitiveConfig` fields for vector retrieval

**Files:**
- Modify: `crates/config/src/schema/cognitive.rs`

**Step 1: Add new config fields**

In `crates/config/src/schema/cognitive.rs`, add to the `CognitiveConfig` struct:

```rust
/// Enable dynamic fact retrieval using vector search (default: true).
#[serde(default = "default_true")]
pub dynamic_facts_enabled: bool,

/// Max static facts (identity baseline) per prompt (default: 10).
#[serde(default = "default_static_fact_limit")]
pub static_fact_limit: usize,

/// Max dynamic facts (query-relevant) per prompt (default: 15).
#[serde(default = "default_dynamic_fact_limit")]
pub dynamic_fact_limit: usize,

/// Number of candidate facts to fetch from vector search before FSRS re-ranking (default: 30).
#[serde(default = "default_vector_top_k")]
pub vector_top_k: usize,

/// Minimum cosine similarity threshold for vector search results (default: 0.55).
#[serde(default = "default_min_similarity")]
pub min_similarity: f64,
```

Add the default functions:

```rust
fn default_true() -> bool { true }
fn default_static_fact_limit() -> usize { 10 }
fn default_dynamic_fact_limit() -> usize { 15 }
fn default_vector_top_k() -> usize { 30 }
fn default_min_similarity() -> f64 { 0.55 }
```

**Note:** The `Default` impl needs to be updated from `#[derive(Default)]` to a manual impl that uses these defaults. Check if the struct already uses `#[derive(Default)]` — if so, replace with manual impl.

**Step 2: Verify compilation**

Run: `cargo check -p config`
Expected: PASS

**Step 3: Commit**

```bash
git add crates/config/src/schema/cognitive.rs
git commit -m "feat(config): add cognitive vector retrieval config fields"
```

---

### Task 6: Update `CognitiveContextSource` for two-tier injection

**Files:**
- Modify: `crates/cognitive/src/context_source.rs`
- Modify: `crates/context_engine/src/source.rs` (add `intent_summary`)

**Step 1: Add `intent_summary` to `SourceContext`**

In `crates/context_engine/src/source.rs`, add field to `SourceContext`:

```rust
pub struct SourceContext {
    pub channel: String,
    pub chat_id: String,
    pub message: Option<String>,
    /// Condensed intent summary for relevance-filtered sources.
    /// Falls back to `message` if not set.
    pub intent_summary: Option<String>,
}
```

Update all places that construct `SourceContext` to include `intent_summary: None` (the assembler's `build_system_prompt` at line 228-232).

**Step 2: Write failing tests for two-tier injection**

Add to test module in `crates/cognitive/src/context_source.rs`:

```rust
#[tokio::test]
async fn test_dynamic_tier_with_message() {
    let pool = setup().await;
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let rule_repo = ProceduralRuleRepo::new(pool);

    // Insert facts across domains
    fact_repo.upsert(&test_fact("identity", "name", "Jayden")).await.unwrap();
    fact_repo.upsert(&test_fact("energy", "peak_hours", "10am-12pm")).await.unwrap();
    fact_repo.upsert(&test_fact("preferences", "editor", "neovim")).await.unwrap();

    let embedder = Arc::new(MockEmbedder::new(vec![
        // Simulate: peak_hours is most similar to the query
        // We need to know the fact IDs — they're UUIDs from test_fact()
        // So instead, just verify the section headers appear
    ]));

    let source = CognitiveContextSource::new(fact_repo, rule_repo)
        .with_embedder(embedder);

    let ctx = SourceContext {
        channel: "test".into(),
        chat_id: "c1".into(),
        message: Some("what are my peak hours".into()),
        intent_summary: None,
    };

    let result = source.provide(&ctx).await.unwrap();
    assert!(result.contains("User Understanding"));
}

#[tokio::test]
async fn test_static_tier_without_message() {
    let pool = setup().await;
    let fact_repo = SemanticFactRepo::new(pool.clone());
    let rule_repo = ProceduralRuleRepo::new(pool);

    fact_repo.upsert(&test_fact("identity", "name", "Jayden")).await.unwrap();

    let source = CognitiveContextSource::new(fact_repo, rule_repo);
    let ctx = SourceContext {
        channel: "test".into(),
        chat_id: "c1".into(),
        message: None,
        intent_summary: None,
    };

    let result = source.provide(&ctx).await.unwrap();
    assert!(result.contains("User Understanding"));
    assert!(result.contains("Jayden"));
    // Should NOT contain dynamic section
    assert!(!result.contains("Relevant Personal Context"));
}
```

**Step 3: Update `CognitiveContextSource`**

In `crates/cognitive/src/context_source.rs`:

Add embedder field and builder method:

```rust
pub struct CognitiveContextSource {
    fact_repo: SemanticFactRepo,
    rule_repo: ProceduralRuleRepo,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,
    cache: Mutex<Option<CachedModel>>,
    config: CognitiveRetrievalConfig,
}

/// Config subset for retrieval (avoid depending on full config crate).
#[derive(Debug, Clone)]
pub struct CognitiveRetrievalConfig {
    pub dynamic_facts_enabled: bool,
    pub static_fact_limit: usize,
    pub dynamic_fact_limit: usize,
    pub vector_top_k: usize,
    pub min_similarity: f64,
}

impl Default for CognitiveRetrievalConfig {
    fn default() -> Self {
        Self {
            dynamic_facts_enabled: true,
            static_fact_limit: 10,
            dynamic_fact_limit: 15,
            vector_top_k: 30,
            min_similarity: 0.55,
        }
    }
}

impl CognitiveContextSource {
    pub fn new(fact_repo: SemanticFactRepo, rule_repo: ProceduralRuleRepo) -> Self {
        Self {
            fact_repo,
            rule_repo,
            embedder: None,
            cache: Mutex::new(None),
            config: CognitiveRetrievalConfig::default(),
        }
    }

    pub fn with_embedder(mut self, embedder: Arc<dyn SemanticFactEmbedder>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    pub fn with_config(mut self, config: CognitiveRetrievalConfig) -> Self {
        self.config = config;
        self
    }
    // ... rest of impl
}
```

Update `provide()` to use two-tier injection:

```rust
async fn provide(&self, ctx: &SourceContext) -> Option<String> {
    let (model, rules_text) = self.get_cached_or_load().await;

    let mut sections = Vec::new();
    sections.push("# User Understanding".to_string());

    // ── Static tier: top facts by importance across all domains ──
    let domain_sections = [
        ("Identity", &model.identity),
        ("Energy & Rhythms", &model.energy),
        ("Work Patterns", &model.work),
        ("Finance", &model.finance),
        ("Learning", &model.learning),
        ("Preferences", &model.preferences),
    ];

    for (label, facts) in &domain_sections {
        if !facts.is_empty() {
            let mut domain_facts = facts.to_vec();
            domain_facts.sort_by(|a, b| {
                let a_score = a.confidence * a.stability;
                let b_score = b.confidence * b.stability;
                b_score.partial_cmp(&a_score).unwrap_or(std::cmp::Ordering::Equal)
            });
            domain_facts.truncate(self.config.static_fact_limit);
            let lines: Vec<String> = domain_facts
                .iter()
                .map(|f| format!("- {}: {} = {}", f.subject, f.predicate, f.object))
                .collect();
            sections.push(format!("## {label}\n{}", lines.join("\n")));
        }
    }

    if !rules_text.is_empty() {
        sections.push(format!("## Learned Patterns\n{rules_text}"));
    }

    // ── Dynamic tier: vector-searched relevant facts ──
    let query = ctx.intent_summary.as_deref()
        .or(ctx.message.as_deref())
        .unwrap_or("");

    if self.config.dynamic_facts_enabled && !query.is_empty() {
        use crate::repos::USER_MODEL_DOMAINS;
        use crate::retrieval::retrieve_relevant_facts;

        let results = retrieve_relevant_facts(
            &self.fact_repo,
            self.embedder.as_deref(),
            query,
            USER_MODEL_DOMAINS,
            self.config.dynamic_fact_limit,
            self.config.vector_top_k,
            0.0, // no situational boost in context source
        ).await;

        if let Ok(facts) = results {
            let relevant_lines: Vec<String> = facts
                .iter()
                .filter(|f| f.score > 0.3) // minimum relevance threshold
                .map(|f| {
                    if f.similarity.map(|s| s > 0.6).unwrap_or(false) {
                        format!(
                            "- {}: {} = {} (relevance: {:.2})",
                            f.fact.subject, f.fact.predicate, f.fact.object, f.score
                        )
                    } else {
                        format!("- {}: {} = {}", f.fact.subject, f.fact.predicate, f.fact.object)
                    }
                })
                .collect();

            if !relevant_lines.is_empty() {
                sections.push(format!(
                    "## Relevant Personal Context (for this conversation)\n{}",
                    relevant_lines.join("\n")
                ));
            }
        }
    }

    let output = sections.join("\n\n");

    if output.lines().count() <= 1 {
        None
    } else {
        Some(output)
    }
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(context_source)'`
Expected: PASS

**Step 5: Run full cognitive tests**

Run: `cargo nextest run -p cognitive`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/cognitive/src/context_source.rs crates/context_engine/src/source.rs
git commit -m "feat(cognitive): two-tier context injection with dynamic vector-searched facts"
```

---

### Task 7: Implement `SemanticFactEmbedderImpl` in agent crate

**Files:**
- Create: `crates/agent/src/cognitive_embedder.rs`
- Modify: `crates/agent/src/lib.rs` (add module)

**Step 1: Create the implementation**

Create `crates/agent/src/cognitive_embedder.rs`:

```rust
//! Production implementation of `SemanticFactEmbedder`.
//!
//! Wraps `EmbeddingEngine` (for vector generation) and `VectorStore`
//! (for LanceDB persistence). Constructed in `AgentLoopBuilder` and
//! injected into `CognitiveContextSource`.

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use cognitive::embedder::SemanticFactEmbedder;
use cognitive::types::SemanticFact;
use tools::EmbeddingEngine;

/// Production embedder for cognitive semantic facts.
pub struct SemanticFactEmbedderImpl {
    engine: Arc<EmbeddingEngine>,
    store: storage::VectorStore,
}

impl SemanticFactEmbedderImpl {
    pub fn new(engine: Arc<EmbeddingEngine>, store: storage::VectorStore) -> Self {
        Self { engine, store }
    }

    /// Compose the text to embed from an SPO triple.
    fn fact_text(fact: &SemanticFact) -> String {
        format!("{} {} {}", fact.subject, fact.predicate, fact.object)
    }
}

#[async_trait]
impl SemanticFactEmbedder for SemanticFactEmbedderImpl {
    async fn embed_and_store_fact(&self, fact: &SemanticFact) -> common::Result<()> {
        let text = Self::fact_text(fact);
        let engine = self.engine.clone();
        let embedding = engine.embed_async(text.clone()).await?;

        self.store
            .upsert_cognitive_fact(
                &fact.id,
                &embedding,
                &fact.domain,
                &text,
                fact.confidence as f32,
                fact.stability as f32,
                fact.confidence as f32,
            )
            .await
            .map_err(|e| common::KlyntbotError::Storage(e))?;

        debug!(fact_id = %fact.id, "Embedded cognitive fact");
        Ok(())
    }

    async fn remove_embedding(&self, fact_id: &str) -> common::Result<()> {
        self.store
            .delete("cognitive_fact_embeddings", fact_id)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e))?;
        debug!(fact_id = %fact_id, "Removed cognitive fact embedding");
        Ok(())
    }

    async fn search_similar(
        &self,
        query: &str,
        domains: &[&str],
        top_k: usize,
        min_similarity: f64,
    ) -> common::Result<Vec<(String, f64)>> {
        let engine = self.engine.clone();
        let query_embedding = engine.embed_async(query.to_string()).await?;

        let results = self
            .store
            .search_cognitive_facts(&query_embedding, domains, top_k, min_similarity)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e))?;

        Ok(results)
    }

    async fn reindex_all(&self, facts: &[SemanticFact]) -> common::Result<usize> {
        let mut count = 0;
        for fact in facts {
            match self.embed_and_store_fact(fact).await {
                Ok(()) => count += 1,
                Err(e) => warn!(fact_id = %fact.id, "Failed to reindex fact: {e}"),
            }
        }
        debug!("Reindexed {count}/{} cognitive facts", facts.len());
        Ok(count)
    }

    fn is_available(&self) -> bool {
        self.engine.is_available()
    }
}
```

**Step 2: Add module to agent lib**

In `crates/agent/src/lib.rs`, add:

```rust
pub mod cognitive_embedder;
```

**Step 3: Verify compilation**

Run: `cargo check -p agent`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/cognitive_embedder.rs crates/agent/src/lib.rs
git commit -m "feat(agent): implement SemanticFactEmbedderImpl"
```

---

### Task 8: Wire embedder into `BackgroundConsolidationService`

**Files:**
- Modify: `crates/cognitive/src/consolidation.rs`
- Modify: `crates/cognitive/src/background.rs`

**Step 1: Add embedder to `consolidate_fact` and `consolidate_batch`**

In `crates/cognitive/src/consolidation.rs`, update both functions to accept an optional embedder:

```rust
pub async fn consolidate_fact(
    candidate: &SemanticFact,
    repo: &SemanticFactRepo,
    handler: &dyn ConsolidationHandler,
    embedder: Option<&dyn SemanticFactEmbedder>,
) -> common::Result<MemoryOp> {
    // ... existing logic unchanged ...
    // After each repo.upsert(), add:
    // if let Some(emb) = embedder {
    //     if let Err(e) = emb.embed_and_store_fact(candidate).await {
    //         warn!("Failed to embed fact '{}': {e}", candidate.id);
    //     }
    // }
    // After supersede (DELETE), add:
    // if let Some(emb) = embedder {
    //     if let Err(e) = emb.remove_embedding(&id).await {
    //         warn!("Failed to remove embedding '{}': {e}", id);
    //     }
    // }
}

pub async fn consolidate_batch(
    candidates: &[SemanticFact],
    repo: &SemanticFactRepo,
    handler: &dyn ConsolidationHandler,
    embedder: Option<&dyn SemanticFactEmbedder>,
) -> Vec<MemoryOp> {
    let mut ops = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        match consolidate_fact(candidate, repo, handler, embedder).await {
            // ... same as before
        }
    }
    ops
}
```

**Step 2: Update `BackgroundConsolidationService::start()` to accept embedder**

In `crates/cognitive/src/background.rs`:

```rust
pub fn start(
    mut event_rx: broadcast::Receiver<DomainEvent>,
    extraction: Arc<dyn ExtractionHandler>,
    consolidation: Arc<dyn ConsolidationHandler>,
    repo: SemanticFactRepo,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,  // NEW
    cancel: CancellationToken,
    pipeline_tx: Option<tokio::sync::mpsc::UnboundedSender<PipelineEvent>>,
) -> Self {
```

Update all `consolidate_batch` calls inside `start()` to pass `embedder.as_deref()`:

```rust
let ops = consolidate_batch(
    &facts,
    &repo,
    consolidation.as_ref(),
    embedder.as_deref(),
).await;
```

**Step 3: Update `reflection.rs` to pass `None` for embedder**

In `crates/cognitive/src/reflection.rs`, update the `consolidate_batch` call:

```rust
consolidate_batch(&validated, fact_repo, consolidation, None).await;
```

(Reflection can optionally embed too, but for now passing `None` keeps it simple.)

**Step 4: Update all callers in tests and integration tests**

Search for all `consolidate_fact(` and `consolidate_batch(` calls and add the `None` or embedder parameter.

**Step 5: Verify compilation and tests**

Run: `cargo nextest run -p cognitive`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/cognitive/src/consolidation.rs crates/cognitive/src/background.rs crates/cognitive/src/reflection.rs
git commit -m "feat(cognitive): wire embedder into consolidation pipeline"
```

---

### Task 9: Wire everything in `AgentLoopBuilder`

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:230-289`

**Step 1: Update builder to create and inject embedder**

In `crates/agent/src/agent_loop/builder.rs`, around lines 230-289 where `CognitiveContextSource` and `BackgroundConsolidationService` are constructed:

```rust
// After cognitive migrations run, create embedder
let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());

// Create SemanticFactEmbedder if embedding engine is available
let cognitive_embedder: Option<Arc<dyn cognitive::SemanticFactEmbedder>> =
    if let Some(ref engine) = self.embedding_engine {
        if let Some(ref vector_store) = self.vector_store {
            Some(Arc::new(crate::cognitive_embedder::SemanticFactEmbedderImpl::new(
                Arc::clone(engine),
                vector_store.clone(),
            )))
        } else {
            None
        }
    } else {
        None
    };

// Context source with embedder
sources.push(Box::new(
    cognitive::CognitiveContextSource::new(fact_repo.clone(), rule_repo)
        .with_embedder_opt(cognitive_embedder.clone()),
));

// Background service with embedder
let bg_service = cognitive::background::BackgroundConsolidationService::start(
    event_rx,
    extraction,
    consolidation,
    fact_repo,
    cognitive_embedder,  // NEW parameter
    cancel.clone(),
    self.pipeline_tx.take(),
);
```

**Note:** Check what fields `self.embedding_engine` and `self.vector_store` are called in the builder — they may have different names. Look at how `EmbeddingEngineImpl` is constructed in the builder to follow the same pattern.

**Step 2: Verify compilation**

Run: `cargo check -p agent`
Expected: PASS

**Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): wire SemanticFactEmbedder into agent loop builder"
```

---

### Task 10: Update integration tests

**Files:**
- Modify: `tests/integration/cognitive.rs`

**Step 1: Update existing integration tests**

All calls to `retrieve_facts()` need to become `retrieve_relevant_facts()` with the new signature. All calls to `consolidate_batch()` and `consolidate_fact()` need the extra `None` embedder parameter.

Search for all references in `tests/integration/cognitive.rs`:

```rust
// Old:
let results = retrieve_facts(&repo, "productivity", 10, 0.5).await.unwrap();
// New:
let results = retrieve_relevant_facts(&repo, None, "", &["productivity"], 10, 30, 0.5).await.unwrap();

// Old:
consolidate_batch(&facts, &repo, &consolidation).await;
// New:
consolidate_batch(&facts, &repo, &consolidation, None).await;
```

Also update the import:

```rust
// Old:
use klyntbot::cognitive::retrieval::retrieve_facts;
// New:
use klyntbot::cognitive::retrieval::retrieve_relevant_facts;
```

**Step 2: Run integration tests**

Run: `cargo nextest run -E 'test(cognitive)'`
Expected: PASS

**Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: PASS

**Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (excluding pre-existing desktop exceptions)

**Step 5: Commit**

```bash
git add tests/integration/cognitive.rs
git commit -m "test: update cognitive integration tests for vector-enabled retrieval"
```

---

### Task 11: Update `SourceContext` construction sites

**Files:**
- Modify: `crates/context_engine/src/assembler.rs:228-232`
- Modify: any other files that construct `SourceContext`

**Step 1: Find all SourceContext construction sites**

Search for `SourceContext {` across the codebase. Update each to include `intent_summary: None`:

```rust
let ctx = SourceContext {
    channel: channel.to_string(),
    chat_id: chat_id.to_string(),
    message: message.map(|s| s.to_string()),
    intent_summary: None,
};
```

Also update test files that construct `SourceContext`.

**Step 2: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: PASS

**Step 3: Commit**

```bash
git add -A
git commit -m "refactor: add intent_summary field to all SourceContext sites"
```

---

### Task 12: Final verification

**Step 1: Full build**

Run: `cargo build --workspace`
Expected: PASS

**Step 2: Full test suite**

Run: `cargo nextest run --workspace`
Expected: PASS

**Step 3: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 4: Format check**

Run: `cargo fmt --all --check`
Expected: PASS

**Step 5: Final commit (if any formatting fixes needed)**

```bash
cargo fmt --all
git add -A
git commit -m "style: format after R1 cognitive vector search implementation"
```
