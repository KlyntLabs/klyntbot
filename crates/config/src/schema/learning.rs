//! Learning system configuration (adaptive confidence thresholds).

use serde::{Deserialize, Serialize};

use super::core::default_true;

/// Learning system configuration (adaptive confidence thresholds).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningConfig {
    /// Enable/disable the learning system (default: true).
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// How often the background analysis loop runs, in seconds (default: 3600 = 1 hour).
    #[serde(default = "default_learning_analysis_interval")]
    pub analysis_interval_secs: u64,
    /// Lower bound for adaptive threshold (default: 0.4).
    #[serde(default = "default_min_threshold")]
    pub min_threshold: f32,
    /// Upper bound for adaptive threshold (default: 0.9).
    #[serde(default = "default_max_threshold")]
    pub max_threshold: f32,
    /// Minimum outcomes required before threshold adaptation (default: 50).
    #[serde(default = "default_min_outcomes_for_adaptation")]
    pub min_outcomes_for_adaptation: usize,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            analysis_interval_secs: default_learning_analysis_interval(),
            min_threshold: default_min_threshold(),
            max_threshold: default_max_threshold(),
            min_outcomes_for_adaptation: default_min_outcomes_for_adaptation(),
        }
    }
}

fn default_learning_analysis_interval() -> u64 {
    3600
}

fn default_min_threshold() -> f32 {
    0.4
}

fn default_max_threshold() -> f32 {
    0.9
}

fn default_min_outcomes_for_adaptation() -> usize {
    50
}
