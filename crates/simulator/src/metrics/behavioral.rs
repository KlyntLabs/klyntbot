/// Compute a composite personalisation score from three inputs.
///
/// - `fact_coverage`: fraction of known facts retained (0.0 – 1.0)
/// - `retrieval_precision`: precision of recent retrievals (0.0 – 1.0)
/// - `correction_rate`: fraction of messages that were corrections (0.0 – 1.0)
///
/// Formula: `fact_coverage * 0.4 + retrieval_precision * 0.3 + (1 - correction_rate) * 0.3`
pub fn personalization_score(
    fact_coverage: f64,
    retrieval_precision: f64,
    correction_rate: f64,
) -> f64 {
    let correction_rate_inverse = 1.0 - correction_rate;
    fact_coverage * 0.4 + retrieval_precision * 0.3 + correction_rate_inverse * 0.3
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_scores() {
        let score = personalization_score(1.0, 1.0, 0.0);
        assert!((score - 1.0).abs() < 1e-9);
    }

    #[test]
    fn worst_scores() {
        let score = personalization_score(0.0, 0.0, 1.0);
        assert!((score - 0.0).abs() < 1e-9);
    }

    #[test]
    fn mixed_scores() {
        // 0.85 * 0.4 + 0.8 * 0.3 + (1 - 0.2) * 0.3
        // = 0.34 + 0.24 + 0.24 = 0.82
        let score = personalization_score(0.85, 0.8, 0.2);
        assert!((score - 0.82).abs() < 1e-9);
    }
}
