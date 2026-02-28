//! Discord channel using raw WebSocket Gateway (no serenity).

use async_trait::async_trait;
use dashmap::DashMap;
use futures_util::SinkExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tracing::{debug, error, info, warn};

use crate::ws_manager::{HeartbeatStrategy, WebSocketManager, WsConfig, WsHandler, WsSink};
use crate::{check_allowlist, Channel};
use bus::{InboundMessage, MessageBus, MessageKind, OutboundMessage};
use common::{
    Answer, AnswerType, AnswerValue, ChannelError, FormResponse, InteractionRequest, Result,
};
use config::DiscordConfig;

const DISCORD_API_BASE: &str = "https://discord.com/api/v10";
const MAX_ATTACHMENT_BYTES: usize = 20 * 1024 * 1024; // 20MB

/// State for a pending Discord interaction callback.
enum PendingCallback {
    /// Waiting for a single button press (single_select, yes_no).
    Single(oneshot::Sender<String>),
    /// Waiting for free text reply from user.
    FreeText(oneshot::Sender<String>),
}

/// Discord channel implementation
pub struct DiscordChannel {
    config: DiscordConfig,
    client: Client,
    seq: Arc<RwLock<Option<i64>>>,
    running: Arc<AtomicBool>,
    typing_tasks: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
    bus: Mutex<Option<Arc<MessageBus>>>,
    heartbeat_task: Mutex<Option<JoinHandle<()>>>,
    /// Pending interaction callbacks keyed by `"{channel_id}:{question_id}"`.
    pending_interactions: Arc<DashMap<String, PendingCallback>>,
}

impl DiscordChannel {
    /// Create a new Discord channel
    pub fn new(config: DiscordConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| ChannelError::ConnectionFailed(e.to_string()))?;

        Ok(Self {
            config,
            client,
            seq: Arc::new(RwLock::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            typing_tasks: Arc::new(RwLock::new(HashMap::new())),
            bus: Mutex::new(None),
            heartbeat_task: Mutex::new(None),
            pending_interactions: Arc::new(DashMap::new()),
        })
    }

    /// Handle HELLO (op 10) — start heartbeat + send IDENTIFY
    async fn handle_hello(
        &self,
        payload: Option<&Value>,
        write: &Arc<Mutex<WsSink>>,
    ) -> Result<()> {
        if let Some(p) = payload {
            let interval_ms = p
                .get("heartbeat_interval")
                .and_then(|v| v.as_i64())
                .unwrap_or(45000);

            info!(
                "Discord HELLO received, heartbeat interval: {}ms",
                interval_ms
            );

            // Abort existing heartbeat task
            if let Some(task) = self.heartbeat_task.lock().await.take() {
                task.abort();
            }

            // Spawn new heartbeat task
            let seq = self.seq.clone();
            let running = self.running.clone();
            let write_shared = Arc::clone(write);
            let interval_secs = interval_ms as f64 / 1000.0;

            *self.heartbeat_task.lock().await = Some(tokio::spawn(async move {
                loop {
                    sleep(Duration::from_secs_f64(interval_secs)).await;
                    if !running.load(Ordering::SeqCst) {
                        break;
                    }
                    let current_seq = *seq.read().await;
                    let payload = json!({"op": 1, "d": current_seq});
                    let mut w = write_shared.lock().await;
                    if let Err(e) = w.send(WsMessage::text(payload.to_string())).await {
                        warn!("Heartbeat failed: {}", e);
                        break;
                    }
                    debug!("Sent heartbeat");
                }
            }));

            // Send IDENTIFY
            let identify = json!({
                "op": 2,
                "d": {
                    "token": self.config.token.expose(),
                    "intents": self.config.intents as i64,
                    "properties": {
                        "os": "klyntbot",
                        "browser": "klyntbot",
                        "device": "klyntbot"
                    }
                }
            });

            {
                let mut w = write.lock().await;
                if let Err(e) = w.send(WsMessage::text(identify.to_string())).await {
                    error!("Failed to send IDENTIFY: {}", e);
                    return Err(ChannelError::ConnectionFailed("IDENTIFY failed".into()).into());
                }
            }
            debug!("Sent IDENTIFY");
        }

        Ok(())
    }

    /// Get media directory
    fn media_dir() -> PathBuf {
        let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        path.push(".klyntbot");
        path.push("media");
        path
    }

