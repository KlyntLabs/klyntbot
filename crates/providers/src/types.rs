//! Core types for LLM providers.

use async_trait::async_trait;
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use std::sync::Arc;

use common::{KlyntbotError, MessageRole, ProviderError, Result};

/// Map an HTTP status code to the appropriate provider error.
///
/// Centralizes the status → error mapping that was previously duplicated
/// across openai_compat, anthropic_native, and transcription modules.
///
/// `provider_name` is used in error messages to identify which provider failed.
pub(crate) fn map_http_error(status_code: u16, body: String, provider_name: &str) -> KlyntbotError {
    match status_code {
        429 => {
            // Try to extract Retry-After header value from the body (some providers include it)
            let retry_after = extract_retry_after(&body);
            KlyntbotError::Provider(ProviderError::RateLimited {
                provider: provider_name.to_string(),
                retry_after,
            })
        }
        401 | 403 => KlyntbotError::Provider(ProviderError::AuthFailed {
            provider: provider_name.to_string(),
            config_key: format!("providers.{}.apiKey", provider_name),
        }),
        _ => KlyntbotError::Provider(ProviderError::InvalidResponse(format!(
            "HTTP {}: {}",
            status_code, body
        ))),
    }
}

/// Try to extract a retry-after value from the error body (best-effort).
fn extract_retry_after(body: &str) -> Option<u64> {
    // Some providers include retry_after in JSON response
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(secs) = v.get("retry_after").and_then(|v| v.as_u64()) {
            return Some(secs);
        }
        // OpenAI-style: error.retry_after
        if let Some(secs) = v
            .get("error")
            .and_then(|e| e.get("retry_after"))
            .and_then(|v| v.as_u64())
        {
            return Some(secs);
        }
    }
    None
}

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

    /// Token usage (provided by some events, e.g. message_start/message_delta)
    pub usage: Option<Usage>,
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

/// Response format for structured output
#[derive(Debug, Clone)]
pub enum ResponseFormat {
    /// Plain text response (default)
    Text,
    /// JSON object response (model outputs valid JSON)
    JsonObject,
    /// JSON schema response (model outputs JSON conforming to schema)
    JsonSchema { name: String, schema: Value },
}

/// Parameters for chat completion requests
#[derive(Debug, Clone)]
pub struct ChatParams {
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub response_format: Option<ResponseFormat>,
}

impl ChatParams {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            temperature: None,
            max_tokens: None,
            response_format: None,
        }
    }

    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    pub fn with_max_tokens(mut self, tokens: u32) -> Self {
        self.max_tokens = Some(tokens);
        self
    }

    pub fn with_response_format(mut self, format: ResponseFormat) -> Self {
        self.response_format = Some(format);
        self
    }
}

/// Health status of an LLM provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderHealth {
    /// Provider is healthy and responding normally.
    Healthy,
    /// Provider is responding but with degraded performance (e.g., high latency).
    Degraded(String),
    /// Provider is not responding or returning errors.
    Unhealthy(String),
    /// Health status is unknown (no check implemented).
    Unknown,
}

/// Trait for LLM providers
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a chat completion request (non-streaming)
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmResponse>;

    /// Send a streaming chat completion request
    /// Default implementation falls back to non-streaming chat()
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmStream> {
        // Default: call chat() and emit a single chunk
        let response = self.chat(messages, tools, params).await?;

        let chunk = LlmStreamChunk {
            content: response.content,
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(response.finish_reason),
            reasoning_content: response.reasoning_content,
            usage: Some(response.usage),
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

    /// Count tokens for the given messages and tools.
    /// Default: character-based estimation (4 chars ≈ 1 token).
    async fn count_tokens(&self, messages: &[Message], tools: Option<&[Value]>) -> Result<usize> {
        let json = serde_json::to_string(&(messages, tools)).unwrap_or_default();
        Ok(json.len() / 4)
    }

    /// Provider capabilities
    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::default()
    }

    /// Context window size for the current model
    fn context_window(&self) -> usize {
        DEFAULT_CONTEXT_WINDOW
    }

    /// Check provider health. Default returns `Unknown`.
    async fn health_check(&self) -> Result<ProviderHealth> {
        Ok(ProviderHealth::Unknown)
    }

    /// Optional dedicated provider for lightweight classification tasks.
    /// Returns `None` by default (use self for classification).
    fn classifier_provider(&self) -> Option<DynProvider> {
        None
    }
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

/// Provider capability flags for adaptive orchestration
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub extended_thinking: bool,
    pub structured_outputs: bool,
    pub prompt_caching: bool,
    pub native_token_counting: bool,
    pub vision: bool,
    pub streaming: bool,
    pub tool_choice_required: bool,
    pub parallel_tool_calls: bool,
}

impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            extended_thinking: false,
            structured_outputs: false,
            prompt_caching: false,
            native_token_counting: false,
            vision: true,
            streaming: true,
            tool_choice_required: false,
            parallel_tool_calls: true,
        }
    }
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub cache_read_tokens: u32,
    #[serde(default)]
    pub cache_write_tokens: u32,
}

impl Usage {
    /// Get total token count
    pub fn total(&self) -> u32 {
        self.total_tokens
    }
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
    /// A mid-execution context update injected by LiveContextRefresher.
    /// Serialized as system role with XML tags when sent to the LLM.
    #[serde(rename = "context_update")]
    ContextUpdate {
        reason: String,
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

pub const DEFAULT_CONTEXT_WINDOW: usize = 128_000;

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

    /// Create an assistant message with tool calls and optional text content.
    pub fn assistant_with_content_and_tools(
        content: Option<String>,
        tool_calls: Vec<ToolCallMessage>,
    ) -> Self {
        Self::Assistant {
            content,
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

    /// Create a mid-execution context update message.
    pub fn context_update(reason: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ContextUpdate {
            reason: reason.into(),
            content: content.into(),
        }
    }

    /// Get the role of this message
    pub fn role(&self) -> MessageRole {
        match self {
            Message::System { .. } => MessageRole::System,
            Message::User { .. } => MessageRole::User,
            Message::Assistant { .. } => MessageRole::Assistant,
            Message::Tool { .. } => MessageRole::Tool,
            Message::ContextUpdate { .. } => MessageRole::System,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_has_cache_fields() {
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cache_read_tokens: 10,
            cache_write_tokens: 5,
        };
        assert_eq!(usage.total(), 150);
        assert_eq!(usage.cache_read_tokens, 10);
    }

    #[test]
    fn test_context_window_constant() {
        assert_eq!(DEFAULT_CONTEXT_WINDOW, 128_000);
    }

    #[test]
    fn test_chat_params_with_response_format() {
        let params = ChatParams::new("gpt-4o").with_response_format(ResponseFormat::JsonObject);
        assert!(params.response_format.is_some());
        assert!(matches!(
            params.response_format,
            Some(ResponseFormat::JsonObject)
        ));
    }

    #[test]
    fn test_chat_params_default_no_response_format() {
        let params = ChatParams::new("gpt-4o");
        assert!(params.response_format.is_none());
    }

    #[test]
    fn stream_chunk_with_usage() {
        let chunk = LlmStreamChunk {
            content: None,
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some("end_turn".into()),
            reasoning_content: None,
            usage: Some(Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                cache_read_tokens: 20,
                cache_write_tokens: 0,
            }),
        };
        assert_eq!(chunk.usage.unwrap().cache_read_tokens, 20);
    }

    #[test]
    fn test_response_format_json_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "name": { "type": "string" } },
            "required": ["name"]
        });
        let format = ResponseFormat::JsonSchema {
            name: "person".to_string(),
            schema: schema.clone(),
        };
        match format {
            ResponseFormat::JsonSchema { name, schema: s } => {
                assert_eq!(name, "person");
                assert_eq!(s, schema);
            }
            _ => panic!("Expected JsonSchema variant"),
        }
    }

    #[test]
    fn context_update_role_is_system() {
        let msg = Message::context_update("memory_promoted", "User likes coffee");
        assert_eq!(msg.role(), MessageRole::System);
    }

    #[test]
    fn context_update_serde_round_trip() {
        let msg = Message::context_update("memory_promoted", "User likes coffee");
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "context_update");
        assert_eq!(json["reason"], "memory_promoted");
        assert_eq!(json["content"], "User likes coffee");
    }
}
