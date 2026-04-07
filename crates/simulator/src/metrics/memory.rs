use crate::persona::FactTriple;
use std::collections::HashSet;

const NEGATION_MARKERS: &[&str] = &[
    "not ",
    "no longer ",
    "stopped ",
    "quit ",
    "never ",
    "don't ",
    "doesn't ",
    "isn't ",
    "aren't ",
    "wasn't ",
];

/// Measure the fraction of known facts that exist unsuperseded in the repo.
///
/// Uses three matching strategies:
/// 1. **Exact match**: subject, predicate, object all match exactly (ideal for LLM extraction)
/// 2. **Content match**: the fact's object field contains the known fact's key terms
///    (handles heuristic extraction which stores the full message as the object)
/// 3. **Subject + object match**: subject matches and the stored object contains the known
///    fact's object value (catches templates that omit the predicate, e.g.
///    "Just so you know, I'm a software engineer")
///
/// Fetches all active facts in a single query instead of issuing one FTS query per known fact.
pub async fn measure_knowledge_retention(
    repo: &cognitive::SemanticFactRepo,
    known_facts: &[FactTriple],
) -> f64 {
    if known_facts.is_empty() {
        return 1.0;
    }

    // Single query: get ALL unsuperseded facts
    let all_facts = repo.list_all_active().await.unwrap_or_default();

    let mut found = 0u32;
    for fact in known_facts {
        // Hoist lowercased variants out of the inner loop
        let pred_lower = fact.predicate.to_lowercase();
        let obj_lower_known = fact.object.to_lowercase();

        let retained = all_facts.iter().any(|r| {
            // Strategy 1: exact triple match
            if r.subject == fact.subject && r.predicate == fact.predicate && r.object == fact.object
            {
                return true;
            }
            // Strategy 2: content match (object contains both predicate and object terms)
            let obj_lower = r.object.to_lowercase();
            if obj_lower.contains(&pred_lower) && obj_lower.contains(&obj_lower_known) {
                return true;
            }
            // Strategy 3: subject match + object contains the fact's object value
            // Reject if negation markers appear before the match position
            if r.subject == fact.subject && obj_lower.contains(&obj_lower_known) {
                let has_negation = if let Some(pos) = obj_lower.find(&obj_lower_known) {
                    let prefix = &obj_lower[..pos];
                    NEGATION_MARKERS.iter().any(|neg| prefix.contains(neg))
                } else {
                    false
                };
                return !has_negation;
            }
            false
        });
        if retained {
            found += 1;
        }
    }

    found as f64 / known_facts.len() as f64
}