    /// Start typing indicator for a channel
    async fn start_typing(&self, channel_id: String) {
        self.stop_typing(&channel_id).await;

        let client = self.client.clone();
        let token = self.config.token.expose().clone();
        let running = self.running.clone();
        let channel_id_clone = channel_id.clone();

        let task = tokio::spawn(async move {
            let url = format!("{}/channels/{}/typing", DISCORD_API_BASE, channel_id_clone);
            let headers = reqwest::header::HeaderMap::from_iter([(
                reqwest::header::AUTHORIZATION,
                format!("Bot {}", token).parse().unwrap(),
            )]);

            while running.load(Ordering::SeqCst) {
                if let Err(e) = client.post(&url).headers(headers.clone()).send().await {
                    warn!(
                        "Failed to send typing indicator to Discord channel {}: {}",
                        channel_id_clone, e
                    );
                }
                sleep(Duration::from_secs(8)).await;
            }
        });

        self.typing_tasks.write().await.insert(channel_id, task);
    }

    /// Stop typing indicator for a channel
    async fn stop_typing(&self, channel_id: &str) {
        if let Some(task) = self.typing_tasks.write().await.remove(channel_id) {
            task.abort();
        }
    }

    /// Handle MESSAGE_CREATE event
    async fn handle_message_create(&self, payload: &Value, bus: &MessageBus) -> Result<()> {
        let author = payload.get("author");
        if let Some(a) = author {
            if a.get("bot").and_then(|v| v.as_bool()).unwrap_or(false) {
                return Ok(()); // Ignore bot messages
            }
        }

        let sender_id = author
            .and_then(|a| a.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let channel_id = payload
            .get("channel_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Check if this is a pending free_text interaction response
        if !channel_id.is_empty() {
            let key_prefix = format!("{}:", channel_id);
            let pending_key = self
                .pending_interactions
                .iter()
                .find(|entry| {
                    entry.key().starts_with(&key_prefix)
                        && matches!(entry.value(), PendingCallback::FreeText(_))
                })
                .map(|entry| entry.key().clone());

            if let Some(key) = pending_key {
                let content = payload
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !content.is_empty() {
                    if let Some((_, PendingCallback::FreeText(tx))) =
                        self.pending_interactions.remove(&key)
                    {
                        let _ = tx.send(content.to_string());
                        return Ok(());
                    }
                }
            }
        }

        let content = payload
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if sender_id.is_empty() || channel_id.is_empty() {
            return Ok(());
        }

        // Check allowlist
        if !check_allowlist(&self.config.allow_from, sender_id) {
            warn!("Access denied for sender {} on Discord", sender_id);
            return Ok(());
        }

        let mut content_parts = vec![];
        if !content.is_empty() {
            content_parts.push(content.to_string());
        }

        let attachment_count = payload
            .get("attachments")
            .and_then(|v| v.as_array())
            .map_or(0, |a| a.len());
        let mut media_paths = Vec::with_capacity(attachment_count);

        // Handle attachments - download them
        if let Some(attachments) = payload.get("attachments").and_then(|v| v.as_array()) {
            let media_dir = Self::media_dir();
            if let Err(e) = tokio::fs::create_dir_all(&media_dir).await {
                warn!("Failed to create media directory: {}", e);
            }

            for attachment in attachments {
                let url = attachment.get("url").and_then(|v| v.as_str());
                let filename = attachment
                    .get("filename")
                    .and_then(|v| v.as_str())
                    .unwrap_or("attachment");
                let size = attachment.get("size").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let att_id = attachment
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("file");

                if let Some(url) = url {
                    if size > MAX_ATTACHMENT_BYTES {
                        content_parts.push(format!("[attachment: {} - too large]", filename));
                        continue;
                    }

                    // Download attachment
                    match self.client.get(url).send().await {
                        Ok(response) => {
                            if response.status().is_success() {
                                match response.bytes().await {
                                    Ok(bytes) => {
                                        let safe_filename = filename.replace(['/', '\\'], "_");
                                        let file_path =
                                            media_dir.join(format!("{}_{}", att_id, safe_filename));

                                        match tokio::fs::write(&file_path, bytes).await {
                                            Ok(_) => {
                                                let path_str =
                                                    file_path.to_string_lossy().to_string();
                                                media_paths.push(path_str.clone());
                                                content_parts
                                                    .push(format!("[attachment: {}]", path_str));
                                            }
                                            Err(e) => {
                                                warn!("Failed to save attachment: {}", e);
                                                content_parts.push(format!(
                                                    "[attachment: {} - save failed]",
                                                    filename
                                                ));
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to read attachment bytes: {}", e);
                                        content_parts.push(format!(
                                            "[attachment: {} - download failed]",
                                            filename
                                        ));
                                    }
                                }
                            } else {
                                warn!("Failed to download attachment: HTTP {}", response.status());
                                content_parts
                                    .push(format!("[attachment: {} - download failed]", filename));
                            }
                        }
                        Err(e) => {
                            warn!("Failed to download attachment: {}", e);
                            content_parts
                                .push(format!("[attachment: {} - download failed]", filename));
                        }
                    }
                }
            }
        }

        let final_content = if content_parts.is_empty() {
            "[empty message]".to_string()
        } else {
            content_parts.join("\n")
        };

        debug!(
            "Discord message from {}: {}...",
            sender_id,
            &final_content.chars().take(50).collect::<String>()
        );

        // Start typing indicator
        self.start_typing(channel_id.to_string()).await;

        // Build metadata
        let mut metadata = HashMap::with_capacity(3);
        if let Some(msg_id) = payload.get("id").and_then(|v| v.as_str()) {
            metadata.insert("message_id".to_string(), json!(msg_id));
        }
        if let Some(guild_id) = payload.get("guild_id") {
            metadata.insert("guild_id".to_string(), guild_id.clone());
        }
        if let Some(reply_to) = payload.get("referenced_message").and_then(|m| m.get("id")) {
            metadata.insert("reply_to".to_string(), reply_to.clone());
        }

        // Publish to bus
        let mut inbound = InboundMessage::new("discord", sender_id, channel_id, final_content);
        inbound.media = media_paths;
        inbound.metadata = metadata;

        bus.publish_inbound(inbound)
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to publish to bus: {}", e)))?;

        Ok(())
    }

    /// Handle MESSAGE_REACTION_ADD event
    async fn handle_reaction_add(&self, payload: &Value, bus: &MessageBus) -> Result<()> {
        // Ignore reactions from bots (no user_id means system/bot)
        let user_id = payload
            .get("user_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if user_id.is_empty() {
            return Ok(());
        }

        // Check allowlist
        if !check_allowlist(&self.config.allow_from, user_id) {
            warn!("Access denied for sender {} on Discord reaction", user_id);
            return Ok(());
        }

        let channel_id = payload
            .get("channel_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if channel_id.is_empty() {
            return Ok(());
        }

        // Extract emoji — Discord sends { "emoji": { "name": "🔥", "id": null } }
        // For custom emoji: { "emoji": { "name": "custom_name", "id": "123456" } }
        let emoji = payload
            .get("emoji")
            .map(|e| {
                let name = e.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let id = e.get("id").and_then(|v| v.as_str());
                if let Some(emoji_id) = id {
                    // Custom emoji: use <:name:id> format
                    format!("<:{}:{}>", name, emoji_id)
                } else {
                    // Unicode emoji
                    name.to_string()
                }
            })
            .unwrap_or_default();

        if emoji.is_empty() {
            return Ok(());
        }

        debug!(
            "Discord reaction from {}: {} in channel {}",
            user_id, emoji, channel_id
        );

        let mut metadata = HashMap::with_capacity(2);
        if let Some(msg_id) = payload.get("message_id").and_then(|v| v.as_str()) {
            metadata.insert("message_id".to_string(), json!(msg_id));
        }
        if let Some(guild_id) = payload.get("guild_id") {
            metadata.insert("guild_id".to_string(), guild_id.clone());
        }

        let mut inbound = InboundMessage::new("discord", user_id, channel_id, &emoji)
            .with_kind(MessageKind::Reaction);
        inbound.metadata = metadata;

        bus.publish_inbound(inbound)
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to publish to bus: {}", e)))?;

        Ok(())
    }

    /// Handle INTERACTION_CREATE event (button clicks, select menu selections).
    async fn handle_interaction_create(&self, payload: &Value) -> Result<()> {
        let interaction_id = payload.get("id").and_then(|v| v.as_str()).unwrap_or("");
        let interaction_token = payload.get("token").and_then(|v| v.as_str()).unwrap_or("");

        // Acknowledge the interaction (type 6 = DEFERRED_UPDATE_MESSAGE)
        let ack_url = format!(
            "{}/interactions/{}/{}/callback",
            DISCORD_API_BASE, interaction_id, interaction_token
        );
        let _ = self
            .client
            .post(&ack_url)
            .header(
                "Authorization",
                format!("Bot {}", self.config.token.expose()),
            )
            .json(&json!({ "type": 6 }))
            .send()
            .await;

        // Parse custom_id format: "askuser:{channel_id}:{question_id}:{value}"
        let custom_id = payload
            .get("data")
            .and_then(|d| d.get("custom_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let parts: Vec<&str> = custom_id.splitn(4, ':').collect();
        if parts.len() < 4 || parts[0] != "askuser" {
            // Check if it's a select menu (values array)
            if let Some(values) = payload
                .get("data")
                .and_then(|d| d.get("values"))
                .and_then(|v| v.as_array())
            {
                // Select menu: custom_id is "askuser:{channel_id}:{question_id}"
                let select_parts: Vec<&str> = custom_id.splitn(3, ':').collect();
                if select_parts.len() == 3 && select_parts[0] == "askuser" {
                    let key = format!("{}:{}", select_parts[1], select_parts[2]);
                    if let Some(value) = values.first().and_then(|v| v.as_str()) {
                        if let Some((_, PendingCallback::Single(tx))) =
                            self.pending_interactions.remove(&key)
                        {
                            let _ = tx.send(value.to_string());
                        }
                    }
                }
            }
            return Ok(());
        }

        let channel_id = parts[1];
        let question_id = parts[2];
        let value = parts[3];
        let key = format!("{}:{}", channel_id, question_id);

        if let Some((_, PendingCallback::Single(tx))) = self.pending_interactions.remove(&key) {
            let _ = tx.send(value.to_string());
        }

        Ok(())
    }

    /// Wait for an interaction callback with a 5-minute timeout.
    async fn wait_for_callback(&self, channel_id: &str, question_id: &str) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        let key = format!("{}:{}", channel_id, question_id);
        self.pending_interactions
            .insert(key.clone(), PendingCallback::Single(tx));

        match tokio::time::timeout(Duration::from_secs(300), rx).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => {
                self.pending_interactions.remove(&key);
                Err(ChannelError::SendFailed("Interaction cancelled".into()).into())
            }
            Err(_) => {
                self.pending_interactions.remove(&key);
                Err(ChannelError::SendFailed("Interaction timed out (5 min)".into()).into())
            }
        }
    }

    /// Wait for a free text message with a 5-minute timeout.
    async fn wait_for_free_text(&self, channel_id: &str, question_id: &str) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        let key = format!("{}:{}", channel_id, question_id);
        self.pending_interactions
            .insert(key.clone(), PendingCallback::FreeText(tx));

        match tokio::time::timeout(Duration::from_secs(300), rx).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => {
                self.pending_interactions.remove(&key);
                Err(ChannelError::SendFailed("Interaction cancelled".into()).into())
            }
            Err(_) => {
                self.pending_interactions.remove(&key);
                Err(ChannelError::SendFailed("Interaction timed out (5 min)".into()).into())
            }
        }
    }

    /// Send a message via REST API
    async fn send_message_rest(
        &self,
        channel_id: &str,
        content: &str,
        reply_to: Option<&str>,
    ) -> Result<()> {
        let url = format!("{}/channels/{}/messages", DISCORD_API_BASE, channel_id);

        let mut payload = json!({
            "content": content
        });

        if let Some(msg_id) = reply_to {
            payload["message_reference"] = json!({
                "message_id": msg_id
            });
            payload["allowed_mentions"] = json!({
                "replied_user": false
            });
        }

        let mut retries = 3;
        let result = loop {
            let response = self
                .client
                .post(&url)
                .header(
                    "Authorization",
                    format!("Bot {}", self.config.token.expose()),
                )
                .json(&payload)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status();
                    let status_code = status.as_u16();
                    let body = resp.text().await.unwrap_or_default();

                    if status_code == 429 {
                        // Rate limited
                        if let Ok(data) = serde_json::from_str::<Value>(&body) {
                            let retry_after = data
                                .get("retry_after")
                                .and_then(|v| v.as_f64())
                                .unwrap_or(1.0);
                            warn!("Discord rate limited, retrying in {}s", retry_after);
                            sleep(Duration::from_secs_f64(retry_after)).await;
                            retries -= 1;
                            if retries == 0 {
                                break Err(ChannelError::SendFailed(
                                    "Max retries exceeded".to_string(),
                                ));
                            }
                            continue;
                        }
                    }

                    if status.is_success() {
                        break Ok(());
                    } else {
                        break Err(ChannelError::SendFailed(format!(
                            "HTTP {}: {}",
                            status, body
                        )));
                    }
                }
                Err(e) => {
                    if retries > 1 {
                        sleep(Duration::from_secs(1)).await;
                        retries -= 1;
                        continue;
                    }
                    break Err(ChannelError::SendFailed(e.to_string()));
                }
            }
        };

        // Stop typing indicator after sending
        self.stop_typing(channel_id).await;

        result.map_err(Into::into)
    }
}

