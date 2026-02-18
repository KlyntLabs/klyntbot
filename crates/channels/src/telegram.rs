//! Telegram channel using direct HTTP Bot API (long polling).
//!
//! Lightweight implementation without teloxide - just reqwest HTTP calls.

use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Cached compiled regexes for markdown-to-HTML conversion.
struct MarkdownRegexes {
    code_block: Regex,
    inline_code: Regex,
    header: Regex,
    blockquote: Regex,
    link: Regex,
    bold_star: Regex,
    bold_underscore: Regex,
    italic: Regex,
    strikethrough: Regex,
    bullet: Regex,
}

fn markdown_regexes() -> &'static MarkdownRegexes {
    static REGEXES: OnceLock<MarkdownRegexes> = OnceLock::new();
    REGEXES.get_or_init(|| MarkdownRegexes {
        code_block: Regex::new(r"```[\w]*\n?([\s\S]*?)```").unwrap(),
        inline_code: Regex::new(r"`([^`]+)`").unwrap(),
        header: Regex::new(r"(?m)^#{1,6}\s+(.+)$").unwrap(),
        blockquote: Regex::new(r"(?m)^>\s*(.*)$").unwrap(),
        link: Regex::new(r"\[([^\]]+)\]\(([^)]+)\)").unwrap(),
        bold_star: Regex::new(r"\*\*(.+?)\*\*").unwrap(),
        bold_underscore: Regex::new(r"__(.+?)__").unwrap(),
        italic: Regex::new(r"(?:^|(?P<pre>[^a-zA-Z0-9]))_([^_]+)_(?:$|(?P<post>[^a-zA-Z0-9]))")
            .unwrap(),
        strikethrough: Regex::new(r"~~(.+?)~~").unwrap(),
        bullet: Regex::new(r"(?m)^[-*]\s+").unwrap(),
    })
}

use crate::{check_allowlist, Channel};
use bus::{InboundMessage, MessageBus, OutboundMessage};
use common::{ChannelError, Result};
use config::schema::TelegramConfig;
use providers::TranscriptionProvider;

/// Telegram channel implementation
pub struct TelegramChannel {
    config: TelegramConfig,
    client: Client,
    api_base: String,
    transcriber: Option<TranscriptionProvider>,
    running: Arc<AtomicBool>,
    typing_tasks: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
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
            typing_tasks: Arc::new(RwLock::new(HashMap::new())),
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
                "allowed_updates": ["message"]
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

    /// Handle a single update
    async fn handle_update(&self, update: &Value, bus: &MessageBus) -> Result<()> {
        let message = match update.get("message") {
            Some(m) => m,
            None => return Ok(()), // Not a message update
        };

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
                            debug!(
                                "Transcribed voice: {}...",
                                &text.chars().take(50).collect::<String>()
                            );
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
            &content.chars().take(50).collect::<String>()
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
                    "system",
                    "telegram_reset",
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
        // Stop any existing typing task for this chat
        self.stop_typing(chat_id).await;

        let chat_id_str = chat_id.to_string();
        let api_base = self.api_base.clone();
        let client = self.client.clone();

        // Spawn a task that continuously sends typing actions
        let task = tokio::spawn(async move {
            let chat_id_i64: i64 = match chat_id_str.parse() {
                Ok(id) => id,
                Err(_) => return,
            };

            loop {
                let params = json!({
                    "chat_id": chat_id_i64,
                    "action": "typing"
                });

                let url = format!("{}/sendChatAction", api_base);
                if let Err(e) = client.post(&url).json(&params).send().await {
                    tracing::warn!(
                        "Failed to send typing indicator to Telegram chat {}: {}",
                        chat_id_i64,
                        e
                    );
                }

                // Send typing action every 4 seconds (Telegram typing lasts ~5s)
                sleep(Duration::from_secs(4)).await;
            }
        });

        // Store the task so we can cancel it later
        self.typing_tasks
            .write()
            .await
            .insert(chat_id.to_string(), task);
    }

