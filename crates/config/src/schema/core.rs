//! Core configuration types: Secret, Config, AgentsConfig, ToolsConfig, GatewayConfig, TodoConfig.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

use super::channels::ChannelsConfig;
use super::providers::ProvidersConfig;

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
}

impl Config {
    /// Get the workspace path (expanded)
    pub fn workspace_path(&self) -> PathBuf {
        let path = &self.agents.defaults.workspace;
        if path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                home.join(path.trim_start_matches("~/"))
            } else {
                PathBuf::from(path)
            }
        } else {
            PathBuf::from(path)
        }
    }

    /// Detect the active provider based on which API keys are configured.
    pub fn active_provider_name(&self) -> &str {
        if !self.providers.anthropic.api_key.is_empty() {
            "anthropic"
        } else if !self.providers.openai.api_key.is_empty() {
            "openai"
        } else if !self.providers.openrouter.api_key.is_empty() {
            "openrouter"
        } else if !self.providers.deepseek.api_key.is_empty() {
            "deepseek"
        } else {
            "none"
        }
    }

    /// Get the standardized todo store path (P0 fix for path inconsistency)
    pub fn todo_store_path(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klyntbot")
            .join("todos.jsonl")
    }

    /// Set the API key for a provider by name.
    pub fn set_provider_key(&mut self, provider_name: &str, key: String) {
        match provider_name {
            "anthropic" => self.providers.anthropic.api_key = Secret::new(key),
            "openai" => self.providers.openai.api_key = Secret::new(key),
            "deepseek" => self.providers.deepseek.api_key = Secret::new(key),
            "gemini" => self.providers.gemini.api_key = Secret::new(key),
            "openrouter" => self.providers.openrouter.api_key = Secret::new(key),
            _ => {}
        }
    }
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct AgentsConfig {
    #[serde(default)]
    pub defaults: AgentDefaults,
}

/// Default agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentDefaults {
    #[serde(default = "default_workspace")]
    pub workspace: String,

    #[serde(default = "default_model")]
    pub model: String,

    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    #[serde(default = "default_temperature")]
    pub temperature: f32,

    #[serde(default = "default_max_iterations")]
    pub max_tool_iterations: u32,
}

impl Default for AgentDefaults {
    fn default() -> Self {
        Self {
            workspace: default_workspace(),
            model: default_model(),
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            max_tool_iterations: default_max_iterations(),
        }
    }
}

fn default_workspace() -> String {
    "~/.klyntbot/workspace".to_string()
}

fn default_model() -> String {
    "anthropic/claude-opus-4-5".to_string()
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

/// Gateway/HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayConfig {
    #[serde(default = "default_gateway_host")]
    pub host: String,

    #[serde(default = "default_gateway_port")]
    pub port: u16,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            host: default_gateway_host(),
            port: default_gateway_port(),
        }
    }
}

fn default_gateway_host() -> String {
    "0.0.0.0".to_string()
}

fn default_gateway_port() -> u16 {
    18790
}

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

/// Todo system configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoConfig {
    #[serde(default)]
    pub notifications: TodoNotificationConfig,
    #[serde(default)]
    pub focus: TodoFocusConfig,
}

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
    /// Path to decision log file (default: ~/.klyntbot/decision_log.jsonl)
    #[serde(default)]
    pub log_path: Option<PathBuf>,
}

impl Default for ConfidenceConfig {
    fn default() -> Self {
        Self {
            threshold: default_confidence_threshold(),
            enabled: true,
            log_path: None,
        }
    }
}

/// Todo notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoNotificationConfig {
    #[serde(default = "default_notification_targets")]
    pub targets: Vec<String>,
    #[serde(default = "default_true")]
    pub focus_reminders: bool,
    #[serde(default = "default_true")]
    pub daily_digest: bool,
    #[serde(default = "default_digest_time")]
    pub daily_digest_time: String,
}

impl Default for TodoNotificationConfig {
    fn default() -> Self {
        Self {
            targets: vec!["os_native".to_string()],
            focus_reminders: true,
            daily_digest: true,
            daily_digest_time: default_digest_time(),
        }
    }
}

/// Todo focus mode configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoFocusConfig {
    #[serde(default = "default_max_slots")]
    pub max_slots: usize,
    #[serde(default = "default_deadline_hours")]
    pub deadline_hours: u64,
}

impl Default for TodoFocusConfig {
    fn default() -> Self {
        Self {
            max_slots: default_max_slots(),
            deadline_hours: default_deadline_hours(),
        }
    }
}

fn default_notification_targets() -> Vec<String> {
    vec!["os_native".to_string()]
}

fn default_confidence_threshold() -> f32 {
    0.7
}

pub(crate) fn default_true() -> bool {
    true
}

fn default_digest_time() -> String {
    "09:00".to_string()
}

fn default_max_slots() -> usize {
    3
}

fn default_deadline_hours() -> u64 {
    18
}
