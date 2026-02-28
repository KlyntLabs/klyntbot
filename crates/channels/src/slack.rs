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

use crate::shared::InteractionTracker;
use crate::ws_manager::{HeartbeatStrategy, WebSocketManager, WsConfig, WsHandler, WsSink};
use crate::{check_allowlist, Channel};
use bus::{InboundMessage, MessageBus, MessageKind, OutboundMessage};
use common::{
    utils::truncate_chars, Answer, AnswerType, AnswerValue, ChannelError, FormResponse,
    InteractionRequest, Result,
};
use config::SlackConfig;

const SLACK_API_BASE: &str = "https://slack.com/api";

/// Slack channel implementation using Socket Mode
pub struct SlackChannel {
    config: SlackConfig,
    client: Client,
    bot_user_id: Arc<RwLock<Option<String>>>,
    running: Arc<AtomicBool>,
    bus: Mutex<Option<Arc<MessageBus>>>,
    interactions: InteractionTracker,
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
            interactions: InteractionTracker::new(),
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
            if let Some(ref payload) = envelope.payload {
                self.handle_event_payload(payload, bus).await?;
            }
        } else if envelope.envelope_type == "interactive" {
            // Handle interactive (block_actions from buttons/selects)
            if let Some(ref payload) = envelope.payload {
                self.handle_interactive_payload(payload).await;
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

        // Handle reaction_added events
        if event_type == "reaction_added" {
            return self.handle_reaction_added(event, bus).await;
        }

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
            truncate_chars(text, 80, "")
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

        // Intercept free-text replies for pending interactions
        if let Some(key) = self.interactions.find_free_text_key(chat_id) {
            if self.interactions.resolve_free_text(&key, cleaned_text.clone()) {
                return Ok(());
            }
        }

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

    /// Handle a reaction_added event from Slack
    async fn handle_reaction_added(&self, event: &Value, bus: &MessageBus) -> Result<()> {
        let sender_id = event.get("user").and_then(|v| v.as_str()).unwrap_or("");

        if sender_id.is_empty() {
            return Ok(());
        }

        // Ignore self reactions
        let bot_id = self.bot_user_id.read().await;
        if let Some(bot_user_id) = bot_id.as_ref() {
            if sender_id == bot_user_id {
                return Ok(());
            }
        }

        // Check allowlist
        if !check_allowlist(&self.config.allow_from, sender_id) {
            warn!("Access denied for sender {} on Slack reaction", sender_id);
            return Ok(());
        }

        // Extract channel from item (reaction_added has item.channel, not top-level channel)
        let chat_id = event
            .get("item")
            .and_then(|item| item.get("channel"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if chat_id.is_empty() {
            return Ok(());
        }

        // Extract reaction name and convert to emoji
        let reaction_name = event.get("reaction").and_then(|v| v.as_str()).unwrap_or("");

        if reaction_name.is_empty() {
            return Ok(());
        }

        let emoji = slack_reaction_to_unicode(reaction_name);

        debug!(
            "Slack reaction from {}: {} ({}) in channel {}",
            sender_id, emoji, reaction_name, chat_id
        );

        let inbound = InboundMessage::new("slack", sender_id, chat_id, &emoji)
            .with_kind(MessageKind::Reaction);

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

    /// Handle an interactive payload (block_actions from buttons/selects).
    async fn handle_interactive_payload(&self, payload: &Value) {
        let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if payload_type != "block_actions" {
            debug!("Ignoring interactive payload type: {}", payload_type);
            return;
        }

        let actions = match payload.get("actions").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return,
        };

        for action in actions {
            let action_id = action
                .get("action_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if !action_id.starts_with("askuser:") {
                continue;
            }

            let parts: Vec<&str> = action_id.split(':').collect();
            let action_type = action.get("type").and_then(|v| v.as_str()).unwrap_or("");

            let (key, value) = if action_type == "static_select" {
                // Select menu: action_id = "askuser:{channel}:{question_id}", value in selected_option
                if parts.len() < 3 {
                    continue;
                }
                let key = format!("{}:{}", parts[1], parts[2]);
                let value = action
                    .get("selected_option")
                    .and_then(|o| o.get("value"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                (key, value)
            } else {
                // Button: action_id = "askuser:{channel}:{question_id}:{value}"
                if parts.len() < 4 {
                    continue;
                }
                let key = format!("{}:{}", parts[1], parts[2]);
                let value = parts[3].to_string();
                (key, value)
            };

            self.interactions.resolve_single(&key, value);
        }
    }

    /// Send a message with Block Kit blocks via REST API.
    async fn send_message_with_blocks(
        &self,
        channel: &str,
        text: &str,
        blocks: &[Value],
    ) -> Result<()> {
        let url = format!("{}/chat.postMessage", SLACK_API_BASE);
        let payload = json!({
            "channel": channel,
            "text": text,
            "blocks": blocks,
        });

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
            let body = response.text().await.unwrap_or_default();
            return Err(ChannelError::SendFailed(format!("HTTP {}: {}", status, body)).into());
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

/// Convert a Slack reaction shortcode to its Unicode emoji representation.
/// Falls back to `:name:` format for unknown reactions.
fn slack_reaction_to_unicode(name: &str) -> String {
    match name {
        "thumbsup" | "+1" => "\u{1F44D}".to_string(),
        "thumbsdown" | "-1" => "\u{1F44E}".to_string(),
        "heart" => "\u{2764}\u{FE0F}".to_string(),
        "tada" => "\u{1F389}".to_string(),
        "confused" => "\u{1F615}".to_string(),
        "eyes" => "\u{1F440}".to_string(),
        "fire" => "\u{1F525}".to_string(),
        "rocket" => "\u{1F680}".to_string(),
        "white_check_mark" => "\u{2705}".to_string(),
        "x" => "\u{274C}".to_string(),
        "wave" => "\u{1F44B}".to_string(),
        "clap" => "\u{1F44F}".to_string(),
        "100" => "\u{1F4AF}".to_string(),
        "raised_hands" => "\u{1F64C}".to_string(),
        "thinking_face" => "\u{1F914}".to_string(),
        "laughing" | "satisfied" => "\u{1F606}".to_string(),
        "cry" => "\u{1F622}".to_string(),
        "pray" => "\u{1F64F}".to_string(),
        other => format!(":{}:", other),
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
        let ts = if use_thread { thread_ts } else { None };

        let formatted = crate::formatter::formatter_for("slack").format(&msg.content);
        let limit = crate::utils::max_length("slack");
        let chunks = crate::utils::split_message(&formatted, limit);

        for chunk in &chunks {
            self.send_message(msg.chat_id.as_str(), chunk, ts).await?;
        }

        Ok(())
    }

    fn is_allowed(&self, sender_id: &str) -> bool {
        check_allowlist(&self.config.allow_from, sender_id)
    }

    fn supports_interaction(&self) -> bool {
        true
    }

    async fn send_interaction(
        &self,
        chat_id: &str,
        request: &InteractionRequest,
    ) -> Result<FormResponse> {
        let mut answers = Vec::new();

        for question in &request.questions {
            let answer = match &question.answer_type {
                AnswerType::SingleSelect { options } => {
                    let blocks =
                        build_slack_select_blocks(chat_id, &question.id, &question.text, options);
                    self.send_message_with_blocks(chat_id, &question.text, &blocks)
                        .await?;

                    let value = self.interactions.wait_for_callback(chat_id, &question.id).await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::Selected { value },
                    }
                }
                AnswerType::YesNo { .. } => {
                    let blocks = build_slack_yes_no_blocks(chat_id, &question.id, &question.text);
                    self.send_message_with_blocks(chat_id, &question.text, &blocks)
                        .await?;

                    let value = self.interactions.wait_for_callback(chat_id, &question.id).await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::YesNo {
                            answer: value == "yes",
                        },
                    }
                }
                AnswerType::MultiSelect { options } => {
                    // Simplified: present as single select (same as Discord/Telegram)
                    let blocks =
                        build_slack_select_blocks(chat_id, &question.id, &question.text, options);
                    self.send_message_with_blocks(chat_id, &question.text, &blocks)
                        .await?;

                    let value = self.interactions.wait_for_callback(chat_id, &question.id).await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::MultiSelected {
                            values: vec![value],
                        },
                    }
                }
                AnswerType::FreeText { placeholder } => {
                    let prompt = if let Some(ph) = placeholder {
                        format!("*{}*\n{}\n_{}_", request.title, question.text, ph)
                    } else {
                        format!("*{}*\n{}", request.title, question.text)
                    };

                    self.send_message(chat_id, &prompt, None).await?;

                    let content = self.interactions.wait_for_free_text(chat_id, &question.id).await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::Text { content },
                    }
                }
            };

            answers.push(answer);
        }

        Ok(FormResponse::Completed(answers))
    }
}

/// Build Slack Block Kit blocks with buttons for select options.
/// Uses `actions` block with button elements.
fn build_slack_select_blocks(
    channel_id: &str,
    question_id: &str,
    question: &str,
    options: &[common::AnswerOption],
) -> Vec<Value> {
    let buttons: Vec<Value> = options
        .iter()
        .map(|opt| {
            json!({
                "type": "button",
                "text": { "type": "plain_text", "text": opt.label },
                "action_id": format!("askuser:{}:{}:{}", channel_id, question_id, opt.value),
                "value": opt.value,
            })
        })
        .collect();

    vec![
        json!({
            "type": "section",
            "text": { "type": "mrkdwn", "text": question },
        }),
        json!({
            "type": "actions",
            "elements": buttons,
        }),
    ]
}

/// Build Slack Block Kit blocks with Yes/No buttons.
fn build_slack_yes_no_blocks(channel_id: &str, question_id: &str, question: &str) -> Vec<Value> {
    vec![
        json!({
            "type": "section",
            "text": { "type": "mrkdwn", "text": question },
        }),
        json!({
            "type": "actions",
            "elements": [
                {
                    "type": "button",
                    "text": { "type": "plain_text", "text": "Yes" },
                    "style": "primary",
                    "action_id": format!("askuser:{}:{}:yes", channel_id, question_id),
                    "value": "yes",
                },
                {
                    "type": "button",
                    "text": { "type": "plain_text", "text": "No" },
                    "style": "danger",
                    "action_id": format!("askuser:{}:{}:no", channel_id, question_id),
                    "value": "no",
                },
            ],
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> SlackConfig {
        SlackConfig::default()
    }

    fn make_config_with_allowlist(allow: Vec<String>) -> SlackConfig {
        SlackConfig {
            allow_from: allow,
            ..Default::default()
        }
    }

    #[test]
    fn test_channel_name() {
        let channel = SlackChannel::new(make_config()).unwrap();
        assert_eq!(channel.name(), "slack");
    }

    #[test]
    fn test_default_config_values() {
        let config = SlackConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.bot_token.expose(), "");
        assert_eq!(config.app_token.expose(), "");
        assert!(config.allow_from.is_empty());
        assert_eq!(config.mode, "socket");
        assert_eq!(config.group_policy, "none");
    }

    #[test]
    fn test_is_allowed_empty_allowlist() {
        let channel = SlackChannel::new(make_config()).unwrap();
        assert!(channel.is_allowed("U12345"));
        assert!(channel.is_allowed("anyone"));
    }

    #[test]
    fn test_is_allowed_with_allowlist() {
        let channel =
            SlackChannel::new(make_config_with_allowlist(vec!["U12345".to_string()])).unwrap();
        assert!(channel.is_allowed("U12345"));
        assert!(!channel.is_allowed("U99999"));
    }

    #[test]
    fn test_is_allowed_compound_id() {
        let channel =
            SlackChannel::new(make_config_with_allowlist(vec!["U12345".to_string()])).unwrap();
        assert!(channel.is_allowed("U12345|name"));
        assert!(!channel.is_allowed("U99999|name"));
    }

    #[test]
    fn test_socket_envelope_parse() {
        let raw = json!({
            "envelope_id": "env-123",
            "type": "events_api",
            "payload": {
                "event": {
                    "type": "message",
                    "user": "U123",
                    "channel": "C456",
                    "text": "Hello!"
                }
            }
        });

        let envelope: SocketEnvelope = serde_json::from_value(raw).unwrap();
        assert_eq!(envelope.envelope_id, "env-123");
        assert_eq!(envelope.envelope_type, "events_api");
        assert!(envelope.payload.is_some());
    }

    #[test]
    fn test_socket_envelope_without_payload() {
        let raw = json!({
            "envelope_id": "env-456",
            "type": "hello"
        });

        let envelope: SocketEnvelope = serde_json::from_value(raw).unwrap();
        assert_eq!(envelope.envelope_type, "hello");
        assert!(envelope.payload.is_none());
    }

    #[test]
    fn test_event_payload_message_parse() {
        let payload = json!({
            "event": {
                "type": "message",
                "user": "U123",
                "channel": "C456",
                "text": "Hello bot!",
                "channel_type": "im",
                "ts": "1234567890.123456"
            }
        });

        let event = payload.get("event").unwrap();
        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap();
        assert_eq!(event_type, "message");
        let user = event.get("user").and_then(|v| v.as_str()).unwrap();
        assert_eq!(user, "U123");
        let channel_type = event.get("channel_type").and_then(|v| v.as_str()).unwrap();
        assert_eq!(channel_type, "im");
    }

    #[test]
    fn test_event_payload_app_mention_parse() {
        let payload = json!({
            "event": {
                "type": "app_mention",
                "user": "U123",
                "channel": "C456",
                "text": "<@BOTID> help me",
                "channel_type": "channel",
                "ts": "1234567890.123456"
            }
        });

        let event = payload.get("event").unwrap();
        let event_type = event.get("type").and_then(|v| v.as_str()).unwrap();
        assert_eq!(event_type, "app_mention");
    }

    #[test]
    fn test_subtype_messages_ignored() {
        let payload = json!({
            "event": {
                "type": "message",
                "subtype": "bot_message",
                "user": "U123",
                "channel": "C456",
                "text": "bot reply"
            }
        });

        let event = payload.get("event").unwrap();
        let subtype = event.get("subtype").and_then(|v| v.as_str());
        assert!(subtype.is_some()); // Should be filtered out
    }

    #[test]
    fn test_bot_mention_strip() {
        let bot_user_id = "UBOT123";
        let text = format!("<@{}> help me with this", bot_user_id);
        let re = regex::Regex::new(&format!(r"<@{}>\s*", regex::escape(bot_user_id))).unwrap();
        let cleaned = re.replace_all(&text, "").trim().to_string();
        assert_eq!(cleaned, "help me with this");
    }

    #[test]
    fn test_thread_ts_extraction() {
        let event = json!({
            "type": "message",
            "user": "U123",
            "channel": "C456",
            "text": "reply",
            "thread_ts": "1234567890.000001",
            "ts": "1234567890.000002"
        });

        let thread_ts = event
            .get("thread_ts")
            .or(event.get("ts"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(thread_ts, "1234567890.000001");
    }

    #[test]
    fn test_thread_ts_falls_back_to_ts() {
        let event = json!({
            "type": "message",
            "user": "U123",
            "channel": "C456",
            "text": "new message",
            "ts": "1234567890.000002"
        });

        let thread_ts = event
            .get("thread_ts")
            .or(event.get("ts"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(thread_ts, "1234567890.000002");
    }

    #[tokio::test]
    async fn test_stop_sets_running_false() {
        let channel = SlackChannel::new(make_config()).unwrap();
        channel.running.store(true, Ordering::SeqCst);
        channel.stop().await.unwrap();
        assert!(!channel.running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_auth_response_parse_success() {
        let raw = json!({
            "ok": true,
            "user_id": "UBOT123"
        });
        let auth: AuthTestResponse = serde_json::from_value(raw).unwrap();
        assert!(auth.ok);
        assert_eq!(auth.user_id.unwrap(), "UBOT123");
    }

    #[test]
    fn test_auth_response_parse_failure() {
        let raw = json!({
            "ok": false,
            "error": "invalid_auth"
        });
        let auth: AuthTestResponse = serde_json::from_value(raw).unwrap();
        assert!(!auth.ok);
        assert_eq!(auth.error.unwrap(), "invalid_auth");
    }

    #[test]
    fn test_slack_select_blocks_3_options() {
        use common::AnswerOption;

        let options = vec![
            AnswerOption {
                value: "high".into(),
                label: "High".into(),
                description: None,
            },
            AnswerOption {
                value: "medium".into(),
                label: "Medium".into(),
                description: None,
            },
            AnswerOption {
                value: "low".into(),
                label: "Low".into(),
                description: None,
            },
        ];

        let blocks = build_slack_select_blocks("C123", "priority", "Pick priority", &options);
        assert_eq!(blocks.len(), 2); // section + actions

        // Section block
        assert_eq!(blocks[0]["type"], "section");
        assert_eq!(blocks[0]["text"]["text"].as_str().unwrap(), "Pick priority");

        // Actions block with 3 buttons
        assert_eq!(blocks[1]["type"], "actions");
        let elements = blocks[1]["elements"].as_array().unwrap();
        assert_eq!(elements.len(), 3);

        for el in elements {
            assert_eq!(el["type"], "button");
        }

        // Check first button
        assert_eq!(elements[0]["text"]["text"], "High");
        assert_eq!(
            elements[0]["action_id"].as_str().unwrap(),
            "askuser:C123:priority:high"
        );
        assert_eq!(elements[0]["value"], "high");
    }

    #[test]
    fn test_slack_yes_no_blocks() {
        let blocks = build_slack_yes_no_blocks("C456", "confirm", "Are you sure?");
        assert_eq!(blocks.len(), 2); // section + actions

        assert_eq!(blocks[0]["type"], "section");

        let elements = blocks[1]["elements"].as_array().unwrap();
        assert_eq!(elements.len(), 2);

        // Yes button: primary style
        assert_eq!(elements[0]["text"]["text"], "Yes");
        assert_eq!(elements[0]["style"], "primary");
        assert_eq!(
            elements[0]["action_id"].as_str().unwrap(),
            "askuser:C456:confirm:yes"
        );

        // No button: danger style
        assert_eq!(elements[1]["text"]["text"], "No");
        assert_eq!(elements[1]["style"], "danger");
        assert_eq!(
            elements[1]["action_id"].as_str().unwrap(),
            "askuser:C456:confirm:no"
        );
    }

    #[test]
    fn test_slack_action_id_format() {
        use common::AnswerOption;

        let options = vec![
            AnswerOption {
                value: "oauth2".into(),
                label: "OAuth 2.0".into(),
                description: Some("Industry standard".into()),
            },
            AnswerOption {
                value: "jwt".into(),
                label: "JWT".into(),
                description: None,
            },
        ];

        let blocks = build_slack_select_blocks("C789", "auth_method", "Choose auth", &options);
        let elements = blocks[1]["elements"].as_array().unwrap();

        for el in elements {
            let action_id = el["action_id"].as_str().unwrap();
            assert!(action_id.starts_with("askuser:C789:auth_method:"));
            let parts: Vec<&str> = action_id.split(':').collect();
            assert_eq!(parts.len(), 4);
            assert_eq!(parts[0], "askuser");
            assert_eq!(parts[1], "C789");
            assert_eq!(parts[2], "auth_method");
        }
    }

    #[test]
    fn test_interactive_payload_parse() {
        // Simulate a block_actions payload from a button click
        let payload = json!({
            "type": "block_actions",
            "user": { "id": "U123" },
            "channel": { "id": "C456" },
            "actions": [{
                "action_id": "askuser:C456:priority:high",
                "type": "button",
                "value": "high"
            }]
        });

        let payload_type = payload.get("type").and_then(|v| v.as_str()).unwrap();
        assert_eq!(payload_type, "block_actions");

        let actions = payload.get("actions").and_then(|v| v.as_array()).unwrap();
        assert_eq!(actions.len(), 1);

        let action_id = actions[0]["action_id"].as_str().unwrap();
        let parts: Vec<&str> = action_id.split(':').collect();
        assert_eq!(parts, vec!["askuser", "C456", "priority", "high"]);
    }
}
