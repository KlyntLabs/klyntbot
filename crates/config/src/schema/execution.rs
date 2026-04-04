//! Execution pipeline configuration — budget-bounded model.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    /// Per-skill budget overrides. Keys are skill names.
    #[serde(default)]
    pub skill_budgets: HashMap<String, SkillBudgetOverride>,
}

/// Per-skill budget override in config.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillBudgetOverride {
    pub normal_tokens: Option<u64>,
    pub normal_turns: Option<u32>,
    pub deep_multiplier: Option<f32>,
    pub ultra_multiplier: Option<f32>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            safety_timeout_secs: default_safety_timeout(),
            adaptive_depth: true,
            skill_budgets: HashMap::new(),
        }
    }
}

fn default_safety_timeout() -> u64 {
    600
}