/// Compute precision and recall for a single retrieval query.
///
/// - **Precision** = |intersection| / |retrieved|
/// - **Recall** = |intersection| / |relevant|
///
/// Returns `(precision, recall)`. Both are 0.0 when the respective
/// denominator is empty.
pub fn measure_retrieval_quality(retrieved_ids: &[String], relevant_ids: &[String]) -> (f64, f64) {
    if retrieved_ids.is_empty() && relevant_ids.is_empty() {
        return (1.0, 1.0);
    }

    let retrieved: HashSet<&str> = retrieved_ids.iter().map(|s| s.as_str()).collect();
    let relevant: HashSet<&str> = relevant_ids.iter().map(|s| s.as_str()).collect();

    let intersection = retrieved.intersection(&relevant).count() as f64;

    let precision = if retrieved.is_empty() {
        0.0
    } else {
        intersection / retrieved.len() as f64
    };

    let recall = if relevant.is_empty() {
        0.0
    } else {
        intersection / relevant.len() as f64
    };

    (precision, recall)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ── Knowledge retention tests ─────────────────────────────────────

    /// Helper: create an in-memory pool with cognitive migrations applied.
    async fn test_pool() -> (storage::StoragePool, sqlx::SqlitePool) {
        let pool = storage::StoragePool::connect_in_memory()
            .await
            .expect("in-memory pool");
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &cognitive::cognitive_migrations())
            .await
            .expect("cognitive migrations");
        (pool, inner)
    }

    #[tokio::test]
    async fn retention_finds_heuristic_extracted_facts() {
        let (pool, inner) = test_pool().await;
        // Keep the StoragePool alive so the in-memory DB is not dropped.
        std::mem::forget(pool);

        let repo = cognitive::SemanticFactRepo::new(inner);

        // Insert a fact as the HeuristicExtractionHandler would:
        //  - subject: "user"
        //  - predicate: "stated"  (not the original "works_as")
        //  - object: the full message text (template that omits the predicate)
        let fact = cognitive::SemanticFact {
            id: "heuristic-1".to_string(),
            domain: "personal".to_string(),
            subject: "user".to_string(),
            predicate: "stated".to_string(),
            object: "Just so you know, I'm a software engineer".to_string(),
            confidence: 0.5,
            source: "heuristic".to_string(),
            valid_from: chrono::Utc::now().to_rfc3339(),
            valid_until: None,
            recorded_at: chrono::Utc::now().to_rfc3339(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: "semantic".to_string(),
            scope_type: "global".to_string(),
            scope_id: None,
        };
        repo.upsert(&fact).await.expect("upsert fact");

        // The known fact from the persona profile uses the real predicate.
        let known = vec![FactTriple {
            subject: "user".to_string(),
            predicate: "works_as".to_string(),
            object: "software engineer".to_string(),
        }];

        let retention = measure_knowledge_retention(&repo, &known).await;
        // Strategy 3 should match: subject == "user" and object contains "software engineer"
        assert!(
            (retention - 1.0).abs() < 1e-9,
            "expected retention 1.0, got {retention}"
        );
    }

    #[tokio::test]
    async fn retention_rejects_negated_facts() {
        let (pool, inner) = test_pool().await;
        std::mem::forget(pool);

        let repo = cognitive::SemanticFactRepo::new(inner);

        let fact = cognitive::SemanticFact {
            id: "negated-1".to_string(),
            domain: "personal".to_string(),
            subject: "user".to_string(),
            predicate: "stated".to_string(),
            object: "I am no longer a software engineer".to_string(),
            confidence: 0.5,
            source: "heuristic".to_string(),
            valid_from: chrono::Utc::now().to_rfc3339(),
            valid_until: None,
            recorded_at: chrono::Utc::now().to_rfc3339(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: "semantic".to_string(),
            scope_type: "global".to_string(),
            scope_id: None,
        };
        repo.upsert(&fact).await.expect("upsert negated fact");

        let known = vec![FactTriple {
            subject: "user".to_string(),
            predicate: "works_as".to_string(),
            object: "software engineer".to_string(),
        }];

        let retention = measure_knowledge_retention(&repo, &known).await;
        assert!(
            (retention - 0.0).abs() < 1e-9,
            "negated fact should NOT count as retained, got {retention}"
        );
    }

    // ── Retrieval quality tests ───────────────────────────────────────

    #[test]
    fn precision_recall_all_relevant() {
        let ids: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let (p, r) = measure_retrieval_quality(&ids, &ids);
        assert!((p - 1.0).abs() < 1e-9);
        assert!((r - 1.0).abs() < 1e-9);
    }

    #[test]
    fn precision_recall_partial() {
        let retrieved: Vec<String> = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        let relevant: Vec<String> = vec!["a".into(), "c".into(), "e".into(), "f".into()];

        let (p, r) = measure_retrieval_quality(&retrieved, &relevant);

        // intersection = {a, c} => 2
        // precision = 2 / 4 = 0.5
        // recall = 2 / 4 = 0.5
        assert!((p - 0.5).abs() < 1e-9);
        assert!((r - 0.5).abs() < 1e-9);
    }

    #[test]
    fn precision_recall_no_overlap() {
        let retrieved: Vec<String> = vec!["a".into(), "b".into()];
        let relevant: Vec<String> = vec!["c".into(), "d".into()];

        let (p, r) = measure_retrieval_quality(&retrieved, &relevant);
        assert!((p - 0.0).abs() < 1e-9);
        assert!((r - 0.0).abs() < 1e-9);
    }

    #[test]
    fn precision_recall_empty_retrieved() {
        let retrieved: Vec<String> = vec![];
        let relevant: Vec<String> = vec!["a".into()];

        let (p, r) = measure_retrieval_quality(&retrieved, &relevant);
        assert!((p - 0.0).abs() < 1e-9);
        assert!((r - 0.0).abs() < 1e-9);
    }

    #[test]
    fn precision_recall_both_empty() {
        let (p, r) = measure_retrieval_quality(&[], &[]);
        assert!((p - 1.0).abs() < 1e-9);
        assert!((r - 1.0).abs() < 1e-9);
    }
}
