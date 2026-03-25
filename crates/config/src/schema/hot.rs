//! Hot-reloadable configuration subset.
//!
//! Fields here take effect immediately without restart. The full `Config`
//! still requires restart for structural changes (channels, provider init,
//! feature enable/disable flags).

use super::Config;

/// Hot-reloadable subset of Config.
///
/// Extracted from `Config` via `From<&Config>`. Shared as
/// `Arc<RwLock<HotConfig>>` between AppCore and the agent pipeline.
#[derive(Debug, Clone, PartialEq)]
pub struct HotConfig {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub max_tool_iterations: u32,
    pub pipeline_timeout_secs: u64,
    pub monthly_budget_usd: Option<f64>,
}

/// Describes which fields changed between two HotConfig snapshots.
#[derive(Debug, Default)]
pub struct HotConfigDiff {
    pub model_changed: bool,
    pub temperature_changed: bool,
    pub max_tokens_changed: bool,
    pub max_tool_iterations_changed: bool,
    pub pipeline_timeout_changed: bool,
    pub budget_changed: bool,
}

impl HotConfigDiff {
    pub fn has_changes(&self) -> bool {
        self.model_changed
            || self.temperature_changed
            || self.max_tokens_changed
            || self.max_tool_iterations_changed
            || self.pipeline_timeout_changed
            || self.budget_changed
    }
}

impl From<&Config> for HotConfig {
    fn from(config: &Config) -> Self {
        Self {
            model: config.agents.defaults.model.clone(),
            temperature: config.agents.defaults.temperature,
            max_tokens: config.agents.defaults.max_tokens,
            max_tool_iterations: config.agents.defaults.max_tool_iterations,
            pipeline_timeout_secs: config.agents.defaults.pipeline_timeout_secs,
            monthly_budget_usd: config.agents.monthly_budget_usd,
        }
    }
}

impl HotConfig {
    /// Compare two snapshots and return which fields changed.
    pub fn diff(&self, other: &HotConfig) -> HotConfigDiff {
        HotConfigDiff {
            model_changed: self.model != other.model,
            temperature_changed: (self.temperature - other.temperature).abs() > f32::EPSILON,
            max_tokens_changed: self.max_tokens != other.max_tokens,
            max_tool_iterations_changed: self.max_tool_iterations != other.max_tool_iterations,
            pipeline_timeout_changed: self.pipeline_timeout_secs != other.pipeline_timeout_secs,
            budget_changed: self.monthly_budget_usd != other.monthly_budget_usd,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Config;

    #[test]
    fn test_hot_config_from_config() {
        let mut config = Config::default();
        config.agents.defaults.model = "test-model".to_string();
        config.agents.defaults.temperature = 0.5;
        config.agents.defaults.max_tokens = 4096;
        config.agents.defaults.max_tool_iterations = 10;
        config.agents.defaults.pipeline_timeout_secs = 120;
        config.agents.monthly_budget_usd = Some(50.0);

        let hot = HotConfig::from(&config);
        assert_eq!(hot.model, "test-model");
        assert_eq!(hot.temperature, 0.5);
        assert_eq!(hot.max_tokens, 4096);
        assert_eq!(hot.max_tool_iterations, 10);
        assert_eq!(hot.pipeline_timeout_secs, 120);
        assert_eq!(hot.monthly_budget_usd, Some(50.0));
    }

    #[test]
    fn test_hot_config_diff_detects_model_change() {
        let a = HotConfig {
            model: "model-a".into(),
            ..HotConfig::from(&Config::default())
        };
        let b = HotConfig {
            model: "model-b".into(),
            ..HotConfig::from(&Config::default())
        };
        assert!(a.diff(&b).model_changed);
        assert!(!a.diff(&b).temperature_changed);
    }

    #[test]
    fn test_hot_config_diff_no_changes() {
        let a = HotConfig::from(&Config::default());
        let b = HotConfig::from(&Config::default());
        let diff = a.diff(&b);
        assert!(!diff.has_changes());
    }
}
