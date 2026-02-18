//! Unit-level tests for individual channel implementations.
//!
//! These tests focus on message parsing, allowlist filtering, configuration
//! validation, and error handling for each channel—without requiring live
//! network connections.

mod common;

use klyntbot::bus::{InboundMessage, MessageBus, OutboundMessage};
use klyntbot::channels::check_allowlist;
use klyntbot::config::{DiscordConfig, EmailConfig, QQConfig, SlackConfig, WhatsAppConfig};
use klyntbot::ChannelManager;
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────
// Shared allowlist / check_allowlist tests
// ─────────────────────────────────────────────────────────────

#[test]
fn test_check_allowlist_empty_allows_all() {
    let allow_from: Vec<String> = vec![];
    assert!(check_allowlist(&allow_from, "anyone"));
    assert!(check_allowlist(&allow_from, ""));
}

#[test]
fn test_check_allowlist_exact_match() {
    let allow_from = vec!["user_123".to_string(), "user_456".to_string()];
    assert!(check_allowlist(&allow_from, "user_123"));
    assert!(check_allowlist(&allow_from, "user_456"));
    assert!(!check_allowlist(&allow_from, "user_789"));
}

#[test]
fn test_check_allowlist_compound_id() {
    // Telegram sends "12345|username" compound IDs
    let allow_from = vec!["12345".to_string()];
    assert!(check_allowlist(&allow_from, "12345|alice"));

    let allow_from2 = vec!["alice".to_string()];
    assert!(check_allowlist(&allow_from2, "12345|alice"));
}

#[test]
fn test_check_allowlist_no_partial_match() {
    let allow_from = vec!["user_12".to_string()];
    assert!(!check_allowlist(&allow_from, "user_123"));
}

// ─────────────────────────────────────────────────────────────
// Discord channel tests
// ─────────────────────────────────────────────────────────────

#[test]
fn test_discord_config_defaults() {
    let config = DiscordConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.token.expose(), "");
    assert!(config.allow_from.is_empty());
    assert_eq!(
        config.gateway_url,
        "wss://gateway.discord.gg/?v=10&encoding=json"
    );
    // GUILD_MESSAGES | DIRECT_MESSAGES | MESSAGE_CONTENT
    assert_eq!(config.intents, 37377);
}

#[test]
fn test_discord_channel_creation_empty_token() {
    use klyntbot::channels::DiscordChannel;
    // Should succeed even with empty token (fails on connect, not create)
    let config = DiscordConfig::default();
    let channel = DiscordChannel::new(config);
    assert!(channel.is_ok());
}

#[test]
fn test_discord_channel_name() {
    use klyntbot::channels::{Channel, DiscordChannel};
    let config = DiscordConfig::default();
    let channel = DiscordChannel::new(config).unwrap();
    assert_eq!(channel.name(), "discord");
}

#[test]
fn test_discord_is_allowed_empty_allowlist() {
    use klyntbot::channels::{Channel, DiscordChannel};
    let config = DiscordConfig::default();
    let channel = DiscordChannel::new(config).unwrap();
    assert!(channel.is_allowed("any_user"));
}

#[test]
fn test_discord_is_allowed_with_allowlist() {
    use klyntbot::channels::{Channel, DiscordChannel};
    let config = DiscordConfig {
        allow_from: vec!["user_abc".to_string()],
        ..Default::default()
    };
    let channel = DiscordChannel::new(config).unwrap();
    assert!(channel.is_allowed("user_abc"));
    assert!(!channel.is_allowed("user_xyz"));
}

// ─────────────────────────────────────────────────────────────
// Slack channel tests
// ─────────────────────────────────────────────────────────────

#[test]
fn test_slack_config_defaults() {
    let config = SlackConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.bot_token.expose(), "");
    assert_eq!(config.app_token.expose(), "");
    assert!(config.allow_from.is_empty());
    assert_eq!(config.mode, "socket");
    assert_eq!(config.group_policy, "none");
}

#[test]
fn test_slack_channel_creation() {
    use klyntbot::channels::SlackChannel;
    let config = SlackConfig::default();
    let channel = SlackChannel::new(config);
    assert!(channel.is_ok());
}

#[test]
fn test_slack_channel_name() {
    use klyntbot::channels::{Channel, SlackChannel};
    let config = SlackConfig::default();
    let channel = SlackChannel::new(config).unwrap();
    assert_eq!(channel.name(), "slack");
}

