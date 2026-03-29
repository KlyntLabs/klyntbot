use serde::{Deserialize, Serialize};

/// Per-request parameter overrides for autotuner experiments.
/// Each field is Option — None means "use Config default."
/// All fields are #[serde(default)] for forward-compatible deserialization.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct TrialParams {
    // Phase 1: SkillRouter knobs
    pub skill_keyword_weight: Option<f64>,
    pub skill_semantic_weight: Option<f64>,
    pub skill_activation_threshold: Option<f64>,

    // Phase 1: IntentAnalyzer knobs
    pub heuristic_confidence_threshold: Option<f64>,
    pub llm_classifier_timeout_ms: Option<u64>,

    // Phase 1: Cognitive retrieval relevance weights (3 of 6 tuned in Phase 1)
    pub relevance_weight_semantic: Option<f64>,
    pub relevance_weight_retrievability: Option<f64>,
    pub relevance_weight_situation: Option<f64>,

    // Phase 2: FSRS tuning
    pub fsrs_desired_retention: Option<f64>, // default 0.9, bounds [0.70, 0.99]

    // Phase 2: Accumulation thresholds
    // NOTE: These are read ONCE at startup by BackgroundConsolidationService and cannot
    // be dynamically overridden mid-run. They only take effect when the champion is promoted
    // and the service is restarted. Shadow scoring cannot evaluate these — they are
    // "promotion-time" params, not "per-message" params.
    pub accumulate_promote_threshold: Option<usize>, // default 5, bounds [2, 15]
    pub accumulate_min_days: Option<usize>,          // default 3, bounds [1, 10]

    // Phase 2: Vector search
    pub vector_top_k: Option<usize>, // default 30, bounds [10, 100]
    pub min_similarity: Option<f64>, // default 0.55, bounds [0.30, 0.80]

    // Phase 2: Remaining 3 relevance weights (completes the 6-factor set)
    pub relevance_weight_importance: Option<f64>, // default 0.15, bounds [0.05, 0.40]
    pub relevance_weight_frequency: Option<f64>,  // default 0.10, bounds [0.02, 0.30]
    pub relevance_weight_temporal: Option<f64>,   // default 0.05, bounds [0.01, 0.20]

    // Phase 3: Query rewriting
    /// Minimum enrichment confidence to inject into InsightForge (bounds [0.3, 0.95]).
    /// Higher = more selective (fewer rewrites), lower = more aggressive.
    pub rewrite_confidence_threshold: Option<f64>,
    /// Max context signals to include in heuristic enrichment (bounds [1, 6]).
    /// Higher = richer enrichment at the cost of noise.
    pub rewrite_max_signals: Option<usize>,
    /// Minimum enriched query length to accept (bounds [5, 30]).
    /// Shorter enrichments are discarded as too vague.
    pub rewrite_min_enrichment_length: Option<usize>,

    // Phase 4: Hierarchical note retrieval (5 params)
    pub relevance_weight_hierarchy: Option<f64>,
    pub relevance_weight_path_coherence: Option<f64>,
    pub tree_top_k: Option<usize>,
    pub tree_min_similarity: Option<f64>,
    pub hybrid_bias: Option<f64>,

    // Phase 5: Community graph (4 params)
    pub relevance_weight_community: Option<f64>,
    pub relevance_weight_cross_note: Option<f64>,
    pub community_top_k: Option<usize>,
    pub community_min_similarity: Option<f64>,
}

impl TrialParams {
    /// Resolve all 6 relevance weights to a normalized array that sums to 1.0.
    /// All 6 weights can come from TrialParams; any that are `None` fall back to
    /// the corresponding Config default.
    /// Returns [semantic, retrievability, importance, frequency, situation, temporal].
    pub fn resolve_relevance_weights(&self, defaults: &[f64; 6]) -> [f64; 6] {
        let raw = [
            self.relevance_weight_semantic.unwrap_or(defaults[0]),
            self.relevance_weight_retrievability.unwrap_or(defaults[1]),
            self.relevance_weight_importance.unwrap_or(defaults[2]),
            self.relevance_weight_frequency.unwrap_or(defaults[3]),
            self.relevance_weight_situation.unwrap_or(defaults[4]),
            self.relevance_weight_temporal.unwrap_or(defaults[5]),
        ];
        let sum: f64 = raw.iter().sum();
        if sum > 0.0 {
            raw.map(|w| w / sum)
        } else {
            *defaults
        }
    }

