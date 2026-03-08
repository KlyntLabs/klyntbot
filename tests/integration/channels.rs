//! Integration tests for channel bus routing, message dispatch, capacity,
//! and channel start/send behaviour requiring async runtimes.

use klyntbot::bus::{InboundMessage, MessageBus, OutboundMessage};
use klyntbot::config::{EmailConfig, SlackConfig};
use klyntbot::ChannelManager;
use std::sync::Arc;

use super::common;

// ─────────────────────────────────────────────────────────────
// Bus routing
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn message_routes_through_bus() {
    let bus = MessageBus::new(10);

    let inbound = InboundMessage::new("telegram", "user_123", "chat_456", "Hello from Telegram!");
    bus.publish_inbound(inbound).await.unwrap();

    let mut inbound_rx = bus.take_inbound_rx().unwrap();
    let mut outbound_rx = bus.take_outbound_rx().unwrap();

    let received = inbound_rx.recv().await.unwrap();
    assert_eq!(received.channel.as_str(), "telegram");
    assert_eq!(received.sender_id, "user_123");
    assert_eq!(received.chat_id.as_str(), "chat_456");
    assert_eq!(received.content, "Hello from Telegram!");

    let response = OutboundMessage::new("telegram", "chat_456", "Hello back!");
    bus.publish_outbound(response).await.unwrap();

    let outbound = outbound_rx.recv().await.unwrap();
    assert_eq!(outbound.channel.as_str(), "telegram");
    assert_eq!(outbound.chat_id.as_str(), "chat_456");
    assert_eq!(outbound.content, "Hello back!");
}

#[tokio::test]
async fn outbound_dispatches_to_correct_channel() {
    let bus = MessageBus::new(10);

    let telegram_msg = OutboundMessage::new("telegram", "chat_1", "Telegram message");
    let discord_msg = OutboundMessage::new("discord", "guild_1", "Discord message");
    let slack_msg = OutboundMessage::new("slack", "channel_1", "Slack message");

    bus.publish_outbound(telegram_msg).await.unwrap();
    bus.publish_outbound(discord_msg).await.unwrap();
    bus.publish_outbound(slack_msg).await.unwrap();

    let mut outbound_rx = bus.take_outbound_rx().unwrap();

    let msg1 = outbound_rx.recv().await.unwrap();
    assert_eq!(msg1.channel.as_str(), "telegram");
    assert_eq!(msg1.content, "Telegram message");

    let msg2 = outbound_rx.recv().await.unwrap();
    assert_eq!(msg2.channel.as_str(), "discord");
    assert_eq!(msg2.content, "Discord message");

    let msg3 = outbound_rx.recv().await.unwrap();
    assert_eq!(msg3.channel.as_str(), "slack");
    assert_eq!(msg3.content, "Slack message");
}

#[tokio::test]
async fn concurrent_messages_all_received() {
    let bus = Arc::new(MessageBus::new(100));
    let mut inbound_rx = bus.take_inbound_rx().unwrap();

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

    for handle in handles {
        handle.await.unwrap();
    }

    let mut received = Vec::new();
    for _ in 0..10 {
        let msg = inbound_rx.recv().await.unwrap();
        received.push(msg.content);
    }

    assert_eq!(received.len(), 10);
    for i in 0..10 {
        assert!(
            received.contains(&format!("Message {}", i)),
            "Missing Message {i}"
        );
    }
}

#[tokio::test]
async fn message_metadata_preserved_through_bus() {
    let bus = MessageBus::new(10);

    let mut inbound = InboundMessage::new(
        "telegram",
        "user_alice",
        "chat_private",
        "Important message",
    );
    inbound.timestamp = chrono::Utc::now();

    bus.publish_inbound(inbound).await.unwrap();

    let mut inbound_rx = bus.take_inbound_rx().unwrap();
    let received = inbound_rx.recv().await.unwrap();
    assert_eq!(received.channel.as_str(), "telegram");
    assert_eq!(received.sender_id, "user_alice");
    assert_eq!(received.chat_id.as_str(), "chat_private");
    assert_eq!(received.content, "Important message");
    assert!(received.timestamp.timestamp() > 0);
}

