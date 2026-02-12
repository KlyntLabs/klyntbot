//! QQ channel using direct WebSocket connection to QQ Bot API.

use async_trait::async_trait;
use futures_util::{SinkExt, StreamExt};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};
use tracing::{debug, error, info, warn};

use bus::{InboundMessage, MessageBus, OutboundMessage};
use crate::{check_allowlist, Channel};
use config::QQConfig;
use common::{ChannelError, Result};

const QQ_API_BASE: &str = "https://api.sgroup.qq.com";
const QQ_WS_URL: &str = "wss://api.sgroup.qq.com/websocket";

/// QQ channel implementation
pub struct QQChannel {
    config: QQConfig,
    client: Client,
    access_token: Arc<RwLock<Option<String>>>,
    processed_ids: Arc<RwLock<VecDeque<String>>>,
    running: Arc<AtomicBool>,
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

    /// WebSocket gateway loop
    async fn gateway_loop(&self, bus: Arc<MessageBus>) -> Result<()> {
        // Authenticate first
        let token = self.authenticate().await?;
        *self.access_token.write().await = Some(token.clone());

        info!("Connecting to QQ Gateway: {}", QQ_WS_URL);

        let (ws_stream, _) = connect_async(QQ_WS_URL)
            .await
            .map_err(|e| ChannelError::ConnectionFailed(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();
        let mut seq: Option<i64> = None;
        let _heartbeat_interval = Duration::from_secs(30);

        info!("Connected to QQ Gateway");

        while self.running.load(Ordering::SeqCst) {
            let msg = match tokio::time::timeout(Duration::from_secs(35), read.next()).await {
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
                    // Timeout - send heartbeat
                    let payload = json!({"op": 1, "d": seq});
                    if let Err(e) = write.send(WsMessage::text(payload.to_string())).await {
                        error!("Failed to send heartbeat: {}", e);
                        break;
                    }
                    debug!("Sent heartbeat");
                    continue;
                }
            };

            if let WsMessage::Text(text) = msg {
                if let Err(e) = self.handle_gateway_event(&text, &bus, &mut seq).await {
                    error!("Error handling gateway event: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Handle a gateway event
    async fn handle_gateway_event(
        &self,
        text: &str,
        bus: &MessageBus,
        seq: &mut Option<i64>,
    ) -> Result<()> {
        let payload: WsPayload = serde_json::from_str(text).map_err(|e| {
            ChannelError::SendFailed(format!("Failed to parse gateway message: {}", e))
        })?;

        // Update sequence
        if let Some(s) = payload.s {
            *seq = Some(s);
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
                        "C2C_MESSAGE_CREATE" => {
                            if let Some(d) = payload.d {
                                self.handle_c2c_message(&d, bus).await?;
                            }
                        }
                        "DIRECT_MESSAGE_CREATE" => {
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
                return Err(
                    ChannelError::ConnectionFailed("Reconnect requested".to_string()).into(),
                );
            }
            9 => {
                // INVALID_SESSION
                warn!("QQ Gateway invalid session");
                return Err(ChannelError::ConnectionFailed("Invalid session".to_string()).into());
            }
            11 => {
                // HEARTBEAT_ACK
                debug!("Heartbeat ACK received");
            }
            _ => {
                debug!("Unknown opcode: {}", payload.op);
            }
        }

        Ok(())
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

        super::reconnect_loop("QQ", &self.running, || self.gateway_loop(bus.clone())).await;

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        self.send_c2c_message(msg.chat_id.as_str(), &msg.content).await
    }

    fn is_allowed(&self, sender_id: &str) -> bool {
        check_allowlist(&self.config.allow_from, sender_id)
    }
}
