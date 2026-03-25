use serde::{Deserialize, Serialize};

use super::core::default_true;

/// Configuration for the Work Context inference engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkContextConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_inference_interval")]
    pub inference_interval_mins: u64,

    #[serde(default = "default_assignment_threshold")]
    pub assignment_threshold: f64,

    #[serde(default = "default_merge_threshold")]
    pub merge_threshold: f64,

    #[serde(default = "default_max_dormancy_days")]
    pub max_dormancy_days: f64,

    #[serde(default = "default_max_active_contexts")]
    pub max_active_contexts: usize,

    #[serde(default = "default_semantic_weight")]
    pub semantic_weight: f64,

    #[serde(default = "default_temporal_weight")]
    pub temporal_weight: f64,

    #[serde(default = "default_resource_weight")]
    pub resource_weight: f64,
}

impl Default for WorkContextConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            inference_interval_mins: default_inference_interval(),
            assignment_threshold: default_assignment_threshold(),
            merge_threshold: default_merge_threshold(),
            max_dormancy_days: default_max_dormancy_days(),
            max_active_contexts: default_max_active_contexts(),
            semantic_weight: default_semantic_weight(),
            temporal_weight: default_temporal_weight(),
            resource_weight: default_resource_weight(),
        }
    }
}

fn default_inference_interval() -> u64 {
    5
}
fn default_assignment_threshold() -> f64 {
    0.55
}
fn default_merge_threshold() -> f64 {
    0.85
}
fn default_max_dormancy_days() -> f64 {
    7.0
}
fn default_max_active_contexts() -> usize {
    50
}
fn default_semantic_weight() -> f64 {
    0.70
}
fn default_temporal_weight() -> f64 {
    0.15
}
fn default_resource_weight() -> f64 {
    0.15
}
