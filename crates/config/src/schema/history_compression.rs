use serde::{Deserialize, Serialize};

fn default_tier1_ratio() -> f32 {
    0.35
}
fn default_tier2_ratio() -> f32 {
    0.12
}
fn default_high_threshold() -> f64 {
    0.70
}
fn default_low_threshold() -> f64 {
    0.40
}
fn default_demotion_threshold() -> usize {
    30
}
fn default_8() -> usize {
    8
}
fn default_12() -> usize {
    12
}
fn default_16() -> usize {
    16
}

/// Configuration for tiered history compression.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryCompressionConfig {
    /// Override model for summarization LLM calls.
    /// None = use agents.defaults.model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Use cognitive 12-factor scoring for tier promotion.
    #[serde(default = "super::core::default_true")]
    pub use_cognitive_scoring: bool,

    /// Only compress new messages on session resume.
    #[serde(default = "super::core::default_true")]
    pub delta_only_on_resume: bool,

    /// Tier 0 verbatim message count per depth mode.
    #[serde(default)]
    pub tier0_messages: TierZeroConfig,

    /// Target compression ratio for Tier 1 summaries.
    #[serde(default = "default_tier1_ratio")]
    pub tier1_ratio: f32,

    /// Target compression ratio for Tier 2 summaries.
    #[serde(default = "default_tier2_ratio")]
    pub tier2_ratio: f32,

    /// Cognitive score threshold for promoting old turns to Tier 1.
    #[serde(default = "default_high_threshold")]
    pub high_relevance_threshold: f64,

    /// Cognitive score threshold for keeping turns in Tier 1 vs Tier 2.
    #[serde(default = "default_low_threshold")]
    pub low_relevance_threshold: f64,

    /// Turns from current end before Tier 1 demotes to Tier 2.
    #[serde(default = "default_demotion_threshold")]
    pub tier1_demotion_threshold: usize,
}

impl Default for HistoryCompressionConfig {
    fn default() -> Self {
        Self {
            model: None,
            use_cognitive_scoring: true,
            delta_only_on_resume: true,
            tier0_messages: TierZeroConfig::default(),
            tier1_ratio: 0.35,
            tier2_ratio: 0.12,
            high_relevance_threshold: 0.70,
            low_relevance_threshold: 0.40,
            tier1_demotion_threshold: 30,
        }
    }
}

/// Tier 0 verbatim message count per depth mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TierZeroConfig {
    #[serde(default = "default_8")]
    pub normal: usize,
    #[serde(default = "default_12")]
    pub deep_think: usize,
    #[serde(default = "default_16")]
    pub ultra: usize,
}

impl Default for TierZeroConfig {
    fn default() -> Self {
        Self {
            normal: 8,
            deep_think: 12,
            ultra: 16,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_values() {
        let config = HistoryCompressionConfig::default();
        assert!(config.model.is_none());
        assert!(config.use_cognitive_scoring);
        assert!(config.delta_only_on_resume);
        assert_eq!(config.tier0_messages.normal, 8);
        assert_eq!(config.tier0_messages.deep_think, 12);
        assert_eq!(config.tier0_messages.ultra, 16);
        assert!((config.tier1_ratio - 0.35).abs() < f32::EPSILON);
        assert!((config.tier2_ratio - 0.12).abs() < f32::EPSILON);
        assert!((config.high_relevance_threshold - 0.70).abs() < f64::EPSILON);
        assert!((config.low_relevance_threshold - 0.40).abs() < f64::EPSILON);
        assert_eq!(config.tier1_demotion_threshold, 30);
    }

    #[test]
    fn test_config_roundtrip_json() {
        let json = r#"{
            "model": "claude-haiku-4-5-20251001",
            "useCognitiveScoring": false,
            "tier0Messages": { "normal": 10, "deepThink": 14, "ultra": 20 },
            "tier1Ratio": 0.40
        }"#;
        let config: HistoryCompressionConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model.as_deref(), Some("claude-haiku-4-5-20251001"));
        assert!(!config.use_cognitive_scoring);
        assert_eq!(config.tier0_messages.normal, 10);
        // Unset fields get defaults
        assert!(config.delta_only_on_resume);
        assert!((config.tier2_ratio - 0.12).abs() < f32::EPSILON);
    }

    #[test]
    fn test_empty_json_uses_all_defaults() {
        let config: HistoryCompressionConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config.tier0_messages.normal, 8);
        assert!(config.use_cognitive_scoring);
    }
}