#[test]
fn test_slack_is_allowed_empty_allowlist() {
    use klyntbot::channels::{Channel, SlackChannel};
    let config = SlackConfig::default();
    let channel = SlackChannel::new(config).unwrap();
    assert!(channel.is_allowed("U12345"));
}

#[test]
fn test_slack_is_allowed_with_allowlist() {
    use klyntbot::channels::{Channel, SlackChannel};
    let config = SlackConfig {
        allow_from: vec!["U12345".to_string()],
        ..Default::default()
    };
    let channel = SlackChannel::new(config).unwrap();
    assert!(channel.is_allowed("U12345"));
    assert!(!channel.is_allowed("U99999"));
}

#[tokio::test]
async fn test_slack_start_rejects_empty_tokens() {
    use klyntbot::channels::{Channel, SlackChannel};
    let config = SlackConfig::default();
    let channel = SlackChannel::new(config).unwrap();
    let bus = common::test_message_bus();
    let result = channel.start(bus).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not configured"));
}

// ─────────────────────────────────────────────────────────────
// QQ channel tests
// ─────────────────────────────────────────────────────────────

#[test]
fn test_qq_config_defaults() {
    let config = QQConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.app_id, "");
    assert_eq!(config.secret.expose(), "");
    assert!(config.allow_from.is_empty());
}

#[test]
fn test_qq_channel_creation() {
    use klyntbot::channels::QQChannel;
    let config = QQConfig::default();
    let channel = QQChannel::new(config);
    assert!(channel.is_ok());
}

#[test]
fn test_qq_channel_name() {
    use klyntbot::channels::{Channel, QQChannel};
    let config = QQConfig::default();
    let channel = QQChannel::new(config).unwrap();
    assert_eq!(channel.name(), "qq");
}

#[test]
fn test_qq_is_allowed_empty_allowlist() {
    use klyntbot::channels::{Channel, QQChannel};
    let config = QQConfig::default();
    let channel = QQChannel::new(config).unwrap();
    assert!(channel.is_allowed("any_openid"));
}

#[test]
fn test_qq_is_allowed_with_allowlist() {
    use klyntbot::channels::{Channel, QQChannel};
    let config = QQConfig {
        allow_from: vec!["openid_abc".to_string()],
        ..Default::default()
    };
    let channel = QQChannel::new(config).unwrap();
    assert!(channel.is_allowed("openid_abc"));
    assert!(!channel.is_allowed("openid_xyz"));
}

#[tokio::test]
async fn test_qq_start_rejects_empty_credentials() {
    use klyntbot::channels::{Channel, QQChannel};
    let config = QQConfig::default();
    let channel = QQChannel::new(config).unwrap();
    let bus = common::test_message_bus();
    let result = channel.start(bus).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not configured"));
}

// ─────────────────────────────────────────────────────────────
// WhatsApp channel tests
// ─────────────────────────────────────────────────────────────

#[test]
fn test_whatsapp_config_defaults() {
    let config = WhatsAppConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.bridge_url, "ws://localhost:3001");
    assert!(config.allow_from.is_empty());
}

#[test]
fn test_whatsapp_channel_creation() {
    use klyntbot::channels::WhatsAppChannel;
    let config = WhatsAppConfig::default();
    let _channel = WhatsAppChannel::new(config).unwrap();
}

#[test]
fn test_whatsapp_channel_name() {
    use klyntbot::channels::{Channel, WhatsAppChannel};
    let config = WhatsAppConfig::default();
    let channel = WhatsAppChannel::new(config).unwrap();
    assert_eq!(channel.name(), "whatsapp");
}

#[test]
fn test_whatsapp_is_allowed_empty_allowlist() {
    use klyntbot::channels::{Channel, WhatsAppChannel};
    let config = WhatsAppConfig::default();
    let channel = WhatsAppChannel::new(config).unwrap();
    assert!(channel.is_allowed("628123456789@s.whatsapp.net"));
}

#[test]
fn test_whatsapp_is_allowed_with_allowlist() {
    use klyntbot::channels::{Channel, WhatsAppChannel};
    let config = WhatsAppConfig {
        allow_from: vec!["628123456789@s.whatsapp.net".to_string()],
        ..Default::default()
    };
    let channel = WhatsAppChannel::new(config).unwrap();
    assert!(channel.is_allowed("628123456789@s.whatsapp.net"));
    assert!(!channel.is_allowed("other@s.whatsapp.net"));
}

#[tokio::test]
async fn test_whatsapp_send_fails_without_connection() {
    use klyntbot::channels::{Channel, WhatsAppChannel};
    let config = WhatsAppConfig::default();
    let channel = WhatsAppChannel::new(config).unwrap();
    let msg = OutboundMessage::new("whatsapp", "628123@s.whatsapp.net", "Hello");
    let result = channel.send(&msg).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not connected"));
}

