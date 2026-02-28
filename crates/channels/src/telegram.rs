//! Telegram channel using direct HTTP Bot API (long polling).
//!
//! Lightweight implementation without teloxide - just reqwest HTTP calls.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::formatter::ChannelFormatter;
use crate::shared::{InteractionTracker, TypingManager};
use crate::{check_allowlist, Channel};
use bus::{InboundMessage, MessageBus, MessageKind, OutboundMessage};
use common::{
    utils::truncate_chars, Answer, AnswerType, AnswerValue, ChannelError, FormResponse,
    InteractionRequest, Result,
};
use config::schema::TelegramConfig;
use providers::TranscriptionProvider;

/// Telegram channel implementation
pub struct TelegramChannel {
    config: TelegramConfig,
    client: Client,
    api_base: String,
    transcriber: Option<TranscriptionProvider>,
    running: Arc<AtomicBool>,
    typing: TypingManager,
    interactions: InteractionTracker,
}

impl TelegramChannel {
    /// Create a new Telegram channel
    pub fn new(config: TelegramConfig, groq_api_key: Option<String>) -> Result<Self> {
        let mut client_builder = Client::builder().timeout(Duration::from_secs(30));

        // Configure proxy if specified
        if let Some(ref proxy_url) = config.proxy {
            match reqwest::Proxy::all(proxy_url) {
                Ok(proxy) => {
                    info!("Telegram using proxy: {}", proxy_url);
                    client_builder = client_builder.proxy(proxy);
                }
                Err(e) => {
                    warn!("Failed to configure Telegram proxy: {}", e);
                }
            }
        }

        let client = client_builder
            .build()
            .map_err(|e| ChannelError::ConnectionFailed(e.to_string()))?;

        let api_base = format!("https://api.telegram.org/bot{}", config.token.expose());

        let transcriber = groq_api_key
            .filter(|k| !k.is_empty())
            .map(TranscriptionProvider::new)
            .transpose()?;

        Ok(Self {
            config,
            client,
            api_base,
            transcriber,
            running: Arc::new(AtomicBool::new(false)),
            typing: TypingManager::new(),
            interactions: InteractionTracker::new(),
        })
    }

    /// Call Telegram Bot API with retry logic
    async fn api_call(&self, method: &str, params: Value) -> Result<Value> {
        self.api_call_with_retry(method, params, 3).await
    }

    /// Call Telegram Bot API with configurable retry attempts
    async fn api_call_with_retry(
        &self,
        method: &str,
        params: Value,
        max_retries: u32,
    ) -> Result<Value> {
        let url = format!("{}/{}", self.api_base, method);
        let mut last_error = None;

        for attempt in 0..max_retries {
            match self.client.post(&url).json(&params).send().await {
                Ok(response) => {
                    match response.json::<TelegramResponse>().await {
                        Ok(result) => {
                            if result.ok {
                                return Ok(result.result);
                            } else {
                                // API returned an error
                                return Err(ChannelError::SendFailed(format!(
                                    "Telegram API error: {:?}",
                                    result.description
                                ))
                                .into());
                            }
                        }
                        Err(e) => {
                            last_error = Some(format!("Failed to parse response: {}", e));
                        }
                    }
                }
                Err(e) => {
                    last_error = Some(e.to_string());
                }
            }

            // Exponential backoff: 1s, 2s, 4s...
            if attempt < max_retries - 1 {
                let delay = Duration::from_secs(2u64.pow(attempt));
                debug!("Telegram API call failed, retrying in {:?}", delay);
                sleep(delay).await;
            }
        }

        Err(ChannelError::ConnectionFailed(format!(
            "Failed after {} retries: {}",
            max_retries,
            last_error.unwrap_or_else(|| "Unknown error".to_string())
        ))
        .into())
    }

