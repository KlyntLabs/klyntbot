//! Anthropic native API provider.
//!
//! Uses Anthropic's Messages API directly (not OpenAI-compat) to access
//! native features: prompt caching, token counting, extended thinking.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, warn};

use common::{KlyntbotError, ProviderError, Result};
use config::Secret;

use crate::types::{
    ChatParams, LlmProvider, LlmResponse, Message, ProviderCapabilities, ToolCall, Usage,
};

const ANTHROPIC_CONTEXT_WINDOW: usize = 200_000;
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic native API provider with prompt caching and token counting.
pub struct AnthropicNativeProvider {
    client: Client,
    api_key: Secret<String>,
    base_url: String,
    model: String,
}

impl AnthropicNativeProvider {
    /// Create a new Anthropic native provider.
    pub fn new(api_key: Secret<String>, base_url: String, model: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            api_key,
            base_url,
            model,
        }
    }

    /// Extract system prompt from messages (first System message, if any).
    fn extract_system_prompt(messages: &[Message]) -> Option<String> {
        messages.iter().find_map(|m| match m {
            Message::System { content } => Some(content.clone()),
            _ => None,
        })
    }

    /// Convert internal Message types to Anthropic API message format.
    ///
    /// System messages are handled separately via `extract_system_prompt`.
    /// Tool result messages are wrapped as `tool_result` content blocks inside
    /// a `user` role message (Anthropic requires tool results in user turns).
    pub fn convert_messages(&self, messages: &[Message]) -> Vec<Value> {
        let mut result = Vec::new();

        for msg in messages {
            match msg {
                Message::System { .. } => {
                    // Handled separately as the top-level "system" field
                }
                Message::User { content } => {
                    let content_blocks = match content {
                        crate::types::UserContent::Text(text) => {
                            json!([{"type": "text", "text": text}])
                        }
                        crate::types::UserContent::MultiPart(parts) => {
                            let blocks: Vec<Value> = parts
                                .iter()
                                .map(|part| match part {
                                    crate::types::ContentPart::Text { text } => {
                                        json!({"type": "text", "text": text})
                                    }
                                    crate::types::ContentPart::ImageUrl { image_url } => {
                                        json!({
                                            "type": "image",
                                            "source": {
                                                "type": "url",
                                                "url": image_url.url,
                                            }
                                        })
                                    }
                                })
                                .collect();
                            Value::Array(blocks)
                        }
                    };
                    result.push(json!({"role": "user", "content": content_blocks}));
                }
                Message::Assistant {
                    content,
                    tool_calls,
                    ..
                } => {
                    let mut blocks = Vec::new();
                    if let Some(text) = content {
                        if !text.is_empty() {
                            blocks.push(json!({"type": "text", "text": text}));
                        }
                    }
                    if let Some(calls) = tool_calls {
                        for call in calls {
                            // ToolCallMessage: call.function.name, call.function.arguments (String)
                            let input: Value =
                                serde_json::from_str(&call.function.arguments).unwrap_or(json!({}));
                            blocks.push(json!({
                                "type": "tool_use",
                                "id": call.id,
                                "name": call.function.name,
                                "input": input,
                            }));
                        }
                    }
                    if !blocks.is_empty() {
                        result.push(json!({"role": "assistant", "content": blocks}));
                    }
                }
                Message::Tool {
                    tool_call_id,
                    content,
                    ..
                } => {
                    // Anthropic expects tool results as user messages with tool_result content blocks
                    result.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": content,
                        }]
                    }));
                }
            }
        }

        result
    }

    /// Convert OpenAI-format tool schemas to Anthropic format.
    ///
    /// OpenAI: `{ "type": "function", "function": { "name", "description", "parameters" } }`
    /// Anthropic: `{ "name", "description", "input_schema" }`
    pub fn convert_tools(&self, openai_tools: &[Value]) -> Vec<Value> {
        openai_tools
            .iter()
            .filter_map(|tool| {
                let func = tool.get("function")?;
                let name = func.get("name")?.as_str()?;
                let description = func.get("description").and_then(|d| d.as_str());
                let parameters = func.get("parameters").cloned().unwrap_or(json!({}));

                let mut anthropic_tool = json!({
                    "name": name,
                    "input_schema": parameters,
                });

                if let Some(desc) = description {
                    anthropic_tool["description"] = json!(desc);
                }

                Some(anthropic_tool)
            })
            .collect()
    }

    /// Parse Anthropic Messages API response into LlmResponse.
    fn parse_response(&self, body: Value) -> Result<LlmResponse> {
        let mut text_content = String::new();
        let mut tool_calls = Vec::new();
        let mut reasoning_content: Option<String> = None;

        if let Some(content_blocks) = body["content"].as_array() {
            for block in content_blocks {
                match block["type"].as_str() {
                    Some("text") => {
                        if let Some(text) = block["text"].as_str() {
                            text_content.push_str(text);
                        }
                    }
                    Some("tool_use") => {
                        let id = block["id"].as_str().unwrap_or("").to_string();
                        let name = block["name"].as_str().unwrap_or("").to_string();
                        let input = block["input"].clone();
                        tool_calls.push(ToolCall {
                            id,
                            name,
                            arguments: input,
                        });
                    }
                    Some("thinking") => {
                        if let Some(text) = block["thinking"].as_str() {
                            reasoning_content = Some(text.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }

        // Map Anthropic stop_reason to OpenAI-style finish_reason
        let finish_reason = match body["stop_reason"].as_str() {
            Some("end_turn") | None => "stop".to_string(),
            Some("tool_use") => "tool_calls".to_string(),
            Some("max_tokens") => "length".to_string(),
            Some(other) => other.to_string(),
        };

        // Parse usage
        let usage = if let Some(usage_obj) = body.get("usage") {
            Usage {
                prompt_tokens: usage_obj["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: usage_obj["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: (usage_obj["input_tokens"].as_u64().unwrap_or(0)
                    + usage_obj["output_tokens"].as_u64().unwrap_or(0))
                    as u32,
                cache_read_tokens: usage_obj["cache_read_input_tokens"].as_u64().unwrap_or(0)
                    as u32,
                cache_write_tokens: usage_obj["cache_creation_input_tokens"]
                    .as_u64()
                    .unwrap_or(0) as u32,
            }
        } else {
            Usage::default()
        };

        let content = if text_content.is_empty() {
            None
        } else {
            Some(text_content)
        };

        Ok(LlmResponse {
            content,
            tool_calls,
            finish_reason,
            usage,
            reasoning_content,
        })
    }
}

#[async_trait]
impl LlmProvider for AnthropicNativeProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmResponse> {
        let url = format!("{}/v1/messages", self.base_url);

        // Build request body
        let mut body = json!({
            "model": params.model,
            "messages": self.convert_messages(messages),
            "max_tokens": params.max_tokens.unwrap_or(4096),
        });

        // System prompt with cache_control for prompt caching
        if let Some(system_prompt) = Self::extract_system_prompt(messages) {
            body["system"] = json!([{
                "type": "text",
                "text": system_prompt,
                "cache_control": {"type": "ephemeral"}
            }]);
        }

        if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        }

        // Convert and add tools
        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = json!(self.convert_tools(tools));
            }
        }

        debug!(
            "Calling Anthropic native: model={}, messages={}",
            params.model,
            messages.len()
        );

        let response = self
            .client
            .post(&url)
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            return if status.as_u16() == 429 {
                Err(KlyntbotError::Provider(ProviderError::RateLimited))
            } else if status.as_u16() == 401 || status.as_u16() == 403 {
                Err(KlyntbotError::Provider(ProviderError::AuthFailed))
            } else {
                Err(KlyntbotError::Provider(ProviderError::InvalidResponse(
                    format!("HTTP {}: {}", status, error_text),
                )))
            };
        }

        let response_body: Value = response.json().await.map_err(|e| {
            ProviderError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        self.parse_response(response_body)
    }

    async fn count_tokens(&self, messages: &[Message], tools: Option<&[Value]>) -> Result<usize> {
        let url = format!("{}/v1/messages/count_tokens", self.base_url);

        let mut body = json!({
            "model": self.model,
            "messages": self.convert_messages(messages),
        });

        if let Some(system_prompt) = Self::extract_system_prompt(messages) {
            body["system"] = json!(system_prompt);
        }

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = json!(self.convert_tools(tools));
            }
        }

        let response = self
            .client
            .post(&url)
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            warn!(
                "Token counting failed (HTTP {}), falling back to estimation: {}",
                status, error_text
            );
            // Fall back to character-based estimation
            let json = serde_json::to_string(&(messages, tools)).unwrap_or_default();
            return Ok(json.len() / 4);
        }

        let body: Value = response.json().await.map_err(|e| {
            ProviderError::InvalidResponse(format!("Failed to parse token count response: {}", e))
        })?;

        Ok(body["input_tokens"].as_u64().unwrap_or(0) as usize)
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            extended_thinking: true,
            structured_outputs: false,
            prompt_caching: true,
            native_token_counting: true,
            vision: true,
            streaming: true,
            tool_choice_required: true,
            parallel_tool_calls: true,
        }
    }

    fn context_window(&self) -> usize {
        ANTHROPIC_CONTEXT_WINDOW
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn default_model(&self) -> &str {
        &self.model
    }

    fn name(&self) -> &str {
        "anthropic-native"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FunctionCall, ToolCallMessage};

    fn test_provider() -> AnthropicNativeProvider {
        AnthropicNativeProvider::new(
            Secret::new("test-key".to_string()),
            "https://api.anthropic.com".to_string(),
            "claude-sonnet-4-20250514".to_string(),
        )
    }

    #[test]
    fn test_convert_messages_to_anthropic_format() {
        let provider = test_provider();
        let messages = vec![Message::user("Hello"), Message::assistant("Hi there")];
        let result = provider.convert_messages(&messages);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"][0]["type"], "text");
        assert_eq!(result[0]["content"][0]["text"], "Hello");
        assert_eq!(result[1]["role"], "assistant");
        assert_eq!(result[1]["content"][0]["text"], "Hi there");
    }

    #[test]
    fn test_convert_messages_skips_system() {
        let provider = test_provider();
        let messages = vec![
            Message::system("You are a helpful assistant"),
            Message::user("Hello"),
        ];
        let result = provider.convert_messages(&messages);
        // System messages should be filtered out
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
    }

    #[test]
    fn test_extract_system_prompt() {
        let messages = vec![Message::system("You are helpful"), Message::user("Hi")];
        let system = AnthropicNativeProvider::extract_system_prompt(&messages);
        assert_eq!(system, Some("You are helpful".to_string()));
    }

    #[test]
    fn test_extract_system_prompt_none() {
        let messages = vec![Message::user("Hi")];
        let system = AnthropicNativeProvider::extract_system_prompt(&messages);
        assert!(system.is_none());
    }

    #[test]
    fn test_convert_messages_with_tool_calls() {
        let provider = test_provider();
        let messages = vec![Message::Assistant {
            content: None,
            tool_calls: Some(vec![ToolCallMessage {
                id: "call_1".to_string(),
                r#type: "function".to_string(),
                function: FunctionCall {
                    name: "read_file".to_string(),
                    arguments: r#"{"path":"/tmp/test.txt"}"#.to_string(),
                },
            }]),
            reasoning_content: None,
        }];
        let result = provider.convert_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        assert_eq!(result[0]["content"][0]["type"], "tool_use");
        assert_eq!(result[0]["content"][0]["name"], "read_file");
        assert_eq!(result[0]["content"][0]["id"], "call_1");
        assert_eq!(result[0]["content"][0]["input"]["path"], "/tmp/test.txt");
    }

    #[test]
    fn test_convert_messages_tool_result() {
        let provider = test_provider();
        let messages = vec![Message::tool("call_1", "read_file", "file contents here")];
        let result = provider.convert_messages(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"][0]["type"], "tool_result");
        assert_eq!(result[0]["content"][0]["tool_use_id"], "call_1");
        assert_eq!(result[0]["content"][0]["content"], "file contents here");
    }

    #[test]
    fn test_convert_tool_schema_to_anthropic_format() {
        let provider = test_provider();
        let openai_tools = vec![json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a file",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"]
                }
            }
        })];
        let result = provider.convert_tools(&openai_tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "read_file");
        assert_eq!(result[0]["description"], "Read a file");
        assert_eq!(result[0]["input_schema"]["type"], "object");
        assert_eq!(
            result[0]["input_schema"]["properties"]["path"]["type"],
            "string"
        );
    }

    #[test]
    fn test_parse_response_text() {
        let provider = test_provider();
        let body = json!({
            "content": [{"type": "text", "text": "Hello!"}],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 100,
                "output_tokens": 50,
                "cache_read_input_tokens": 20,
                "cache_creation_input_tokens": 10,
            }
        });
        let response = provider.parse_response(body).unwrap();
        assert_eq!(response.content, Some("Hello!".to_string()));
        assert!(response.tool_calls.is_empty());
        assert_eq!(response.finish_reason, "stop");
        assert_eq!(response.usage.prompt_tokens, 100);
        assert_eq!(response.usage.completion_tokens, 50);
        assert_eq!(response.usage.total_tokens, 150);
        assert_eq!(response.usage.cache_read_tokens, 20);
        assert_eq!(response.usage.cache_write_tokens, 10);
    }

    #[test]
    fn test_parse_response_tool_use() {
        let provider = test_provider();
        let body = json!({
            "content": [{
                "type": "tool_use",
                "id": "toolu_01",
                "name": "read_file",
                "input": {"path": "/tmp/test.txt"}
            }],
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 50, "output_tokens": 30}
        });
        let response = provider.parse_response(body).unwrap();
        assert!(response.content.is_none());
        assert_eq!(response.tool_calls.len(), 1);
        assert_eq!(response.tool_calls[0].name, "read_file");
        assert_eq!(response.tool_calls[0].id, "toolu_01");
        assert_eq!(response.tool_calls[0].arguments["path"], "/tmp/test.txt");
        assert_eq!(response.finish_reason, "tool_calls");
    }

    #[test]
    fn test_parse_response_missing_usage() {
        let provider = test_provider();
        let body = json!({
            "content": [{"type": "text", "text": "Hi"}],
            "stop_reason": "end_turn",
        });
        let response = provider.parse_response(body).unwrap();
        assert_eq!(response.usage.prompt_tokens, 0);
        assert_eq!(response.usage.total_tokens, 0);
    }

    #[test]
    fn test_parse_response_max_tokens_stop() {
        let provider = test_provider();
        let body = json!({
            "content": [{"type": "text", "text": "truncated..."}],
            "stop_reason": "max_tokens",
            "usage": {"input_tokens": 10, "output_tokens": 4096}
        });
        let response = provider.parse_response(body).unwrap();
        assert_eq!(response.finish_reason, "length");
    }

    #[test]
    fn test_capabilities_returns_native_features() {
        let provider = test_provider();
        let caps = provider.capabilities();
        assert!(caps.extended_thinking);
        assert!(caps.prompt_caching);
        assert!(caps.native_token_counting);
        assert!(caps.vision);
        assert!(caps.streaming);
        assert!(caps.parallel_tool_calls);
        assert!(!caps.structured_outputs);
    }

    #[test]
    fn test_context_window_200k() {
        let provider = test_provider();
        assert_eq!(provider.context_window(), 200_000);
    }

    #[test]
    fn test_provider_name_and_model() {
        let provider = test_provider();
        assert_eq!(provider.name(), "anthropic-native");
        assert_eq!(provider.default_model(), "claude-sonnet-4-20250514");
    }
}
