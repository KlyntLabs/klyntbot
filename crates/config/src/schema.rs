//! Configuration schema using serde.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

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

/// Channels configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsConfig {
    #[serde(default)]
    pub telegram: TelegramConfig,

    #[serde(default)]
    pub discord: DiscordConfig,

    #[serde(default)]
    pub whatsapp: WhatsAppConfig,

    #[serde(default)]
    pub slack: SlackConfig,

    #[serde(default)]
    pub email: EmailConfig,

    #[serde(default)]
    pub qq: QQConfig,

    #[serde(default)]
    pub feishu: FeishuConfig,

    #[serde(default)]
    pub dingtalk: DingTalkConfig,

    #[serde(default)]
    pub mochat: MochatConfig,
}

/// Telegram channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct TelegramConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub token: Secret<String>,

    #[serde(default)]
    pub allow_from: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub proxy: Option<String>,
}

/// Discord channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscordConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub token: Secret<String>,

    #[serde(default)]
    pub allow_from: Vec<String>,

    #[serde(default = "default_discord_gateway_url")]
    pub gateway_url: String,

    #[serde(default = "default_discord_intents")]
    pub intents: u32,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            token: Secret::default(),
            allow_from: Vec::new(),
            gateway_url: default_discord_gateway_url(),
            intents: default_discord_intents(),
        }
    }
}

fn default_discord_gateway_url() -> String {
    "wss://gateway.discord.gg/?v=10&encoding=json".to_string()
}

fn default_discord_intents() -> u32 {
    37377 // GUILD_MESSAGES | DIRECT_MESSAGES | MESSAGE_CONTENT
}

/// WhatsApp channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhatsAppConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_bridge_url")]
    pub bridge_url: String,

    #[serde(default)]
    pub allow_from: Vec<String>,
}

impl Default for WhatsAppConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bridge_url: default_bridge_url(),
            allow_from: Vec::new(),
        }
    }
}

fn default_bridge_url() -> String {
    "ws://localhost:3001".to_string()
}

/// Slack channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlackConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub bot_token: Secret<String>,

    #[serde(default)]
    pub app_token: Secret<String>,

    #[serde(default)]
    pub allow_from: Vec<String>,

    #[serde(default = "default_slack_mode")]
    pub mode: String,

    #[serde(default = "default_slack_group_policy")]
    pub group_policy: String,

    #[serde(default)]
    pub group_allow_from: Vec<String>,

    #[serde(default)]
    pub dm: SlackDmConfig,
}

impl Default for SlackConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bot_token: Secret::default(),
            app_token: Secret::default(),
            allow_from: Vec::new(),
            mode: default_slack_mode(),
            group_policy: default_slack_group_policy(),
            group_allow_from: Vec::new(),
            dm: SlackDmConfig::default(),
        }
    }
}

/// Slack DM-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SlackDmConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub allow_from: Vec<String>,
}

/// Email channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub imap_host: String,

    #[serde(default = "default_imap_port")]
    pub imap_port: u16,

    #[serde(default)]
    pub imap_username: String,

    #[serde(default)]
    pub imap_password: Secret<String>,

    #[serde(default = "default_imap_mailbox")]
    pub imap_mailbox: String,

    #[serde(default = "default_imap_use_ssl")]
    pub imap_use_ssl: bool,

    #[serde(default)]
    pub smtp_host: String,

    #[serde(default = "default_smtp_port")]
    pub smtp_port: u16,

    #[serde(default)]
    pub smtp_username: String,

    #[serde(default)]
    pub smtp_password: Secret<String>,

    #[serde(default = "default_smtp_use_tls")]
    pub smtp_use_tls: bool,

    #[serde(default)]
    pub smtp_use_ssl: bool,

    #[serde(default)]
    pub from_address: String,

    #[serde(default)]
    pub allow_from: Vec<String>,

    #[serde(default)]
    pub consent_granted: bool,

    #[serde(default = "default_auto_reply_enabled")]
    pub auto_reply_enabled: bool,

    #[serde(default = "default_max_body_chars")]
    pub max_body_chars: u32,

    #[serde(default = "default_mark_seen")]
    pub mark_seen: bool,

    #[serde(default = "default_poll_interval_seconds")]
    pub poll_interval_seconds: u32,

    #[serde(default = "default_subject_prefix")]
    pub subject_prefix: String,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            imap_host: String::new(),
            imap_port: default_imap_port(),
            imap_username: String::new(),
            imap_password: Secret::default(),
            imap_mailbox: default_imap_mailbox(),
            imap_use_ssl: default_imap_use_ssl(),
            smtp_host: String::new(),
            smtp_port: default_smtp_port(),
            smtp_username: String::new(),
            smtp_password: Secret::default(),
            smtp_use_tls: default_smtp_use_tls(),
            smtp_use_ssl: false,
            from_address: String::new(),
            allow_from: Vec::new(),
            consent_granted: false,
            auto_reply_enabled: default_auto_reply_enabled(),
            max_body_chars: default_max_body_chars(),
            mark_seen: default_mark_seen(),
            poll_interval_seconds: default_poll_interval_seconds(),
            subject_prefix: default_subject_prefix(),
        }
    }
}

