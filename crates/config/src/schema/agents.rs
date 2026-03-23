//! Agent configuration: AgentsConfig, AgentDefaults.

use serde::{Deserialize, Serialize};

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct AgentsConfig {
    #[serde(default)]
    pub defaults: AgentDefaults,

    /// Optional monthly LLM cost budget in USD.
    /// When set, the system emits warnings at 80% and 100% of budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monthly_budget_usd: Option<f64>,

    /// Optional directory for runtime-loaded external skills.
    /// Defaults to `~/.klyntbot/.agents/skills/` if not set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skills_dir: Option<String>,
}

/// Default agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefaults {
    #[serde(default = "default_workspace")]
    pub workspace: String,

    #[serde(default = "default_model")]
    pub model: String,

    /// Explicit active provider name (e.g., "anthropic", "deepseek").
    /// When set, takes priority over model-name auto-detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,

    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    #[serde(default = "default_temperature")]
    pub temperature: f32,

    #[serde(default = "default_max_iterations")]
    pub max_tool_iterations: u32,

    #[serde(default = "default_max_concurrent_subagents")]
    pub max_concurrent_subagents: usize,

    /// Maximum wall-clock time for a single pipeline execution (seconds).
    /// Default: 300 (5 minutes). Set to 0 to disable.
    #[serde(default = "default_pipeline_timeout_secs")]
    pub pipeline_timeout_secs: u64,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        Self {
            workspace: default_workspace(),
            model: default_model(),
            provider: None,
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            max_tool_iterations: default_max_iterations(),
            max_concurrent_subagents: default_max_concurrent_subagents(),
            pipeline_timeout_secs: default_pipeline_timeout_secs(),
        }
    }
}

/// Default model identifier used when no model is configured.
pub const DEFAULT_MODEL: &str = "anthropic/claude-opus-4-5";

fn default_workspace() -> String {
    "~/.klyntbot/workspace".to_string()
}

fn default_model() -> String {
    DEFAULT_MODEL.to_string()
}

fn default_max_tokens() -> u32 {
    8192
}

fn default_temperature() -> f32 {
    0.7
}

fn default_max_iterations() -> u32 {
    20
}

fn default_max_concurrent_subagents() -> usize {
    3
}

fn default_pipeline_timeout_secs() -> u64 {
    300
}

/// Configuration for the skill discovery system.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SkillConfig {
    /// Additional directories to scan for skills (beyond data_dir/skills/).
    #[serde(default)]
    pub extra_skill_dirs: Vec<String>,

    /// Orchestrator selection threshold (semantic score >= this to consider).
    #[serde(default = "default_orchestrator_threshold")]
    pub orchestrator_semantic_threshold: f64,

    /// Per-message skill activation threshold.
    #[serde(default = "default_activation_threshold")]
    pub activation_threshold: f64,

    /// Max non-orchestrator skills activated per message.
    #[serde(default = "default_max_activated_skills")]
    pub max_activated_skills: usize,
}

fn default_orchestrator_threshold() -> f64 {
    0.5
}
fn default_activation_threshold() -> f64 {
    0.4
}
fn default_max_activated_skills() -> usize {
    3
}
