//! Core types for LLM providers.

use async_trait::async_trait;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;

use crate::error::Result;

/// Streaming chunk from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStreamChunk {
    /// Incremental content (text delta)
    pub content: Option<String>,

    /// Tool call delta (accumulated across chunks)
    pub tool_call_delta: Option<ToolCallDelta>,

    /// True if this is the final chunk
    pub is_final: bool,

    /// Finish reason (only present in final chunk)
    pub finish_reason: Option<String>,

    /// Reasoning content delta (for thinking models)
    pub reasoning_content: Option<String>,
}

/// Tool call delta for streaming
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
}

/// Stream type alias
pub type LlmStream = Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>;

/// Trait for LLM providers
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a chat completion request (non-streaming)
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        model: Option<&str>,
    ) -> Result<LlmResponse>;

    /// Send a streaming chat completion request
    /// Default implementation falls back to non-streaming chat()
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        model: Option<&str>,
    ) -> Result<LlmStream> {
        // Default: call chat() and emit a single chunk
        let response = self.chat(messages, tools, model).await?;

        let chunk = LlmStreamChunk {
            content: response.content,
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(response.finish_reason),
            reasoning_content: response.reasoning_content,
        };

        Ok(Box::pin(futures_util::stream::once(
            async move { Ok(chunk) },
        )))
    }

    /// Check if streaming is supported
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Get the default model for this provider
    fn default_model(&self) -> &str;

    /// Provider name (for logging)
    fn name(&self) -> &str;
}

/// Type alias for dynamic provider
pub type DynProvider = Arc<dyn LlmProvider>;

/// LLM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    /// Response content (text)
    pub content: Option<String>,

    /// Tool calls requested by the LLM
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,

    /// Finish reason
    pub finish_reason: String,

    /// Token usage
    pub usage: Usage,

    /// Reasoning content (for thinking models like DeepSeek-R1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

/// Tool call request from LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Tool call ID
    pub id: String,

    /// Tool name
    pub name: String,

    /// Tool arguments (parsed JSON)
    pub arguments: Value,
}

impl ToolCall {
    /// Convert this tool call to a message format
    pub fn to_message(&self) -> ToolCallMessage {
        ToolCallMessage {
            id: self.id.clone(),
            r#type: "function".to_string(),
            function: FunctionCall {
                name: self.name.clone(),
                arguments: serde_json::to_string(&self.arguments).unwrap_or_default(),
            },
        }
    }
}

/// Convert a slice of tool calls to messages
pub fn tool_calls_to_messages(tool_calls: &[ToolCall]) -> Vec<ToolCallMessage> {
    tool_calls.iter().map(|tc| tc.to_message()).collect()
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Message {
    System {
        content: String,
    },
    User {
        content: UserContent,
    },
    Assistant {
        #[serde(skip_serializing_if = "Option::is_none")]
        content: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tool_calls: Option<Vec<ToolCallMessage>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reasoning_content: Option<String>,
    },
    Tool {
        tool_call_id: String,
        name: String,
        content: String,
    },
}

/// Tool call in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallMessage {
    pub id: String,
    pub r#type: String,
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

/// User message content (text or multipart)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UserContent {
    Text(String),
    MultiPart(Vec<ContentPart>),
}

/// Content part for multipart messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

/// Image URL for vision models
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

impl Message {
    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::System {
            content: content.into(),
        }
    }

    /// Create a user message with text
    pub fn user(content: impl Into<String>) -> Self {
        Self::User {
            content: UserContent::Text(content.into()),
        }
    }

    /// Create a user message with multipart content
    pub fn user_multipart(parts: Vec<ContentPart>) -> Self {
        Self::User {
            content: UserContent::MultiPart(parts),
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::Assistant {
            content: Some(content.into()),
            tool_calls: None,
            reasoning_content: None,
        }
    }

    /// Create an assistant message with tool calls
    pub fn assistant_with_tools(tool_calls: Vec<ToolCallMessage>) -> Self {
        Self::Assistant {
            content: None,
            tool_calls: Some(tool_calls),
            reasoning_content: None,
        }
    }

    /// Create a tool result message
    pub fn tool(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        Self::Tool {
            tool_call_id: tool_call_id.into(),
            name: name.into(),
            content: content.into(),
        }
    }
}
