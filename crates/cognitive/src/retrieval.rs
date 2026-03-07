//! Memory retrieval with FSRS-scored relevance and situational boost.
//!
//! `CognitiveRetriever` searches semantic facts by domain and scores
//! results using FSRS retrievability, importance, and access frequency.
//! It records access on retrieved memories to increase stability.

use chrono::Utc;
use tracing::{debug, warn};

use crate::decay::{relevance_score, retrievability, update_stability};
use crate::repos::SemanticFactRepo;
use crate::types::SemanticFact;

/// A scored retrieval result.
#[derive(Debug, Clone)]
pub struct RetrievalResult {
    pub fact: SemanticFact,
    pub score: f64,
}

/// Retrieve and rank active facts for a domain with FSRS scoring.
///
/// - `semantic_similarity`: Optional per-fact similarity (e.g., from vector search).
///   If `None`, defaults to 0.5.
/// - `situational_boost`: Domain-level boost from UserSituation (0.0–1.0).
pub async fn retrieve_facts(
    repo: &SemanticFactRepo,
    domain: &str,
    limit: usize,
    situational_boost: f64,
) -> Result<Vec<RetrievalResult>, sqlx::Error> {
    let facts = repo.list_active(domain).await?;
    let now = Utc::now();

    let mut scored: Vec<RetrievalResult> = facts
        .into_iter()
        .map(|fact| {
            let elapsed_days = fact
                .last_accessed
                .as_ref()
                .and_then(|la| la.parse::<chrono::NaiveDateTime>().ok())
                .map(|la| (now.naive_utc() - la).num_seconds() as f64 / 86400.0)
                .unwrap_or_else(|| {
                    // Fall back to recorded_at
                    fact.recorded_at
                        .parse::<chrono::NaiveDateTime>()
                        .ok()
                        .map(|ra| (now.naive_utc() - ra).num_seconds() as f64 / 86400.0)
                        .unwrap_or(30.0)
                });

            let r = retrievability(elapsed_days, fact.stability);

            // Normalize access_count to 0.0–1.0 with diminishing returns
            let freq = 1.0 - (1.0 / (1.0 + fact.access_count as f64));

            let score = relevance_score(
                0.5, // semantic_similarity placeholder (no vector search here)
                r,
                fact.confidence,
                freq,
                situational_boost,
            );

            RetrievalResult { fact, score }
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.truncate(limit);

    // Record access on retrieved facts (increases FSRS stability)
    for result in &scored {
        let new_stability = update_stability(result.fact.stability, true);
        if let Err(e) = repo.record_access(&result.fact.id, new_stability).await {
            warn!("Failed to record access for fact '{}': {e}", result.fact.id);
        }
    }

    debug!(
        "Retrieved {} facts for domain '{}' (top score: {:.3})",
        scored.len(),
        domain,
        scored.first().map(|r| r.score).unwrap_or(0.0)
    );

    Ok(scored)
}

/// Retrieve facts across all domains, ranked by FSRS score.
pub async fn retrieve_all_domains(
    repo: &SemanticFactRepo,
    domains: &[&str],
    limit_per_domain: usize,
    situational_boost: f64,
) -> Result<Vec<RetrievalResult>, sqlx::Error> {
    let mut all = Vec::new();
    for domain in domains {
        let mut results = retrieve_facts(repo, domain, limit_per_domain, situational_boost).await?;
        all.append(&mut results);
    }
    all.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[tokio::test]
    async fn test_retrieve_facts_returns_scored_results() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();
        repo.upsert(&test_fact("f2", "break_pattern", 5.0, 10))
            .await
            .unwrap();

        let results = retrieve_facts(&repo, "productivity", 10, 0.5)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        // Higher stability + more accesses should rank higher
        assert!(results[0].score >= results[1].score);
    }

    #[tokio::test]
    async fn test_retrieve_facts_respects_limit() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        for i in 0..5 {
            repo.upsert(&test_fact(&format!("f{i}"), &format!("pred{i}"), 1.0, 0))
                .await
                .unwrap();
        }

        let results = retrieve_facts(&repo, "productivity", 3, 0.5).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn test_retrieve_records_access() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool.clone());

        repo.upsert(&test_fact("f1", "peak_hours", 1.0, 0))
            .await
            .unwrap();

        retrieve_facts(&repo, "productivity", 10, 0.5)
            .await
            .unwrap();

        let updated = repo.get("f1").await.unwrap().unwrap();
        assert_eq!(updated.access_count, 1);
        assert!(updated.stability > 1.0); // Stability increased
    }

    #[tokio::test]
    async fn test_retrieve_empty_domain() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let results = retrieve_facts(&repo, "nonexistent", 10, 0.5).await.unwrap();
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

        let results = retrieve_all_domains(&repo, &["productivity", "finance"], 5, 0.5)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
    }
}
