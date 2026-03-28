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

/// Configurable relevance weights for the 8-factor scoring formula.
#[derive(Debug, Clone, Copy)]
pub struct RelevanceWeights {
    pub semantic: f64,
    pub retrievability: f64,
    pub importance: f64,
    pub frequency: f64,
    pub situation: f64,
    pub temporal: f64,
    pub hierarchy: f64,
    pub path_coherence: f64,
}

impl Default for RelevanceWeights {
    fn default() -> Self {
        Self {
            semantic: 0.25,
            retrievability: 0.15,
            importance: 0.10,
            frequency: 0.05,
            situation: 0.20,
            temporal: 0.05,
            hierarchy: 0.10,
            path_coherence: 0.10,
        }
    }
}

/// Combined relevance score for memory retrieval ranking.
/// See `RelevanceWeights::default()` for default weight distribution.
pub fn relevance_score(
    semantic_similarity: f64,
    retrievability: f64,
    importance: f64,
    access_frequency: f64,
    situational_boost: f64,
    temporal_recency: f64,
    hierarchy_score: f64,
    path_coherence: f64,
    weights: &RelevanceWeights,
) -> f64 {
    (semantic_similarity * weights.semantic
        + retrievability * weights.retrievability
        + importance * weights.importance
        + access_frequency * weights.frequency
        + situational_boost * weights.situation
        + temporal_recency * weights.temporal
        + hierarchy_score * weights.hierarchy
        + path_coherence * weights.path_coherence)
        .clamp(0.0, 1.0)
}

/// Compute temporal recency score from a valid_from timestamp string.
/// Returns a value between 0.1 and 1.0 — recent facts score higher.
pub fn temporal_recency_score(valid_from: &str) -> f64 {
    let now = chrono::Utc::now();
    let age_days: f64 = chrono::DateTime::parse_from_rfc3339(valid_from)
        .map(|dt| (now - dt.with_timezone(&chrono::Utc)).num_days().max(0) as f64)
        .map_err(|e| e.to_string())
        .or_else(|_| {
            valid_from
                .parse::<chrono::NaiveDateTime>()
                .map(|ndt| (now.naive_utc() - ndt).num_days().max(0) as f64)
                .map_err(|e| e.to_string())
        })
        .or_else(|_| {
            chrono::NaiveDate::parse_from_str(valid_from, "%Y-%m-%d")
                .map(|d| {
                    (now.naive_utc() - d.and_hms_opt(0, 0, 0).unwrap())
                        .num_days()
                        .max(0) as f64
                })
                .map_err(|e| e.to_string())
        })
        .unwrap_or(30.0);
    (1.0_f64 / (1.0_f64 + age_days / 30.0_f64)).max(0.1_f64)
}

/// Update stability after a retrieval event.
/// Successful retrieval increases stability (diminishing returns via ln curve),
/// capped at `max_stability` to prevent ranking domination by frequently accessed facts.
/// Failed retrieval leaves stability unchanged.
pub fn update_stability(current: f64, success: bool, max_stability: f64) -> f64 {
    if success {
        (current + (1.0 + current).ln().max(0.1)).min(max_stability)
    } else {
        current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: RelevanceWeights = RelevanceWeights {
        semantic: 0.3,
        retrievability: 0.2,
        importance: 0.15,
        frequency: 0.1,
        situation: 0.25,
        temporal: 0.05,
        hierarchy: 0.0,
        path_coherence: 0.0,
    };
    const MAX_S: f64 = 30.0;

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
        let score = relevance_score(0.8, 0.9, 0.7, 0.5, 0.6, 0.5, 0.0, 0.5, &W);
        assert!(score > 0.0 && score <= 1.0);
    }

    #[test]
    fn test_relevance_score_clamps() {
        let score = relevance_score(1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.5, &W);
        assert!((score - 1.0).abs() < f64::EPSILON);

        let score = relevance_score(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, &W);
        assert!((score - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_relevance_score_custom_weights() {
        // All weight on semantic similarity
        let custom = RelevanceWeights {
            semantic: 1.0,
            retrievability: 0.0,
            importance: 0.0,
            frequency: 0.0,
            situation: 0.0,
            temporal: 0.0,
            hierarchy: 0.0,
            path_coherence: 0.0,
        };
        let score = relevance_score(0.8, 0.1, 0.1, 0.1, 0.1, 0.0, 0.0, 0.5, &custom);
        assert!((score - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_temporal_recency_score_recent() {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
        let score = temporal_recency_score(&now);
        assert!(
            score > 0.95,
            "Today's fact should score near 1.0, got {score}"
        );
    }

    #[test]
    fn test_temporal_recency_score_old() {
        let score = temporal_recency_score("2020-01-01T00:00:00");
        assert!(score < 0.2, "Very old fact should score low, got {score}");
        assert!(score >= 0.1, "Score should be at least 0.1, got {score}");
    }

    #[test]
    fn test_new_stability_after_successful_retrieval() {
        let old = 1.0;
        let new = update_stability(old, true, MAX_S);
        assert!(new > old);
    }

    #[test]
    fn test_stability_unchanged_on_failed_retrieval() {
        let old = 5.0;
        let new = update_stability(old, false, MAX_S);
        assert!((new - old).abs() < f64::EPSILON);
    }

    #[test]
    fn test_stability_capped_at_max() {
        let high = update_stability(29.0, true, MAX_S);
        assert!(high <= 30.0, "Stability should be capped at max_stability");
        // Repeated retrievals should not exceed the cap
        let mut s = 1.0;
        for _ in 0..1000 {
            s = update_stability(s, true, MAX_S);
        }
        assert!(
            s <= 30.0,
            "Stability should never exceed max_stability after many retrievals"
        );
        assert!(
            (s - 30.0).abs() < f64::EPSILON,
            "Should reach exactly max_stability"
        );
    }

    #[test]
    fn test_stability_custom_max() {
        let mut s = 1.0;
        for _ in 0..100 {
            s = update_stability(s, true, 10.0);
        }
        assert!(
            (s - 10.0).abs() < f64::EPSILON,
            "Should cap at custom max_stability of 10.0"
        );
    }

    #[test]
    fn test_stability_growth_diminishes() {
        let s1 = update_stability(1.0, true, MAX_S) - 1.0;
        let s10 = update_stability(10.0, true, MAX_S) - 10.0;
        // Growth from stability=1 should be less than growth from stability=10
        // but the *relative* growth rate decreases
        assert!(s1 > 0.0);
        assert!(s10 > 0.0);
        // At higher stability, absolute growth is larger but relative is smaller
        assert!(s10 / 10.0 < s1 / 1.0);
    }

    #[test]
    fn test_extended_relevance_score_hierarchy_and_coherence() {
        let weights = RelevanceWeights {
            semantic: 0.25,
            retrievability: 0.15,
            importance: 0.10,
            frequency: 0.05,
            situation: 0.20,
            temporal: 0.05,
            hierarchy: 0.10,
            path_coherence: 0.10,
        };
        let score = relevance_score(
            0.8, 0.9, 0.7, 0.5, 0.6, 0.4, 1.0, 0.8, &weights,
        );
        // 0.25*0.8 + 0.15*0.9 + 0.10*0.7 + 0.05*0.5 + 0.20*0.6 + 0.05*0.4 + 0.10*1.0 + 0.10*0.8 = 0.75
        assert!((score - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_extended_score_backward_compat_non_note() {
        let weights = RelevanceWeights::default();
        let score = relevance_score(0.8, 0.9, 0.7, 0.5, 0.6, 0.4, 0.0, 0.5, &weights);
        assert!(score > 0.0 && score <= 1.0);
    }
}
