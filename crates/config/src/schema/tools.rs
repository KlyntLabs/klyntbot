//! Tools configuration: ToolsConfig, PermissionsConfig, WebToolsConfig, ExecToolConfig.

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
    pub exec: ExecToolConfig,

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

/// Exec tool configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecToolConfig {
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    #[serde(default)]
    pub allowed_commands: Vec<String>,
}

impl Default for ExecToolConfig {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
            allowed_commands: Vec::new(),
        }
    }
}

fn default_timeout() -> u64 {
    60
}

fn default_web_max_results() -> u8 {
    5
}
