//! Execution pipeline configuration — budget-bounded model.

use serde::{Deserialize, Serialize};

use super::core::default_true;

/// Execution pipeline configuration. Replaces the old `pipeline_timeout_secs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionConfig {
    /// Safety wall-clock timeout in seconds. Catches deadlocks only.
    /// Should never fire in normal operation. Default: 600.
    #[serde(default = "default_safety_timeout")]
    pub safety_timeout_secs: u64,

    /// Enable adaptive depth suggestions from Mirror. Default: true.
    #[serde(default = "default_true")]
    pub adaptive_depth: bool,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            safety_timeout_secs: default_safety_timeout(),
            adaptive_depth: true,
        }
    }
}

fn default_safety_timeout() -> u64 {
    600
}
