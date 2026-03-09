//! FSRS-inspired decay and relevance scoring for memory retrieval.

/// FSRS-inspired retrievability: probability of successful recall.
/// R = exp(ln(0.9) * elapsed_days / stability)
///
/// At stability=S days, retrievability is 0.9 (90% recall probability).
/// As elapsed_days grows beyond stability, recall probability drops exponentially.
pub fn retrievability(elapsed_days: f64, stability: f64) -> f64 {
    if stability <= 0.0 {
        return 0.0;
    }
    (0.9_f64.ln() * elapsed_days / stability).exp()
}

/// Combined relevance score for memory retrieval ranking.
///
/// Weights: semantic 0.3, retrievability 0.2, importance 0.15, frequency 0.1, situation 0.25
pub fn relevance_score(
    semantic_similarity: f64,
    retrievability: f64,
    importance: f64,
    access_frequency: f64,
    situational_boost: f64,
) -> f64 {
    (semantic_similarity * 0.3
        + retrievability * 0.2
        + importance * 0.15
        + access_frequency * 0.1
        + situational_boost * 0.25)
        .clamp(0.0, 1.0)
}

/// Maximum stability value to prevent runaway inflation from frequent retrievals.
const MAX_STABILITY: f64 = 30.0;

/// Update stability after a retrieval event.
/// Successful retrieval increases stability (diminishing returns via ln curve),
/// capped at `MAX_STABILITY` to prevent ranking domination by frequently accessed facts.
/// Failed retrieval leaves stability unchanged.
pub fn update_stability(current: f64, success: bool) -> f64 {
    if success {
        (current + (1.0 + current).ln().max(0.1)).min(MAX_STABILITY)
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrievability_fresh_memory() {
        let r = retrievability(0.0, 1.0);
        assert!((r - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_retrievability_decays_over_time() {
        let r1 = retrievability(1.0, 1.0);
        let r7 = retrievability(7.0, 1.0);
        assert!(r1 > r7);
    }

    #[test]
    fn test_higher_stability_resists_decay() {
        let low_stability = retrievability(7.0, 1.0);
        let high_stability = retrievability(7.0, 10.0);
        assert!(high_stability > low_stability);
    }

    #[test]
    fn test_retrievability_at_stability_equals_point_nine() {
        // At elapsed_days == stability, R should be exactly 0.9
        let r = retrievability(5.0, 5.0);
        assert!((r - 0.9).abs() < 0.001);
    }

    #[test]
    fn test_zero_stability_returns_zero() {
        assert_eq!(retrievability(1.0, 0.0), 0.0);
        assert_eq!(retrievability(1.0, -1.0), 0.0);
    }

    #[test]
    fn test_relevance_score_combines_factors() {
        let score = relevance_score(0.8, 0.9, 0.7, 0.5, 0.6);
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_relevance_score_clamps() {
        let score = relevance_score(1.0, 1.0, 1.0, 1.0, 1.0);
        assert!((score - 1.0).abs() < f64::EPSILON);

        let score = relevance_score(0.0, 0.0, 0.0, 0.0, 0.0);
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_new_stability_after_successful_retrieval() {
        let old = 1.0;
        let new = update_stability(old, true);
        assert!(new > old);
    }

    #[test]
    fn test_stability_unchanged_on_failed_retrieval() {
        let old = 5.0;
        let new = update_stability(old, false);
        assert!((new - old).abs() < f64::EPSILON);
    }

    #[test]
    fn test_stability_capped_at_max() {
        let high = update_stability(29.0, true);
        assert!(high <= 30.0, "Stability should be capped at MAX_STABILITY");
        // Repeated retrievals should not exceed the cap
        let mut s = 1.0;
        for _ in 0..1000 {
            s = update_stability(s, true);
        }
        assert!(
            s <= 30.0,
            "Stability should never exceed MAX_STABILITY after many retrievals"
        );
        assert!(
            (s - 30.0).abs() < f64::EPSILON,
            "Should reach exactly MAX_STABILITY"
        );
    }

    #[test]
    fn test_stability_growth_diminishes() {
        let s1 = update_stability(1.0, true) - 1.0;
        let s10 = update_stability(10.0, true) - 10.0;
        // Growth from stability=1 should be less than growth from stability=10
        // but the *relative* growth rate decreases
        assert!(s1 > 0.0);
        assert!(s10 > 0.0);
        // At higher stability, absolute growth is larger but relative is smaller
        assert!(s10 / 10.0 < s1 / 1.0);
    }
}
