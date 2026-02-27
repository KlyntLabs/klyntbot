//! Core configuration types: Secret, Config (root composition struct).

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

use super::agents::AgentsConfig;
use super::calendar::CalendarConfig;
use super::channels::ChannelsConfig;
use super::confidence::ConfidenceConfig;
use super::conversation::ConversationConfig;
use super::finance::FinanceConfig;
use super::gateway::GatewayConfig;
use super::learning::LearningConfig;
use super::orchestrator::OrchestratorConfig;
use super::packs::PacksConfig;
use super::plugins::PluginsConfig;
use super::project::ProjectConfig;
use super::providers::{ProviderManagerConfig, ProvidersConfig};
use super::todo::TodoConfig;
use super::tools::ToolsConfig;

/// Expand a leading `~` in a path to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with('~') {
        if let Some(home) = dirs::home_dir() {
            return home.join(path.trim_start_matches("~/"));
        }
    }
    PathBuf::from(path)
}

/// Wrapper that redacts sensitive values in Debug/Display output.
/// Use `.expose()` to access the inner value.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret<T>(T);

impl<T> Secret<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &T {
        &self.0
    }

    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl<T> fmt::Display for Secret<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl Default for Secret<String> {
    fn default() -> Self {
        Self(String::new())
    }
}

impl Secret<String> {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Root configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct Config {
    #[serde(default)]
    pub agents: AgentsConfig,

    #[serde(default)]
    pub channels: ChannelsConfig,

    #[serde(default)]
    pub providers: ProvidersConfig,

    #[serde(default)]
    pub tools: ToolsConfig,

    #[serde(default)]
    pub gateway: GatewayConfig,

    #[serde(default)]
    pub todo: TodoConfig,

    #[serde(default)]
    pub confidence: ConfidenceConfig,

    #[serde(default)]
    pub calendar: CalendarConfig,

    #[serde(default)]
    pub project: ProjectConfig,

    #[serde(default)]
    pub conversation: ConversationConfig,

    #[serde(default)]
    pub learning: LearningConfig,

    #[serde(default)]
    pub finance: FinanceConfig,

    #[serde(default)]
    pub orchestrator: OrchestratorConfig,

    /// Provider manager routing (primary/fallback/classifier)
    #[serde(default)]
    pub provider_manager: ProviderManagerConfig,

    #[serde(default = "default_timezone")]
    pub timezone: String,

    /// Data directory for SQLite + LanceDB storage files (default: ~/.klyntbot).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,

    /// Feature packs (controls which skills and config sections are active).
    #[serde(default)]
    pub packs: PacksConfig,

    /// Plugin system configuration.
    #[serde(default)]
    pub plugins: PluginsConfig,
}

impl Config {
    /// Get the workspace path (expanded)
    pub fn workspace_path(&self) -> PathBuf {
        expand_tilde(&self.agents.defaults.workspace)
    }

    /// Resolve the data directory path, expanding `~` and defaulting to `~/.klyntbot`.
    pub fn data_dir_path(&self) -> PathBuf {
        match &self.data_dir {
            Some(dir) => expand_tilde(dir),
            None => expand_tilde("~/.klyntbot"),
        }
    }

    /// Return all provider configs keyed by name (detection priority order).
    fn all_providers(&self) -> [(&str, &super::providers::ProviderConfig); 12] {
        [
            ("anthropic", &self.providers.anthropic),
            ("openai", &self.providers.openai),
            ("openrouter", &self.providers.openrouter),
            ("deepseek", &self.providers.deepseek),
            ("gemini", &self.providers.gemini),
            ("groq", &self.providers.groq),
            ("vllm", &self.providers.vllm),
            ("zhipu", &self.providers.zhipu),
            ("dashscope", &self.providers.dashscope),
            ("moonshot", &self.providers.moonshot),
            ("minimax", &self.providers.minimax),
            ("aihubmix", &self.providers.aihubmix),
        ]
    }

    /// Detect the active provider.
    ///
    /// Resolution: explicit `agents.defaults.provider` field first,
    /// then auto-detect from which API keys are configured.
    pub fn active_provider_name(&self) -> &str {
        // Check explicit provider field first
        if let Some(ref name) = self.agents.defaults.provider {
            if !name.is_empty() && self.is_provider_configured(name) {
                return name;
            }
        }

        // Fall back to auto-detection: first provider with a non-empty key
        for (name, pc) in &self.all_providers() {
            if !pc.api_key.is_empty() {
                return name;
            }
        }
        "none"
    }

    /// Check if a provider has an API key configured.
    pub fn is_provider_configured(&self, name: &str) -> bool {
        self.all_providers()
            .iter()
            .any(|(n, pc)| *n == name && !pc.api_key.is_empty())
    }

    /// Set the API key for a provider by name.
    pub fn set_provider_key(&mut self, provider_name: &str, key: String) {
        let secret = Secret::new(key);
        match provider_name {
            "anthropic" => self.providers.anthropic.api_key = secret,
            "openai" => self.providers.openai.api_key = secret,
            "openrouter" => self.providers.openrouter.api_key = secret,
            "deepseek" => self.providers.deepseek.api_key = secret,
            "gemini" => self.providers.gemini.api_key = secret,
            "groq" => self.providers.groq.api_key = secret,
            "vllm" => self.providers.vllm.api_key = secret,
            "zhipu" => self.providers.zhipu.api_key = secret,
            "dashscope" => self.providers.dashscope.api_key = secret,
            "moonshot" => self.providers.moonshot.api_key = secret,
            "minimax" => self.providers.minimax.api_key = secret,
            "aihubmix" => self.providers.aihubmix.api_key = secret,
            _ => {}
        }
    }
}

// ============================================================================
// Shared default functions used across multiple section modules
// ============================================================================

/// Auto-detect system timezone, fallback to UTC
fn default_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string())
}

/// Default `true` value for serde defaults — used by many config sections.
pub(crate) fn default_true() -> bool {
    true
}

/// Default semantic threshold — shared by todo search and conversation search.
pub(crate) fn default_semantic_threshold() -> f64 {
    0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_is_empty() {
        let empty_secret: Secret<String> = Secret::default();
        assert!(empty_secret.is_empty());

        let non_empty_secret = Secret::new("value".to_string());
        assert!(!non_empty_secret.is_empty());
    }
}
