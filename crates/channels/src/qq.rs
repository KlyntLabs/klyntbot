//! QQ channel using direct WebSocket connection to QQ Bot API.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{debug, info, warn};

use crate::ws_manager::{HeartbeatStrategy, WebSocketManager, WsConfig, WsHandler, WsSink};
use crate::{check_allowlist, Channel};
use bus::{InboundMessage, MessageBus, OutboundMessage};
use common::{ChannelError, Result};
use config::QQConfig;

const QQ_API_BASE: &str = "https://api.sgroup.qq.com";
const QQ_WS_URL: &str = "wss://api.sgroup.qq.com/websocket";

/// QQ channel implementation
pub struct QQChannel {
    config: QQConfig,
    client: Client,
    access_token: Arc<RwLock<Option<String>>>,
    processed_ids: Arc<RwLock<VecDeque<String>>>,
    running: Arc<AtomicBool>,
    bus: Mutex<Option<Arc<MessageBus>>>,
    seq: RwLock<Option<i64>>,
}

#[derive(Debug, Deserialize)]
struct AuthResponse {
    access_token: String,
    expires_in: u64,
}

#[derive(Debug, Deserialize)]
struct WsPayload {
    op: i32,
    d: Option<Value>,
    s: Option<i64>,
    t: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HelloPayload {
    heartbeat_interval: u64,
}

#[derive(Debug, Deserialize)]
struct C2CMessageCreate {
    id: String,
    author: Author,
    content: String,
}

#[derive(Debug, Deserialize)]
struct Author {
    id: Option<String>,
    user_openid: Option<String>,
}

impl QQChannel {
    /// Create a new QQ channel
    pub fn new(config: QQConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ChannelError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            config,
            client,
            access_token: Arc::new(RwLock::new(None)),
            processed_ids: Arc::new(RwLock::new(VecDeque::with_capacity(1000))),
            running: Arc::new(AtomicBool::new(false)),
            bus: Mutex::new(None),
            seq: RwLock::new(None),
        })
    }

    /// Authenticate and get access token
    async fn authenticate(&self) -> Result<String> {
        let url = format!("{}/app/getAppAccessToken", QQ_API_BASE);
        let payload = json!({
            "appId": self.config.app_id,
            "clientSecret": self.config.secret.expose(),
        });

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionFailed(format!("Auth request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ChannelError::ConnectionFailed(format!(
                "Auth failed HTTP {}: {}",
                status, text
            ))
            .into());
        }

        let auth: AuthResponse = response.json().await.map_err(|e| {
            ChannelError::ConnectionFailed(format!("Failed to parse auth response: {}", e))
        })?;

        info!("QQ authenticated, token expires in {}s", auth.expires_in);
        Ok(auth.access_token)
    }

    /// Handle a gateway event
    async fn handle_gateway_event(&self, text: &str, bus: &MessageBus) -> Result<bool> {
        let payload: WsPayload = serde_json::from_str(text).map_err(|e| {
            ChannelError::SendFailed(format!("Failed to parse gateway message: {}", e))
        })?;

        // Update sequence
        if let Some(s) = payload.s {
            *self.seq.write().await = Some(s);
        }

        match payload.op {
            10 => {
                // HELLO
                info!("QQ Gateway HELLO received");
                if let Some(d) = payload.d {
                    if let Ok(hello) = serde_json::from_value::<HelloPayload>(d) {
                        info!("Heartbeat interval: {}ms", hello.heartbeat_interval);
                    }
                }
            }
            0 => {
                // Dispatch event
                if let Some(event_type) = payload.t.as_deref() {
                    match event_type {
                        "READY" => {
                            info!("QQ Gateway READY");
                            if let Some(d) = payload.d {
                                debug!("Ready payload: {:?}", d);
                            }
                        }
                        "C2C_MESSAGE_CREATE" | "DIRECT_MESSAGE_CREATE" => {
                            if let Some(d) = payload.d {
                                self.handle_c2c_message(&d, bus).await?;
                            }
                        }
                        _ => {
                            debug!("Unhandled event: {}", event_type);
                        }
                    }
                }
            }
            7 => {
                // RECONNECT
                info!("QQ Gateway requested reconnect");
                return Ok(false);
            }
            9 => {
                // INVALID_SESSION
                warn!("QQ Gateway invalid session");
                return Ok(false);
            }
            11 => {
                // HEARTBEAT_ACK
                debug!("Heartbeat ACK received");
            }
            _ => {
                debug!("Unknown opcode: {}", payload.op);
            }
        }

        Ok(true)
    }

    /// Handle C2C (direct/private) message
    async fn handle_c2c_message(&self, data: &Value, bus: &MessageBus) -> Result<()> {
        let msg: C2CMessageCreate = serde_json::from_value(data.clone())
            .map_err(|e| ChannelError::SendFailed(format!("Failed to parse C2C message: {}", e)))?;

        // Deduplication
        {
            let mut processed = self.processed_ids.write().await;
            if processed.contains(&msg.id) {
                return Ok(());
            }
            processed.push_back(msg.id.clone());
            if processed.len() > 1000 {
                processed.pop_front();
            }
        }

        let user_id = msg
            .author
            .id
            .or(msg.author.user_openid)
            .unwrap_or_else(|| "unknown".to_string());

        let content = msg.content.trim();
        if content.is_empty() {
            return Ok(());
        }

        // Check allowlist
        if !check_allowlist(&self.config.allow_from, &user_id) {
            warn!("Access denied for sender {} on QQ", user_id);
            return Ok(());
        }

        debug!("QQ message from {}: {}", user_id, content);

        // Publish to bus
        let inbound = InboundMessage::new("qq", user_id.as_str(), user_id.as_str(), content);
        bus.publish_inbound(inbound)
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to publish to bus: {}", e)))?;

        Ok(())
    }

    /// Send a C2C message via REST API
    async fn send_c2c_message(&self, openid: &str, content: &str) -> Result<()> {
        let token = self
            .access_token
            .read()
            .await
            .clone()
            .ok_or_else(|| ChannelError::SendFailed("No access token".to_string()))?;

        let url = format!("{}/v2/users/{}/messages", QQ_API_BASE, openid);
        let payload = json!({
            "content": content,
            "msg_type": 0,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("QQBot {}", token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!("HTTP {}: {}", status, text)).into());
        }

        Ok(())
    }
}

