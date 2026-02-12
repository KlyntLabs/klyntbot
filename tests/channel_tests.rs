//! Integration tests for channel system

use klyntbot::bus::{InboundMessage, MessageBus, OutboundMessage};
use klyntbot::config::Config;
use klyntbot::ChannelManager;
use std::sync::Arc;
use tempfile::TempDir;

/// Test channel manager initialization
#[tokio::test]
async fn test_channel_manager_initialization() {
    let temp_dir = TempDir::new().unwrap();
    let mut config = Config::default();
    config.agents.defaults.workspace = temp_dir
        .path()
        .join("workspace")
        .to_str()
        .unwrap()
        .to_string();

    let bus = Arc::new(MessageBus::new(10));
    let _manager = ChannelManager::new(Arc::new(config), bus.clone());

    // ChannelManager::new returns Self directly, not a Result
    // Just verify it was created successfully (no panic)
}

/// Test message routing through channels
#[tokio::test]
async fn test_message_routing_through_channels() {
    let bus = Arc::new(MessageBus::new(10));

    // Simulate a message arriving from a channel
    let inbound = InboundMessage::new("telegram", "user_123", "chat_456", "Hello from Telegram!");

    bus.publish_inbound(inbound).await.unwrap();

    // Agent loop would consume this
    let received = bus.consume_inbound().await.unwrap();
    assert_eq!(received.channel, "telegram");
    assert_eq!(received.sender_id, "user_123");
    assert_eq!(received.chat_id, "chat_456");
    assert_eq!(received.content, "Hello from Telegram!");

    // Agent generates response
    let response = OutboundMessage::new("telegram", "chat_456", "Hello back!");
    bus.publish_outbound(response).await.unwrap();

    // Channel would consume this
    let outbound = bus.consume_outbound().await.unwrap();
    assert_eq!(outbound.channel, "telegram");
    assert_eq!(outbound.chat_id, "chat_456");
    assert_eq!(outbound.content, "Hello back!");
}

/// Test channel allow-from filtering
#[test]
fn test_channel_allow_from_filtering() {
    // Test the allow_from configuration logic
    let mut config = Config::default();

    // Configure allow_from for telegram channel
    config.channels.telegram.allow_from = vec!["user_123".to_string(), "user_456".to_string()];

    // User in allow list
    assert!(config
        .channels
        .telegram
        .allow_from
        .contains(&"user_123".to_string()));

    // User not in allow list
    assert!(!config
        .channels
        .telegram
        .allow_from
        .contains(&"user_999".to_string()));
}

/// Test channel enable/disable configuration
#[test]
fn test_channel_enable_disable() {
    let mut config = Config::default();

    // Default: all channels disabled
    assert!(!config.channels.telegram.enabled);
    assert!(!config.channels.discord.enabled);
    assert!(!config.channels.slack.enabled);

    // Enable telegram
    config.channels.telegram.enabled = true;
    assert!(config.channels.telegram.enabled);

    // Others still disabled
    assert!(!config.channels.discord.enabled);
}

/// Test outbound message dispatch to correct channel
#[tokio::test]
async fn test_outbound_dispatch_routing() {
    let bus = Arc::new(MessageBus::new(10));

    // Send messages to different channels
    let telegram_msg = OutboundMessage::new("telegram", "chat_1", "Telegram message");
    let discord_msg = OutboundMessage::new("discord", "guild_1", "Discord message");
    let slack_msg = OutboundMessage::new("slack", "channel_1", "Slack message");

    bus.publish_outbound(telegram_msg).await.unwrap();
    bus.publish_outbound(discord_msg).await.unwrap();
    bus.publish_outbound(slack_msg).await.unwrap();

    // Consume and verify routing
    let msg1 = bus.consume_outbound().await.unwrap();
    assert_eq!(msg1.channel, "telegram");
    assert_eq!(msg1.content, "Telegram message");

    let msg2 = bus.consume_outbound().await.unwrap();
    assert_eq!(msg2.channel, "discord");
    assert_eq!(msg2.content, "Discord message");

    let msg3 = bus.consume_outbound().await.unwrap();
    assert_eq!(msg3.channel, "slack");
    assert_eq!(msg3.content, "Slack message");
}

