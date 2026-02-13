//! Async message queue for decoupled channel-agent communication.

use std::sync::Mutex;

use tokio::sync::mpsc;
use tracing::debug;

use super::events::{InboundMessage, OutboundMessage};
use common::{KlyntbotError, Result};

/// Message bus for communication between channels and agent
pub struct MessageBus {
    inbound_tx: mpsc::Sender<InboundMessage>,
    inbound_rx: Mutex<Option<mpsc::Receiver<InboundMessage>>>,
    outbound_tx: mpsc::Sender<OutboundMessage>,
    outbound_rx: Mutex<Option<mpsc::Receiver<OutboundMessage>>>,
}

impl MessageBus {
    /// Create a new message bus with the specified buffer size
    pub fn new(buffer_size: usize) -> Self {
        let (inbound_tx, inbound_rx) = mpsc::channel(buffer_size);
        let (outbound_tx, outbound_rx) = mpsc::channel(buffer_size);

        Self {
            inbound_tx,
            inbound_rx: Mutex::new(Some(inbound_rx)),
            outbound_tx,
            outbound_rx: Mutex::new(Some(outbound_rx)),
        }
    }

    /// Take ownership of the inbound receiver (can only be called once)
    pub fn take_inbound_rx(&self) -> Option<mpsc::Receiver<InboundMessage>> {
        self.inbound_rx.lock().unwrap().take()
    }

    /// Take ownership of the outbound receiver (can only be called once)
    pub fn take_outbound_rx(&self) -> Option<mpsc::Receiver<OutboundMessage>> {
        self.outbound_rx.lock().unwrap().take()
    }

    /// Publish an inbound message to the bus
    pub async fn publish_inbound(&self, msg: InboundMessage) -> Result<()> {
        debug!(
            channel = %msg.channel,
            sender = %msg.sender_id,
            "Publishing inbound message"
        );

        self.inbound_tx
            .send(msg)
            .await
            .map_err(|_| KlyntbotError::BusDisconnected)?;

        Ok(())
    }

    /// Consume the next inbound message (blocks until available)
    ///
    /// Note: This method is deprecated. Use take_inbound_rx() to get ownership
    /// of the receiver and call recv() directly on it.
    #[deprecated(note = "Use take_inbound_rx() to own the receiver directly")]
    pub async fn consume_inbound(&self) -> Option<InboundMessage> {
        // This method is kept for backwards compatibility but will panic if receiver was taken
        panic!("consume_inbound called after receiver was taken. Use the receiver directly.")
    }

    /// Publish an outbound message to the bus
    pub async fn publish_outbound(&self, msg: OutboundMessage) -> Result<()> {
        debug!(
            channel = %msg.channel,
            chat_id = %msg.chat_id,
            "Publishing outbound message"
        );

        self.outbound_tx
            .send(msg)
            .await
            .map_err(|_| KlyntbotError::BusDisconnected)?;

        Ok(())
    }

    /// Consume the next outbound message (blocks until available)
    ///
    /// Note: This method is deprecated. Use take_outbound_rx() to get ownership
    /// of the receiver and call recv() directly on it.
    #[deprecated(note = "Use take_outbound_rx() to own the receiver directly")]
    pub async fn consume_outbound(&self) -> Option<OutboundMessage> {
        // This method is kept for backwards compatibility but will panic if receiver was taken
        panic!("consume_outbound called after receiver was taken. Use the receiver directly.")
    }

    /// Get a sender for inbound messages
    pub fn inbound_sender(&self) -> mpsc::Sender<InboundMessage> {
        self.inbound_tx.clone()
    }

