//! Confidence evaluation configuration.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::core::default_true;

/// Confidence evaluation configuration (LLM-driven decision engine)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceConfig {
    /// Threshold below which ask_user is triggered (default: 0.7)
    #[serde(default = "default_confidence_threshold")]
    pub threshold: f32,
    /// Enable/disable confidence evaluation (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Per-tool confidence threshold overrides (tool_name → threshold).
    /// Tools not listed here fall back to the global `threshold`.
    #[serde(default)]
    pub tool_overrides: HashMap<String, f32>,
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            threshold: default_confidence_threshold(),
            enabled: true,
            tool_overrides: HashMap::new(),
        }
    }
}

fn default_confidence_threshold() -> f32 {
    0.7
}
