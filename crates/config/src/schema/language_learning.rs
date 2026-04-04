//! Language learning configuration.

use serde::{Deserialize, Serialize};

/// Feedback level for pronunciation corrections.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FeedbackLevel {
    /// Post-turn summary card.
    #[default]
    Summary,
    /// Real-time overlay on persistent weak spots.
    Overlay,
    /// Background scoring, surface on request only.
    Silent,
}

/// Controls how aggressively pronunciation feedback is shown.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedbackConfig {
    /// Default feedback level.
    #[serde(default)]
    pub default_level: FeedbackLevel,
    /// FSRS stability below which feedback escalates to Overlay.
    #[serde(default = "default_escalation_threshold")]
    pub escalation_threshold: f32,
    /// Minimum encounters before escalation is considered.
    #[serde(default = "default_min_encounters")]
    pub min_encounters: u32,
}

impl Default for FeedbackConfig {
    fn default() -> Self {
        Self {
            default_level: FeedbackLevel::default(),
            escalation_threshold: default_escalation_threshold(),
            min_encounters: default_min_encounters(),
        }
    }
}

/// Top-level language learning configuration.
///
/// Target languages are read from `LanguageConfig::target_lang` (in `config.language`)
/// to avoid duplicating language preferences across config sections.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageLearningConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub feedback: FeedbackConfig,
}

fn default_escalation_threshold() -> f32 {
    0.3
}

fn default_min_encounters() -> u32 {
    5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = LanguageLearningConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.feedback.default_level, FeedbackLevel::Summary);
        assert!((config.feedback.escalation_threshold - 0.3).abs() < 0.01);
        assert_eq!(config.feedback.min_encounters, 5);
    }

    #[test]
    fn deserialize_with_overrides() {
        let json = r#"{"enabled": true, "feedback": {"defaultLevel": "overlay"}}"#;
        let config: LanguageLearningConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.feedback.default_level, FeedbackLevel::Overlay);
    }
}
