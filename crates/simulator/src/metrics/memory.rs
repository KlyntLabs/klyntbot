use crate::persona::FactTriple;
use std::collections::HashSet;

/// Measure the fraction of known facts that exist unsuperseded in the repo.
///
/// For each fact triple, we search via FTS for `"{subject} {predicate} {object}"`.
/// A fact counts as retained if any result has matching subject, predicate, and
/// object with `superseded_at` being `None`.
pub async fn measure_knowledge_retention(
    repo: &cognitive::SemanticFactRepo,
    known_facts: &[FactTriple],
) -> f64 {
    if known_facts.is_empty() {
        return 1.0;
    }

    let mut found = 0u32;

    for fact in known_facts {
        let query = format!("{} {} {}", fact.subject, fact.predicate, fact.object);
        let results = match repo.search_fts(&query, None, 5).await {
            Ok(r) => r,
            Err(_) => continue,
        };

        let retained = results.iter().any(|r| {
            r.subject == fact.subject
                && r.predicate == fact.predicate
                && r.object == fact.object
                && r.superseded_at.is_none()
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