#[tokio::test]
async fn bus_capacity_blocks_then_resumes() {
    let bus = MessageBus::new(5);

    for i in 0..5 {
        let msg = InboundMessage::new("test", "user", "chat", format!("Message {}", i));
        bus.publish_inbound(msg).await.unwrap();
    }

    let mut inbound_rx = bus.take_inbound_rx().unwrap();
    let bus = Arc::new(bus);

    let bus_clone = bus.clone();
    let send_task = tokio::spawn(async move {
        let msg = InboundMessage::new("test", "user", "chat", "Overflow message");
        bus_clone.publish_inbound(msg).await
    });

    // Consume one message to make space
    let _ = inbound_rx.recv().await.unwrap();

    let result = tokio::time::timeout(std::time::Duration::from_secs(1), send_task).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn error_message_routes_through_bus() {
    let bus = MessageBus::new(10);

    let error_msg = OutboundMessage::new(
        "telegram",
        "chat_error",
        "Error: Could not process your request",
    );
    bus.publish_outbound(error_msg).await.unwrap();

    let mut outbound_rx = bus.take_outbound_rx().unwrap();
    let received = outbound_rx.recv().await.unwrap();
    assert!(received.content.contains("Error:"));
}

#[tokio::test]
async fn multi_channel_broadcast() {
    let bus = MessageBus::new(10);

    let channels = vec!["telegram", "discord", "slack"];
    let broadcast_content = "System announcement: Maintenance scheduled";

    for channel in &channels {
        let msg = OutboundMessage::new(*channel, "broadcast", broadcast_content);
        bus.publish_outbound(msg).await.unwrap();
    }

    let mut outbound_rx = bus.take_outbound_rx().unwrap();

    for _ in 0..channels.len() {
        let msg = outbound_rx.recv().await.unwrap();
        assert_eq!(msg.content, broadcast_content);
        assert!(channels.contains(&msg.channel.as_str()));
    }
}

// ─────────────────────────────────────────────────────────────
// Cross-channel message identity
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn inbound_preserves_channel_identity() {
    let channels = ["telegram", "discord", "slack", "email"];
    for channel in channels {
        let msg = InboundMessage::new(channel, "user", "chat", "Hello");
        assert_eq!(msg.channel.as_str(), channel);
        assert_eq!(msg.session_key().to_string(), format!("{}:chat", channel));
    }
}

#[tokio::test]
async fn outbound_preserves_channel_identity() {
    let channels = ["telegram", "discord", "slack", "email"];
    for channel in channels {
        let msg = OutboundMessage::new(channel, "chat", "Reply");
        assert_eq!(msg.channel.as_str(), channel);
    }
}

#[tokio::test]
async fn bus_routes_inbound_in_order() {
    let bus = MessageBus::new(50);

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
// Channel start/send behaviour
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn slack_start_rejects_empty_tokens() {
    use klyntbot::channels::{Channel, SlackChannel};
    let config = SlackConfig::default();
    let channel = SlackChannel::new(config).unwrap();
    let bus = common::test_message_bus();
    let result = channel.start(bus).await;
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not configured"));
}

#[tokio::test]
async fn email_start_rejects_missing_consent() {
    use klyntbot::channels::{Channel, EmailChannel};
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
async fn email_start_rejects_missing_config_fields() {
    use klyntbot::channels::{Channel, EmailChannel};
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
async fn email_send_skips_when_auto_reply_disabled() {
    use klyntbot::channels::{Channel, EmailChannel};
    let config = EmailConfig {
        auto_reply_enabled: false,
        ..Default::default()
    };
    let channel = EmailChannel::new(config).unwrap();
    let msg = OutboundMessage::new("email", "user@example.com", "Hello");
    let result = channel.send(&msg).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn email_send_rejects_empty_recipient() {
    use klyntbot::channels::{Channel, EmailChannel};
    let config = EmailConfig::default();
    let channel = EmailChannel::new(config).unwrap();
    let msg = OutboundMessage::new("email", "", "Hello");
    let result = channel.send(&msg).await;
    assert!(result.is_err());
}

// ─────────────────────────────────────────────────────────────
// ChannelManager wiring
// ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn channel_manager_creates_with_no_channels() {
    let (temp_dir, _workspace) = common::test_workspace();
    let config = common::test_config(&temp_dir);
    let bus = common::test_message_bus();
    let _manager =
        ChannelManager::new(Arc::new(config), bus).expect("Failed to create channel manager");
}

#[tokio::test]
async fn channel_manager_initializes_with_none_enabled() {
    let (temp_dir, _workspace) = common::test_workspace();
    let config = common::test_config(&temp_dir);
    let bus = common::test_message_bus();
    let manager =
        ChannelManager::new(Arc::new(config), bus).expect("Failed to create channel manager");
    let result = manager.initialize_channels().await;
    assert!(result.is_ok());
}

// -----------------------------------------------------------------
// Bus message ordering
// -----------------------------------------------------------------

#[tokio::test]
async fn bus_preserves_message_ordering() {
    let bus = MessageBus::new(100);

    // Send multiple messages in order
    for i in 0..10 {
        let msg = InboundMessage::new("test", "user", "chat", format!("Message {}", i));
        bus.publish_inbound(msg).await.unwrap();
    }

    // Get receiver
    let mut inbound_rx = bus.take_inbound_rx().unwrap();

    // Verify they come out in order
    for i in 0..10 {
        let received = inbound_rx.recv().await.unwrap();
        assert_eq!(received.content, format!("Message {}", i));
    }
}
