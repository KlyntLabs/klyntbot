/// Compute a composite personalisation score from three inputs.
///
/// - `fact_coverage`: fraction of known facts retained (0.0 – 1.0)
/// - `retrieval_precision`: precision of recent retrievals (0.0 – 1.0)
/// - `retrieval_recall`: recall of recent retrievals (0.0 – 1.0)
///
/// Formula: `fact_coverage * 0.4 + retrieval_precision * 0.3 + retrieval_recall * 0.3`
pub fn personalization_score(
    fact_coverage: f64,
    retrieval_precision: f64,
    retrieval_recall: f64,
) -> f64 {
    fact_coverage * 0.4 + retrieval_precision * 0.3 + retrieval_recall * 0.3
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_scores() {
        let score = personalization_score(1.0, 1.0, 1.0);
        assert!((score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn worst_scores() {
        let score = personalization_score(0.0, 0.0, 0.0);
        assert!((score - 0.0).abs() < 1e-9);
    }

    #[test]
    fn mixed_scores() {
        // 0.85 * 0.4 + 0.8 * 0.3 + 0.7 * 0.3
        // = 0.34 + 0.24 + 0.21 = 0.79
        let score = personalization_score(0.85, 0.8, 0.7);
        assert!((score - 0.79).abs() < 1e-9);
    }
}
