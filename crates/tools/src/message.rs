//! Message tool for cross-channel communication.

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::mpsc;
use tracing::debug;

use super::{RoutingContext, Tool};
use bus::OutboundMessage;
use common::{Result, ToolError};

/// Tool to send messages to channels
pub struct MessageTool {
    outbound_tx: mpsc::Sender<OutboundMessage>,
}

impl MessageTool {
    pub fn new(outbound_tx: mpsc::Sender<OutboundMessage>) -> Self {
        Self { outbound_tx }
    }
}

#[async_trait]
impl Tool for MessageTool {
    fn name(&self) -> &str {
        "message"
    }

    fn description(&self) -> &str {
        "Send a message to the user. Use this when you want to communicate something."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The message content to send"
                },
                "channel": {
                    "type": "string",
                    "description": "Optional: target channel (telegram, discord, etc.)"
                },
                "chat_id": {
                    "type": "string",
                    "description": "Optional: target chat/user ID"
                }
            },
            "required": ["content"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> Result<String> {
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidParams("missing 'content' parameter".to_string()))?;

        // Use provided context or optional overrides from args
        let channel = args
            .get("channel")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| ctx.channel.as_str().to_string());

        let chat_id = args
            .get("chat_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
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
