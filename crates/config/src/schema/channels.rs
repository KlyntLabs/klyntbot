//! Chat channel configuration structs.

use serde::{Deserialize, Serialize};

use super::core::Secret;

/// Channels configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ChannelsConfig {
    #[serde(default)]
    pub telegram: TelegramConfig,

    #[serde(default)]
    pub discord: DiscordConfig,

    #[serde(default)]
    pub slack: SlackConfig,

    #[serde(default)]
    pub email: EmailConfig,
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
    // GUILD_MESSAGES (1<<9) | GUILD_MESSAGE_REACTIONS (1<<10) | DIRECT_MESSAGES (1<<12)
    // | DIRECT_MESSAGE_REACTIONS (1<<13) | MESSAGE_CONTENT (1<<15)
    46593
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

fn default_slack_mode() -> String {
    "socket".to_string()
}

fn default_slack_group_policy() -> String {
    "none".to_string()
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_channels_config_default() {
        let config = ChannelsConfig::default();
        assert!(!config.telegram.enabled);
        assert!(!config.discord.enabled);
        assert!(!config.slack.enabled);
        assert!(!config.email.enabled);
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
}