// ─────────────────────────────────────────────────────────────
// Email channel tests
// ─────────────────────────────────────────────────────────────

#[test]
fn test_email_config_defaults() {
    let config = EmailConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.imap_port, 993);
    assert_eq!(config.smtp_port, 587);
    assert!(config.imap_use_ssl);
    assert!(config.smtp_use_tls);
    assert!(!config.consent_granted);
    assert!(config.auto_reply_enabled);
    assert_eq!(config.max_body_chars, 12000);
    assert!(config.mark_seen);
    assert_eq!(config.poll_interval_seconds, 30);
    assert_eq!(config.subject_prefix, "Re: ");
    assert_eq!(config.imap_mailbox, "INBOX");
}

#[test]
fn test_email_channel_creation() {
    use klyntbot::channels::EmailChannel;
    let config = EmailConfig::default();
    let channel = EmailChannel::new(config);
    assert!(channel.is_ok());
}

#[test]
fn test_email_channel_name() {
    use klyntbot::channels::{Channel, EmailChannel};
    let config = EmailConfig::default();
    let channel = EmailChannel::new(config).unwrap();
    assert_eq!(channel.name(), "email");
}

#[test]
fn test_email_is_allowed_empty_allowlist() {
    use klyntbot::channels::{Channel, EmailChannel};
    let config = EmailConfig::default();
    let channel = EmailChannel::new(config).unwrap();
    assert!(channel.is_allowed("anyone@example.com"));
}

#[test]
fn test_email_is_allowed_with_allowlist() {
    use klyntbot::channels::{Channel, EmailChannel};
    let config = EmailConfig {
        allow_from: vec!["trusted@example.com".to_string()],
        ..Default::default()
    };
    let channel = EmailChannel::new(config).unwrap();
    assert!(channel.is_allowed("trusted@example.com"));
    assert!(!channel.is_allowed("untrusted@example.com"));
}

#[tokio::test]
async fn test_email_start_rejects_missing_consent() {
    use klyntbot::channels::{Channel, EmailChannel};
    // consent_granted is false by default
    let config = EmailConfig {
        enabled: true,
        ..Default::default()
    };
    let channel = EmailChannel::new(config).unwrap();
    let bus = common::test_message_bus();
    let result = channel.start(bus).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("consent"));
}

#[tokio::test]
async fn test_email_start_rejects_missing_config_fields() {
    use klyntbot::channels::{Channel, EmailChannel};
    // All host/user/pass fields are empty
    let config = EmailConfig {
        enabled: true,
        consent_granted: true,
        ..Default::default()
    };
    let channel = EmailChannel::new(config).unwrap();
    let bus = common::test_message_bus();
    let result = channel.start(bus).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not configured") || err_msg.contains("Missing"));
}