#[async_trait]
impl WsHandler for QQChannel {
    async fn on_connected(&self, _write: &Arc<Mutex<WsSink>>) -> Result<Option<HeartbeatStrategy>> {
        info!("Connected to QQ Gateway");
        Ok(None) // Use default heartbeat from config
    }

    async fn on_text_message(&self, text: &str, _write: &Arc<Mutex<WsSink>>) -> Result<bool> {
        let bus_guard = self.bus.lock().await;
        if let Some(bus) = bus_guard.as_ref() {
            return self.handle_gateway_event(text, bus).await;
        }
        Ok(true)
    }

    async fn on_disconnected(&self) {
        debug!("Disconnected from QQ Gateway");
    }
}

#[async_trait]
impl Channel for QQChannel {
    fn name(&self) -> &str {
        "qq"
    }

    async fn start(&self, bus: Arc<MessageBus>) -> Result<()> {
        if self.config.app_id.is_empty() || self.config.secret.is_empty() {
            return Err(ChannelError::ConnectionFailed(
                "QQ app_id and secret not configured".to_string(),
            )
            .into());
        }

        self.running.store(true, Ordering::SeqCst);

        // Authenticate before connecting
        let token = self.authenticate().await?;
        *self.access_token.write().await = Some(token);
        *self.bus.lock().await = Some(bus);

        let config = WsConfig {
            url: QQ_WS_URL.to_string(),
            heartbeat: HeartbeatStrategy::Timeout {
                timeout: Duration::from_secs(35),
                build_payload: Box::new(move || {
                    // Build heartbeat with current sequence (best-effort read)
                    let payload = json!({"op": 1, "d": null});
                    WsMessage::text(payload.to_string())
                }),
            },
            ..Default::default()
        };

        let manager = WebSocketManager::new(self.running.clone());

        super::reconnect_loop("QQ", &self.running, || manager.run(&config, self)).await;

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        self.send_c2c_message(msg.chat_id.as_str(), &msg.content)
            .await
    }

