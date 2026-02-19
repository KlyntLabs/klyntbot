//! WhatsApp channel via WebSocket bridge to Node.js Baileys.

use async_trait::async_trait;
use futures_util::SinkExt;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{debug, error, info, warn};

use crate::ws_manager::{HeartbeatStrategy, WebSocketManager, WsConfig, WsHandler, WsSink};
use crate::{check_allowlist, Channel};
use bus::{InboundMessage, MessageBus, OutboundMessage};
use common::{ChannelError, Result};
use config::WhatsAppConfig;

/// WhatsApp channel implementation
pub struct WhatsAppChannel {
    config: WhatsAppConfig,
    running: Arc<AtomicBool>,
    bus: Mutex<Option<Arc<MessageBus>>>,
    ws_writer: Arc<Mutex<Option<Arc<Mutex<WsSink>>>>>,
}

impl WhatsAppChannel {
    /// Create a new WhatsApp channel
    pub fn new(config: WhatsAppConfig) -> Result<Self> {
        Ok(Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            bus: Mutex::new(None),
            ws_writer: Arc::new(Mutex::new(None)),
        })
    }

    /// Handle a message from the bridge
    async fn handle_message(&self, text: &str, bus: &MessageBus) -> Result<()> {
        let data: Value = serde_json::from_str(text)
            .map_err(|e| ChannelError::SendFailed(format!("Failed to parse message: {}", e)))?;

        let msg_type = data.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match msg_type {
            "qr" => {
                if let Some(qr) = data.get("qr").and_then(|v| v.as_str()) {
                    info!("WhatsApp QR code: {}", qr);
                    info!("Scan this QR code with WhatsApp to authenticate");
                }
            }
            "message" => {
                self.handle_incoming_message(&data, bus).await?;
            }
            "status" => {
                if let Some(status) = data.get("status").and_then(|v| v.as_str()) {
                    info!("WhatsApp status: {}", status);
                }
            }
            "error" => {
                if let Some(error) = data.get("error").and_then(|v| v.as_str()) {
                    error!("WhatsApp error: {}", error);
                }
            }
            _ => {
                debug!("Unknown message type: {}", msg_type);
            }
        }

        Ok(())
    }

    /// Handle an incoming message
    async fn handle_incoming_message(&self, data: &Value, bus: &MessageBus) -> Result<()> {
        let sender_id = data.get("from").and_then(|v| v.as_str()).unwrap_or("");

        let chat_id = data
            .get("chatId")
            .and_then(|v| v.as_str())
            .unwrap_or(sender_id);

        let content = data.get("body").and_then(|v| v.as_str()).unwrap_or("");

        if sender_id.is_empty() {
            return Ok(());
        }

        // Check allowlist
        if !check_allowlist(&self.config.allow_from, sender_id) {
            warn!("Access denied for sender {} on WhatsApp", sender_id);
            return Ok(());
        }

        debug!("WhatsApp message from {}: {}", sender_id, content);

        // Publish to bus
        let inbound = InboundMessage::new("whatsapp", sender_id, chat_id, content);
        bus.publish_inbound(inbound)
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to publish to bus: {}", e)))?;

        Ok(())
    }
}

#[async_trait]
impl WsHandler for WhatsAppChannel {
    async fn on_connected(&self, write: &Arc<Mutex<WsSink>>) -> Result<Option<HeartbeatStrategy>> {
        // Store the write half for send()
        *self.ws_writer.lock().await = Some(Arc::clone(write));
        info!("Connected to WhatsApp bridge");
        Ok(None) // Use default heartbeat from config
    }

    async fn on_text_message(&self, text: &str, _write: &Arc<Mutex<WsSink>>) -> Result<bool> {
        let bus_guard = self.bus.lock().await;
        if let Some(bus) = bus_guard.as_ref() {
            self.handle_message(text, bus).await?;
        }
        Ok(true)
    }

    async fn on_disconnected(&self) {
        *self.ws_writer.lock().await = None;
    }
}

#[async_trait]
impl Channel for WhatsAppChannel {
    fn name(&self) -> &str {
        "whatsapp"
    }

    async fn start(&self, bus: Arc<MessageBus>) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        *self.bus.lock().await = Some(bus);

        let config = WsConfig {
            url: self.config.bridge_url.clone(),
            heartbeat: HeartbeatStrategy::Timeout {
                timeout: Duration::from_secs(30),
                build_payload: Box::new(|| WsMessage::Ping(vec![].into())),
            },
            ..Default::default()
        };

        let manager = WebSocketManager::new(self.running.clone());

        super::reconnect_loop("WhatsApp", &self.running, || manager.run(&config, self)).await;

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        let writer_guard = self.ws_writer.lock().await;

        let ws = writer_guard
            .as_ref()
            .ok_or_else(|| ChannelError::SendFailed("WhatsApp bridge not connected".to_string()))?;

