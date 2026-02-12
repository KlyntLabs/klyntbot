//! WhatsApp channel via WebSocket bridge to Node.js Baileys.

use async_trait::async_trait;
use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_tungstenite::{
    connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, warn};

use bus::{InboundMessage, MessageBus, OutboundMessage};
use crate::{check_allowlist, Channel};
use config::WhatsAppConfig;
use common::{ChannelError, Result};

type WsWriter = SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, WsMessage>;

/// WhatsApp channel implementation
pub struct WhatsAppChannel {
    config: WhatsAppConfig,
    running: Arc<AtomicBool>,
    ws_writer: Arc<Mutex<Option<WsWriter>>>,
}

impl WhatsAppChannel {
    /// Create a new WhatsApp channel
    pub fn new(config: WhatsAppConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
            ws_writer: Arc::new(Mutex::new(None)),
        }
    }

    /// WebSocket connection loop
    async fn ws_loop(&self, bus: Arc<MessageBus>) -> Result<()> {
        info!("Connecting to WhatsApp bridge: {}", self.config.bridge_url);

        let (ws_stream, _) = connect_async(&self.config.bridge_url)
            .await
            .map_err(|e| ChannelError::ConnectionFailed(e.to_string()))?;

        let (write, mut read) = ws_stream.split();

        // Store the write half for sending messages
        {
            let mut writer = self.ws_writer.lock().await;
            *writer = Some(write);
        }

        info!("Connected to WhatsApp bridge");

        while self.running.load(Ordering::SeqCst) {
            let msg = match tokio::time::timeout(Duration::from_secs(30), read.next()).await {
                Ok(Some(Ok(msg))) => msg,
                Ok(Some(Err(e))) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                Ok(None) => {
                    warn!("WebSocket closed");
                    break;
                }
                Err(_) => {
                    // Timeout - send ping
                    let mut writer = self.ws_writer.lock().await;
                    if let Some(ws) = writer.as_mut() {
                        if let Err(e) = ws.send(WsMessage::Ping(vec![].into())).await {
                            error!("Failed to send ping: {}", e);
                            break;
                        }
                    }
                    continue;
                }
            };

            if let WsMessage::Text(text) = msg {
                if let Err(e) = self.handle_message(&text, &bus).await {
                    error!("Error handling message: {}", e);
                }
            }
        }

        // Clear the writer when connection ends
        {
            let mut writer = self.ws_writer.lock().await;
            *writer = None;
        }

        Ok(())
    }

    /// Handle a message from the bridge
    async fn handle_message(&self, text: &str, bus: &MessageBus) -> Result<()> {
        let data: Value = serde_json::from_str(text)
            .map_err(|e| ChannelError::SendFailed(format!("Failed to parse message: {}", e)))?;

        let msg_type = data.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match msg_type {
            "qr" => {
                // QR code for authentication
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
impl Channel for WhatsAppChannel {
    fn name(&self) -> &str {
        "whatsapp"
    }

    async fn start(&self, bus: Arc<MessageBus>) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        super::reconnect_loop("WhatsApp", &self.running, || self.ws_loop(bus.clone())).await;

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        let mut writer = self.ws_writer.lock().await;

        let ws = writer
            .as_mut()
            .ok_or_else(|| ChannelError::SendFailed("WhatsApp bridge not connected".to_string()))?;

        // Create JSON payload matching Python implementation
        let payload = json!({
            "type": "send",
            "to": msg.chat_id,
            "text": msg.content
        });

        let payload_str = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::SendFailed(format!("Failed to serialize message: {}", e)))?;

        ws.send(WsMessage::Text(payload_str.into()))
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to send message: {}", e)))?;

        debug!("Sent WhatsApp message to {}: {}", msg.chat_id, msg.content);

        Ok(())
    }

    fn is_allowed(&self, sender_id: &str) -> bool {
        check_allowlist(&self.config.allow_from, sender_id)
    }
}