    /// Stop typing indicator for a chat
    async fn stop_typing(&self, chat_id: &str) {
        if let Some(task) = self.typing_tasks.write().await.remove(chat_id) {
            task.abort();
        }
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

    /// Split message into chunks that fit Telegram's 4096 character limit
    fn split_message(text: &str) -> Vec<String> {
        const MAX_LENGTH: usize = 4096;

        if text.len() <= MAX_LENGTH {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::with_capacity(text.len() / MAX_LENGTH + 1);
        let mut current_chunk = String::new();

        // Split by lines to avoid breaking in the middle of formatting
        for line in text.lines() {
            // If a single line is longer than MAX_LENGTH, split it by characters
            if line.len() > MAX_LENGTH {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.clone());
                    current_chunk.clear();
                }

                // Split long line into chunks
                let mut remaining = line;
                while remaining.len() > MAX_LENGTH {
                    let split_point = MAX_LENGTH;
                    chunks.push(remaining[..split_point].to_string());
                    remaining = &remaining[split_point..];
                }
                if !remaining.is_empty() {
                    current_chunk = remaining.to_string();
                }
                continue;
            }

            // Check if adding this line would exceed the limit
            let line_with_newline = if current_chunk.is_empty() {
                line.to_string()
            } else {
                format!("\n{}", line)
            };

            if current_chunk.len() + line_with_newline.len() > MAX_LENGTH {
                // Start a new chunk
                chunks.push(current_chunk.clone());
                current_chunk = line.to_string();
            } else {
                current_chunk.push_str(&line_with_newline);
            }
        }

        if !current_chunk.is_empty() {
            chunks.push(current_chunk);
        }

        chunks
    }

    /// Convert markdown to Telegram HTML
    /// Based on the Python implementation for robust handling
    fn markdown_to_html(text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }

        let re = markdown_regexes();
        let mut result = text.to_string();

        // 1. Extract and protect code blocks first (preserve from other processing)
        let mut code_blocks: Vec<String> = Vec::new();
        result = re
            .code_block
            .replace_all(&result, |caps: &regex::Captures| {
                code_blocks.push(caps.get(1).unwrap().as_str().to_string());
                format!("\x00CB{}\x00", code_blocks.len() - 1)
            })
            .to_string();

        // 2. Extract and protect inline code
        let mut inline_codes: Vec<String> = Vec::new();
        result = re
            .inline_code
            .replace_all(&result, |caps: &regex::Captures| {
                inline_codes.push(caps.get(1).unwrap().as_str().to_string());
                format!("\x00IC{}\x00", inline_codes.len() - 1)
            })
            .to_string();

        // 3. Headers # Title -> just the title text
        result = re.header.replace_all(&result, "$1").to_string();

        // 4. Blockquotes > text -> just the text
        result = re.blockquote.replace_all(&result, "$1").to_string();

        // 5. Escape HTML special characters
        result = result
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;");