#[tokio::test]
async fn test_email_send_skips_when_auto_reply_disabled() {
    use klyntbot::channels::{Channel, EmailChannel};
    let config = EmailConfig {
        auto_reply_enabled: false,
        ..Default::default()
    };
    let channel = EmailChannel::new(config).unwrap();
    let msg = OutboundMessage::new("email", "user@example.com", "Hello");
    // Should succeed (skip) not error
    let result = channel.send(&msg).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_email_send_rejects_empty_recipient() {
    use klyntbot::channels::{Channel, EmailChannel};
    let config = EmailConfig::default();
    let channel = EmailChannel::new(config).unwrap();
    let msg = OutboundMessage::new("email", "", "Hello");
    let result = channel.send(&msg).await;
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────
// ChannelManager tests
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_channel_manager_creates_with_no_channels() {
    let (temp_dir, _workspace) = common::test_workspace();
    let config = common::test_config(&temp_dir);
    let bus = common::test_message_bus();
    let _manager =
        ChannelManager::new(Arc::new(config), bus).expect("Failed to create channel manager");
}

#[tokio::test]
async fn test_channel_manager_initialize_with_no_enabled() {
    let (temp_dir, _workspace) = common::test_workspace();
    let config = common::test_config(&temp_dir);
    let bus = common::test_message_bus();
    let manager =
        ChannelManager::new(Arc::new(config), bus).expect("Failed to create channel manager");
    let result = manager.initialize_channels().await;
    assert!(result.is_ok());
}

// ─────────────────────────────────────────────────────────────
// Cross-channel message routing tests
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_inbound_message_preserves_channel_identity() {
    let channels = ["telegram", "discord", "slack", "whatsapp", "qq", "email"];
    for channel in channels {
        let msg = InboundMessage::new(channel, "user", "chat", "Hello");
        assert_eq!(msg.channel.as_str(), channel);
        assert_eq!(msg.session_key().to_string(), format!("{}:chat", channel));
    }
}

#[tokio::test]
async fn test_outbound_message_preserves_channel_identity() {
    let channels = ["telegram", "discord", "slack", "whatsapp", "qq", "email"];
    for channel in channels {
        let msg = OutboundMessage::new(channel, "chat", "Reply");
        assert_eq!(msg.channel.as_str(), channel);
    }
}

#[tokio::test]
async fn test_bus_routes_to_correct_channel() {
    let bus = MessageBus::new(50);

    // Send messages from different channels
    for (i, channel) in ["telegram", "discord", "slack"].iter().enumerate() {
        let msg = InboundMessage::new(*channel, "user", format!("chat_{}", i), "Hello");
        bus.publish_inbound(msg).await.unwrap();
    }

    let mut rx = bus.take_inbound_rx().unwrap();

    let m1 = rx.recv().await.unwrap();
    assert_eq!(m1.channel.as_str(), "telegram");

    let m2 = rx.recv().await.unwrap();
    assert_eq!(m2.channel.as_str(), "discord");

    let m3 = rx.recv().await.unwrap();
    assert_eq!(m3.channel.as_str(), "slack");
}

// ─────────────────────────────────────────────────────────────
// Message content edge cases
// ─────────────────────────────────────────────────────────────

#[test]
fn test_inbound_message_with_unicode() {
    let msg = InboundMessage::new("telegram", "user", "chat", "你好世界 🌍 مرحبا");
    assert_eq!(msg.content, "你好世界 🌍 مرحبا");
}

#[test]
fn test_inbound_message_with_empty_content() {
    let msg = InboundMessage::new("telegram", "user", "chat", "");
    assert_eq!(msg.content, "");
}

#[test]
fn test_inbound_message_validate_size_within_limit() {
    let msg = InboundMessage::new("telegram", "user", "chat", "Normal message");
    assert!(msg.validate().is_ok());
}

#[test]
fn test_inbound_message_validate_size_exceeds_limit() {
    let huge = "x".repeat(70_000);
    let msg = InboundMessage::new("telegram", "user", "chat", huge);
    assert!(msg.validate().is_err());
}

#[test]
fn test_outbound_message_builder_chain() {
    let msg = OutboundMessage::new("discord", "guild_123", "Hello")
        .with_reply_to("msg_456")
        .with_media("https://example.com/img.png");

    assert_eq!(msg.channel.as_str(), "discord");
    assert_eq!(msg.chat_id.as_str(), "guild_123");
    assert_eq!(msg.content, "Hello");
    assert_eq!(msg.reply_to, Some("msg_456".to_string()));
    assert_eq!(msg.media.len(), 1);
}

// ─────────────────────────────────────────────────────────────
// Shared test fixtures validation
// ─────────────────────────────────────────────────────────────

#[test]
fn test_fixtures_test_config() {
    let (temp_dir, _) = common::test_workspace();
    let config = common::test_config(&temp_dir);
    assert_eq!(config.agents.defaults.model, "mock-model");
    assert_eq!(config.agents.defaults.max_tokens, 1024);
    assert_eq!(config.agents.defaults.temperature, 0.0);
}

#[test]
fn test_fixtures_test_workspace() {
    let (temp_dir, workspace) = common::test_workspace();
    assert!(workspace.exists());
    assert!(workspace.is_dir());
    drop(temp_dir); // cleanup
}

#[tokio::test]
async fn test_fixtures_test_session_manager() {
    let (mut manager, _temp_dir) = common::test_session_manager().await;
    let session = manager.get_or_create("test:chat").await.unwrap();
    assert_eq!(session.key, "test:chat");
}

#[test]
fn test_fixtures_test_provider() {
    let provider = common::test_provider("Hello from mock!");
    assert_eq!(provider.call_count(), 0);
}

#[test]
fn test_fixtures_sample_messages() {
    let inbound = common::sample_inbound("telegram", "Hello");
    assert_eq!(inbound.channel.as_str(), "telegram");
    assert_eq!(inbound.content, "Hello");

    let outbound = common::sample_outbound("discord", "Reply");
    assert_eq!(outbound.channel.as_str(), "discord");
    assert_eq!(outbound.content, "Reply");
}