fn default_imap_port() -> u16 {
    993
}

fn default_smtp_port() -> u16 {
    587
}

fn default_imap_mailbox() -> String {
    "INBOX".to_string()
}

fn default_imap_use_ssl() -> bool {
    true
}

fn default_smtp_use_tls() -> bool {
    true
}

fn default_auto_reply_enabled() -> bool {
    true
}

fn default_max_body_chars() -> u32 {
    12000
}

fn default_mark_seen() -> bool {
    true
}

fn default_poll_interval_seconds() -> u32 {
    30
}

fn default_subject_prefix() -> String {
    "Re: ".to_string()
}

fn default_slack_mode() -> String {
    "socket".to_string()
}

fn default_slack_group_policy() -> String {
    "none".to_string()
}

/// QQ channel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct QQConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub app_id: String,

    #[serde(default)]
    pub secret: Secret<String>,

    #[serde(default)]
    pub allow_from: Vec<String>,
}

/// Feishu/Lark channel configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeishuConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub app_id: String,

    #[serde(default)]
    pub app_secret: Secret<String>,

    #[serde(default)]
    pub encrypt_key: Secret<String>,

    #[serde(default)]
    pub verification_token: String,

    #[serde(default)]
    pub allow_from: Vec<String>,
}

/// DingTalk channel configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DingTalkConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub client_id: String,

    #[serde(default)]
    pub client_secret: Secret<String>,

    #[serde(default)]
    pub allow_from: Vec<String>,
}

/// Mochat channel configuration (pragmatic subset)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MochatConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default = "default_mochat_base_url")]
    pub base_url: String,

    #[serde(default)]
    pub socket_url: String,

    #[serde(default)]
    pub claw_token: Secret<String>,

    #[serde(default)]
    pub agent_user_id: String,

    #[serde(default)]
    pub sessions: Vec<String>,

    #[serde(default)]
    pub panels: Vec<String>,

    #[serde(default)]
    pub allow_from: Vec<String>,
}

impl Default for MochatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            base_url: default_mochat_base_url(),
            socket_url: String::new(),
            claw_token: Secret::default(),
            agent_user_id: String::new(),
            sessions: Vec::new(),
            panels: Vec::new(),
            allow_from: Vec::new(),
        }
    }
}

fn default_mochat_base_url() -> String {
    "https://mochat.io".to_string()
}

/// LLM providers configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersConfig {
    #[serde(default)]
    pub anthropic: ProviderConfig,

    #[serde(default)]
    pub openai: ProviderConfig,

    #[serde(default)]
    pub openrouter: ProviderConfig,

    #[serde(default)]
    pub deepseek: ProviderConfig,

    #[serde(default)]
    pub gemini: ProviderConfig,

    #[serde(default)]
    pub groq: ProviderConfig,

    #[serde(default)]
    pub vllm: ProviderConfig,

    #[serde(default)]
    pub zhipu: ProviderConfig,

    #[serde(default)]
    pub dashscope: ProviderConfig,

    #[serde(default)]
    pub moonshot: ProviderConfig,

    #[serde(default)]
    pub minimax: ProviderConfig,

    #[serde(default)]
    pub aihubmix: ProviderConfig,
}

/// Individual provider configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    #[serde(default)]
    pub api_key: Secret<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_base: Option<String>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<HashMap<String, String>>,
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

