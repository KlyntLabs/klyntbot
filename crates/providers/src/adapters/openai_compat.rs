//! OpenAI-compatible HTTP client for LLM providers.
//!
//! Replaces Python's LiteLLM with direct reqwest HTTP calls to provider APIs
//! using OpenAI chat completion format.

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, warn};

use common::{build_http_client, ProviderError, Result};

use crate::registry::ProviderRegistry;
use crate::types::{
    ChatParams, LlmProvider, LlmResponse, LlmStream, LlmStreamChunk, Message, ProviderCapabilities,
    ProviderHealth, ResponseFormat, ToolCall, ToolCallDelta, ToolCallMessage, Usage,
    DEFAULT_CONTEXT_WINDOW,
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
        let client = build_http_client(Duration::from_secs(120))?;

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

    /// Look up the context window size for common models.
    ///
    /// Uses prefix matching against known model families. Returns
    /// `DEFAULT_CONTEXT_WINDOW` (128K) for unrecognized models.
    fn model_context_window(model: &str) -> usize {
        let model = model.to_lowercase();

        // o-series reasoning models
        if model.starts_with("o1-") {
            return 128_000;
        }
        if model.starts_with("o1") {
            return 200_000;
        }
        if model.starts_with("o3-mini") {
            return 200_000;
        }
        if model.starts_with("o3") || model.starts_with("o4") {
            return 200_000;
        }

        // GPT-4o family
        if model.starts_with("gpt-4o") || model.starts_with("chatgpt-4o") {
            return 128_000;
        }

        // GPT-4 turbo variants (must check before generic gpt-4)
        if model.starts_with("gpt-4-turbo")
            || model.starts_with("gpt-4-1106")
            || model.starts_with("gpt-4-0125")
        {
            return 128_000;
        }

        // GPT-4 32K
        if model.starts_with("gpt-4-32k") {
            return 32_768;
        }

        // GPT-4 base (8K)
        if model.starts_with("gpt-4") {
            return 8_192;
        }

        // GPT-3.5 turbo
        if model.starts_with("gpt-3.5-turbo") {
            return 16_385;
        }

        DEFAULT_CONTEXT_WINDOW
    }

    /// Serialize a `ResponseFormat` to the OpenAI `response_format` JSON field.
    fn serialize_response_format(format: &ResponseFormat) -> Value {
        match format {
            ResponseFormat::Text => json!({"type": "text"}),
            ResponseFormat::JsonObject => json!({"type": "json_object"}),
            ResponseFormat::JsonSchema { name, schema } => json!({
                "type": "json_schema",
                "json_schema": {
                    "name": name,
                    "schema": schema,
                    "strict": true,
                }
            }),
        }
    }

    /// Parse OpenAI API response into our LlmResponse
    fn parse_response(&self, response: ChatCompletionResponse) -> Result<LlmResponse> {
        let choice = response
            .choices
            .first()
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".to_string()))?;

        let message = &choice.message;
        let call_count = message.tool_calls.as_ref().map_or(0, |c| c.len());
        let mut tool_calls = Vec::with_capacity(call_count);

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

    /// Build the common request body shared by `chat()` and `chat_stream()`.
    fn build_request_body(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        stream: bool,
    ) -> Value {
        let overrides = ProviderRegistry::get_model_overrides(&params.model);

        let mut body = json!({
            "model": params.model,
            "messages": messages,
        });

        if stream {
            body["stream"] = json!(true);
        }

        if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        } else if let Some(temp) = overrides.get("temperature").and_then(|v| v.as_f64()) {
            body["temperature"] = json!(temp);
        }
        if let Some(max_tokens) = params.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        } else if let Some(mt) = overrides.get("max_tokens").and_then(|v| v.as_u64()) {
            body["max_tokens"] = json!(mt);
        }

        if let Some(tools) = tools {
            if !tools.is_empty() {
                body["tools"] = json!(tools);
                body["tool_choice"] = json!("auto");
            }
        }

        if let Some(ref format) = params.response_format {
            if !matches!(format, ResponseFormat::Text) {
                body["response_format"] = Self::serialize_response_format(format);
            }
        }

        body
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
        let body = self.build_request_body(messages, tools, params, false);

        debug!(
            "Calling LLM: model={}, messages={}",
            params.model,
            messages.len()
        );

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

            return Err(crate::types::map_http_error(
                status.as_u16(),
                error_text,
                self.name(),
            ));
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
        let body = self.build_request_body(messages, tools, params, true);

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

            return Err(crate::types::map_http_error(
                status.as_u16(),
                error_text,
                self.name(),
            ));
        }

        // OpenAI SSE has no event: lines — ignore the event_type parameter
        Ok(crate::streaming::sse_chunk_stream(
            response,
            |_event_type, data| Self::parse_sse_chunk(data),
        ))
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

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            structured_outputs: true,
            ..ProviderCapabilities::default()
        }
    }

    fn context_window(&self) -> usize {
        Self::model_context_window(&self.default_model)
    }

    async fn health_check(&self) -> Result<ProviderHealth> {
        let url = format!("{}/models", self.api_base);
        let health_client = build_http_client(Duration::from_secs(5))?;

        let mut request = health_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key));

        for (key, value) in &self.extra_headers {
            request = request.header(key, value);
        }

        match request.send().await {
            Ok(response) if response.status().is_success() => Ok(ProviderHealth::Healthy),
            Ok(response) => Ok(ProviderHealth::Unhealthy(format!(
                "HTTP {}",
                response.status()
            ))),
            Err(e) if e.is_timeout() => Ok(ProviderHealth::Degraded(
                "Health check timed out (5s)".to_string(),
            )),
            Err(e) => Ok(ProviderHealth::Unhealthy(e.to_string())),
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_provider(model: &str) -> OpenAiCompatProvider {
        OpenAiCompatProvider::new("https://api.openai.com/v1", "test-key", model).unwrap()
    }

    // --- G-22: Context window mapping tests ---

    #[test]
    fn test_context_window_gpt4_base() {
        assert_eq!(OpenAiCompatProvider::model_context_window("gpt-4"), 8_192);
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4-0613"),
            8_192
        );
    }

    #[test]
    fn test_context_window_gpt4_32k() {
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4-32k"),
            32_768
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4-32k-0613"),
            32_768
        );
    }

    #[test]
    fn test_context_window_gpt4_turbo() {
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4-turbo"),
            128_000
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4-turbo-preview"),
            128_000
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4-1106-preview"),
            128_000
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4-0125-preview"),
            128_000
        );
    }

    #[test]
    fn test_context_window_gpt4o() {
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4o"),
            128_000
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4o-mini"),
            128_000
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-4o-2024-05-13"),
            128_000
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("chatgpt-4o-latest"),
            128_000
        );
    }

    #[test]
    fn test_context_window_gpt35() {
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-3.5-turbo"),
            16_385
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("gpt-3.5-turbo-0125"),
            16_385
        );
    }

    #[test]
    fn test_context_window_o_series() {
        assert_eq!(OpenAiCompatProvider::model_context_window("o1"), 200_000);
        assert_eq!(
            OpenAiCompatProvider::model_context_window("o1-mini"),
            128_000
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("o1-preview"),
            128_000
        );
        assert_eq!(
            OpenAiCompatProvider::model_context_window("o3-mini"),
            200_000
        );
        assert_eq!(OpenAiCompatProvider::model_context_window("o3"), 200_000);
    }

    #[test]
    fn test_context_window_unknown_model_defaults() {
        assert_eq!(
            OpenAiCompatProvider::model_context_window("some-custom-model"),
            DEFAULT_CONTEXT_WINDOW
        );
    }

    #[test]
    fn test_context_window_case_insensitive() {
        assert_eq!(OpenAiCompatProvider::model_context_window("GPT-4"), 8_192);
        assert_eq!(
            OpenAiCompatProvider::model_context_window("GPT-4O-Mini"),
            128_000
        );
    }

    #[test]
    fn test_context_window_on_provider_instance() {
        let provider = test_provider("gpt-4");
        assert_eq!(provider.context_window(), 8_192);

        let provider = test_provider("gpt-4o");
        assert_eq!(provider.context_window(), 128_000);

        let provider = test_provider("unknown-model");
        assert_eq!(provider.context_window(), DEFAULT_CONTEXT_WINDOW);
    }

    // --- G-24: Structured output tests ---

    #[test]
    fn test_capabilities_reports_structured_outputs() {
        let provider = test_provider("gpt-4o");
        let caps = provider.capabilities();
        assert!(caps.structured_outputs);
        assert!(caps.streaming);
        assert!(caps.vision);
    }

    #[test]
    fn test_serialize_response_format_text() {
        let result = OpenAiCompatProvider::serialize_response_format(&ResponseFormat::Text);
        assert_eq!(result, json!({"type": "text"}));
    }

    #[test]
    fn test_serialize_response_format_json_object() {
        let result = OpenAiCompatProvider::serialize_response_format(&ResponseFormat::JsonObject);
        assert_eq!(result, json!({"type": "json_object"}));
    }

    #[test]
    fn test_serialize_response_format_json_schema() {
        let schema = json!({
            "type": "object",
            "properties": {"name": {"type": "string"}},
            "required": ["name"]
        });
        let result = OpenAiCompatProvider::serialize_response_format(&ResponseFormat::JsonSchema {
            name: "person".to_string(),
            schema: schema.clone(),
        });
        assert_eq!(result["type"], "json_schema");
        assert_eq!(result["json_schema"]["name"], "person");
        assert_eq!(result["json_schema"]["schema"], schema);
        assert_eq!(result["json_schema"]["strict"], true);
    }

    #[test]
    fn test_parse_sse_done_marker() {
        let result = OpenAiCompatProvider::parse_sse_chunk("[DONE]").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_provider_name() {
        let provider = test_provider("gpt-4o");
        assert_eq!(provider.name(), "openai-compat");
        assert_eq!(provider.default_model(), "gpt-4o");
    }

    // --- G-47: Model override resolution tests ---

    #[test]
    fn test_model_overrides_returned_for_kimi() {
        use crate::registry::ProviderRegistry;
        let overrides = ProviderRegistry::get_model_overrides("kimi-k2.5");
        assert!(overrides.contains_key("temperature"));
        assert_eq!(overrides["temperature"].as_f64().unwrap(), 1.0);
    }

    #[test]
    fn test_model_overrides_empty_for_standard_models() {
        use crate::registry::ProviderRegistry;
        let overrides = ProviderRegistry::get_model_overrides("gpt-4o");
        assert!(overrides.is_empty());
    }
}