    /// Long polling loop
    async fn poll_updates(&self, bus: Arc<MessageBus>) -> Result<()> {
        let mut offset: i64 = 0;

        info!("Starting Telegram long polling");

        // Set bot commands
        if let Err(e) = self.set_bot_commands().await {
            warn!("Failed to set bot commands: {}", e);
        }

        while self.running.load(Ordering::SeqCst) {
            let params = json!({
                "offset": offset,
                "timeout": 30,
                "allowed_updates": ["message", "message_reaction", "callback_query"]
            });

            match self.api_call("getUpdates", params).await {
                Ok(result) => {
                    if let Some(updates) = result.as_array() {
                        for update in updates {
                            if let Err(e) = self.handle_update(update, &bus).await {
                                error!("Error handling update: {}", e);
                            }

                            // Update offset
                            if let Some(update_id) =
                                update.get("update_id").and_then(|v| v.as_i64())
                            {
                                offset = update_id + 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Polling error: {}", e);
                    sleep(Duration::from_secs(5)).await;
                }
            }
        }

        Ok(())
    }

    /// Handle a reaction update (message_reaction field in the update)
    async fn handle_reaction_update(&self, reaction: &Value, bus: &MessageBus) -> Result<()> {
        // Extract user info — Telegram sends "user" or "actor_chat" for reaction updates
        let user_id = reaction
            .get("user")
            .and_then(|u| u.get("id"))
            .and_then(|v| v.as_i64())
            .map(|id| id.to_string())
            .unwrap_or_default();

        if user_id.is_empty() {
            debug!("Reaction update missing user ID, skipping");
            return Ok(());
        }

        // Check allowlist
        if !check_allowlist(&self.config.allow_from, &user_id) {
            warn!("Access denied for sender {} on Telegram reaction", user_id);
            return Ok(());
        }

        let chat_id = reaction
            .get("chat")
            .and_then(|c| c.get("id"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                ChannelError::InvalidConfig("Reaction update missing chat.id".to_string())
            })?;

        // Extract emoji from new_reaction array
        // Telegram sends: "new_reaction": [{"type": "emoji", "emoji": "👍"}]
        let emoji = reaction
            .get("new_reaction")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|r| r.get("emoji"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if emoji.is_empty() {
            // Reaction was removed (new_reaction is empty), ignore
            debug!("Telegram reaction removed (empty new_reaction), skipping");
            return Ok(());
        }

        debug!(
            "Telegram reaction from {}: {} in chat {}",
            user_id, emoji, chat_id
        );

        let inbound = InboundMessage::new("telegram", &user_id, chat_id.to_string(), &emoji)
            .with_kind(MessageKind::Reaction);

        bus.publish_inbound(inbound)
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to publish to bus: {}", e)))?;

        Ok(())
    }

    /// Handle a single update
    async fn handle_update(&self, update: &Value, bus: &MessageBus) -> Result<()> {
        // Handle callback_query updates (from InlineKeyboard buttons)
        if let Some(callback) = update.get("callback_query") {
            return self.handle_callback_query(callback).await;
        }

        // Handle reaction updates
        if let Some(reaction) = update.get("message_reaction") {
            return self.handle_reaction_update(reaction, bus).await;
        }

        let message = match update.get("message") {
            Some(m) => m,
            None => return Ok(()), // Not a message update
        };

        // Check if this is a pending free_text interaction response
        if let Some(chat_id) = message
            .get("chat")
            .and_then(|c| c.get("id"))
            .and_then(|v| v.as_i64())
        {
            let chat_id_str = chat_id.to_string();
            if let Some(key) = self.interactions.find_free_text_key(&chat_id_str) {
                if let Some(text) = message.get("text").and_then(|v| v.as_str()) {
                    if self.interactions.resolve_free_text(&key, text.to_string()) {
                        return Ok(());
                    }
                }
            }
        }

        // Extract user and chat info
        let user = message.get("from").ok_or_else(|| {
            ChannelError::InvalidConfig("Message missing 'from' field".to_string())
        })?;

        let user_id = user
            .get("id")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| ChannelError::InvalidConfig("User missing 'id'".to_string()))?;

        let username = user.get("username").and_then(|v| v.as_str());

        let sender_id = if let Some(uname) = username {
            format!("{}|{}", user_id, uname)
        } else {
            user_id.to_string()
        };

        // Check allowlist
        if !check_allowlist(&self.config.allow_from, &sender_id) {
            warn!("Access denied for sender {} on Telegram", sender_id);
            return Ok(());
        }

        let chat_id = message
            .get("chat")
            .and_then(|c| c.get("id"))
            .and_then(|v| v.as_i64())
            .ok_or_else(|| ChannelError::InvalidConfig("Message missing chat.id".to_string()))?;

        // Handle commands
        if let Some(text) = message.get("text").and_then(|v| v.as_str()) {
            if text.starts_with('/') {
                return self.handle_command(text, chat_id, bus).await;
            }
        }

        // Start typing indicator
        let chat_id_str = chat_id.to_string();
        self.start_typing(&chat_id_str).await;

        // Build content from message
        let mut content_parts = Vec::with_capacity(2);
        let mut media_paths = Vec::with_capacity(2);

        // Text content
        if let Some(text) = message.get("text").and_then(|v| v.as_str()) {
            content_parts.push(text.to_string());
        }
        if let Some(caption) = message.get("caption").and_then(|v| v.as_str()) {
            content_parts.push(caption.to_string());
        }

        // Handle media (photos, voice, audio, documents)
        if let Some(photo) = message.get("photo").and_then(|v| v.as_array()) {
            if let Some(largest) = photo.last() {
                if let Ok(path) = self.download_media(largest, "image").await {
                    media_paths.push(path.clone());
                    content_parts.push(format!("[image: {}]", path));
                }
            }
        }

        if let Some(voice) = message.get("voice") {
            if let Ok(path) = self.download_media(voice, "voice").await {
                // Transcribe if transcriber available
                if let Some(ref transcriber) = self.transcriber {
                    match transcriber.transcribe(&path).await {
                        Ok(text) => {
                            debug!("Transcribed voice: {}...", truncate_chars(&text, 50, ""));
                            content_parts.push(format!("[transcription: {}]", text));
                        }
                        Err(e) => {
                            warn!("Transcription failed: {}", e);
                            content_parts.push(format!("[voice: {}]", path));
                        }
                    }
                } else {
                    content_parts.push(format!("[voice: {}]", path));
                }
                media_paths.push(path);
            }
        }

        if let Some(audio) = message.get("audio") {
            if let Ok(path) = self.download_media(audio, "audio").await {
                media_paths.push(path.clone());
                content_parts.push(format!("[audio: {}]", path));
            }
        }

        if let Some(document) = message.get("document") {
            if let Ok(path) = self.download_media(document, "file").await {
                media_paths.push(path.clone());
                content_parts.push(format!("[file: {}]", path));
            }
        }

        let content = if content_parts.is_empty() {
            "[empty message]".to_string()
        } else {
            content_parts.join("\n")
        };

        debug!(
            "Telegram message from {}: {}...",
            sender_id,
            truncate_chars(&content, 50, "")
        );

        // Publish to bus
        let mut inbound = InboundMessage::new("telegram", sender_id, chat_id_str, content);
        inbound.media = media_paths;

        bus.publish_inbound(inbound)
            .await
            .map_err(|e| ChannelError::SendFailed(format!("Failed to publish to bus: {}", e)))?;

        Ok(())
    }

    /// Download media file
    async fn download_media(&self, file_obj: &Value, media_type: &str) -> Result<String> {
        let file_id = file_obj
            .get("file_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::SendFailed("Missing file_id".to_string()))?;

        // Get file path
        let file_info = self
            .api_call("getFile", json!({"file_id": file_id}))
            .await?;
        let file_path = file_info
            .get("file_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::SendFailed("Missing file_path".to_string()))?;

        // Download file
        let download_url = format!(
            "https://api.telegram.org/file/bot{}/{}",
            self.config.token.expose(),
            file_path
        );

        let bytes = self
            .client
            .get(&download_url)
            .send()
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?
            .bytes()
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;

        // Save to media directory
        let media_dir = dirs::home_dir()
            .ok_or_else(|| ChannelError::SendFailed("Cannot find home dir".to_string()))?
            .join(".klyntbot")
            .join("media");

        tokio::fs::create_dir_all(&media_dir)
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;

        let ext = self.get_extension(
            media_type,
            file_obj.get("mime_type").and_then(|v| v.as_str()),
        );
        let filename = format!("{}{}", &file_id[..16.min(file_id.len())], ext);
        let file_path_local = media_dir.join(&filename);

        tokio::fs::write(&file_path_local, bytes)
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))?;

        Ok(file_path_local.to_string_lossy().to_string())
    }

    /// Get file extension
    fn get_extension(&self, media_type: &str, mime_type: Option<&str>) -> &str {
        if let Some(mime) = mime_type {
            match mime {
                "image/jpeg" => return ".jpg",
                "image/png" => return ".png",
                "audio/ogg" => return ".ogg",
                "audio/mpeg" => return ".mp3",
                _ => {}
            }
        }

        match media_type {
            "image" => ".jpg",
            "voice" => ".ogg",
            "audio" => ".mp3",
            _ => "",
        }
    }

    /// Handle bot commands
    async fn handle_command(&self, text: &str, chat_id: i64, bus: &MessageBus) -> Result<()> {
        let parts: Vec<&str> = text.split_whitespace().collect();
        let command = parts.first().copied().unwrap_or("");

        match command {
            "/start" => {
                self.send_message(
                    chat_id,
                    "Hi! I'm klyntbot.\n\nSend me a message and I'll respond!\nType /help to see available commands.",
                )
                .await?;
            }
            "/help" => {
                self.send_message(
                    chat_id,
                    "<b>klyntbot commands</b>\n\n/start — Start the bot\n/reset — Reset conversation history\n/help — Show this help message\n\nJust send me a text message to chat!",
                )
                .await?;
            }
            "/reset" => {
                // Clear session by publishing a reset message to the bus
                let session_key = format!("telegram:{}", chat_id);
                let reset_msg = InboundMessage::new(
                    common::SYSTEM_CHANNEL,
                    common::TELEGRAM_RESET_SENDER,
                    session_key.as_str(),
                    "__RESET_SESSION__",
                );

                // Publish reset request
                if let Err(e) = bus.publish_inbound(reset_msg).await {
                    warn!("Failed to publish reset message: {}", e);
                }

                self.send_message(
                    chat_id,
                    "Conversation history cleared! Starting fresh.\n\nPrevious messages have been removed from memory. You can now start a new conversation.",
                )
                .await?;
            }
            _ => {
                // Unknown command - ignore
            }
        }

        Ok(())
    }

    /// Send a text message
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<()> {
        let params = json!({
            "chat_id": chat_id,
            "text": text,
            "parse_mode": "HTML"
        });

        self.api_call("sendMessage", params).await?;
        Ok(())
    }

    /// Start continuous typing indicator for a chat
    async fn start_typing(&self, chat_id: &str) {
        let chat_id_i64: i64 = match chat_id.parse() {
            Ok(id) => id,
            Err(_) => return,
        };
        let api_base = self.api_base.clone();
        let client = self.client.clone();

        // Telegram typing expires after ~5s, so resend every 4s
        self.typing
            .start(chat_id, Duration::from_secs(4), move || {
                let client = client.clone();
                let api_base = api_base.clone();
                async move {
                    let params = json!({
                        "chat_id": chat_id_i64,
                        "action": "typing"
                    });
                    let url = format!("{}/sendChatAction", api_base);
                    if let Err(e) = client.post(&url).json(&params).send().await {
                        warn!(
                            "Failed to send typing indicator to Telegram chat {}: {}",
                            chat_id_i64, e
                        );
                    }
                }
            })
            .await;
    }

    /// Stop typing indicator for a chat
    async fn stop_typing(&self, chat_id: &str) {
        self.typing.stop(chat_id).await;
    }

    /// Handle a callback_query from an inline keyboard button press.
    async fn handle_callback_query(&self, callback: &Value) -> Result<()> {
        let callback_id = callback.get("id").and_then(|v| v.as_str()).unwrap_or("");

        let data = callback.get("data").and_then(|v| v.as_str()).unwrap_or("");

        // Answer the callback to dismiss the loading spinner
        let _ = self
            .api_call(
                "answerCallbackQuery",
                json!({ "callback_query_id": callback_id }),
            )
            .await;

        // Parse callback_data format: "askuser:{chat_id}:{question_id}:{value}"
        let parts: Vec<&str> = data.splitn(4, ':').collect();
        if parts.len() < 4 || parts[0] != "askuser" {
            debug!("Ignoring callback with unexpected data format: {}", data);
            return Ok(());
        }

        let chat_id = parts[1];
        let question_id = parts[2];
        let value = parts[3];
        let key = format!("{}:{}", chat_id, question_id);

        // Resolve the pending interaction
        self.interactions.resolve_single(&key, value.to_string());

        // Edit the original message to show the selection (remove keyboard)
        if let Some(msg) = callback.get("message") {
            if let (Some(msg_chat_id), Some(msg_id)) = (
                msg.get("chat")
                    .and_then(|c| c.get("id"))
                    .and_then(|v| v.as_i64()),
                msg.get("message_id").and_then(|v| v.as_i64()),
            ) {
                let original_text = msg
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Question");
                let _ = self
                    .api_call(
                        "editMessageText",
                        json!({
                            "chat_id": msg_chat_id,
                            "message_id": msg_id,
                            "text": format!("{}\n\n✓ Selected: {}", original_text, value),
                        }),
                    )
                    .await;
            }
        }

        Ok(())
    }

    /// Set bot commands menu
    async fn set_bot_commands(&self) -> Result<()> {
        let params = json!({
            "commands": [
                {"command": "start", "description": "Start the bot"},
                {"command": "reset", "description": "Reset conversation history"},
                {"command": "help", "description": "Show available commands"}
            ]
        });

        self.api_call("setMyCommands", params).await?;
        Ok(())
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn start(&self, bus: Arc<MessageBus>) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        self.poll_updates(bus).await
    }

    async fn stop(&self) -> Result<()> {
        self.running.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn send(&self, msg: &OutboundMessage) -> Result<()> {
        // Stop typing indicator for this chat
        self.stop_typing(msg.chat_id.as_str()).await;

        let chat_id: i64 = msg
            .chat_id
            .as_str()
            .parse()
            .map_err(|_| ChannelError::SendFailed("Invalid chat_id".to_string()))?;

        let formatted = crate::formatter::formatter_for("telegram").format(&msg.content);
        let limit = crate::utils::max_length("telegram");
        let chunks = crate::utils::split_message(&formatted, limit);

        for (i, chunk) in chunks.iter().enumerate() {
            let params = json!({
                "chat_id": chat_id,
                "text": chunk,
                "parse_mode": "HTML"
            });

            match self.api_call("sendMessage", params).await {
                Ok(_) => {}
                Err(_) => {
                    // Fallback: strip HTML tags from this chunk and send as plain text
                    warn!(
                        "HTML parsing failed for chunk {}, falling back to plain text",
                        i
                    );
                    let plain_chunk = crate::formatter::PlainTextFormatter.format(chunk);

                    let params = json!({
                        "chat_id": chat_id,
                        "text": plain_chunk
                    });
                    self.api_call("sendMessage", params).await?;
                }
            }

            // Small delay between chunks to avoid rate limiting
            if i < chunks.len() - 1 {
                sleep(Duration::from_millis(100)).await;
            }
        }

        Ok(())
    }

    fn is_allowed(&self, sender_id: &str) -> bool {
        check_allowlist(&self.config.allow_from, sender_id)
    }

    async fn send_typing(&self, chat_id: &str) -> Result<()> {
        self.start_typing(chat_id).await;
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
        let chat_id_i64: i64 = chat_id
            .parse()
            .map_err(|_| ChannelError::SendFailed("Invalid chat_id for interaction".into()))?;

        let mut answers = Vec::new();

        for question in &request.questions {
            let answer = match &question.answer_type {
                AnswerType::SingleSelect { options } => {
                    let keyboard = build_single_select_keyboard(chat_id, &question.id, options);
                    self.api_call(
                        "sendMessage",
                        json!({
                            "chat_id": chat_id_i64,
                            "text": format!("<b>{}</b>\n{}", request.title, question.text),
                            "parse_mode": "HTML",
                            "reply_markup": keyboard,
                        }),
                    )
                    .await?;

                    let value = self
                        .interactions
                        .wait_for_callback(chat_id, &question.id)
                        .await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::Selected { value },
                    }
                }
                AnswerType::YesNo { .. } => {
                    let keyboard = build_yes_no_keyboard(chat_id, &question.id);
                    self.api_call(
                        "sendMessage",
                        json!({
                            "chat_id": chat_id_i64,
                            "text": format!("<b>{}</b>\n{}", request.title, question.text),
                            "parse_mode": "HTML",
                            "reply_markup": keyboard,
                        }),
                    )
                    .await?;

                    let value = self
                        .interactions
                        .wait_for_callback(chat_id, &question.id)
                        .await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::YesNo {
                            answer: value == "yes",
                        },
                    }
                }
                AnswerType::MultiSelect { options } => {
                    // For multi_select, present as single_select buttons
                    // (simplified: user selects one at a time, "Done" to finish)
                    let keyboard = build_single_select_keyboard(chat_id, &question.id, options);
                    self.api_call(
                        "sendMessage",
                        json!({
                            "chat_id": chat_id_i64,
                            "text": format!("<b>{}</b>\n{}\n<i>(Select one option)</i>", request.title, question.text),
                            "parse_mode": "HTML",
                            "reply_markup": keyboard,
                        }),
                    )
                    .await?;

                    let value = self
                        .interactions
                        .wait_for_callback(chat_id, &question.id)
                        .await?;
                    Answer {
                        question_id: question.id.clone(),
                        value: AnswerValue::MultiSelected {
                            values: vec![value],
                        },
                    }
                }
                AnswerType::FreeText { placeholder } => {
                    let prompt = if let Some(ph) = placeholder {
                        format!(
                            "<b>{}</b>\n{}\n<i>({})</i>",
                            request.title, question.text, ph
                        )
                    } else {
                        format!("<b>{}</b>\n{}", request.title, question.text)
                    };

                    self.api_call(
                        "sendMessage",
                        json!({
                            "chat_id": chat_id_i64,
                            "text": prompt,
                            "parse_mode": "HTML",
                        }),
                    )
                    .await?;

                    let content = self
                        .interactions
                        .wait_for_free_text(chat_id, &question.id)
                        .await?;
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

/// Build an InlineKeyboardMarkup for single_select options.
/// Buttons arranged in rows of 2, with callback_data `"askuser:{chat_id}:{question_id}:{value}"`.
fn build_single_select_keyboard(
    chat_id: &str,
    question_id: &str,
    options: &[common::AnswerOption],
) -> Value {
    let buttons: Vec<Value> = options
        .iter()
        .map(|opt| {
            json!({
                "text": opt.label,
                "callback_data": format!("askuser:{}:{}:{}", chat_id, question_id, opt.value),
            })
        })
        .collect();

    // Arrange in rows of 2
    let rows: Vec<Value> = buttons
        .chunks(2)
        .map(|chunk| Value::Array(chunk.to_vec()))
        .collect();

    json!({ "inline_keyboard": rows })
}

/// Build an InlineKeyboardMarkup for yes/no question.
/// Single row with Yes and No buttons.
fn build_yes_no_keyboard(chat_id: &str, question_id: &str) -> Value {
    json!({
        "inline_keyboard": [[
            {
                "text": "✅ Yes",
                "callback_data": format!("askuser:{}:{}:yes", chat_id, question_id),
            },
            {
                "text": "❌ No",
                "callback_data": format!("askuser:{}:{}:no", chat_id, question_id),
            },
        ]]
    })
}

/// Telegram API response
#[derive(Debug, Deserialize)]
struct TelegramResponse {
    ok: bool,
    result: Value,
    description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formatter::{ChannelFormatter, TelegramFormatter};
    use crate::utils;
    use common::AnswerOption;

    #[test]
    fn test_markdown_to_html_code_blocks() {
        let f = TelegramFormatter;
        let input =
            "Here's some code:\n```rust\nfn main() {\n    let v: Vec<i32> = vec![];\n}\n```\nDone!";
        let output = f.format(input);
        assert!(output.contains("<pre><code>"));
        assert!(output.contains("fn main()"));
        assert!(output.contains("&lt;i32&gt;")); // < and > should be escaped in code
    }

    #[test]
    fn test_markdown_to_html_inline_code() {
        let f = TelegramFormatter;
        let input = "Use `println!` to print.";
        let output = f.format(input);
        assert!(output.contains("<code>println!</code>"));
    }

    #[test]
    fn test_markdown_to_html_bold_italic() {
        let f = TelegramFormatter;
        let input = "This is **bold** and this is _italic_ and **_both_**.";
        let output = f.format(input);
        assert!(output.contains("<b>bold</b>"));
        assert!(output.contains("<i>italic</i>"));
    }

    #[test]
    fn test_markdown_to_html_links() {
        let f = TelegramFormatter;
        let input = "Check out [Rust](https://rust-lang.org).";
        let output = f.format(input);
        assert!(output.contains(r#"<a href="https://rust-lang.org">Rust</a>"#));
    }

    #[test]
    fn test_markdown_to_html_strikethrough() {
        let f = TelegramFormatter;
        let input = "This is ~~wrong~~ correct.";
        let output = f.format(input);
        assert!(output.contains("<s>wrong</s>"));
    }

    #[test]
    fn test_markdown_to_html_bullets() {
        let f = TelegramFormatter;
        let input = "List:\n- Item 1\n- Item 2";
        let output = f.format(input);
        assert!(output.contains("• Item 1"));
        assert!(output.contains("• Item 2"));
    }

    #[test]
    fn test_message_split_short() {
        let limit = utils::max_length("telegram");
        let input = "Short message";
        let chunks = utils::split_message(input, limit);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Short message");
    }

    #[test]
    fn test_message_split_long() {
        let limit = utils::max_length("telegram");
        let long_line = "a".repeat(5000);
        let chunks = utils::split_message(&long_line, limit);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= limit);
        }
    }

    // --- Interaction keyboard tests ---

    #[test]
    fn test_single_select_keyboard_3_options() {
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

        let keyboard = build_single_select_keyboard("123", "priority", &options);
        let rows = keyboard["inline_keyboard"].as_array().unwrap();

        // 3 options → 2 rows (2 + 1)
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].as_array().unwrap().len(), 2);
        assert_eq!(rows[1].as_array().unwrap().len(), 1);

        // Check callback_data format
        let first_btn = &rows[0][0];
        assert_eq!(first_btn["text"], "High");
        assert_eq!(first_btn["callback_data"], "askuser:123:priority:high");
    }

    #[test]
    fn test_yes_no_keyboard() {
        let keyboard = build_yes_no_keyboard("456", "confirm");
        let rows = keyboard["inline_keyboard"].as_array().unwrap();

        // 1 row with 2 buttons
        assert_eq!(rows.len(), 1);
        let buttons = rows[0].as_array().unwrap();
        assert_eq!(buttons.len(), 2);

        assert!(buttons[0]["text"].as_str().unwrap().contains("Yes"));
        assert_eq!(buttons[0]["callback_data"], "askuser:456:confirm:yes");
        assert!(buttons[1]["text"].as_str().unwrap().contains("No"));
        assert_eq!(buttons[1]["callback_data"], "askuser:456:confirm:no");
    }

    #[test]
    fn test_callback_data_format() {
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

        let keyboard = build_single_select_keyboard("789", "auth_method", &options);
        let rows = keyboard["inline_keyboard"].as_array().unwrap();

        // Verify all callback_data follows the expected format
        for row in rows {
            for btn in row.as_array().unwrap() {
                let data = btn["callback_data"].as_str().unwrap();
                assert!(data.starts_with("askuser:789:auth_method:"));
            }
        }
    }

    #[test]
    fn test_message_split_multiline() {
        let limit = utils::max_length("telegram");
        let mut lines = Vec::new();
        for i in 0..200 {
            lines.push(format!("Line {} with some content", i));
        }
        let input = lines.join("\n");
        let chunks = utils::split_message(&input, limit);

        if input.len() > limit {
            assert!(chunks.len() > 1);
        }

        for chunk in &chunks {
            assert!(chunk.len() <= limit);
        }
    }
}
