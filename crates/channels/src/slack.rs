//! Slack channel using Socket Mode WebSocket.

use async_trait::async_trait;
use futures_util::SinkExt;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
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
use config::SlackConfig;

const SLACK_API_BASE: &str = "https://slack.com/api";

/// Slack channel implementation using Socket Mode
pub struct SlackChannel {
    config: SlackConfig,
    client: Client,
    bot_user_id: Arc<RwLock<Option<String>>>,
    running: Arc<AtomicBool>,
    bus: Mutex<Option<Arc<MessageBus>>>,
}

#[derive(Debug, Deserialize)]
struct AuthTestResponse {
    ok: bool,
    user_id: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SocketUrlResponse {
    ok: bool,
    url: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SocketEnvelope {
    envelope_id: String,
    #[serde(rename = "type")]
    envelope_type: String,
    payload: Option<Value>,
}

impl SlackChannel {
    /// Create a new Slack channel
    pub fn new(config: SlackConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ChannelError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            config,
            client,
            bot_user_id: Arc::new(RwLock::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            bus: Mutex::new(None),
        })
    }

    /// Get bot user ID via auth.test
    async fn authenticate(&self) -> Result<String> {
        let url = format!("{}/auth.test", SLACK_API_BASE);

        let response = self
            .client
            .post(&url)
            .bearer_auth(self.config.bot_token.expose())
            .send()
            .await
            .map_err(|e| ChannelError::ConnectionFailed(format!("Auth request failed: {}", e)))?;

        let auth: AuthTestResponse = response.json().await.map_err(|e| {
            ChannelError::ConnectionFailed(format!("Failed to parse auth response: {}", e))
        })?;

        if !auth.ok {
            return Err(ChannelError::ConnectionFailed(format!(
                "Auth failed: {}",
                auth.error.unwrap_or_else(|| "unknown".to_string())
            ))
            .into());
        }

        let user_id = auth.user_id.ok_or_else(|| {
            ChannelError::ConnectionFailed("No user_id in auth response".to_string())
        })?;

        info!("Slack authenticated as {}", user_id);
        Ok(user_id)
    }

    /// Get Socket Mode WebSocket URL
    async fn get_socket_url(&self) -> Result<String> {
        let url = format!("{}/apps.connections.open", SLACK_API_BASE);

        let response = self
            .client
            .post(&url)
            .bearer_auth(self.config.app_token.expose())
            .send()
            .await
            .map_err(|e| {
                ChannelError::ConnectionFailed(format!("Socket URL request failed: {}", e))
            })?;

        let socket_resp: SocketUrlResponse = response.json().await.map_err(|e| {
            ChannelError::ConnectionFailed(format!("Failed to parse socket URL response: {}", e))
        })?;

        if !socket_resp.ok {
            return Err(ChannelError::ConnectionFailed(format!(
                "Socket URL failed: {}",
                socket_resp.error.unwrap_or_else(|| "unknown".to_string())
            ))
            .into());
        }

        Ok(socket_resp.url.ok_or_else(|| {
            ChannelError::ConnectionFailed("No URL in socket response".to_string())
        })?)
    }

    /// Handle a Socket Mode envelope
    async fn handle_envelope(
        &self,
        text: &str,
        bus: &MessageBus,
        write: &Arc<Mutex<WsSink>>,
    ) -> Result<()> {
        let envelope: SocketEnvelope = serde_json::from_str(text)
            .map_err(|e| ChannelError::SendFailed(format!("Failed to parse envelope: {}", e)))?;

        debug!("Slack envelope: type={}", envelope.envelope_type);

        // Send ACK immediately
        let ack = json!({
            "envelope_id": envelope.envelope_id
        });
        {
            let mut w = write.lock().await;
            if let Err(e) = w.send(WsMessage::text(ack.to_string())).await {
                warn!("Failed to send ACK: {}", e);
            }
        }

        // Handle events_api
        if envelope.envelope_type == "events_api" {
            if let Some(payload) = envelope.payload {
                self.handle_event_payload(&payload, bus).await?;
            }
        }

        Ok(())
    }

    /// Handle events_api payload
    async fn handle_event_payload(&self, payload: &Value, bus: &MessageBus) -> Result<()> {
        let event = payload.get("event");
        if event.is_none() {
            return Ok(());
        }
        let event = event.unwrap();

        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let subtype = event.get("subtype").and_then(|v| v.as_str());

        // Only handle message and app_mention events
        if event_type != "message" && event_type != "app_mention" {
            return Ok(());
        }

        // Ignore bot/system messages (any subtype = not a normal user message)
        if subtype.is_some() {
            return Ok(());
        }

        let sender_id = event.get("user").and_then(|v| v.as_str()).unwrap_or("");

        let chat_id = event.get("channel").and_then(|v| v.as_str()).unwrap_or("");

        let text = event.get("text").and_then(|v| v.as_str()).unwrap_or("");

        if sender_id.is_empty() || chat_id.is_empty() {
            return Ok(());
        }

        // Ignore self messages
        let bot_id = self.bot_user_id.read().await;
        if let Some(bot_user_id) = bot_id.as_ref() {
            if sender_id == bot_user_id {
                return Ok(());
            }

            // Avoid double-processing: prefer app_mention over message for mentions
            if event_type == "message" && text.contains(&format!("<@{}>", bot_user_id)) {
                return Ok(());
            }
        }

        let channel_type = event
            .get("channel_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        debug!(
            "Slack event: type={} user={} channel={} channel_type={} text={}",
            event_type,
            sender_id,
            chat_id,
            channel_type,
            &text.chars().take(80).collect::<String>()
        );

        // Check allowlist
        if !check_allowlist(&self.config.allow_from, sender_id) {
            warn!("Access denied for sender {} on Slack", sender_id);
            return Ok(());
        }

        // For channels/groups, only respond to mentions or in DMs
        if channel_type != "im" && event_type != "app_mention" {
            if let Some(bot_user_id) = bot_id.as_ref() {
                if !text.contains(&format!("<@{}>", bot_user_id)) {
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }

        // Strip bot mention
        let cleaned_text = if let Some(bot_user_id) = bot_id.as_ref() {
            let re = Regex::new(&format!(r"<@{}>\s*", regex::escape(bot_user_id)))
                .map_err(|e| ChannelError::SendFailed(format!("Regex error: {}", e)))?;
            re.replace_all(text, "").trim().to_string()
        } else {
            text.to_string()
        };

        // Add :eyes: reaction (best-effort)
        if let Some(ts) = event.get("ts").and_then(|v| v.as_str()) {
            if let Err(e) = self.add_reaction(chat_id, ts, "eyes").await {
                warn!("Failed to add reaction to Slack message {}: {}", ts, e);
            }
        }

        // Get thread_ts for threading
        let thread_ts = event
            .get("thread_ts")
            .or(event.get("ts"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Publish to bus
        let mut inbound = InboundMessage::new("slack", sender_id, chat_id, &cleaned_text);
        if let Some(ts) = thread_ts {
            inbound.metadata.insert(
                "slack".to_string(),
                json!({
                    "thread_ts": ts,
                    "channel_type": channel_type,
                }),
            );
        }

        bus.publish_inbound(inbound)
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to publish to bus: {}", e)))?;

        Ok(())
    }

    /// Add a reaction to a message
    async fn add_reaction(&self, channel: &str, timestamp: &str, name: &str) -> Result<()> {
        let url = format!("{}/reactions.add", SLACK_API_BASE);
        let payload = json!({
            "channel": channel,
            "timestamp": timestamp,
            "name": name,
        });

        if let Err(e) = self
            .client
            .post(&url)
            .bearer_auth(self.config.bot_token.expose())
            .json(&payload)
            .send()
            .await
        {
            warn!(
                "Failed to add reaction to Slack message {}: {}",
                timestamp, e
            );
        }

        Ok(())
    }

    /// Send a message via REST API
    async fn send_message(&self, channel: &str, text: &str, thread_ts: Option<&str>) -> Result<()> {
        let url = format!("{}/chat.postMessage", SLACK_API_BASE);
        let mut payload = json!({
            "channel": channel,
            "text": text,
        });

        if let Some(ts) = thread_ts {
            payload["thread_ts"] = json!(ts);
        }

        let response = self
            .client
            .post(&url)
            .bearer_auth(self.config.bot_token.expose())
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
impl WsHandler for SlackChannel {
    async fn on_connected(&self, _write: &Arc<Mutex<WsSink>>) -> Result<Option<HeartbeatStrategy>> {
        info!("Connected to Slack Socket Mode");
        Ok(None) // Use default heartbeat from config
    }

    async fn on_text_message(&self, text: &str, write: &Arc<Mutex<WsSink>>) -> Result<bool> {
        let bus_guard = self.bus.lock().await;
        if let Some(bus) = bus_guard.as_ref() {
            self.handle_envelope(text, bus, write).await?;
        }
        Ok(true)
    }

    async fn on_disconnected(&self) {
        debug!("Disconnected from Slack Socket Mode");
    }
}

#[async_trait]
impl Channel for SlackChannel {
    fn name(&self) -> &str {
        "slack"
    }

    async fn start(&self, bus: Arc<MessageBus>) -> Result<()> {
        if self.config.bot_token.is_empty() || self.config.app_token.is_empty() {
            return Err(ChannelError::ConnectionFailed(
                "Slack bot_token and app_token not configured".to_string(),
            )
            .into());
        }

        // Authenticate and get bot user ID
        let bot_user_id = self.authenticate().await?;
        *self.bot_user_id.write().await = Some(bot_user_id);

        self.running.store(true, Ordering::SeqCst);
        *self.bus.lock().await = Some(bus);

        // Get a fresh socket URL for each connection attempt
        let get_socket_url = || self.get_socket_url();

        super::reconnect_loop("Slack", &self.running, || async {
            let socket_url = get_socket_url().await?;

            let config = WsConfig {
                url: socket_url,
                heartbeat: HeartbeatStrategy::Timeout {
                    timeout: Duration::from_secs(35),
                    build_payload: Box::new(|| WsMessage::Ping(vec![].into())),
                },
                ..Default::default()
            };

            let manager = WebSocketManager::new(self.running.clone());
            manager.run(&config, self).await
        })
        .await;

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        let thread_ts = msg
            .metadata
            .get("slack")
            .and_then(|v| v.get("thread_ts"))
            .and_then(|v| v.as_str());

        let channel_type = msg
            .metadata
            .get("slack")
            .and_then(|v| v.get("channel_type"))
            .and_then(|v| v.as_str());

        // Only use thread for non-DM messages
        let use_thread = thread_ts.is_some() && channel_type != Some("im");

        self.send_message(
            msg.chat_id.as_str(),
            &msg.content,
            if use_thread { thread_ts } else { None },
        )
        .await
    }

    fn is_allowed(&self, sender_id: &str) -> bool {
        check_allowlist(&self.config.allow_from, sender_id)
    }
}
