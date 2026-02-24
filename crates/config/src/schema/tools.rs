//! Tools configuration: ToolsConfig, PermissionsConfig, WebToolsConfig.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::core::Secret;

/// Tools configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct ToolsConfig {
    #[serde(default)]
    pub web: WebToolsConfig,

    #[serde(default)]
    pub browser: BrowserConfig,

    #[serde(default)]
    pub restrict_to_workspace: bool,

    /// Optional per-channel permission levels for tool access control.
    /// Keys are channel names (e.g., "telegram", "discord", "cli").
    /// Values are permission levels: "readOnly", "standard", "elevated", "admin".
    /// When absent, all tools are allowed on all channels.
    #[serde(default)]
    pub permissions: Option<PermissionsConfig>,
}

/// Permission configuration for tool access control.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionsConfig {
    /// Default permission level for channels not explicitly listed.
    #[serde(default = "default_permission_level")]
    pub default_level: String,

    /// Per-channel permission level overrides.
    #[serde(default)]
    pub channels: HashMap<String, String>,
}

fn default_permission_level() -> String {
    "standard".to_string()
}

/// Web tools configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebToolsConfig {
    #[serde(default)]
    pub brave_api_key: Secret<String>,

    #[serde(default = "default_web_max_results")]
    pub max_results: u8,
}

impl Default for WebToolsConfig {
    fn default() -> Self {
        Self {
            brave_api_key: Secret::default(),
            max_results: default_web_max_results(),
        }
    }
}

fn default_web_max_results() -> u8 {
    5
}

/// Controls how the browser tool guards write actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum TrustLevel {
    /// Ask the user before every write action.
    Strict,
    /// (Default) Ask the user only for detected dangerous actions.
    #[default]
    Autonomous,
    /// Execute all actions immediately without any confirmation gate.
    Full,
}

/// Browser automation tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub trust_level: TrustLevel,

    #[serde(default = "default_session_timeout_secs")]
    pub session_timeout_secs: u64,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            trust_level: TrustLevel::default(),
            session_timeout_secs: default_session_timeout_secs(),
        }
    }
}

fn default_session_timeout_secs() -> u64 {
    300
}

#[cfg(test)]
mod browser_config_tests {
    use super::*;

    #[test]
    fn test_browser_config_defaults() {
        let cfg = BrowserConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.trust_level, TrustLevel::Autonomous);
        assert_eq!(cfg.session_timeout_secs, 300);
    }

    #[test]
    fn test_browser_config_serde_roundtrip() {
        let json = r#"{"enabled": true, "trustLevel": "strict", "sessionTimeoutSecs": 60}"#;
        let cfg: BrowserConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.trust_level, TrustLevel::Strict);
        assert_eq!(cfg.session_timeout_secs, 60);
        // Also verify serialization produces camelCase
        let serialized = serde_json::to_string(&cfg).unwrap();
        assert!(serialized.contains("sessionTimeoutSecs"));
    }

    #[test]
    fn test_trust_level_serde() {
        assert_eq!(
            serde_json::from_str::<TrustLevel>(r#""autonomous""#).unwrap(),
            TrustLevel::Autonomous
        );
        assert_eq!(
            serde_json::from_str::<TrustLevel>(r#""strict""#).unwrap(),
            TrustLevel::Strict
        );
        assert_eq!(
            serde_json::from_str::<TrustLevel>(r#""full""#).unwrap(),
            TrustLevel::Full
        );
    }

    #[test]
    fn test_tools_config_browser_default_trust() {
        let cfg = ToolsConfig::default();
        assert!(!cfg.browser.enabled);
        assert_eq!(cfg.browser.trust_level, TrustLevel::Autonomous);
    }
}