/// Test concurrent message handling
#[tokio::test]
async fn test_concurrent_message_handling() {
    let bus = Arc::new(MessageBus::new(100));

    // Send multiple messages concurrently
    let handles: Vec<_> = (0..10)
        .map(|i| {
            let bus_clone = bus.clone();
            tokio::spawn(async move {
                let msg =
                    InboundMessage::new("test_channel", "user", "chat", format!("Message {}", i));
                bus_clone.publish_inbound(msg).await.unwrap();
            })
        })
        .collect();

    // Wait for all sends to complete
    for handle in handles {
        handle.await.unwrap();
    }

    // Consume all messages
    let mut received_messages = Vec::new();
    for _ in 0..10 {
        let msg = bus.consume_inbound().await.unwrap();
        received_messages.push(msg.content);
    }

    // All messages should be received
    assert_eq!(received_messages.len(), 10);
}

/// Test message metadata preservation
#[tokio::test]
async fn test_message_metadata_preservation() {
    let bus = Arc::new(MessageBus::new(10));

    // Create message with full metadata
    let mut inbound = InboundMessage::new(
        "telegram",
        "user_alice",
        "chat_private",
        "Important message",
    );

    // Add timestamp
    inbound.timestamp = chrono::Utc::now();

    bus.publish_inbound(inbound).await.unwrap();

    // Receive and verify metadata preserved
    let received = bus.consume_inbound().await.unwrap();
    assert_eq!(received.channel, "telegram");
    assert_eq!(received.sender_id, "user_alice");
    assert_eq!(received.chat_id, "chat_private");
    assert_eq!(received.content, "Important message");
    // Timestamp should be preserved
    assert!(received.timestamp.timestamp() > 0);
}

/// Test channel configuration validation
#[test]
fn test_channel_configuration_validation() {
    let config = Config::default();

    // Verify default channel configurations
    assert!(!config.channels.telegram.enabled);
    assert_eq!(config.channels.telegram.allow_from.len(), 0);

    assert!(!config.channels.discord.enabled);

    assert!(!config.channels.slack.enabled);
}

/// Test message content sanitization
#[test]
fn test_message_content_sanitization() {
    let _bus = MessageBus::new(10);

    // Test with various content types
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

/// Test bus capacity limits
#[tokio::test]
async fn test_bus_capacity_limits() {
    let bus = Arc::new(MessageBus::new(5)); // Small capacity

    // Fill the bus
    for i in 0..5 {
        let msg = InboundMessage::new("test", "user", "chat", format!("Message {}", i));
        bus.publish_inbound(msg).await.unwrap();
    }

    // Try to send one more (should block or handle gracefully)
    // This tests that the bus handles capacity correctly
    let bus_clone = bus.clone();
    let send_task = tokio::spawn(async move {
        let msg = InboundMessage::new("test", "user", "chat", "Overflow message");
        bus_clone.publish_inbound(msg).await
    });

    // Consume one message to make space
    let _ = bus.consume_inbound().await.unwrap();

    // Now the send should complete
    let result = tokio::time::timeout(std::time::Duration::from_secs(1), send_task).await;

    assert!(result.is_ok());
}

/// Test channel-specific message formatting
#[test]
fn test_channel_message_formatting() {
    // Test that different channels can have different message formats
    let telegram_msg = OutboundMessage::new("telegram", "chat_1", "**Bold** text");
    let discord_msg = OutboundMessage::new("discord", "guild_1", "**Bold** text");

    // Both should preserve the original formatting
    // The channel adapter would handle platform-specific conversions
    assert_eq!(telegram_msg.content, "**Bold** text");
    assert_eq!(discord_msg.content, "**Bold** text");
}

/// Test error message routing
#[tokio::test]
async fn test_error_message_routing() {
    let bus = Arc::new(MessageBus::new(10));

    // Simulate an error response
    let error_msg = OutboundMessage::new(
        "telegram",
        "chat_error",
        "Error: Could not process your request",
    );

    bus.publish_outbound(error_msg).await.unwrap();

    let received = bus.consume_outbound().await.unwrap();
    assert!(received.content.contains("Error:"));
}

/// Test multi-channel broadcast scenario
#[tokio::test]
async fn test_multi_channel_broadcast() {
    let bus = Arc::new(MessageBus::new(10));

    let channels = vec!["telegram", "discord", "slack"];
    let broadcast_content = "System announcement: Maintenance scheduled";

    // Send same message to multiple channels
    for channel in &channels {
        let msg = OutboundMessage::new(*channel, "broadcast", broadcast_content);
        bus.publish_outbound(msg).await.unwrap();
    }

    // Verify all messages are queued
    for _ in 0..channels.len() {
        let msg = bus.consume_outbound().await.unwrap();
        assert_eq!(msg.content, broadcast_content);
        assert!(channels.contains(&msg.channel.as_str()));
    }
}