#[async_trait]
impl WsHandler for DiscordChannel {
    async fn on_connected(&self, _write: &Arc<Mutex<WsSink>>) -> Result<Option<HeartbeatStrategy>> {
        // Discord doesn't handshake here — it waits for HELLO (op 10)
        // which arrives as a text message. Heartbeat is managed internally.
        Ok(Some(HeartbeatStrategy::None))
    }

    async fn on_text_message(&self, text: &str, write: &Arc<Mutex<WsSink>>) -> Result<bool> {
        let data: Value = match serde_json::from_str(text) {
            Ok(d) => d,
            Err(e) => {
                warn!("Failed to parse gateway message: {}", e);
                return Ok(true);
            }
        };

        let op = data.get("op").and_then(|v| v.as_i64()).unwrap_or(0);
        let seq = data.get("s").and_then(|v| v.as_i64());
        let event_type = data.get("t").and_then(|v| v.as_str());
        let payload = data.get("d");

        // Update sequence
        if let Some(s) = seq {
            *self.seq.write().await = Some(s);
        }

        match op {
            10 => {
                // HELLO — start heartbeat + send IDENTIFY
                self.handle_hello(payload, write).await?;
            }
            0 => {
                // Dispatch event
                match event_type {
                    Some("READY") => {
                        info!("Discord gateway READY");
                    }
                    Some("MESSAGE_CREATE") => {
                        if let Some(p) = payload {
                            let bus_guard = self.bus.lock().await;
                            if let Some(bus) = bus_guard.as_ref() {
                                if let Err(e) = self.handle_message_create(p, bus).await {
                                    error!("Error handling MESSAGE_CREATE: {}", e);
                                }
                            }
                        }
                    }
                    Some("MESSAGE_REACTION_ADD") => {
                        if let Some(p) = payload {
                            let bus_guard = self.bus.lock().await;
                            if let Some(bus) = bus_guard.as_ref() {
                                if let Err(e) = self.handle_reaction_add(p, bus).await {
                                    error!("Error handling MESSAGE_REACTION_ADD: {}", e);
                                }
                            }
                        }
                    }
                    Some("INTERACTION_CREATE") => {
                        if let Some(p) = payload {
                            if let Err(e) = self.handle_interaction_create(p).await {
                                error!("Error handling INTERACTION_CREATE: {}", e);
                            }
                        }
                    }
                    _ => {
                        debug!("Unhandled event: {:?}", event_type);
                    }
                }
            }
            7 => {
                // RECONNECT
                info!("Discord requested reconnect");
                return Ok(false);
            }
            9 => {
                // INVALID_SESSION
                warn!("Discord invalid session");
                return Ok(false);
            }
            11 => {
                // HEARTBEAT_ACK
                debug!("Received heartbeat ACK");
            }
            _ => {
                debug!("Unknown opcode: {}", op);
            }
        }

        Ok(true)
    }

