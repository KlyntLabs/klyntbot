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

/// Tuning knobs for `retrieve_relevant_facts`.
#[derive(Debug, Clone)]
pub struct RetrievalParams {
    pub limit: usize,
    pub vector_top_k: usize,
    pub min_similarity: f64,
    pub situational_boost: f64,
}

impl RetrievalParams {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            vector_top_k: 30,
            min_similarity: 0.55,
            situational_boost: 0.0,
        }
    }
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
    params: &RetrievalParams,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    let use_vector = !query.is_empty() && embedder.map(|e| e.is_available()).unwrap_or(false);

    let mut scored = if use_vector {
        let embedder = embedder.unwrap(); // safe: checked above
        match embedder
            .search_similar(query, domains, params.vector_top_k, params.min_similarity)
            .await
        {
            Ok(hits) if hits.len() >= MIN_VECTOR_RESULTS => {
                vector_path(repo, &hits, params.situational_boost).await?
            }
            Ok(hits) => {
                // Too few vector results — merge with fallback
                let mut vector_scored = vector_path(repo, &hits, params.situational_boost).await?;
                let vector_ids: std::collections::HashSet<String> =
                    vector_scored.iter().map(|s| s.fact.id.clone()).collect();
                let mut fallback = fallback_path(repo, domains, params.situational_boost).await?;
                fallback.retain(|s| !vector_ids.contains(&s.fact.id));
                vector_scored.append(&mut fallback);
                vector_scored
            }
            Err(e) => {
                warn!("Vector search failed, using fallback: {e}");
                fallback_path(repo, domains, params.situational_boost).await?
            }
        }
    } else {
        fallback_path(repo, domains, params.situational_boost).await?
    };

    // Sort by FSRS score regardless of path (vector re-ranking can change order)
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(params.limit);

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

/// Retrieve facts across all domains, ranked by FSRS score.
pub async fn retrieve_all_domains(
    repo: &SemanticFactRepo,
    embedder: Option<&dyn SemanticFactEmbedder>,
    query: &str,
    domains: &[&str],
    limit_per_domain: usize,
    situational_boost: f64,
) -> Result<Vec<ScoredFact>, sqlx::Error> {
    let params = RetrievalParams {
        limit: limit_per_domain * domains.len(),
        situational_boost,
        ..RetrievalParams::new(0)
    };
    // retrieve_relevant_facts already returns results sorted by score.
    retrieve_relevant_facts(repo, embedder, query, domains, &params).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedder::SemanticFactEmbedder;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn default_params(limit: usize) -> RetrievalParams {
        RetrievalParams {
            limit,
            situational_boost: 0.5,
            ..RetrievalParams::new(0)
        }
    }

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

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();
        repo.upsert(&test_fact("f2", "break_pattern", 1.0, 0))
            .await
            .unwrap();

        // f2 has higher similarity than f1
        let embedder = MockEmbedder::new(vec![("f1".into(), 0.6), ("f2".into(), 0.9)]);

        let results = retrieve_relevant_facts(
            &repo,
            Some(&embedder),
            "when do I take breaks",
            &["productivity"],
            &default_params(10),
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 2);
        // f2 should rank higher due to 0.9 similarity
        assert_eq!(results[0].fact.id, "f2");
        assert!(results[0].similarity.unwrap() > 0.8);
    }

    #[tokio::test]
    async fn test_fallback_when_embedder_is_none() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();
        repo.upsert(&test_fact("f2", "break_pattern", 5.0, 10))
            .await
            .unwrap();

        let results = retrieve_relevant_facts(
            &repo,
            None,
            "anything",
            &["productivity"],
            &default_params(10),
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 2);
        // All should have similarity = None (fallback path)
        assert!(results.iter().all(|r| r.similarity.is_none()));
    }

    #[tokio::test]
    async fn test_fallback_when_query_is_empty() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();

        let embedder = MockEmbedder::new(vec![]);

        let results = retrieve_relevant_facts(
            &repo,
            Some(&embedder),
            "",
            &["productivity"],
            &default_params(10),
        )
        .await
        .unwrap();

        assert!(!results.is_empty());
        assert!(results[0].similarity.is_none());
    }

    #[tokio::test]
    async fn test_fallback_when_embedder_unavailable() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();

        let embedder = MockEmbedder::unavailable();

        let results = retrieve_relevant_facts(
            &repo,
            Some(&embedder),
            "query",
            &["productivity"],
            &default_params(10),
        )
        .await
        .unwrap();

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
                .await
                .unwrap();
        }

        // Vector search returns only 2 results (below threshold of 3)
        let embedder = MockEmbedder::new(vec![("f0".into(), 0.9), ("f1".into(), 0.8)]);

        let results = retrieve_relevant_facts(
            &repo,
            Some(&embedder),
            "query",
            &["productivity"],
            &default_params(10),
        )
        .await
        .unwrap();

        // Should have more than 2 results (fallback merged in)
        assert!(results.len() > 2);
    }

    #[tokio::test]
    async fn test_scored_fact_records_access() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();

        retrieve_relevant_facts(&repo, None, "", &["productivity"], &default_params(10))
            .await
            .unwrap();

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
                .await
                .unwrap();
        }

        let results =
            retrieve_relevant_facts(&repo, None, "", &["productivity"], &default_params(3))
                .await
                .unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_retrieve_empty_domain() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let results =
            retrieve_relevant_facts(&repo, None, "", &["nonexistent"], &default_params(10))
                .await
                .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_retrieve_all_domains() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let mut f1 = test_fact("f1", "peak_hours", 1.0, 0);
        f1.domain = "productivity".into();
        let mut f2 = test_fact("f2", "budget", 1.0, 0);
        f2.domain = "finance".into();

        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let results = retrieve_all_domains(&repo, None, "", &["productivity", "finance"], 5, 0.5)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
    }
}
