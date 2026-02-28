//! Message tool for cross-channel communication.

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::debug;

use bus::OutboundMessage;
use common::{Result, ToolError};
use tools_core::{RoutingContext, ToolParams};

#[derive(Debug, ToolParams)]
pub struct MessageParams {
    /// The message content to send
    #[param(required)]
    pub content: String,

    /// Optional: target channel (telegram, discord, etc.)
    pub channel: Option<String>,

    /// Optional: target chat/user ID
    pub chat_id: Option<String>,
}

/// Tool to send messages to channels
#[derive(tools_core::Tool)]
#[tool(
    name = "message",
    description = "Send a message to a specific channel (Telegram, Discord, etc.). In CLI/dashboard sessions, respond with text directly instead of calling this tool — your text is streamed to the user automatically.",
    params = "MessageParams"
)]
pub struct MessageTool {
    outbound_tx: mpsc::Sender<OutboundMessage>,
}

impl MessageTool {
    pub fn new(outbound_tx: mpsc::Sender<OutboundMessage>) -> Self {
        Self { outbound_tx }
    }
}

#[async_trait]
impl tools_core::ToolExecute for MessageTool {
    type Params = MessageParams;

    async fn execute(&self, params: MessageParams, ctx: &RoutingContext) -> Result<String> {
        let content = &params.content;

        // In direct mode (CLI/dashboard), the user receives responses via the
        // event stream. Skip the bus and return the content inline so it becomes
        // part of the LLM's final response context.
        if ctx.is_direct_mode {
            debug!("Direct mode: returning message content inline");
            return Ok(content.to_string());
        }

        // Use provided context or optional overrides from args
        let channel = params
            .channel
            .unwrap_or_else(|| ctx.channel.as_str().to_string());

        let chat_id = params
            .chat_id
            .unwrap_or_else(|| ctx.chat_id.as_str().to_string());

        debug!("Sending message to {}:{}", channel, chat_id);

        let msg = OutboundMessage::new(channel, chat_id, content.to_string());

        self.outbound_tx
            .send(msg)
            .await
            .map_err(|_| ToolError::ExecutionFailed("Failed to send message to bus".to_string()))?;

        Ok("Message sent successfully".to_string())
    }
}