        let payload = json!({
            "type": "send",
            "to": msg.chat_id,
            "text": msg.content
        });

        let payload_str = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::SendFailed(format!("Failed to serialize message: {}", e)))?;

        let mut w = ws.lock().await;
        w.send(WsMessage::Text(payload_str.into()))
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to send message: {}", e)))?;

        debug!("Sent WhatsApp message to {}: {}", msg.chat_id, msg.content);

        Ok(())
    }

    fn is_allowed(&self, sender_id: &str) -> bool {
        check_allowlist(&self.config.allow_from, sender_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> WhatsAppConfig {
        WhatsAppConfig::default()
    }

    fn make_config_with_allowlist(allow: Vec<String>) -> WhatsAppConfig {
        WhatsAppConfig {
            allow_from: allow,
            ..Default::default()
        }
    }

    #[test]
    fn test_channel_name() {
        let channel = WhatsAppChannel::new(make_config()).unwrap();
        assert_eq!(channel.name(), "whatsapp");
    }

    #[test]
    fn test_default_config_values() {
        let config = WhatsAppConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.bridge_url, "ws://localhost:3001");
        assert!(config.allow_from.is_empty());
    }

    #[test]
    fn test_is_allowed_empty_allowlist() {
        let channel = WhatsAppChannel::new(make_config()).unwrap();
        assert!(channel.is_allowed("anyone"));
        assert!(channel.is_allowed("+1234567890"));
    }

    #[test]
    fn test_is_allowed_with_allowlist() {
        let channel =
            WhatsAppChannel::new(make_config_with_allowlist(vec!["+1234567890".to_string()]))
                .unwrap();
        assert!(channel.is_allowed("+1234567890"));
        assert!(!channel.is_allowed("+9876543210"));
    }

    #[test]
    fn test_is_allowed_compound_id() {
        let channel =
            WhatsAppChannel::new(make_config_with_allowlist(vec!["+1234567890".to_string()]))
                .unwrap();
        assert!(channel.is_allowed("+1234567890|name"));
        assert!(!channel.is_allowed("+0000000000|name"));
    }

    #[test]
    fn test_message_parse_incoming() {
        let payload = json!({
            "type": "message",
            "from": "+1234567890",
            "chatId": "chat123",
            "body": "Hello there!"
        });

        let msg_type = payload.get("type").and_then(|v| v.as_str()).unwrap();
        assert_eq!(msg_type, "message");
        let from = payload.get("from").and_then(|v| v.as_str()).unwrap();
        assert_eq!(from, "+1234567890");
        let body = payload.get("body").and_then(|v| v.as_str()).unwrap();
        assert_eq!(body, "Hello there!");
    }

    #[test]
    fn test_message_parse_qr() {
        let payload = json!({
            "type": "qr",
            "qr": "2@abc123"
        });

        let msg_type = payload.get("type").and_then(|v| v.as_str()).unwrap();
        assert_eq!(msg_type, "qr");
        let qr = payload.get("qr").and_then(|v| v.as_str()).unwrap();
        assert!(!qr.is_empty());
    }

    #[test]
    fn test_message_parse_status() {
        let payload = json!({
            "type": "status",
            "status": "connected"
        });

        let msg_type = payload.get("type").and_then(|v| v.as_str()).unwrap();
        assert_eq!(msg_type, "status");
    }

    #[test]
    fn test_message_parse_error() {
        let payload = json!({
            "type": "error",
            "error": "connection lost"
        });

        let msg_type = payload.get("type").and_then(|v| v.as_str()).unwrap();
        assert_eq!(msg_type, "error");
    }

    #[test]
    fn test_empty_sender_ignored() {
        let payload = json!({
            "type": "message",
            "from": "",
            "chatId": "chat1",
            "body": "test"
        });

        let from = payload.get("from").and_then(|v| v.as_str()).unwrap_or("");
        assert!(from.is_empty());
    }

    #[test]
    fn test_send_payload_format() {
        let chat_id = "+1234567890";
        let content = "Hello!";
        let payload = json!({
            "type": "send",
            "to": chat_id,
            "text": content
        });

        assert_eq!(payload["type"], "send");
        assert_eq!(payload["to"], chat_id);
        assert_eq!(payload["text"], content);
    }

    #[tokio::test]
    async fn test_stop_sets_running_false() {
        let channel = WhatsAppChannel::new(make_config()).unwrap();
        channel.running.store(true, Ordering::SeqCst);
        channel.stop().await.unwrap();
        assert!(!channel.running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_chat_id_falls_back_to_sender() {
        let payload = json!({
            "type": "message",
            "from": "+1234567890",
            "body": "test"
        });

        let sender_id = payload.get("from").and_then(|v| v.as_str()).unwrap_or("");
        let chat_id = payload
            .get("chatId")
            .and_then(|v| v.as_str())
            .unwrap_or(sender_id);
        assert_eq!(chat_id, "+1234567890");
    }
}