    fn is_allowed(&self, sender_id: &str) -> bool {
        check_allowlist(&self.config.allow_from, sender_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> QQConfig {
        QQConfig::default()
    }

    fn make_config_with_allowlist(allow: Vec<String>) -> QQConfig {
        QQConfig {
            allow_from: allow,
            ..Default::default()
        }
    }

    #[test]
    fn test_channel_name() {
        let channel = QQChannel::new(make_config()).unwrap();
        assert_eq!(channel.name(), "qq");
    }

    #[test]
    fn test_default_config_values() {
        let config = QQConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.app_id, "");
        assert_eq!(config.secret.expose(), "");
        assert!(config.allow_from.is_empty());
    }

    #[test]
    fn test_is_allowed_empty_allowlist() {
        let channel = QQChannel::new(make_config()).unwrap();
        assert!(channel.is_allowed("anyone"));
        assert!(channel.is_allowed("12345"));
    }

    #[test]
    fn test_is_allowed_with_allowlist() {
        let channel =
            QQChannel::new(make_config_with_allowlist(vec!["user1".to_string()])).unwrap();
        assert!(channel.is_allowed("user1"));
        assert!(!channel.is_allowed("user2"));
    }

    #[test]
    fn test_is_allowed_compound_id() {
        let channel =
            QQChannel::new(make_config_with_allowlist(vec!["user1".to_string()])).unwrap();
        assert!(channel.is_allowed("user1|openid"));
        assert!(!channel.is_allowed("user2|openid"));
    }

    #[test]
    fn test_gateway_hello_parse() {
        let msg = r#"{"op":10,"d":{"heartbeat_interval":41250},"s":null,"t":null}"#;
        let payload: WsPayload = serde_json::from_str(msg).unwrap();
        assert_eq!(payload.op, 10);
        let hello: HelloPayload = serde_json::from_value(payload.d.unwrap()).unwrap();
        assert_eq!(hello.heartbeat_interval, 41250);
    }

    #[test]
    fn test_gateway_dispatch_parse() {
        let msg = json!({
            "op": 0,
            "s": 5,
            "t": "C2C_MESSAGE_CREATE",
            "d": {
                "id": "msg1",
                "author": { "id": "user123", "user_openid": null },
                "content": "Hi!"
            }
        });

        let payload: WsPayload = serde_json::from_value(msg).unwrap();
        assert_eq!(payload.op, 0);
        assert_eq!(payload.s, Some(5));
        assert_eq!(payload.t.as_deref(), Some("C2C_MESSAGE_CREATE"));
    }

    #[test]
    fn test_c2c_message_parse() {
        let data = json!({
            "id": "msg1",
            "author": { "id": "user123", "user_openid": "open456" },
            "content": "Hello bot!"
        });

        let msg: C2CMessageCreate = serde_json::from_value(data).unwrap();
        assert_eq!(msg.id, "msg1");
        assert_eq!(msg.author.id, Some("user123".to_string()));
        assert_eq!(msg.author.user_openid, Some("open456".to_string()));
        assert_eq!(msg.content, "Hello bot!");
    }

    #[test]
    fn test_c2c_message_uses_openid_fallback() {
        let data = json!({
            "id": "msg2",
            "author": { "user_openid": "open789" },
            "content": "test"
        });

        let msg: C2CMessageCreate = serde_json::from_value(data).unwrap();
        let user_id = msg
            .author
            .id
            .or(msg.author.user_openid)
            .unwrap_or_else(|| "unknown".to_string());
        assert_eq!(user_id, "open789");
    }

    #[test]
    fn test_gateway_reconnect_opcode() {
        let msg = r#"{"op":7,"d":null,"s":null,"t":null}"#;
        let payload: WsPayload = serde_json::from_str(msg).unwrap();
        assert_eq!(payload.op, 7);
    }

    #[test]
    fn test_gateway_invalid_session_opcode() {
        let msg = r#"{"op":9,"d":false,"s":null,"t":null}"#;
        let payload: WsPayload = serde_json::from_str(msg).unwrap();
        assert_eq!(payload.op, 9);
    }

    #[test]
    fn test_gateway_heartbeat_ack() {
        let msg = r#"{"op":11,"d":null,"s":null,"t":null}"#;
        let payload: WsPayload = serde_json::from_str(msg).unwrap();
        assert_eq!(payload.op, 11);
    }

    #[test]
    fn test_empty_content_ignored() {
        let data = json!({
            "id": "msg3",
            "author": { "id": "user1" },
            "content": "   "
        });

        let msg: C2CMessageCreate = serde_json::from_value(data).unwrap();
        assert!(msg.content.trim().is_empty());
    }

    #[tokio::test]
    async fn test_deduplication() {
        let channel = QQChannel::new(make_config()).unwrap();
        let mut processed = channel.processed_ids.write().await;
        processed.push_back("msg1".to_string());
        assert!(processed.contains(&"msg1".to_string()));
        assert!(!processed.contains(&"msg2".to_string()));
    }

    #[tokio::test]
    async fn test_deduplication_cap() {
        let channel = QQChannel::new(make_config()).unwrap();
        let mut processed = channel.processed_ids.write().await;
        for i in 0..1001 {
            processed.push_back(format!("msg{}", i));
            if processed.len() > 1000 {
                processed.pop_front();
            }
        }
        assert_eq!(processed.len(), 1000);
        assert!(!processed.contains(&"msg0".to_string()));
        assert!(processed.contains(&"msg1000".to_string()));
    }

    #[tokio::test]
    async fn test_stop_sets_running_false() {
        let channel = QQChannel::new(make_config()).unwrap();
        channel.running.store(true, Ordering::SeqCst);
        channel.stop().await.unwrap();
        assert!(!channel.running.load(Ordering::SeqCst));
    }
}