        // 6. Links [text](url) - before bold/italic to handle nested cases
        result = re
            .link
            .replace_all(&result, r#"<a href="$2">$1</a>"#)
            .to_string();

        // 7. Bold **text** or __text__
        result = re.bold_star.replace_all(&result, "<b>$1</b>").to_string();
        result = re
            .bold_underscore
            .replace_all(&result, "<b>$1</b>")
            .to_string();

        // 8. Italic _text_ (avoid matching inside words like some_var_name)
        result = re
            .italic
            .replace_all(&result, "${pre}<i>$2</i>${post}")
            .to_string();

        // 9. Strikethrough ~~text~~
        result = re
            .strikethrough
            .replace_all(&result, "<s>$1</s>")
            .to_string();

        // 10. Bullet lists - item -> • item
        result = re.bullet.replace_all(&result, "• ").to_string();

        // 11. Restore inline code with HTML tags
        for (i, code) in inline_codes.iter().enumerate() {
            let escaped = code
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            result = result.replace(
                &format!("\x00IC{}\x00", i),
                &format!("<code>{}</code>", escaped),
            );
        }

        // 12. Restore code blocks with HTML tags
        for (i, code) in code_blocks.iter().enumerate() {
            let escaped = code
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;");
            result = result.replace(
                &format!("\x00CB{}\x00", i),
                &format!("<pre><code>{}</code></pre>", escaped),
            );
        }

        result
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

        // Stop all typing indicators
        let mut typing_tasks = self.typing_tasks.write().await;
        for (chat_id, task) in typing_tasks.drain() {
            debug!("Stopping typing indicator for chat {}", chat_id);
            task.abort();
        }

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

        let html_content = Self::markdown_to_html(&msg.content);

        // Split message if it exceeds Telegram's 4096 character limit
        let chunks = Self::split_message(&html_content);

        for (i, chunk) in chunks.iter().enumerate() {
            let params = json!({
                "chat_id": chat_id,
                "text": chunk,
                "parse_mode": "HTML"
            });

            match self.api_call("sendMessage", params).await {
                Ok(_) => {}
                Err(_) => {
                    // Fallback to plain text for this chunk
                    warn!(
                        "HTML parsing failed for chunk {}, falling back to plain text",
                        i
                    );
                    let plain_chunk = if chunks.len() > 1 {
                        // For split messages, also split the original content
                        let plain_chunks = Self::split_message(&msg.content);
                        plain_chunks.get(i).unwrap_or(&msg.content).to_string()
                    } else {
                        msg.content.clone()
                    };

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

    #[test]
    fn test_markdown_to_html_code_blocks() {
        let input =
            "Here's some code:\n```rust\nfn main() {\n    let v: Vec<i32> = vec![];\n}\n```\nDone!";
        let output = TelegramChannel::markdown_to_html(input);
        assert!(output.contains("<pre><code>"));
        assert!(output.contains("fn main()"));
        assert!(output.contains("&lt;i32&gt;")); // < and > should be escaped in code
    }

    #[test]
    fn test_markdown_to_html_inline_code() {
        let input = "Use `println!` to print.";
        let output = TelegramChannel::markdown_to_html(input);
        assert!(output.contains("<code>println!</code>"));
    }

    #[test]
    fn test_markdown_to_html_bold_italic() {
        let input = "This is **bold** and this is _italic_ and **_both_**.";
        let output = TelegramChannel::markdown_to_html(input);
        assert!(output.contains("<b>bold</b>"));
        assert!(output.contains("<i>italic</i>"));
    }

    #[test]
    fn test_markdown_to_html_links() {
        let input = "Check out [Rust](https://rust-lang.org).";
        let output = TelegramChannel::markdown_to_html(input);
        assert!(output.contains(r#"<a href="https://rust-lang.org">Rust</a>"#));
    }

    #[test]
    fn test_markdown_to_html_strikethrough() {
        let input = "This is ~~wrong~~ correct.";
        let output = TelegramChannel::markdown_to_html(input);
        assert!(output.contains("<s>wrong</s>"));
    }

    #[test]
    fn test_markdown_to_html_bullets() {
        let input = "List:\n- Item 1\n- Item 2";
        let output = TelegramChannel::markdown_to_html(input);
        assert!(output.contains("• Item 1"));
        assert!(output.contains("• Item 2"));
    }

    #[test]
    fn test_message_split_short() {
        let input = "Short message";
        let chunks = TelegramChannel::split_message(input);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Short message");
    }

    #[test]
    fn test_message_split_long() {
        // Create a message longer than 4096 characters
        let long_line = "a".repeat(5000);
        let chunks = TelegramChannel::split_message(&long_line);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.len() <= 4096);
        }
    }

    #[test]
    fn test_message_split_multiline() {
        let mut lines = Vec::new();
        for i in 0..200 {
            lines.push(format!("Line {} with some content", i));
        }
        let input = lines.join("\n");
        let chunks = TelegramChannel::split_message(&input);

        // Should be split into multiple chunks
        if input.len() > 4096 {
            assert!(chunks.len() > 1);
        }

        // Each chunk should be under the limit
        for chunk in &chunks {
            assert!(chunk.len() <= 4096);
        }
    }
}