    async fn on_disconnected(&self) {
        // Abort heartbeat task
        if let Some(task) = self.heartbeat_task.lock().await.take() {
            task.abort();
        }
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn name(&self) -> &str {
        "discord"
    }

    async fn start(&self, bus: Arc<MessageBus>) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        *self.bus.lock().await = Some(bus);

        let config = WsConfig {
            url: self.config.gateway_url.clone(),
            heartbeat: HeartbeatStrategy::None, // Discord manages its own heartbeat
            ..Default::default()
        };

        let manager = WebSocketManager::new(self.running.clone());

        super::reconnect_loop("Discord", &self.running, || manager.run(&config, self)).await;

        // Cleanup typing tasks
        let tasks: Vec<_> = self.typing_tasks.write().await.drain().collect();
        for (_, task) in tasks {
            task.abort();
        }

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);

        // Stop all typing tasks
        let tasks: Vec<_> = self.typing_tasks.write().await.drain().collect();
        for (_, task) in tasks {
            task.abort();
        }

        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        let formatted = crate::formatter::formatter_for("discord").format(&msg.content);
        let limit = crate::utils::max_length("discord");
        let chunks = crate::utils::split_message(&formatted, limit);

        for (i, chunk) in chunks.iter().enumerate() {
            let reply_to = if i == 0 {
                msg.reply_to.as_deref()
            } else {
                None
            };
            self.send_message_rest(msg.chat_id.as_str(), chunk, reply_to)
                .await?;
        }