fn default_true() -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.agents.defaults.model, "anthropic/claude-opus-4-5");
        assert_eq!(config.agents.defaults.max_tokens, 8192);
        assert_eq!(config.agents.defaults.temperature, 0.7);
        assert_eq!(config.agents.defaults.max_tool_iterations, 20);
        assert_eq!(config.agents.defaults.workspace, "~/.klyntbot/workspace");
    }

    #[test]
    fn test_config_workspace_path_tilde_expansion() {
        let mut config = Config::default();
        config.agents.defaults.workspace = "~/.klyntbot/workspace".to_string();

        let workspace = config.workspace_path();
        assert!(workspace.is_absolute());
        assert!(workspace.to_string_lossy().contains(".klyntbot"));
        assert!(workspace.to_string_lossy().contains("workspace"));
    }

    #[test]
    fn test_config_workspace_path_absolute() {
        let mut config = Config::default();
        config.agents.defaults.workspace = "/absolute/path/workspace".to_string();

        let workspace = config.workspace_path();
        assert_eq!(workspace, PathBuf::from("/absolute/path/workspace"));
    }

    #[test]
    fn test_agent_defaults_serialization() {
        let defaults = AgentDefaults::default();
        let json = serde_json::to_value(&defaults).unwrap();

        assert_eq!(json["model"], "anthropic/claude-opus-4-5");
        assert_eq!(json["maxTokens"], 8192);
        let temp = json["temperature"].as_f64().unwrap();
        assert!(
            (temp - 0.7).abs() < 0.01,
            "temperature should be ~0.7, got {}",
            temp
        );
        assert_eq!(json["maxToolIterations"], 20);
    }

    #[test]
    fn test_telegram_config_default() {
        let config = TelegramConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.token.expose(), "");
        assert_eq!(config.allow_from.len(), 0);
        assert!(config.proxy.is_none());
    }

    #[test]
    fn test_telegram_config_serialization() {
        let config = TelegramConfig {
            enabled: true,
            token: Secret::new("bot123".to_string()),
            allow_from: vec!["user1".to_string(), "user2".to_string()],
            proxy: Some("socks5://localhost:1080".to_string()),
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["token"], "bot123");
        assert_eq!(json["allowFrom"][0], "user1");
        assert_eq!(json["proxy"], "socks5://localhost:1080");
    }

    #[test]
    fn test_discord_config_default() {
        let config = DiscordConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.token.expose(), "");
        assert_eq!(config.allow_from.len(), 0);
    }

    #[test]
    fn test_whatsapp_config_default() {
        let config = WhatsAppConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.bridge_url, "ws://localhost:3001");
        assert_eq!(config.allow_from.len(), 0);
    }

    #[test]
    fn test_slack_config_default() {
        let config = SlackConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.bot_token.expose(), "");
        assert_eq!(config.app_token.expose(), "");
    }

    #[test]
    fn test_email_config_default() {
        let config = EmailConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.imap_port, 993);
        assert_eq!(config.smtp_port, 587);
        assert_eq!(config.imap_host, "");
        assert_eq!(config.smtp_host, "");
    }

    #[test]
    fn test_qq_config_default() {
        let config = QQConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.app_id, "");
        assert_eq!(config.secret.expose(), "");
    }

    #[test]
    fn test_providers_config_default() {
        let config = ProvidersConfig::default();
        assert_eq!(config.anthropic.api_key.expose(), "");
        assert_eq!(config.openai.api_key.expose(), "");
        assert_eq!(config.openrouter.api_key.expose(), "");
        assert_eq!(config.deepseek.api_key.expose(), "");
        assert!(config.anthropic.api_base.is_none());
    }

    #[test]
    fn test_provider_config_with_api_base() {
        let config = ProviderConfig {
            api_key: Secret::new("test-key".to_string()),
            api_base: Some("https://custom.api.com/v1".to_string()),
            extra_headers: None,
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["apiKey"], "test-key");
        assert_eq!(json["apiBase"], "https://custom.api.com/v1");
    }

    #[test]
    fn test_provider_config_without_api_base() {
        let config = ProviderConfig {
            api_key: Secret::new("test-key".to_string()),
            api_base: None,
            extra_headers: None,
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["apiKey"], "test-key");
        assert!(json.get("apiBase").is_none());
    }

    #[test]
    fn test_tools_config_default() {
        let config = ToolsConfig::default();
        assert!(!config.restrict_to_workspace);
        assert_eq!(config.exec.timeout, 60);
        assert_eq!(config.web.brave_api_key.expose(), "");
    }

    #[test]
    fn test_exec_tool_config() {
        let config = ExecToolConfig {
            timeout: 120,
            allowed_commands: vec!["ls".to_string(), "pwd".to_string()],
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["timeout"], 120);
        assert_eq!(json["allowedCommands"][0], "ls");
        assert_eq!(json["allowedCommands"][1], "pwd");
    }

    #[test]
    fn test_channels_config_default() {
        let config = ChannelsConfig::default();
        assert!(!config.telegram.enabled);
        assert!(!config.discord.enabled);
        assert!(!config.whatsapp.enabled);
        assert!(!config.slack.enabled);
        assert!(!config.email.enabled);
        assert!(!config.qq.enabled);
    }

    #[test]
    fn test_full_config_serialization_camel_case() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();

        // Verify camelCase in JSON output
        assert!(json.contains("maxTokens"));
        assert!(json.contains("maxToolIterations"));
        assert!(json.contains("allowFrom"));
        assert!(json.contains("bridgeUrl"));
        assert!(json.contains("botToken"));
        assert!(json.contains("appToken"));
        assert!(json.contains("imapHost"));
        assert!(json.contains("imapPort"));
        assert!(json.contains("smtpHost"));
        assert!(json.contains("apiKey"));
        // apiBase is Option with skip_serializing_if = None, so it won't appear with defaults
        assert!(json.contains("restrictToWorkspace"));
        assert!(json.contains("braveApiKey"));
    }

    #[test]
    fn test_config_round_trip() {
        let mut config = Config::default();
        config.agents.defaults.model = "test-model".to_string();
        config.agents.defaults.max_tokens = 4096;
        config.providers.anthropic.api_key = Secret::new("sk-test-123".to_string());
        config.channels.telegram.enabled = true;
        config.channels.telegram.token = Secret::new("bot-token".to_string());

        // Serialize
        let json = serde_json::to_string(&config).unwrap();

        // Deserialize
        let loaded: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.agents.defaults.model, "test-model");
        assert_eq!(loaded.agents.defaults.max_tokens, 4096);
        assert_eq!(loaded.providers.anthropic.api_key.expose(), "sk-test-123");
        assert!(loaded.channels.telegram.enabled);
        assert_eq!(loaded.channels.telegram.token.expose(), "bot-token");
    }

    #[test]
    fn test_email_config_serialization() {
        let config = EmailConfig {
            enabled: true,
            imap_host: "imap.gmail.com".to_string(),
            imap_port: 993,
            imap_username: "user@gmail.com".to_string(),
            imap_password: Secret::new("password".to_string()),
            imap_mailbox: "INBOX".to_string(),
            imap_use_ssl: true,
            smtp_host: "smtp.gmail.com".to_string(),
            smtp_port: 587,
            smtp_username: "user@gmail.com".to_string(),
            smtp_password: Secret::new("password".to_string()),
            smtp_use_tls: true,
            smtp_use_ssl: false,
            from_address: "user@gmail.com".to_string(),
            allow_from: vec!["trusted@example.com".to_string()],
            consent_granted: false,
            auto_reply_enabled: true,
            max_body_chars: 12000,
            mark_seen: true,
            poll_interval_seconds: 30,
            subject_prefix: "Re: ".to_string(),
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["imapHost"], "imap.gmail.com");
        assert_eq!(json["imapPort"], 993);
        assert_eq!(json["smtpHost"], "smtp.gmail.com");
        assert_eq!(json["smtpPort"], 587);
        assert_eq!(json["fromAddress"], "user@gmail.com");
    }

    #[test]
    fn test_config_with_multiple_providers() {
        let mut config = Config::default();
        config.providers.anthropic.api_key = Secret::new("anthropic-key".to_string());
        config.providers.openai.api_key = Secret::new("openai-key".to_string());
        config.providers.deepseek.api_key = Secret::new("deepseek-key".to_string());
        config.providers.openrouter.api_key = Secret::new("sk-or-v1-test".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.providers.anthropic.api_key.expose(), "anthropic-key");
        assert_eq!(loaded.providers.openai.api_key.expose(), "openai-key");
        assert_eq!(loaded.providers.deepseek.api_key.expose(), "deepseek-key");
        assert_eq!(
            loaded.providers.openrouter.api_key.expose(),
            "sk-or-v1-test"
        );
    }

    #[test]
    fn test_config_with_custom_api_bases() {
        let mut config = Config::default();
        config.providers.anthropic.api_base = Some("https://custom-anthropic.com/v1".to_string());
        config.providers.openai.api_base = Some("https://custom-openai.com/v1".to_string());

        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(
            loaded.providers.anthropic.api_base,
            Some("https://custom-anthropic.com/v1".to_string())
        );
        assert_eq!(
            loaded.providers.openai.api_base,
            Some("https://custom-openai.com/v1".to_string())
        );
    }

    #[test]
    fn test_tools_exec_timeout_default() {
        let config = ExecToolConfig::default();
        assert_eq!(config.timeout, 60);
        assert!(config.allowed_commands.is_empty());
    }

    #[test]
    fn test_web_tools_config() {
        let config = WebToolsConfig {
            brave_api_key: Secret::new("brave-api-key-123".to_string()),
            max_results: 5,
        };

        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["braveApiKey"], "brave-api-key-123");
        assert_eq!(json["maxResults"], 5);
    }

    #[test]
    fn test_config_with_allow_from_lists() {
        let mut config = Config::default();
        config.channels.telegram.allow_from = vec!["123".to_string(), "456".to_string()];
        config.channels.discord.allow_from = vec!["guild1".to_string()];
        config.channels.slack.allow_from = vec!["channel1".to_string(), "channel2".to_string()];

        let json = serde_json::to_string(&config).unwrap();
        let loaded: Config = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.channels.telegram.allow_from.len(), 2);
        assert_eq!(loaded.channels.discord.allow_from.len(), 1);
        assert_eq!(loaded.channels.slack.allow_from.len(), 2);
    }

    #[test]
    fn test_todo_config_default() {
        let config = TodoConfig::default();
        assert_eq!(config.focus.max_slots, 3);
        assert_eq!(config.focus.deadline_hours, 18);
        assert_eq!(config.notifications.targets, vec!["os_native"]);
        assert!(config.notifications.focus_reminders);
        assert!(config.notifications.daily_digest);
        assert_eq!(config.notifications.daily_digest_time, "09:00");
    }

    #[test]
    fn test_todo_notification_config_default() {
        let config = TodoNotificationConfig::default();
        assert_eq!(config.targets.len(), 1);
        assert_eq!(config.targets[0], "os_native");
        assert!(config.focus_reminders);
        assert!(config.daily_digest);
        assert_eq!(config.daily_digest_time, "09:00");
    }

    #[test]
    fn test_todo_focus_config_default() {
        let config = TodoFocusConfig::default();
        assert_eq!(config.max_slots, 3);
        assert_eq!(config.deadline_hours, 18);
    }

    #[test]
    fn test_config_with_todo_field() {
        let config = Config::default();
        assert_eq!(config.todo.focus.max_slots, 3);
    }

    #[test]
    fn test_todo_config_serialization() {
        let config = TodoConfig::default();
        let json = serde_json::to_value(&config).unwrap();

        assert_eq!(json["focus"]["maxSlots"], 3);
        assert_eq!(json["focus"]["deadlineHours"], 18);
        assert_eq!(json["notifications"]["targets"][0], "os_native");
        assert_eq!(json["notifications"]["focusReminders"], true);
        assert_eq!(json["notifications"]["dailyDigest"], true);
        assert_eq!(json["notifications"]["dailyDigestTime"], "09:00");
    }

    #[test]
    fn test_config_without_todo_deserializes() {
        // AC-11.9: Existing config without todo field should deserialize with defaults
        let json = r#"{
            "agents": {
                "defaults": {
                    "workspace": "~/.klyntbot/workspace",
                    "model": "anthropic/claude-opus-4-5",
                    "maxTokens": 8192,
                    "temperature": 0.7,
                    "maxToolIterations": 20
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.todo.focus.max_slots, 3);
        assert_eq!(config.todo.notifications.targets, vec!["os_native"]);
    }

    #[test]
    fn test_todo_config_round_trip() {
        let config = TodoConfig {
            focus: TodoFocusConfig {
                max_slots: 5,
                deadline_hours: 24,
            },
            notifications: TodoNotificationConfig {
                targets: vec!["os_native".to_string(), "telegram".to_string()],
                daily_digest_time: "08:00".to_string(),
                ..Default::default()
            },
        };

        let json = serde_json::to_string(&config).unwrap();
        let loaded: TodoConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.focus.max_slots, 5);
        assert_eq!(loaded.focus.deadline_hours, 24);
        assert_eq!(loaded.notifications.targets.len(), 2);
        assert_eq!(loaded.notifications.daily_digest_time, "08:00");
    }

    #[test]
    fn test_todo_config_camel_case_json() {
        let config = TodoConfig::default();
        let json = serde_json::to_string(&config).unwrap();

        // AC-11.11: Verify camelCase in JSON output
        assert!(json.contains("maxSlots"));
        assert!(json.contains("deadlineHours"));
        assert!(json.contains("focusReminders"));
        assert!(json.contains("dailyDigest"));
        assert!(json.contains("dailyDigestTime"));
    }

    #[test]
    fn test_partial_notification_config_deserializes() {
        // Verify partial notification config (missing targets) deserializes OK
        let json = r#"{
            "todo": {
                "notifications": {
                    "focusReminders": false
                }
            },
            "agents": {
                "defaults": {
                    "workspace": "~/.klyntbot/workspace",
                    "model": "anthropic/claude-opus-4-5",
                    "maxTokens": 8192,
                    "temperature": 0.7,
                    "maxToolIterations": 20
                }
            }
        }"#;

        let config: Config = serde_json::from_str(json).unwrap();
        assert!(!config.todo.notifications.focus_reminders);
    }
}
