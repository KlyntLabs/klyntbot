//! Orchestrator configuration for the intent pipeline.

use serde::{Deserialize, Serialize};

/// Configuration for the intent pipeline orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorConfig {
    /// Confidence threshold above which heuristic classification is accepted (0.0-1.0)
    #[serde(default = "default_heuristic_threshold")]
    pub heuristic_confidence_threshold: f32,

    /// Timeout in milliseconds for the LLM classifier call
    #[serde(default = "default_classifier_timeout")]
    pub llm_classifier_timeout: u64,

    /// Override model for the LLM classifier (uses default agent model if None)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub llm_classifier_model: Option<String>,

    /// Maximum number of escalations per request (Direct → Reactive)
    #[serde(default = "default_max_escalations")]
    pub max_escalations: u32,
}

fn default_heuristic_threshold() -> f32 {
    0.85
}
fn default_classifier_timeout() -> u64 {
    2000
}
fn default_max_escalations() -> u32 {
    1
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            heuristic_confidence_threshold: default_heuristic_threshold(),
            llm_classifier_timeout: default_classifier_timeout(),
            llm_classifier_model: None,
            max_escalations: default_max_escalations(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orchestrator_config_defaults() {
        let config: OrchestratorConfig = serde_json::from_str("{}").unwrap();
        assert!((config.heuristic_confidence_threshold - 0.85).abs() < f32::EPSILON);
        assert_eq!(config.llm_classifier_timeout, 2000);
        assert_eq!(config.llm_classifier_model, None);
        assert_eq!(config.max_escalations, 1);
    }

    #[test]
    fn orchestrator_config_roundtrip() {
        let config = OrchestratorConfig {
            heuristic_confidence_threshold: 0.9,
            llm_classifier_timeout: 5000,
            llm_classifier_model: Some("fast-model".to_string()),
            max_escalations: 2,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: OrchestratorConfig = serde_json::from_str(&json).unwrap();
        assert!((loaded.heuristic_confidence_threshold - 0.9).abs() < f32::EPSILON);
        assert_eq!(loaded.llm_classifier_timeout, 5000);
        assert_eq!(loaded.llm_classifier_model.as_deref(), Some("fast-model"));
        assert_eq!(loaded.max_escalations, 2);
    }

    #[test]
    fn orchestrator_config_camel_case() {
        let config = OrchestratorConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("heuristicConfidenceThreshold"));
        assert!(json.contains("llmClassifierTimeout"));
        assert!(json.contains("maxEscalations"));
    }
}