    /// Resolve all 10 relevance weights to a normalized array that sums to 1.0.
    /// Returns [semantic, retrievability, importance, frequency, situation, temporal,
    ///          hierarchy, path_coherence, community, cross_note].
    pub fn resolve_full_relevance_weights(&self, defaults: &[f64; 10]) -> [f64; 10] {
        let raw = [
            self.relevance_weight_semantic.unwrap_or(defaults[0]),
            self.relevance_weight_retrievability.unwrap_or(defaults[1]),
            self.relevance_weight_importance.unwrap_or(defaults[2]),
            self.relevance_weight_frequency.unwrap_or(defaults[3]),
            self.relevance_weight_situation.unwrap_or(defaults[4]),
            self.relevance_weight_temporal.unwrap_or(defaults[5]),
            self.relevance_weight_hierarchy.unwrap_or(defaults[6]),
            self.relevance_weight_path_coherence.unwrap_or(defaults[7]),
            self.relevance_weight_community.unwrap_or(defaults[8]),
            self.relevance_weight_cross_note.unwrap_or(defaults[9]),
        ];
        let sum: f64 = raw.iter().sum();
        if sum > 0.0 {
            raw.map(|w| w / sum)
        } else {
            *defaults
        }
    }

    /// Returns `true` if any Phase 3 rewrite-related field is `Some`.
    pub fn has_rewrite_params(&self) -> bool {
        self.rewrite_confidence_threshold.is_some()
            || self.rewrite_max_signals.is_some()
            || self.rewrite_min_enrichment_length.is_some()
    }

    /// Returns `true` if any Phase 2 memory-related field is `Some`.
    pub fn has_memory_params(&self) -> bool {
        self.fsrs_desired_retention.is_some()
            || self.accumulate_promote_threshold.is_some()
            || self.accumulate_min_days.is_some()
            || self.vector_top_k.is_some()
            || self.min_similarity.is_some()
            || self.relevance_weight_importance.is_some()
            || self.relevance_weight_frequency.is_some()
            || self.relevance_weight_temporal.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trial_params_default_is_all_none() {
        let params = TrialParams::default();
        assert!(params.skill_keyword_weight.is_none());
        assert!(params.relevance_weight_semantic.is_none());
    }

    #[test]
    fn trial_params_roundtrip_serde() {
        let params = TrialParams {
            skill_keyword_weight: Some(0.65),
            skill_semantic_weight: Some(0.35),
            ..Default::default()
        };
        let json = serde_json::to_string(&params).unwrap();
        let back: TrialParams = serde_json::from_str(&json).unwrap();
        assert_eq!(back.skill_keyword_weight, Some(0.65));
        assert!(back.heuristic_confidence_threshold.is_none());
    }

    #[test]
    fn trial_params_forward_compat_deserialization() {
        let phase1_json = r#"{"skill_keyword_weight": 0.6}"#;
        let params: TrialParams = serde_json::from_str(phase1_json).unwrap();
        assert_eq!(params.skill_keyword_weight, Some(0.6));
        assert!(params.relevance_weight_semantic.is_none());
    }

    #[test]
    fn phase1_champion_deserializes_with_phase2_fields() {
        let json = r#"{"skill_keyword_weight": 0.7, "skill_semantic_weight": 0.3}"#;
        let params: TrialParams = serde_json::from_str(json).unwrap();
        assert!(params.fsrs_desired_retention.is_none());
        assert!(params.vector_top_k.is_none());
        assert!(params.relevance_weight_importance.is_none());
    }

    #[test]
    fn has_memory_params_detects_phase2_fields() {
        let empty = TrialParams::default();
        assert!(!empty.has_memory_params());

        let with_phase2 = TrialParams {
            vector_top_k: Some(50),
            ..Default::default()
        };
        assert!(with_phase2.has_memory_params());
    }

    #[test]
    fn has_rewrite_params_detects_phase3_fields() {
        let empty = TrialParams::default();
        assert!(!empty.has_rewrite_params());

        let with_rewrite = TrialParams {
            rewrite_confidence_threshold: Some(0.5),
            ..Default::default()
        };
        assert!(with_rewrite.has_rewrite_params());
    }

    #[test]
    fn phase2_champion_deserializes_with_phase3_fields() {
        let json = r#"{"vector_top_k": 50, "min_similarity": 0.6}"#;
        let params: TrialParams = serde_json::from_str(json).unwrap();
        assert!(params.rewrite_confidence_threshold.is_none());
        assert!(params.rewrite_max_signals.is_none());
        assert!(params.rewrite_min_enrichment_length.is_none());
    }

    #[test]
    fn normalize_relevance_weights_sums_to_one() {
        let params = TrialParams {
            relevance_weight_semantic: Some(0.40),
            relevance_weight_retrievability: Some(0.25),
            relevance_weight_situation: Some(0.30),
            ..Default::default()
        };
        let weights = params.resolve_relevance_weights(&[0.30, 0.20, 0.15, 0.10, 0.25, 0.05]);
        let sum: f64 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "Weights must sum to 1.0, got {sum}"
        );
    }

    #[test]
    fn resolve_full_relevance_weights_sums_to_one() {
        let params = TrialParams {
            relevance_weight_semantic: Some(0.30),
            relevance_weight_hierarchy: Some(0.20),
            relevance_weight_community: Some(0.25),
            ..Default::default()
        };
        let defaults = [0.20, 0.10, 0.08, 0.05, 0.15, 0.02, 0.10, 0.05, 0.15, 0.10];
        let weights = params.resolve_full_relevance_weights(&defaults);
        let sum: f64 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "All 10 weights must sum to 1.0, got {sum}"
        );
    }
}
