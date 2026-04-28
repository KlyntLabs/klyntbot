use serde::{Deserialize, Serialize};

/// Configuration for the autotuner self-optimization system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTunerConfig {
    /// Cron expression for the nightly experiment cycle (default: "0 2 * * *").
    #[serde(default = "default_schedule")]
    pub schedule: String,

    /// Minimum messages before a trial is eligible for promotion.
    #[serde(default = "default_min_messages")]
    pub min_messages_for_promotion: u32,

    /// Number of consecutive regression days before auto-rollback.
    #[serde(default = "default_rollback_days")]
    pub rollback_after_days: u8,

    // Promotion constraint thresholds
    #[serde(default = "default_correction_improvement")]
    pub min_correction_improvement: f64,
    #[serde(default = "default_max_token_increase")]
    pub max_token_cost_increase: f64,
    #[serde(default = "default_max_response_time_increase")]
    pub max_response_time_increase: f64,
    #[serde(default = "default_max_stability_decrease")]
    pub max_routing_stability_decrease: f64,
    #[serde(default = "default_max_relevance_decrease")]
    pub max_memory_relevance_decrease: f64,

    // Phase 2 constraint thresholds
    #[serde(default = "default_max_retrieval_precision_drop")]
    pub max_retrieval_precision_drop: f64,
    #[serde(default = "default_max_correction_rate_increase")]
    pub max_correction_rate_increase: f64,
    #[serde(default = "default_max_promotion_accuracy_drop")]
    pub max_promotion_accuracy_drop: f64,
}

impl Default for AutoTunerConfig {
    fn default() -> Self {
        Self {
            schedule: default_schedule(),
            min_messages_for_promotion: default_min_messages(),
            rollback_after_days: default_rollback_days(),
            min_correction_improvement: default_correction_improvement(),
            max_token_cost_increase: default_max_token_increase(),
            max_response_time_increase: default_max_response_time_increase(),
            max_routing_stability_decrease: default_max_stability_decrease(),
            max_memory_relevance_decrease: default_max_relevance_decrease(),
            max_retrieval_precision_drop: default_max_retrieval_precision_drop(),
            max_correction_rate_increase: default_max_correction_rate_increase(),
            max_promotion_accuracy_drop: default_max_promotion_accuracy_drop(),
        }
    }
}

fn default_schedule() -> String {
    "0 0 2 * * *".to_string()
}
fn default_min_messages() -> u32 {
    50
}
fn default_rollback_days() -> u8 {
    3
}
fn default_correction_improvement() -> f64 {
    0.05
}
fn default_max_token_increase() -> f64 {
    0.08
}
fn default_max_response_time_increase() -> f64 {
    0.15
}
fn default_max_stability_decrease() -> f64 {
    0.10
}
fn default_max_relevance_decrease() -> f64 {
    0.05
}
fn default_max_retrieval_precision_drop() -> f64 {
    0.05
}
fn default_max_correction_rate_increase() -> f64 {
    0.03
}
fn default_max_promotion_accuracy_drop() -> f64 {
    0.05
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = AutoTunerConfig::default();
        assert_eq!(config.schedule, "0 0 2 * * *");
        assert_eq!(config.min_messages_for_promotion, 50);
        assert_eq!(config.rollback_after_days, 3);
    }

    #[test]
    fn camel_case_serde_roundtrip() {
        let config = AutoTunerConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("minMessagesForPromotion"));
        assert!(json.contains("maxTokenCostIncrease"));
        let back: AutoTunerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.min_messages_for_promotion, 50);
    }
}
