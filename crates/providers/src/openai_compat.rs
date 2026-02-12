//! OpenAI-compatible HTTP client for LLM providers.
//!
//! Replaces Python's LiteLLM with direct reqwest HTTP calls to provider APIs
//! using OpenAI chat completion format.

use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, warn};

use common::{KlyntbotError, ProviderError, Result};

use crate::types::{
    ChatParams, LlmProvider, LlmResponse, LlmStream, LlmStreamChunk, Message, ToolCall,
    ToolCallDelta, ToolCallMessage, Usage,
};

/// OpenAI-compatible provider using direct HTTP
pub struct OpenAiCompatProvider {
    client: Client,
    api_base: String,
    api_key: String,
    default_model: String,
    extra_headers: Vec<(String, String)>,
}

impl OpenAiCompatProvider {
    /// Create a new OpenAI-compatible provider
    pub fn new(
        api_base: impl Into<String>,
        api_key: impl Into<String>,
        default_model: impl Into<String>,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        Ok(Self {
            client,
            api_base: api_base.into(),
            api_key: api_key.into(),
            default_model: default_model.into(),
            extra_headers: Vec::new(),
        })
    }

    /// Add extra headers (e.g., APP-Code for AiHubMix)
    pub fn with_extra_headers(mut self, headers: Vec<(String, String)>) -> Self {
        self.extra_headers = headers;
        self
    }

    /// Parse OpenAI API response into our LlmResponse
    fn parse_response(&self, response: ChatCompletionResponse) -> Result<LlmResponse> {
        let choice = response
            .choices
            .first()
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".to_string()))?;

        let message = &choice.message;
        let mut tool_calls = Vec::new();

        // Parse tool calls if present
        if let Some(calls) = &message.tool_calls {
            for tc in calls {
                // Parse arguments from JSON string
                let arguments: Value = if tc.function.arguments.trim().is_empty() {
                    json!({})
                } else {
                    serde_json::from_str(&tc.function.arguments).unwrap_or_else(|e| {
                        warn!(
                            "Failed to parse tool arguments: {}, raw: {}",
                            e, tc.function.arguments
                        );
                        json!({"raw": tc.function.arguments})
                    })
                };

                tool_calls.push(ToolCall {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    arguments,
                });
            }
        }

        Ok(LlmResponse {
            content: message.content.clone(),
            tool_calls,
            finish_reason: choice
                .finish_reason
                .clone()
                .unwrap_or_else(|| "stop".to_string()),
            usage: response.usage.unwrap_or_default(),
            reasoning_content: message.reasoning_content.clone(),
        })
    }

    /// Parse SSE event data
    fn parse_sse_chunk(data: &str) -> Result<Option<LlmStreamChunk>> {
        // Handle [DONE] marker
        if data.trim() == "[DONE]" {
            return Ok(None);
        }

        // Parse JSON
        let value: Value = serde_json::from_str(data)
            .map_err(|e| ProviderError::InvalidResponse(format!("Invalid SSE JSON: {}", e)))?;

        let choices = value["choices"]
            .as_array()
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in SSE chunk".to_string()))?;

        if choices.is_empty() {
            return Ok(None);
        }

        let choice = &choices[0];
        let delta = &choice["delta"];

        // Extract content delta
        let content = delta["content"].as_str().map(|s| s.to_string());

        // Extract reasoning content delta
        let reasoning_content = delta["reasoning_content"].as_str().map(|s| s.to_string());

        // Extract tool call delta
        let tool_call_delta = if let Some(tool_calls) = delta["tool_calls"].as_array() {
            if !tool_calls.is_empty() {
                let tc = &tool_calls[0];
                Some(ToolCallDelta {
                    index: tc["index"].as_u64().unwrap_or(0) as usize,
                    id: tc["id"].as_str().map(|s| s.to_string()),
                    name: tc["function"]["name"].as_str().map(|s| s.to_string()),
                    arguments: tc["function"]["arguments"].as_str().map(|s| s.to_string()),
                })
            } else {
                None
            }
        } else {
            None
        };

        // Check if final
        let finish_reason = choice["finish_reason"].as_str().map(|s| s.to_string());
        let is_final = finish_reason.is_some();

        Ok(Some(LlmStreamChunk {
            content,
            tool_call_delta,
            is_final,
            finish_reason,
            reasoning_content,
        }))
    }
}

