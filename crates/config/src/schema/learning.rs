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
    /// Active recall configuration (semantic grading, graph propagation, answer modes).
    #[serde(default)]
    pub active_recall: ActiveRecallConfig,
}

impl Default for LearningConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            analysis_interval_secs: default_learning_analysis_interval(),
            min_threshold: default_min_threshold(),
            max_threshold: default_max_threshold(),
            min_outcomes_for_adaptation: default_min_outcomes_for_adaptation(),
            active_recall: ActiveRecallConfig::default(),
        }
    }
}

/// Active recall configuration — semantic grading thresholds, knowledge graph propagation,
/// and default answer mode for the flashcard review system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveRecallConfig {
    #[serde(default = "default_semantic_auto_accept")]
    pub semantic_auto_accept_threshold: f64,
    #[serde(default = "default_semantic_auto_fail")]
    pub semantic_auto_fail_threshold: f64,
    #[serde(default = "default_graph_propagation_strength")]
    pub graph_propagation_strength: String,
    #[serde(default = "default_graph_propagation_daily_cap")]
    pub graph_propagation_daily_cap: usize,
    #[serde(default = "default_answer_mode")]
    pub default_answer_mode: String,
}

fn default_semantic_auto_accept() -> f64 {
    0.78
}
fn default_semantic_auto_fail() -> f64 {
    0.45
}
fn default_graph_propagation_strength() -> String {
    "gentle".into()
}
fn default_graph_propagation_daily_cap() -> usize {
    15
}
fn default_answer_mode() -> String {
    "auto".into()
}

impl Default for ActiveRecallConfig {
    fn default() -> Self {
        Self {
            semantic_auto_accept_threshold: default_semantic_auto_accept(),
            semantic_auto_fail_threshold: default_semantic_auto_fail(),
            graph_propagation_strength: default_graph_propagation_strength(),
            graph_propagation_daily_cap: default_graph_propagation_daily_cap(),
            default_answer_mode: default_answer_mode(),
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
