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
}

impl TrialParams {
    /// Resolve all 6 relevance weights to a normalized array that sums to 1.0.
    /// Phase 1 tunes 3 weights; the other 3 come from Config defaults.
    /// Returns [semantic, retrievability, importance, frequency, situation, temporal].
    pub fn resolve_relevance_weights(
        &self,
        default_importance: f64,
        default_frequency: f64,
        default_temporal: f64,
    ) -> [f64; 6] {
        let raw = [
            self.relevance_weight_semantic.unwrap_or(0.30),
            self.relevance_weight_retrievability.unwrap_or(0.20),
            default_importance,
            default_frequency,
            self.relevance_weight_situation.unwrap_or(0.25),
            default_temporal,
        ];
        let sum: f64 = raw.iter().sum();
        if sum == 0.0 {
            return [1.0 / 6.0; 6];
        }
        raw.map(|w| w / sum)
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
    fn normalize_relevance_weights_sums_to_one() {
        let params = TrialParams {
            relevance_weight_semantic: Some(0.40),
            relevance_weight_retrievability: Some(0.25),
            relevance_weight_situation: Some(0.30),
            ..Default::default()
        };
        let weights = params.resolve_relevance_weights(0.15, 0.10, 0.05);
        let sum: f64 = weights.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-10,
            "Weights must sum to 1.0, got {sum}"
        );
    }
}
