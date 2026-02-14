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

    #[serde(default)]
    pub calendar: CalendarConfig,

    #[serde(default)]
    pub project: ProjectConfig,
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

    /// Get the standardized project store path
    pub fn project_store_path(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klyntbot")
            .join("projects.jsonl")
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

/// Calendar sync configuration (Phase 1 prep for Phase 3)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub username: String,

    #[serde(default)]
    pub password: Secret<String>,

    #[serde(default = "default_caldav_url")]
    pub caldav_url: String,

    #[serde(default = "default_calendar_name")]
    pub calendar_name: String,

    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,

    #[serde(default = "default_conflict_resolution")]
    pub conflict_resolution: String,

    #[serde(default = "default_true")]
    pub auto_sync_due_dates: bool,
}

impl Default for CalendarConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            username: String::new(),
            password: Secret::default(),
            caldav_url: default_caldav_url(),
            calendar_name: default_calendar_name(),
            sync_interval_secs: default_sync_interval_secs(),
            conflict_resolution: default_conflict_resolution(),
            auto_sync_due_dates: true,
        }
    }
}

fn default_caldav_url() -> String {
    "https://caldav.icloud.com".to_string()
}

fn default_calendar_name() -> String {
    "Klyntbot Tasks".to_string()
}

fn default_sync_interval_secs() -> u64 {
    300 // 5 minutes
}

fn default_conflict_resolution() -> String {
    "server_wins".to_string()
}

/// Project management configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    // ========================================================================
    // CalendarConfig tests
    // ========================================================================

    #[test]
    fn test_calendar_config_default() {
        let config = CalendarConfig::default();
        assert!(!config.enabled);
        assert!(config.username.is_empty());
        assert!(config.password.is_empty());
        assert_eq!(config.caldav_url, "https://caldav.icloud.com");
        assert_eq!(config.calendar_name, "Klyntbot Tasks");
        assert_eq!(config.sync_interval_secs, 300);
        assert_eq!(config.conflict_resolution, "server_wins");
        assert!(config.auto_sync_due_dates);
    }

    #[test]
    fn test_calendar_config_secret_redaction() {
        let config = CalendarConfig {
            enabled: true,
            username: "user@example.com".to_string(),
            password: Secret::new("secret123".to_string()),
            caldav_url: "https://caldav.icloud.com".to_string(),
            calendar_name: "Klyntbot".to_string(),
            sync_interval_secs: 300,
            conflict_resolution: "server_wins".to_string(),
            auto_sync_due_dates: true,
        };

        let debug_str = format!("{:?}", config);
        assert!(!debug_str.contains("secret123"));
        assert!(debug_str.contains("[REDACTED]"));
    }

    #[test]
    fn test_calendar_config_serialization_camel_case() {
        let config = CalendarConfig {
            enabled: true,
            username: "user@example.com".to_string(),
            password: Secret::new("pass123".to_string()),
            caldav_url: "https://caldav.icloud.com".to_string(),
            calendar_name: "My Calendar".to_string(),
            sync_interval_secs: 600,
            conflict_resolution: "client_wins".to_string(),
            auto_sync_due_dates: false,
        };

        let json = serde_json::to_string(&config).unwrap();

        // Check camelCase field names
        assert!(json.contains("\"enabled\""));
        assert!(json.contains("\"username\""));
        assert!(json.contains("\"password\""));
        assert!(json.contains("\"caldavUrl\""));
        assert!(json.contains("\"calendarName\""));
        assert!(json.contains("\"syncIntervalSecs\""));
        assert!(json.contains("\"conflictResolution\""));
        assert!(json.contains("\"autoSyncDueDates\""));

        // Check values
        assert!(json.contains("\"user@example.com\""));
        assert!(json.contains("\"pass123\""));
        assert!(json.contains("\"My Calendar\""));
        assert!(json.contains("600"));
        assert!(json.contains("\"client_wins\""));
        assert!(json.contains("false"));
    }

    #[test]
    fn test_calendar_config_deserialization() {
        let json = r#"{
            "enabled": true,
            "username": "test@apple.com",
            "password": "app-password-123",
            "caldavUrl": "https://caldav.icloud.com",
            "calendarName": "Work Tasks",
            "syncIntervalSecs": 900,
            "conflictResolution": "server_wins",
            "autoSyncDueDates": true
        }"#;

        let config: CalendarConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.username, "test@apple.com");
        assert_eq!(config.password.expose(), "app-password-123");
        assert_eq!(config.caldav_url, "https://caldav.icloud.com");
        assert_eq!(config.calendar_name, "Work Tasks");
        assert_eq!(config.sync_interval_secs, 900);
        assert_eq!(config.conflict_resolution, "server_wins");
        assert!(config.auto_sync_due_dates);
    }

    #[test]
    fn test_calendar_config_round_trip() {
        let original = CalendarConfig {
            enabled: true,
            username: "roundtrip@test.com".to_string(),
            password: Secret::new("secure-pass".to_string()),
            caldav_url: "https://caldav.example.com".to_string(),
            calendar_name: "Test Calendar".to_string(),
            sync_interval_secs: 1800,
            conflict_resolution: "merge".to_string(),
            auto_sync_due_dates: false,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CalendarConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(original.enabled, deserialized.enabled);
        assert_eq!(original.username, deserialized.username);
        assert_eq!(original.password.expose(), deserialized.password.expose());
        assert_eq!(original.caldav_url, deserialized.caldav_url);
        assert_eq!(original.calendar_name, deserialized.calendar_name);
        assert_eq!(original.sync_interval_secs, deserialized.sync_interval_secs);
        assert_eq!(
            original.conflict_resolution,
            deserialized.conflict_resolution
        );
        assert_eq!(
            original.auto_sync_due_dates,
            deserialized.auto_sync_due_dates
        );
    }

    #[test]
    fn test_secret_is_empty() {
        let empty_secret: Secret<String> = Secret::default();
        assert!(empty_secret.is_empty());

        let non_empty_secret = Secret::new("value".to_string());
        assert!(!non_empty_secret.is_empty());
    }

    // ========================================================================
    // Config integration tests
    // ========================================================================

    #[test]
    fn test_config_includes_calendar() {
        let config = Config::default();
        assert!(!config.calendar.enabled);
        assert_eq!(config.calendar.caldav_url, "https://caldav.icloud.com");
    }

    #[test]
    fn test_config_calendar_serialization() {
        let mut config = Config::default();
        config.calendar.enabled = true;
        config.calendar.username = "test@example.com".to_string();
        config.calendar.password = Secret::new("password123".to_string());

        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("\"calendar\""));
        assert!(json.contains("\"enabled\": true"));
        assert!(json.contains("\"username\": \"test@example.com\""));
        assert!(json.contains("\"password\": \"password123\""));
    }
}
