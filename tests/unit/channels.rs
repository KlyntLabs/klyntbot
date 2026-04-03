//! Unit tests for channel allowlist filtering, configuration defaults,
//! message construction, and validation — no async, no storage.

use klyntbot::bus::{InboundMessage, OutboundMessage};
use klyntbot::channels::check_allowlist;
use klyntbot::config::{Config, DiscordConfig, EmailConfig, SlackConfig};

// ─────────────────────────────────────────────────────────────
// check_allowlist core logic
// ─────────────────────────────────────────────────────────────

#[test]
fn allowlist_empty_allows_all() {
    let allow_from: Vec<String> = vec![];
    assert!(check_allowlist(&allow_from, "anyone"));
    assert!(check_allowlist(&allow_from, ""));
}

#[test]
fn allowlist_exact_match() {
    let allow_from = vec!["user_123".to_string(), "user_456".to_string()];
    assert!(check_allowlist(&allow_from, "user_123"));
    assert!(check_allowlist(&allow_from, "user_456"));
    assert!(!check_allowlist(&allow_from, "user_789"));
}

#[test]
fn allowlist_compound_id() {
    // Telegram sends "12345|username" compound IDs
    let allow_from = vec!["12345".to_string()];
    assert!(check_allowlist(&allow_from, "12345|alice"));

    let allow_from2 = vec!["alice".to_string()];
    assert!(check_allowlist(&allow_from2, "12345|alice"));
}

#[test]
fn allowlist_no_partial_match() {
    let allow_from = vec!["user_12".to_string()];
    assert!(!check_allowlist(&allow_from, "user_123"));
}

// ─────────────────────────────────────────────────────────────
// Per-channel is_allowed
// ─────────────────────────────────────────────────────────────

#[test]
fn discord_allowlist_filters() {
    use klyntbot::channels::{Channel, DiscordChannel};
    let config = DiscordConfig {
        allow_from: vec!["user_abc".to_string()],
        ..Default::default()
    };
    let channel = DiscordChannel::new(config, std::path::PathBuf::from("/tmp/test-data")).unwrap();
    assert!(channel.is_allowed("user_abc"));
    assert!(!channel.is_allowed("user_xyz"));
}

#[test]
fn slack_allowlist_filters() {
    use klyntbot::channels::{Channel, SlackChannel};
    let config = SlackConfig {
        allow_from: vec!["U12345".to_string()],
        ..Default::default()
    };
    let channel = SlackChannel::new(config).unwrap();
    assert!(channel.is_allowed("U12345"));
    assert!(!channel.is_allowed("U99999"));
}

#[test]
fn email_allowlist_filters() {
    use klyntbot::channels::{Channel, EmailChannel};
    let config = EmailConfig {
        allow_from: vec!["trusted@example.com".to_string()],
        ..Default::default()
    };
    let channel = EmailChannel::new(config).unwrap();
    assert!(channel.is_allowed("trusted@example.com"));
    assert!(!channel.is_allowed("untrusted@example.com"));
}

// ─────────────────────────────────────────────────────────────
// Channel configuration defaults and validation
// ─────────────────────────────────────────────────────────────

#[test]
fn channel_allow_from_filtering() {
    let mut config = Config::default();
    config.channels.telegram.allow_from = vec!["user_123".to_string(), "user_456".to_string()];

    assert!(config
        .channels
        .telegram
        .allow_from
        .contains(&"user_123".to_string()));
    assert!(!config
        .channels
        .telegram
        .allow_from
        .contains(&"user_999".to_string()));
}

#[test]
fn channel_defaults_all_disabled() {
    let config = Config::default();

    assert!(!config.channels.telegram.enabled);
    assert!(!config.channels.discord.enabled);
    assert!(!config.channels.slack.enabled);

    assert_eq!(config.channels.telegram.allow_from.len(), 0);
    assert!(!config.channels.discord.enabled);
    assert!(!config.channels.slack.enabled);
}

#[test]
fn channel_enable_one_leaves_others_disabled() {
    let mut config = Config::default();
    config.channels.telegram.enabled = true;
    assert!(config.channels.telegram.enabled);
    assert!(!config.channels.discord.enabled);
}

// ─────────────────────────────────────────────────────────────
// Message construction and content edge cases
// ─────────────────────────────────────────────────────────────

#[test]
fn message_content_preserves_special_chars() {
    let test_cases = vec![
        "Normal text message",
        "Text with\nnewlines\nand\ttabs",
        "Unicode: 你好世界 🌍",
        "Empty",
        "Very long message that might need truncation in some cases but should be preserved in the bus layer",
    ];

    for content in test_cases {
        let msg = InboundMessage::new("test", "user", "chat", content);
        assert_eq!(msg.content, content);
    }
}

#[test]
fn inbound_message_unicode_preserved() {
    let msg = InboundMessage::new("telegram", "user", "chat", "你好世界 🌍 مرحبا");
    assert_eq!(msg.content, "你好世界 🌍 مرحبا");
}

#[test]
fn inbound_message_empty_content() {
    let msg = InboundMessage::new("telegram", "user", "chat", "");
    assert_eq!(msg.content, "");
}

#[test]
fn inbound_message_validates_within_limit() {
    let msg = InboundMessage::new("telegram", "user", "chat", "Normal message");
    assert!(msg.validate().is_ok());
}

#[test]
fn inbound_message_validates_exceeds_limit() {
    let huge = "x".repeat(70_000);
    let msg = InboundMessage::new("telegram", "user", "chat", huge);
    assert!(msg.validate().is_err());
}

#[test]
fn outbound_message_builder_chain() {
    let msg = OutboundMessage::new("discord", "guild_123", "Hello")
        .with_reply_to("msg_456")
        .with_media("https://example.com/img.png");

    assert_eq!(msg.channel.as_str(), "discord");
    assert_eq!(msg.chat_id.as_str(), "guild_123");
    assert_eq!(msg.content, "Hello");
    assert_eq!(msg.reply_to, Some("msg_456".to_string()));
    assert_eq!(msg.media.len(), 1);
}

#[test]
fn channel_message_formatting_preserves_markdown() {
    let telegram_msg = OutboundMessage::new("telegram", "chat_1", "**Bold** text");
    let discord_msg = OutboundMessage::new("discord", "guild_1", "**Bold** text");

    assert_eq!(telegram_msg.content, "**Bold** text");
    assert_eq!(discord_msg.content, "**Bold** text");
}