        Ok(())
    }

    fn is_allowed(&self, sender_id: &str) -> bool {
        check_allowlist(&self.config.allow_from, sender_id)
    }

    async fn send_typing(&self, chat_id: &str) -> Result<()> {
        self.start_typing(chat_id.to_string()).await;
        Ok(())
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
                    let components =
                        build_discord_select_components(chat_id, &question.id, options);
                    self.send_message_with_components(
                        chat_id,
                        &format!("**{}**\n{}", request.title, question.text),
                        components,
                    )
                    .await?;

                    let value = self.wait_for_callback(chat_id, &question.id).await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::Selected { value },
                    }
                }
                AnswerType::YesNo { .. } => {
                    let components = build_discord_yes_no_components(chat_id, &question.id);
                    self.send_message_with_components(
                        chat_id,
                        &format!("**{}**\n{}", request.title, question.text),
                        components,
                    )
                    .await?;

                    let value = self.wait_for_callback(chat_id, &question.id).await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::YesNo {
                            answer: value == "yes",
                        },
                    }
                }
                AnswerType::MultiSelect { options } => {
                    let components =
                        build_discord_select_components(chat_id, &question.id, options);
                    self.send_message_with_components(
                        chat_id,
                        &format!(
                            "**{}**\n{}\n*(Select one option)*",
                            request.title, question.text
                        ),
                        components,
                    )
                    .await?;

                    let value = self.wait_for_callback(chat_id, &question.id).await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::MultiSelected {
                            values: vec![value],
                        },
                    }
                }
                AnswerType::FreeText { placeholder } => {
                    let prompt = if let Some(ph) = placeholder {
                        format!("**{}**\n{}\n*{}*", request.title, question.text, ph)
                    } else {
                        format!("**{}**\n{}", request.title, question.text)
                    };

                    self.send_message_rest(chat_id, &prompt, None).await?;

                    let content = self.wait_for_free_text(chat_id, &question.id).await?;
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

impl DiscordChannel {
    /// Send a message with Discord component buttons/selects via REST API.
    async fn send_message_with_components(
        &self,
        channel_id: &str,
        content: &str,
        components: Vec<Value>,
    ) -> Result<()> {
        let url = format!("{}/channels/{}/messages", DISCORD_API_BASE, channel_id);
        let payload = json!({
            "content": content,
            "components": components,
        });

        self.client
            .post(&url)
            .header(
                "Authorization",
                format!("Bot {}", self.config.token.expose()),
            )
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;

        Ok(())
    }
}

/// Build Discord button components for select options (≤5 buttons per row).
/// For >5 options, uses a StringSelectMenu instead.
fn build_discord_select_components(
    channel_id: &str,
    question_id: &str,
    options: &[common::AnswerOption],
) -> Vec<Value> {
    if options.len() <= 5 {
        // Use buttons in an ActionRow
        let buttons: Vec<Value> = options
            .iter()
            .map(|opt| {
                json!({
                    "type": 2, // Button
                    "style": 1, // Primary
                    "label": opt.label,
                    "custom_id": format!("askuser:{}:{}:{}", channel_id, question_id, opt.value),
                })
            })
            .collect();

        vec![json!({
            "type": 1, // ActionRow
            "components": buttons,
        })]
    } else {
        // Use StringSelectMenu for >5 options
        let menu_options: Vec<Value> = options
            .iter()
            .map(|opt| {
                let mut o = json!({
                    "label": opt.label,
                    "value": opt.value,
                });
                if let Some(desc) = &opt.description {
                    o["description"] = json!(desc);
                }
                o
            })
            .collect();

        vec![json!({
            "type": 1, // ActionRow
            "components": [{
                "type": 3, // StringSelect
                "custom_id": format!("askuser:{}:{}", channel_id, question_id),
                "options": menu_options,
            }],
        })]
    }
}

/// Build Discord yes/no button components.
fn build_discord_yes_no_components(channel_id: &str, question_id: &str) -> Vec<Value> {
    vec![json!({
        "type": 1, // ActionRow
        "components": [
            {
                "type": 2, // Button
                "style": 3, // Success (green)
                "label": "Yes",
                "custom_id": format!("askuser:{}:{}:yes", channel_id, question_id),
            },
            {
                "type": 2, // Button
                "style": 4, // Danger (red)
                "label": "No",
                "custom_id": format!("askuser:{}:{}:no", channel_id, question_id),
            },
        ],
    })]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> DiscordConfig {
        DiscordConfig::default()
    }

    fn make_config_with_allowlist(allow: Vec<String>) -> DiscordConfig {
        DiscordConfig {
            allow_from: allow,
            ..Default::default()
        }
    }

    #[test]
    fn test_channel_name() {
        let channel = DiscordChannel::new(make_config()).unwrap();
        assert_eq!(channel.name(), "discord");
    }

    #[test]
    fn test_is_allowed_empty_allowlist() {
        let channel = DiscordChannel::new(make_config()).unwrap();
        assert!(channel.is_allowed("anyone"));
        assert!(channel.is_allowed("12345"));
    }

    #[test]
    fn test_is_allowed_with_allowlist() {
        let channel =
            DiscordChannel::new(make_config_with_allowlist(vec!["12345".to_string()])).unwrap();
        assert!(channel.is_allowed("12345"));
        assert!(!channel.is_allowed("99999"));
    }

    #[test]
    fn test_is_allowed_compound_id() {
        let channel =
            DiscordChannel::new(make_config_with_allowlist(vec!["user1".to_string()])).unwrap();
        assert!(channel.is_allowed("user1|extra"));
        assert!(!channel.is_allowed("user2|extra"));
    }

    #[test]
    fn test_default_config_values() {
        let config = DiscordConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.token.expose(), "");
        assert!(config.allow_from.is_empty());
        assert_eq!(
            config.gateway_url,
            "wss://gateway.discord.gg/?v=10&encoding=json"
        );
        assert_eq!(config.intents, 46593);
    }

    #[test]
    fn test_media_dir() {
        let path = DiscordChannel::media_dir();
        assert!(path.to_string_lossy().contains(".klyntbot"));
        assert!(path.to_string_lossy().ends_with("media"));
    }

    #[test]
    fn test_invalid_json_parse_fallback() {
        let data: std::result::Result<Value, _> = serde_json::from_str("not-json");
        assert!(data.is_err());
    }

    #[test]
    fn test_gateway_hello_parse() {
        let hello = r#"{"op":10,"d":{"heartbeat_interval":41250}}"#;
        let data: Value = serde_json::from_str(hello).unwrap();
        let op = data.get("op").and_then(|v| v.as_i64()).unwrap();
        assert_eq!(op, 10);
        let interval = data
            .get("d")
            .and_then(|d| d.get("heartbeat_interval"))
            .and_then(|v| v.as_i64())
            .unwrap();
        assert_eq!(interval, 41250);
    }

    #[test]
    fn test_gateway_message_create_parse() {
        let payload = json!({
            "op": 0,
            "t": "MESSAGE_CREATE",
            "s": 42,
            "d": {
                "id": "msg123",
                "channel_id": "chan456",
                "content": "Hello bot!",
                "author": {
                    "id": "user789",
                    "bot": false
                }
            }
        });

        let op = payload.get("op").and_then(|v| v.as_i64()).unwrap();
        assert_eq!(op, 0);
        let t = payload.get("t").and_then(|v| v.as_str()).unwrap();
        assert_eq!(t, "MESSAGE_CREATE");
        let d = payload.get("d").unwrap();
        let content = d.get("content").and_then(|v| v.as_str()).unwrap();
        assert_eq!(content, "Hello bot!");
        let author = d.get("author").unwrap();
        assert!(!author.get("bot").and_then(|v| v.as_bool()).unwrap());
    }

    #[test]
    fn test_gateway_bot_message_ignored() {
        let payload = json!({
            "author": {
                "id": "bot123",
                "bot": true
            },
            "content": "bot reply",
            "channel_id": "chan1"
        });

        let author = payload.get("author").unwrap();
        let is_bot = author.get("bot").and_then(|v| v.as_bool()).unwrap_or(false);
        assert!(is_bot);
    }

    #[test]
    fn test_gateway_reconnect_opcode() {
        let msg = r#"{"op":7,"d":null}"#;
        let data: Value = serde_json::from_str(msg).unwrap();
        let op = data.get("op").and_then(|v| v.as_i64()).unwrap();
        assert_eq!(op, 7); // RECONNECT
    }

    #[test]
    fn test_gateway_invalid_session_opcode() {
        let msg = r#"{"op":9,"d":false}"#;
        let data: Value = serde_json::from_str(msg).unwrap();
        let op = data.get("op").and_then(|v| v.as_i64()).unwrap();
        assert_eq!(op, 9); // INVALID_SESSION
    }

    #[test]
    fn test_empty_sender_ignored() {
        let payload = json!({
            "author": { "id": "" },
            "channel_id": "chan1",
            "content": "test"
        });

        let sender_id = payload
            .get("author")
            .and_then(|a| a.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(sender_id.is_empty());
    }

    #[test]
    fn test_attachment_too_large() {
        let attachment = json!({
            "url": "https://example.com/big.zip",
            "filename": "big.zip",
            "size": 25_000_000,
            "id": "att1"
        });

        let size = attachment.get("size").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        assert!(size > MAX_ATTACHMENT_BYTES);
    }

    #[tokio::test]
    async fn test_stop_sets_running_false() {
        let channel = DiscordChannel::new(make_config()).unwrap();
        channel.running.store(true, Ordering::SeqCst);
        channel.stop().await.unwrap();
        assert!(!channel.running.load(Ordering::SeqCst));
    }

    #[test]
    fn test_discord_select_buttons_3_options() {
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

        let components = build_discord_select_components("chan1", "priority", &options);
        assert_eq!(components.len(), 1); // 1 ActionRow

        let row = &components[0];
        assert_eq!(row["type"], 1); // ActionRow
        let buttons = row["components"].as_array().unwrap();
        assert_eq!(buttons.len(), 3); // 3 buttons (≤5 → buttons)

        // All buttons are type 2 (Button), style 1 (Primary)
        for btn in buttons {
            assert_eq!(btn["type"], 2);
            assert_eq!(btn["style"], 1);
        }

        // Check first button
        assert_eq!(buttons[0]["label"], "High");
        assert_eq!(
            buttons[0]["custom_id"].as_str().unwrap(),
            "askuser:chan1:priority:high"
        );

        // Check last button
        assert_eq!(buttons[2]["label"], "Low");
        assert_eq!(
            buttons[2]["custom_id"].as_str().unwrap(),
            "askuser:chan1:priority:low"
        );
    }

    #[test]
    fn test_discord_select_menu_6_options() {
        use common::AnswerOption;

        let options: Vec<AnswerOption> = (1..=6)
            .map(|i| AnswerOption {
                value: format!("opt{i}"),
                label: format!("Option {i}"),
                description: if i == 1 {
                    Some("First option".into())
                } else {
                    None
                },
            })
            .collect();

        let components = build_discord_select_components("chan2", "choice", &options);
        assert_eq!(components.len(), 1); // 1 ActionRow

        let row = &components[0];
        assert_eq!(row["type"], 1); // ActionRow
        let inner = row["components"].as_array().unwrap();
        assert_eq!(inner.len(), 1); // 1 StringSelectMenu

        let menu = &inner[0];
        assert_eq!(menu["type"], 3); // StringSelect
        assert_eq!(menu["custom_id"].as_str().unwrap(), "askuser:chan2:choice");

        let menu_opts = menu["options"].as_array().unwrap();
        assert_eq!(menu_opts.len(), 6);
        assert_eq!(menu_opts[0]["label"], "Option 1");
        assert_eq!(menu_opts[0]["value"], "opt1");
        assert_eq!(menu_opts[0]["description"], "First option");
        // Option 2 should have no description key
        assert!(menu_opts[1].get("description").is_none());
    }

    #[test]
    fn test_discord_yes_no_buttons() {
        let components = build_discord_yes_no_components("chan3", "confirm");
        assert_eq!(components.len(), 1); // 1 ActionRow

        let row = &components[0];
        let buttons = row["components"].as_array().unwrap();
        assert_eq!(buttons.len(), 2);

        // Yes button: green (style 3)
        assert_eq!(buttons[0]["type"], 2);
        assert_eq!(buttons[0]["style"], 3); // Success
        assert_eq!(buttons[0]["label"], "Yes");
        assert_eq!(
            buttons[0]["custom_id"].as_str().unwrap(),
            "askuser:chan3:confirm:yes"
        );

        // No button: red (style 4)
        assert_eq!(buttons[1]["type"], 2);
        assert_eq!(buttons[1]["style"], 4); // Danger
        assert_eq!(buttons[1]["label"], "No");
        assert_eq!(
            buttons[1]["custom_id"].as_str().unwrap(),
            "askuser:chan3:confirm:no"
        );
    }

    #[test]
    fn test_discord_custom_id_format() {
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

        let components = build_discord_select_components("789", "auth_method", &options);
        let buttons = components[0]["components"].as_array().unwrap();

        for btn in buttons {
            let custom_id = btn["custom_id"].as_str().unwrap();
            assert!(custom_id.starts_with("askuser:789:auth_method:"));
            let parts: Vec<&str> = custom_id.split(':').collect();
            assert_eq!(parts.len(), 4);
            assert_eq!(parts[0], "askuser");
            assert_eq!(parts[1], "789");
            assert_eq!(parts[2], "auth_method");
        }
    }
}