#[async_trait]
impl LlmProvider for OpenAiCompatProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmResponse> {
        let url = format!("{}/chat/completions", self.api_base);

        // Build request body
        let mut body = json!({
            "model": params.model,
            "messages": messages,
        });

        // Add optional parameters
        if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(max_tokens) = params.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }

        // Add tools if provided
        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = json!(tools);
                body["tool_choice"] = json!("auto");
            }
        }

        debug!("Calling LLM: model={}, messages={}", params.model, messages.len());

        // Build request with authorization header
        let mut request = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        // Add extra headers
        for (key, value) in &self.extra_headers {
            request = request.header(key, value);
        }

        // Send request
        let response = request
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        // Check status
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

        // Parse response
        let chat_response: ChatCompletionResponse = response.json().await.map_err(|e| {
            ProviderError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        self.parse_response(chat_response)
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmStream> {
        let url = format!("{}/chat/completions", self.api_base);

        // Build request body with stream: true
        let mut body = json!({
            "model": params.model,
            "messages": messages,
            "stream": true,
        });

        // Add optional parameters
        if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(max_tokens) = params.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = json!(tools);
                body["tool_choice"] = json!("auto");
            }
        }

        debug!(
            "Calling LLM (streaming): model={}, messages={}",
            params.model,
            messages.len()
        );

        // Build request
        let mut request = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        for (key, value) in &self.extra_headers {
            request = request.header(key, value);
        }

        // Send request and get response stream
        let response = request
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

            return Err(KlyntbotError::Provider(ProviderError::InvalidResponse(
                format!("HTTP {}: {}", status, error_text),
            )));
        }

        // Create SSE stream
        let stream = response.bytes_stream();

        // Create a stream that parses SSE chunks
        let chunk_stream = stream.scan(String::new(), |line_buffer, result| {
            // Process bytes synchronously to avoid borrow issues
            let chunks_result = match result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    line_buffer.push_str(&text);

                    // Process complete lines
                    let mut chunks = Vec::new();
                    while let Some(newline_pos) = line_buffer.find('\n') {
                        let line = line_buffer.drain(..=newline_pos).collect::<String>();
                        let line = line.trim();

                        // Parse SSE format: "data: {...}"
                        if let Some(data) = line.strip_prefix("data: ") {
                            match Self::parse_sse_chunk(data) {
                                Ok(Some(chunk)) => chunks.push(Ok(chunk)),
                                Ok(None) => {} // [DONE] marker or empty chunk
                                Err(e) => {
                                    warn!("Failed to parse SSE chunk: {}", e);
                                    // Continue processing other chunks
                                }
                            }
                        }
                    }
                    chunks
                }
                Err(e) => vec![Err(KlyntbotError::Provider(ProviderError::Http(e.to_string())))],
            };

            // Return async block with processed chunks
            async move { Some(futures_util::stream::iter(chunks_result)) }
        });

        Ok(Box::pin(chunk_stream.flatten()))
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn default_model(&self) -> &str {
        &self.default_model
    }

    fn name(&self) -> &str {
        "openai-compat"
    }
}

/// OpenAI API chat completion response
#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

/// Choice in response
#[derive(Debug, Deserialize)]
struct Choice {
    message: ResponseMessage,
    finish_reason: Option<String>,
}

/// Message in response
#[derive(Debug, Deserialize)]
struct ResponseMessage {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallMessage>>,
    reasoning_content: Option<String>,
}