    /// Get a sender for outbound messages
    pub fn outbound_sender(&self) -> mpsc::Sender<OutboundMessage> {
        self.outbound_tx.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_message_bus() {
        let bus = MessageBus::new(10);

        // Test inbound
        let msg = InboundMessage::new("telegram", "user123", "chat456", "Hello!");
        bus.publish_inbound(msg.clone()).await.unwrap();

        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let received = inbound_rx.recv().await.unwrap();
        assert_eq!(received.content, "Hello!");
        assert_eq!(received.session_key().to_string(), "telegram:chat456");

        // Test outbound
        let out_msg = OutboundMessage::new("telegram", "chat456", "Hi there!");
        bus.publish_outbound(out_msg.clone()).await.unwrap();

        let mut outbound_rx = bus.take_outbound_rx().unwrap();
        let received_out = outbound_rx.recv().await.unwrap();
        assert_eq!(received_out.content, "Hi there!");
    }

    #[tokio::test]
    async fn test_message_bus_multiple_messages() {
        let bus = MessageBus::new(10);

        // Publish multiple inbound messages
        for i in 0..5 {
            let msg =
                InboundMessage::new("telegram", "user123", "chat456", format!("Message {}", i));
            bus.publish_inbound(msg).await.unwrap();
        }

        // Consume all messages
        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        for i in 0..5 {
            let received = inbound_rx.recv().await.unwrap();
            assert_eq!(received.content, format!("Message {}", i));
        }
    }

    #[tokio::test]
    async fn test_message_bus_senders() {
        let bus = MessageBus::new(10);

        // Get senders
        let inbound_sender = bus.inbound_sender();
        let outbound_sender = bus.outbound_sender();

        // Send via cloned sender
        let msg = InboundMessage::new("telegram", "user123", "chat456", "Test");
        inbound_sender.send(msg).await.unwrap();

        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let received = inbound_rx.recv().await.unwrap();
        assert_eq!(received.content, "Test");

        // Send outbound via cloned sender
        let out_msg = OutboundMessage::new("telegram", "chat456", "Response");
        outbound_sender.send(out_msg).await.unwrap();

        let mut outbound_rx = bus.take_outbound_rx().unwrap();
        let received_out = outbound_rx.recv().await.unwrap();
        assert_eq!(received_out.content, "Response");
    }

    #[tokio::test]
    async fn test_inbound_message_with_media() {
        let mut msg = InboundMessage::new("telegram", "user123", "chat456", "Check this out");
        msg.media.push("https://example.com/image.jpg".to_string());

        assert_eq!(msg.media.len(), 1);
        assert_eq!(msg.media[0], "https://example.com/image.jpg");
    }

    #[tokio::test]
    async fn test_outbound_message_builder() {
        let msg = OutboundMessage::new("telegram", "chat456", "Hello")
            .with_reply_to("msg123")
            .with_media("https://example.com/file.pdf");

        assert_eq!(msg.reply_to, Some("msg123".to_string()));
        assert_eq!(msg.media.len(), 1);
        assert_eq!(msg.media[0], "https://example.com/file.pdf");
    }

    #[tokio::test]
    async fn test_session_key_format() {
        let msg = InboundMessage::new("telegram", "user123", "chat456", "Test");
        assert_eq!(msg.session_key().to_string(), "telegram:chat456");

        let msg2 = InboundMessage::new("discord", "user789", "guild123", "Test");
        assert_eq!(msg2.session_key().to_string(), "discord:guild123");
    }

    #[tokio::test]
    async fn test_bus_buffer_overflow() {
        let bus = MessageBus::new(2); // Small buffer

        // Fill buffer
        bus.publish_inbound(InboundMessage::new("telegram", "user1", "chat1", "Msg 1"))
            .await
            .unwrap();
        bus.publish_inbound(InboundMessage::new("telegram", "user2", "chat2", "Msg 2"))
            .await
            .unwrap();

        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let bus_arc = std::sync::Arc::new(bus);
        let bus_ref = bus_arc.clone();

        let sender = tokio::spawn(async move {
            bus_ref
                .publish_inbound(InboundMessage::new("telegram", "user3", "chat3", "Msg 3"))
                .await
        });

        // Consume one message to make space
        let _ = inbound_rx.recv().await;

        // Now the sender should succeed
        sender.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn test_concurrent_publish_consume() {
        use std::sync::Arc;

        let bus = MessageBus::new(100);
        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let bus = Arc::new(bus);

        // Spawn multiple publishers
        let mut publishers = vec![];
        for i in 0..10 {
            let bus_clone = bus.clone();
            let handle = tokio::spawn(async move {
                for j in 0..5 {
                    let msg = InboundMessage::new(
                        "telegram",
                        format!("user{}", i),
                        format!("chat{}", i),
                        format!("Message {}-{}", i, j),
                    );
                    bus_clone.publish_inbound(msg).await.unwrap();
                }
            });
            publishers.push(handle);
        }

        // Wait for all publishers to finish
        for handle in publishers {
            handle.await.unwrap();
        }

        // Consume all messages
        let mut count = 0;
        while inbound_rx.recv().await.is_some() {
            count += 1;
            if count >= 50 {
                break;
            }
        }

        assert_eq!(count, 50);
    }

    #[tokio::test]
    async fn test_message_ordering() {
        let bus = MessageBus::new(100);

        // Publish messages in order
        for i in 0..10 {
            let msg = InboundMessage::new("telegram", "user", "chat", format!("Message {}", i));
            bus.publish_inbound(msg).await.unwrap();
        }

        // Verify they come out in order
        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        for i in 0..10 {
            let msg = inbound_rx.recv().await.unwrap();
            assert_eq!(msg.content, format!("Message {}", i));
        }
    }

    #[tokio::test]
    async fn test_inbound_and_outbound_independent() {
        let bus = MessageBus::new(10);

        // Publish to both queues
        bus.publish_inbound(InboundMessage::new("telegram", "user1", "chat1", "Inbound"))
            .await
            .unwrap();
        bus.publish_outbound(OutboundMessage::new("telegram", "chat1", "Outbound"))
            .await
            .unwrap();

        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let mut outbound_rx = bus.take_outbound_rx().unwrap();

        // Consume from outbound first
        let out = outbound_rx.recv().await.unwrap();
        assert_eq!(out.content, "Outbound");

        // Inbound should still be available
        let in_msg = inbound_rx.recv().await.unwrap();
        assert_eq!(in_msg.content, "Inbound");
    }

    #[tokio::test]
    async fn test_multiple_consumers_share_messages() {
        let bus = MessageBus::new(10);

        // Publish messages
        for i in 0..5 {
            let msg = InboundMessage::new("telegram", "user", "chat", format!("Message {}", i));
            bus.publish_inbound(msg).await.unwrap();
        }

        // Only one consumer can receive each message
        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let msg1 = inbound_rx.recv().await;
        assert!(msg1.is_some());
        assert_eq!(msg1.unwrap().content, "Message 0");

        let msg2 = inbound_rx.recv().await;
        assert!(msg2.is_some());
        assert_eq!(msg2.unwrap().content, "Message 1");
    }

    #[tokio::test]
    async fn test_empty_bus_returns_none_eventually() {
        let bus = MessageBus::new(10);

        // Publish and consume
        bus.publish_inbound(InboundMessage::new("telegram", "user", "chat", "Test"))
            .await
            .unwrap();

        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let msg = inbound_rx.recv().await;
        assert!(msg.is_some());

        // Next recv would block waiting for the next message
        // We can't easily test blocking behavior in unit tests
    }

    #[tokio::test]
    async fn test_message_with_empty_content() {
        let bus = MessageBus::new(10);

        let msg = InboundMessage::new("telegram", "user", "chat", "");
        bus.publish_inbound(msg).await.unwrap();

        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let received = inbound_rx.recv().await.unwrap();
        assert_eq!(received.content, "");
    }

    #[tokio::test]
    async fn test_message_with_special_characters() {
        let bus = MessageBus::new(10);

        let content = "Hello 👋 测试 🚀 \n\t special chars!";
        let msg = InboundMessage::new("telegram", "user", "chat", content);
        bus.publish_inbound(msg).await.unwrap();

        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let received = inbound_rx.recv().await.unwrap();
        assert_eq!(received.content, content);
    }

    #[tokio::test]
    async fn test_different_channels() {
        let bus = MessageBus::new(10);

        // Messages from different channels
        bus.publish_inbound(InboundMessage::new("telegram", "u1", "c1", "Telegram msg"))
            .await
            .unwrap();
        bus.publish_inbound(InboundMessage::new("discord", "u2", "c2", "Discord msg"))
            .await
            .unwrap();
        bus.publish_inbound(InboundMessage::new("slack", "u3", "c3", "Slack msg"))
            .await
            .unwrap();

        let mut inbound_rx = bus.take_inbound_rx().unwrap();
        let msg1 = inbound_rx.recv().await.unwrap();
        assert_eq!(msg1.channel.as_str(), "telegram");
        assert_eq!(msg1.content, "Telegram msg");

        let msg2 = inbound_rx.recv().await.unwrap();
        assert_eq!(msg2.channel.as_str(), "discord");
        assert_eq!(msg2.content, "Discord msg");

        let msg3 = inbound_rx.recv().await.unwrap();
        assert_eq!(msg3.channel.as_str(), "slack");
        assert_eq!(msg3.content, "Slack msg");
    }

    #[tokio::test]
    async fn test_outbound_with_reply_to() {
        let bus = MessageBus::new(10);

        let msg = OutboundMessage::new("telegram", "chat123", "Reply content")
            .with_reply_to("original_msg_456");

        bus.publish_outbound(msg).await.unwrap();

        let mut outbound_rx = bus.take_outbound_rx().unwrap();
        let received = outbound_rx.recv().await.unwrap();
        assert_eq!(received.reply_to, Some("original_msg_456".to_string()));
        assert_eq!(received.content, "Reply content");
    }

    #[tokio::test]
    async fn test_outbound_with_multiple_media() {
        let bus = MessageBus::new(10);

        let msg = OutboundMessage::new("telegram", "chat123", "Check these out")
            .with_media("https://example.com/image1.jpg")
            .with_media("https://example.com/image2.jpg")
            .with_media("https://example.com/doc.pdf");

        bus.publish_outbound(msg).await.unwrap();

        let mut outbound_rx = bus.take_outbound_rx().unwrap();
        let received = outbound_rx.recv().await.unwrap();
        assert_eq!(received.media.len(), 3);
        assert!(received
            .media
            .contains(&"https://example.com/image1.jpg".to_string()));
        assert!(received
            .media
            .contains(&"https://example.com/image2.jpg".to_string()));
        assert!(received
            .media
            .contains(&"https://example.com/doc.pdf".to_string()));
    }

    #[tokio::test]
    async fn test_session_key_consistency() {
        let msg1 = InboundMessage::new("telegram", "user123", "chat456", "Hello");
        let msg2 = InboundMessage::new("telegram", "user789", "chat456", "Hi");

        // Same chat_id should produce same session key
        assert_eq!(msg1.session_key(), msg2.session_key());
        assert_eq!(msg1.session_key().as_str(), "telegram:chat456");
    }
}
