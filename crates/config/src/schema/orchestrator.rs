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

    /// Maximum fabrication retries before accepting fabricated content (default: 2)
    #[serde(default = "default_max_fabrication_retries")]
    pub max_fabrication_retries: u32,

    /// Reaction satisfaction window in minutes (default: 15)
    #[serde(default = "default_satisfaction_window_minutes")]
    pub satisfaction_window_minutes: u64,
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
fn default_max_fabrication_retries() -> u32 {
    2
}
fn default_satisfaction_window_minutes() -> u64 {
    15
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            heuristic_confidence_threshold: default_heuristic_threshold(),
            llm_classifier_timeout: default_classifier_timeout(),
            llm_classifier_model: None,
            max_escalations: default_max_escalations(),
            max_fabrication_retries: default_max_fabrication_retries(),
            satisfaction_window_minutes: default_satisfaction_window_minutes(),
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
        assert_eq!(config.max_fabrication_retries, 2);
        assert_eq!(config.satisfaction_window_minutes, 15);
    }

    #[test]
    fn orchestrator_config_roundtrip() {
        let config = OrchestratorConfig {
            heuristic_confidence_threshold: 0.9,
            llm_classifier_timeout: 5000,
            llm_classifier_model: Some("fast-model".to_string()),
            max_escalations: 2,
            max_fabrication_retries: 5,
            satisfaction_window_minutes: 30,
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: OrchestratorConfig = serde_json::from_str(&json).unwrap();
        assert!((loaded.heuristic_confidence_threshold - 0.9).abs() < f32::EPSILON);
        assert_eq!(loaded.llm_classifier_timeout, 5000);
        assert_eq!(loaded.llm_classifier_model.as_deref(), Some("fast-model"));
        assert_eq!(loaded.max_escalations, 2);
        assert_eq!(loaded.max_fabrication_retries, 5);
        assert_eq!(loaded.satisfaction_window_minutes, 30);
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
