//! Anthropic native API provider.
//!
//! Uses Anthropic's Messages API directly (not OpenAI-compat) to access
//! native features: prompt caching, token counting, extended thinking.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;
use tracing::{debug, warn};

use common::{build_http_client, ProviderError, Result};
use config::{ExtendedThinkingConfig, Secret};

use crate::registry::ProviderRegistry;
use crate::types::{
    CacheAnchor, CacheBreakpoint, CacheTtl, ChatParams, LlmProvider, LlmResponse, LlmStream,
    LlmStreamChunk, Message, ProviderCapabilities, ProviderHealth, ResponseFormat, ToolCall,
    ToolCallDelta, Usage,
};

const ANTHROPIC_CONTEXT_WINDOW: usize = 200_000;
const DEFAULT_ANTHROPIC_VERSION: &str = "2023-06-01";

/// Resolved cache-marker placement: which content block to mark, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedMarker {
    /// Section of the request payload (lower numbers come earlier).
    /// SECTION_SYSTEM = 0, SECTION_TOOLS = 1, SECTION_MESSAGES = 2.
    section: u8,
    /// Index within that section.
    index: usize,
    /// TTL hint.
    ttl: CacheTtl,
}

const SECTION_SYSTEM: u8 = 0;
const SECTION_TOOLS: u8 = 1;
const SECTION_MESSAGES: u8 = 2;

/// Resolve `CacheBreakpoint` anchors against the actual message vec / tools array.
///
/// Returns a list of `ResolvedMarker` sorted ascending by absolute payload position,
/// with at most 4 entries (Anthropic's per-request limit). When more than 4 markers
/// would be emitted, the EARLIEST ones are dropped — caching at a later position
/// implicitly covers everything before it, so trailing markers dominate.
fn resolve_breakpoints(
    messages: &[Message],
    tools: Option<&[Value]>,
    breakpoints: &[CacheBreakpoint],
) -> Vec<ResolvedMarker> {
    let mut out: Vec<ResolvedMarker> = Vec::with_capacity(breakpoints.len());

    for bp in breakpoints {
        match &bp.anchor {
            CacheAnchor::LastSystem => {
                let last = messages
                    .iter()
                    .enumerate()
                    .rev()
                    .find_map(|(i, m)| matches!(m, Message::System { .. }).then_some(i));
                if let Some(i) = last {
                    out.push(ResolvedMarker {
                        section: SECTION_SYSTEM,
                        index: i,
                        ttl: bp.ttl,
                    });
                }
            }
            CacheAnchor::LastTool => {
                if let Some(t) = tools {
                    if !t.is_empty() {
                        out.push(ResolvedMarker {
                            section: SECTION_TOOLS,
                            index: t.len() - 1,
                            ttl: bp.ttl,
                        });
                    }
                }
            }
            CacheAnchor::MessageIndex(n) => {
                if *n < messages.len() {
                    // Convert original index to wire index by counting system
                    // messages at or before this position (they're stripped from
                    // the wire messages array).
                    let system_offset = messages[..=*n]
                        .iter()
                        .filter(|m| matches!(m, Message::System { .. }))
                        .count();
                    let wire_idx = n.saturating_sub(system_offset);
                    out.push(ResolvedMarker {
                        section: SECTION_MESSAGES,
                        index: wire_idx,
                        ttl: bp.ttl,
                    });
                } else {
                    warn!(
                        target: "klynt::providers::anthropic",
                        index = *n,
                        len = messages.len(),
                        "MessageIndex breakpoint out of range; skipping"
                    );
                }
            }
        }
    }

    // Stable sort by (section, index) ascending.
    out.sort_by_key(|r| (r.section, r.index));

    // Anthropic permits at most 4 cache_control blocks per request.
    // Caching at position N implicitly caches everything before N, so when
    // we have to drop, drop the EARLIEST entries.
    if out.len() > 4 {
        let drop_count = out.len() - 4;
        out.drain(..drop_count);
    }

    out
}

/// True if any resolved marker has Persistent TTL and the request thus
/// needs the `anthropic-beta: extended-cache-ttl-2025-04-11` header.
fn needs_extended_cache_ttl_header(resolved: &[ResolvedMarker]) -> bool {
    resolved
        .iter()
        .any(|r| matches!(r.ttl, CacheTtl::Persistent))
}

/// Anthropic native API provider with prompt caching and token counting.
pub struct AnthropicNativeProvider {
    client: Client,
    api_key: Secret<String>,
    base_url: String,
    model: String,
    api_version: String,
    cache_system_prompt: bool,
    extended_thinking: Option<ExtendedThinkingConfig>,
}

impl AnthropicNativeProvider {
    /// Create a new Anthropic native provider.
    ///
    /// The API version defaults to `"2023-06-01"` but can be overridden via the
    /// `ANTHROPIC_API_VERSION` environment variable.
    pub fn new(api_key: Secret<String>, base_url: String, model: String) -> Self {
        let api_version = std::env::var("ANTHROPIC_API_VERSION")
            .unwrap_or_else(|_| DEFAULT_ANTHROPIC_VERSION.to_string());

        let client =
            build_http_client(Duration::from_secs(120)).expect("failed to build HTTP client");

        Self {
            client,
            api_key,
            base_url,
            model,
            api_version,
            cache_system_prompt: true,
            extended_thinking: None,
        }
    }

    /// Set a custom API version.
    pub fn with_api_version(mut self, version: impl Into<String>) -> Self {
        self.api_version = version.into();
        self
    }

    /// Create a test provider with configurable cache_system_prompt flag.
    pub fn new_for_test(cache_system_prompt: bool) -> Self {
        Self {
            client: Client::new(),
            api_key: Secret::new(String::new()),
            base_url: "https://api.anthropic.com".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_version: DEFAULT_ANTHROPIC_VERSION.to_string(),
            cache_system_prompt,
            extended_thinking: None,
        }
    }

    /// Enable or disable prompt caching for system prompts.
    pub fn with_cache_system_prompt(mut self, enabled: bool) -> Self {
        self.cache_system_prompt = enabled;
        self
    }

    /// Configure extended thinking (chain-of-thought).
    pub fn with_extended_thinking(mut self, config: Option<ExtendedThinkingConfig>) -> Self {
        self.extended_thinking = config;
        self
    }

    /// Extract all system prompts from messages, preserving order.
    fn extract_system_prompts(messages: &[Message]) -> Vec<String> {
        messages
            .iter()
            .filter_map(|m| match m {
                Message::System { content } => Some(content.clone()),
                _ => None,
            })
            .collect()
    }

    /// Convert internal Message types to Anthropic API message format.
    ///
    /// System messages are handled separately via `extract_system_prompts`.
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
                    let blocks = match content {
                        crate::types::ToolContent::Text(text) => {
                            vec![json!({"type": "text", "text": text})]
                        }
                        crate::types::ToolContent::MultiPart(parts) => parts
                            .iter()
                            .map(|p| match p {
                                crate::types::ToolContentPart::Text { text } => {
                                    json!({"type": "text", "text": text})
                                }
                                crate::types::ToolContentPart::ImageData { media_type, data } => {
                                    json!({
                                        "type": "image",
                                        "source": {
                                            "type": "base64",
                                            "media_type": media_type,
                                            "data": data,
                                        }
                                    })
                                }
                            })
                            .collect(),
                    };
                    // Anthropic expects tool results as user messages with tool_result content blocks
                    result.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": blocks,
                        }]
                    }));
                }
                Message::ContextUpdate { reason, content } => {
                    result.push(json!({
                        "role": "user",
                        "content": [{"type": "text", "text": Message::format_context_update_tag(reason, content)}]
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

    /// Parse an Anthropic SSE event into an optional stream chunk.
    ///
    /// Anthropic SSE format uses named events:
    /// - `message_start` — initial message metadata
    /// - `content_block_start` — new content block (text, tool_use, thinking)
    /// - `content_block_delta` — streaming delta (text_delta, input_json_delta, thinking_delta)
    /// - `content_block_stop` — block complete
    /// - `message_delta` — stop reason and output usage
    /// - `message_stop` — final event
    /// - `ping` — keepalive (ignored)
    /// - `error` — error in stream
    fn parse_anthropic_sse(event_type: &str, data: &str) -> Result<Option<LlmStreamChunk>> {
        let value: Value = serde_json::from_str(data)
            .map_err(|e| ProviderError::InvalidResponse(format!("Invalid SSE JSON: {}", e)))?;

        match event_type {
            "content_block_start" => {
                let block = &value["content_block"];
                match block["type"].as_str() {
                    Some("tool_use") => {
                        let index = value["index"].as_u64().unwrap_or(0) as usize;
                        let id = block["id"].as_str().map(|s| s.to_string());
                        let name = block["name"].as_str().map(|s| s.to_string());
                        Ok(Some(LlmStreamChunk {
                            content: None,
                            tool_call_delta: Some(ToolCallDelta {
                                index,
                                id,
                                name,
                                arguments: None,
                            }),
                            is_final: false,
                            finish_reason: None,
                            reasoning_content: None,
                            usage: None,
                        }))
                    }
                    // text and thinking blocks will emit content via deltas
                    _ => Ok(None),
                }
            }
            "content_block_delta" => {
                let delta = &value["delta"];
                match delta["type"].as_str() {
                    Some("text_delta") => {
                        let text = delta["text"].as_str().map(|s| s.to_string());
                        Ok(Some(LlmStreamChunk {
                            content: text,
                            tool_call_delta: None,
                            is_final: false,
                            finish_reason: None,
                            reasoning_content: None,
                            usage: None,
                        }))
                    }
                    Some("input_json_delta") => {
                        let index = value["index"].as_u64().unwrap_or(0) as usize;
                        let partial_json = delta["partial_json"].as_str().map(|s| s.to_string());
                        Ok(Some(LlmStreamChunk {
                            content: None,
                            tool_call_delta: Some(ToolCallDelta {
                                index,
                                id: None,
                                name: None,
                                arguments: partial_json,
                            }),
                            is_final: false,
                            finish_reason: None,
                            reasoning_content: None,
                            usage: None,
                        }))
                    }
                    Some("thinking_delta") => {
                        let thinking = delta["thinking"].as_str().map(|s| s.to_string());
                        Ok(Some(LlmStreamChunk {
                            content: None,
                            tool_call_delta: None,
                            is_final: false,
                            finish_reason: None,
                            reasoning_content: thinking,
                            usage: None,
                        }))
                    }
                    _ => Ok(None),
                }
            }
            "message_start" => {
                let usage_val = &value["message"]["usage"];
                let input = usage_val.get("input_tokens").and_then(|v| v.as_u64());
                if let Some(input_tokens) = input {
                    Ok(Some(LlmStreamChunk {
                        content: None,
                        tool_call_delta: None,
                        is_final: false,
                        finish_reason: None,
                        reasoning_content: None,
                        usage: Some(Usage {
                            prompt_tokens: input_tokens as u32,
                            completion_tokens: 0,
                            total_tokens: input_tokens as u32,
                            cache_read_tokens: usage_val
                                .get("cache_read_input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32,
                            cache_write_tokens: usage_val
                                .get("cache_creation_input_tokens")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0)
                                as u32,
                        }),
                    }))
                } else {
                    Ok(None)
                }
            }
            "message_delta" => {
                // Contains stop_reason and output usage
                let stop_reason = value["delta"]["stop_reason"].as_str();
                let finish_reason = match stop_reason {
                    Some("end_turn") => Some("stop".to_string()),
                    Some("tool_use") => Some("tool_calls".to_string()),
                    Some("max_tokens") => Some("length".to_string()),
                    Some(other) => Some(other.to_string()),
                    None => None,
                };
                let usage = value.get("usage").and_then(|u| {
                    u.get("output_tokens")
                        .and_then(|v| v.as_u64())
                        .map(|output| Usage {
                            prompt_tokens: 0,
                            completion_tokens: output as u32,
                            total_tokens: output as u32,
                            cache_read_tokens: 0,
                            cache_write_tokens: 0,
                        })
                });
                Ok(Some(LlmStreamChunk {
                    content: None,
                    tool_call_delta: None,
                    is_final: true,
                    finish_reason,
                    reasoning_content: None,
                    usage,
                }))
            }
            "error" => {
                let error_msg = value["error"]["message"]
                    .as_str()
                    .unwrap_or("Unknown streaming error");
                Err(ProviderError::InvalidResponse(format!(
                    "Anthropic stream error: {}",
                    error_msg
                ))
                .into())
            }
            // content_block_stop, message_stop, ping — no chunk needed
            _ => Ok(None),
        }
    }

    /// Apply structured output configuration to the request body.
    ///
    /// For `JsonSchema`: injects a synthetic tool with the given schema and
    /// forces `tool_choice` so the model must call it, producing conformant JSON.
    /// For `JsonObject`: adds a system instruction requesting JSON output.
    fn apply_response_format(body: &mut Value, format: &ResponseFormat, tools: &mut Vec<Value>) {
        match format {
            ResponseFormat::Text => {}
            ResponseFormat::JsonObject => {
                // Prepend a JSON instruction to the system prompt
                let instruction = "You must respond with valid JSON only. No markdown, no explanation, just JSON.";
                if let Some(system) = body.get_mut("system") {
                    if let Some(arr) = system.as_array_mut() {
                        arr.insert(0, json!({"type": "text", "text": instruction}));
                    }
                } else {
                    body["system"] = json!([{"type": "text", "text": instruction}]);
                }
            }
            ResponseFormat::JsonSchema { name, schema } => {
                // Add a synthetic tool whose input_schema is the desired JSON schema
                let tool = json!({
                    "name": name,
                    "description": format!("Produce output conforming to the {} schema", name),
                    "input_schema": schema,
                });
                tools.push(tool);
                // Force the model to call this specific tool
                body["tool_choice"] = json!({"type": "tool", "name": name});
                body["tools"] = json!(tools);
            }
        }
    }

    /// Build the common request body shared by `chat()` and `chat_stream()`.
    pub fn build_request_body(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        stream: bool,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Value {
        let overrides = ProviderRegistry::get_model_overrides(&params.model);

        let mut body = json!({
            "model": params.model,
            "messages": self.convert_messages(messages),
            "max_tokens": params.max_tokens.unwrap_or(4096),
        });

        if stream {
            body["stream"] = json!(true);
        }

        // Resolve breakpoints — synthesize a legacy fallback if caller passed
        // no explicit breakpoints AND the legacy flag is on.
        let bps_owned: Vec<CacheBreakpoint>;
        let bps: &[CacheBreakpoint] = if cache_breakpoints.is_empty() && self.cache_system_prompt {
            debug!(
                target: "klynt::providers::anthropic",
                "no explicit cache_breakpoints; synthesizing legacy LastSystem/Ephemeral fallback"
            );
            bps_owned = vec![CacheBreakpoint {
                anchor: CacheAnchor::LastSystem,
                ttl: CacheTtl::Ephemeral,
            }];
            &bps_owned
        } else {
            cache_breakpoints
        };

        let resolved = resolve_breakpoints(messages, tools, bps);

        // Build a quick-lookup: section -> set of (index, ttl)
        fn marker_for(resolved: &[ResolvedMarker], section: u8, index: usize) -> Option<CacheTtl> {
            resolved
                .iter()
                .find(|r| r.section == section && r.index == index)
                .map(|r| r.ttl)
        }

        // System prompt — collect all system messages into content block array.
        let system_prompts = Self::extract_system_prompts(messages);
        if !system_prompts.is_empty() {
            let blocks: Vec<Value> = system_prompts
                .iter()
                .enumerate()
                .map(|(i, text)| {
                    let mut block = json!({"type": "text", "text": text});
                    if let Some(ttl) = marker_for(&resolved, SECTION_SYSTEM, i) {
                        block["cache_control"] = match ttl {
                            CacheTtl::Ephemeral => json!({"type": "ephemeral"}),
                            CacheTtl::Persistent => json!({"type": "ephemeral", "ttl": "1h"}),
                        };
                    }
                    block
                })
                .collect();
            body["system"] = json!(blocks);
        }

        if let Some(temp) = params.temperature {
            body["temperature"] = json!(temp);
        } else if let Some(temp) = overrides.get("temperature").and_then(|v| v.as_f64()) {
            body["temperature"] = json!(temp);
        }

        // Extended thinking — inject thinking block and remove temperature
        if let Some(ref et) = self.extended_thinking {
            if et.enabled {
                body["thinking"] = json!({
                    "type": "enabled",
                    "budget_tokens": et.budget_tokens
                });
                body.as_object_mut().unwrap().remove("temperature");
            }
        }

        // Convert and collect tools (mutable for response_format injection)
        let mut anthropic_tools: Vec<Value> =
            tools.map(|t| self.convert_tools(t)).unwrap_or_default();

        if !anthropic_tools.is_empty() {
            body["tools"] = json!(&anthropic_tools);
        }

        // Apply LastTool cache_control on the converted tools array.
        if let Some(tools_arr) = body.get_mut("tools").and_then(|t| t.as_array_mut()) {
            for (i, tool) in tools_arr.iter_mut().enumerate() {
                if let Some(ttl) = marker_for(&resolved, SECTION_TOOLS, i) {
                    tool["cache_control"] = match ttl {
                        CacheTtl::Ephemeral => json!({"type": "ephemeral"}),
                        CacheTtl::Persistent => json!({"type": "ephemeral", "ttl": "1h"}),
                    };
                }
            }
        }

        // Apply structured output format (may inject synthetic tool + tool_choice)
        if let Some(ref format) = params.response_format {
            Self::apply_response_format(&mut body, format, &mut anthropic_tools);
        }

        // Apply MessageIndex cache_control. Anthropic puts cache_control on the
        // LAST content block of the marked message.
        let converted = body.get_mut("messages").and_then(|m| m.as_array_mut());
        if let Some(msgs_arr) = converted {
            for (i, msg) in msgs_arr.iter_mut().enumerate() {
                if let Some(ttl) = marker_for(&resolved, SECTION_MESSAGES, i) {
                    let content = msg.get_mut("content").cloned();
                    if let Some(content) = content {
                        let mut blocks: Vec<Value> = match content {
                            Value::String(s) => vec![json!({"type": "text", "text": s})],
                            Value::Array(a) => a,
                            other => vec![other],
                        };
                        if let Some(last) = blocks.last_mut() {
                            if let Some(obj) = last.as_object_mut() {
                                let cc = match ttl {
                                    CacheTtl::Ephemeral => json!({"type": "ephemeral"}),
                                    CacheTtl::Persistent => {
                                        json!({"type": "ephemeral", "ttl": "1h"})
                                    }
                                };
                                obj.insert("cache_control".to_string(), cc);
                            }
                        }
                        msg["content"] = json!(blocks);
                    }
                }
            }
        }

        body
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
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmResponse> {
        let url = format!("{}/v1/messages", self.base_url);
        let body = self.build_request_body(messages, tools, params, false, cache_breakpoints);

        debug!(
            "Calling Anthropic native: model={}, messages={}, thinking={}",
            params.model,
            messages.len(),
            self.extended_thinking.as_ref().is_some_and(|et| et.enabled),
        );

        // Resolve breakpoints to check if we need the extended-cache-ttl header.
        let bps_owned: Vec<CacheBreakpoint>;
        let bps: &[CacheBreakpoint] = if cache_breakpoints.is_empty() && self.cache_system_prompt {
            bps_owned = vec![CacheBreakpoint {
                anchor: CacheAnchor::LastSystem,
                ttl: CacheTtl::Ephemeral,
            }];
            &bps_owned
        } else {
            cache_breakpoints
        };
        let resolved = resolve_breakpoints(messages, tools, bps);

        let mut request = self
            .client
            .post(&url)
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", &self.api_version)
            .header("content-type", "application/json");

        if needs_extended_cache_ttl_header(&resolved) {
            request = request.header("anthropic-beta", "extended-cache-ttl-2025-04-11");
        }

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

        let response_body: Value = response.json().await.map_err(|e| {
            ProviderError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        self.parse_response(response_body)
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmStream> {
        let url = format!("{}/v1/messages", self.base_url);
        let body = self.build_request_body(messages, tools, params, true, cache_breakpoints);

        debug!(
            "Calling Anthropic native (streaming): model={}, messages={}, thinking={}",
            params.model,
            messages.len(),
            self.extended_thinking.as_ref().is_some_and(|et| et.enabled),
        );

        // Resolve breakpoints to check if we need the extended-cache-ttl header.
        let bps_owned: Vec<CacheBreakpoint>;
        let bps: &[CacheBreakpoint] = if cache_breakpoints.is_empty() && self.cache_system_prompt {
            bps_owned = vec![CacheBreakpoint {
                anchor: CacheAnchor::LastSystem,
                ttl: CacheTtl::Ephemeral,
            }];
            &bps_owned
        } else {
            cache_breakpoints
        };
        let resolved = resolve_breakpoints(messages, tools, bps);

        let mut request = self
            .client
            .post(&url)
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", &self.api_version)
            .header("content-type", "application/json");

        if needs_extended_cache_ttl_header(&resolved) {
            request = request.header("anthropic-beta", "extended-cache-ttl-2025-04-11");
        }

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

        // Anthropic SSE has both `event:` and `data:` lines.
        Ok(crate::streaming::sse_chunk_stream(
            response,
            |event_type, data| Self::parse_anthropic_sse(event_type.unwrap_or(""), data),
        ))
    }

    async fn count_tokens(&self, messages: &[Message], tools: Option<&[Value]>) -> Result<usize> {
        let url = format!("{}/v1/messages/count_tokens", self.base_url);

        let mut body = json!({
            "model": self.model,
            "messages": self.convert_messages(messages),
        });

        let system_prompts = Self::extract_system_prompts(messages);
        if !system_prompts.is_empty() {
            let blocks: Vec<Value> = system_prompts
                .iter()
                .map(|text| json!({"type": "text", "text": text}))
                .collect();
            body["system"] = json!(blocks);
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
            .header("anthropic-version", &self.api_version)
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
            structured_outputs: true,
            prompt_caching: true,
            explicit_cache_markers: true,
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

    async fn health_check(&self) -> Result<ProviderHealth> {
        let url = format!("{}/v1/messages", self.base_url);

        // Send a minimal request — Anthropic doesn't have a /models endpoint,
        // so we POST a tiny messages request and check for a non-error response.
        // A 400 (bad request) still means the API is reachable and auth works.
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1,
            "messages": [{"role": "user", "content": "ping"}],
        });

        match self
            .client
            .post(&url)
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", &self.api_version)
            .header("content-type", "application/json")
            .timeout(Duration::from_secs(5))
            .json(&body)
            .send()
            .await
        {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    Ok(ProviderHealth::Healthy)
                } else if status.as_u16() == 401 || status.as_u16() == 403 {
                    Ok(ProviderHealth::Unhealthy(format!(
                        "Auth failed: HTTP {}",
                        status
                    )))
                } else if status.as_u16() == 429 {
                    Ok(ProviderHealth::Degraded("Rate limited".to_string()))
                } else if status.as_u16() == 529 {
                    Ok(ProviderHealth::Unhealthy("API overloaded".to_string()))
                } else {
                    // 400 or other client errors mean the API is reachable
                    Ok(ProviderHealth::Healthy)
                }
            }
            Err(e) if e.is_timeout() => Ok(ProviderHealth::Degraded(
                "Health check timed out (5s)".to_string(),
            )),
            Err(e) => Ok(ProviderHealth::Unhealthy(e.to_string())),
        }
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
    fn test_extract_system_prompts_single() {
        let messages = vec![Message::system("You are helpful"), Message::user("Hi")];
        let prompts = AnthropicNativeProvider::extract_system_prompts(&messages);
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0], "You are helpful");
    }

    #[test]
    fn test_extract_system_prompts_none() {
        let messages = vec![Message::user("Hi")];
        let prompts = AnthropicNativeProvider::extract_system_prompts(&messages);
        assert!(prompts.is_empty());
    }

    #[test]
    fn extract_system_prompts_collects_all_system_messages() {
        let messages = vec![
            Message::system("You are an assistant."),
            Message::system("Memory: User prefers concise answers."),
            Message::user("Hello"),
            Message::system("Summary: Previous conversation about Rust."),
        ];
        let prompts = AnthropicNativeProvider::extract_system_prompts(&messages);
        assert_eq!(prompts.len(), 3);
        assert_eq!(prompts[0], "You are an assistant.");
        assert_eq!(prompts[1], "Memory: User prefers concise answers.");
        assert_eq!(prompts[2], "Summary: Previous conversation about Rust.");
    }

    #[test]
    fn extract_system_prompts_returns_empty_when_none() {
        let messages = vec![Message::user("Hello")];
        let prompts = AnthropicNativeProvider::extract_system_prompts(&messages);
        assert!(prompts.is_empty());
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
        assert_eq!(result[0]["content"][0]["content"][0]["type"], "text");
        assert_eq!(
            result[0]["content"][0]["content"][0]["text"],
            "file contents here"
        );
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
        assert!(caps.structured_outputs);
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

    // SSE streaming tests

    #[test]
    fn test_parse_sse_text_delta() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
        let chunk = AnthropicNativeProvider::parse_anthropic_sse("content_block_delta", data)
            .unwrap()
            .unwrap();
        assert_eq!(chunk.content, Some("Hello".to_string()));
        assert!(chunk.tool_call_delta.is_none());
        assert!(!chunk.is_final);
        assert!(chunk.finish_reason.is_none());
        assert!(chunk.reasoning_content.is_none());
    }

    #[test]
    fn test_parse_sse_thinking_delta() {
        let data = r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me think..."}}"#;
        let chunk = AnthropicNativeProvider::parse_anthropic_sse("content_block_delta", data)
            .unwrap()
            .unwrap();
        assert!(chunk.content.is_none());
        assert_eq!(chunk.reasoning_content, Some("Let me think...".to_string()));
        assert!(!chunk.is_final);
    }

    #[test]
    fn test_parse_sse_tool_use_start() {
        let data = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"toolu_01","name":"get_weather","input":{}}}"#;
        let chunk = AnthropicNativeProvider::parse_anthropic_sse("content_block_start", data)
            .unwrap()
            .unwrap();
        assert!(chunk.content.is_none());
        let delta = chunk.tool_call_delta.unwrap();
        assert_eq!(delta.index, 1);
        assert_eq!(delta.id, Some("toolu_01".to_string()));
        assert_eq!(delta.name, Some("get_weather".to_string()));
        assert!(delta.arguments.is_none());
        assert!(!chunk.is_final);
    }

    #[test]
    fn test_parse_sse_tool_use_json_delta() {
        let data = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"location\":"}}"#;
        let chunk = AnthropicNativeProvider::parse_anthropic_sse("content_block_delta", data)
            .unwrap()
            .unwrap();
        let delta = chunk.tool_call_delta.unwrap();
        assert_eq!(delta.index, 1);
        assert!(delta.id.is_none());
        assert!(delta.name.is_none());
        assert_eq!(delta.arguments, Some("{\"location\":".to_string()));
    }

    #[test]
    fn test_parse_sse_message_delta_end_turn() {
        let data = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":15}}"#;
        let chunk = AnthropicNativeProvider::parse_anthropic_sse("message_delta", data)
            .unwrap()
            .unwrap();
        assert!(chunk.is_final);
        assert_eq!(chunk.finish_reason, Some("stop".to_string()));
        let usage = chunk.usage.unwrap();
        assert_eq!(usage.completion_tokens, 15);
        assert_eq!(usage.prompt_tokens, 0);
    }

    #[test]
    fn test_parse_sse_message_delta_tool_use() {
        let data = r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":42}}"#;
        let chunk = AnthropicNativeProvider::parse_anthropic_sse("message_delta", data)
            .unwrap()
            .unwrap();
        assert!(chunk.is_final);
        assert_eq!(chunk.finish_reason, Some("tool_calls".to_string()));
    }

    #[test]
    fn test_parse_sse_message_delta_max_tokens() {
        let data = r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens","stop_sequence":null},"usage":{"output_tokens":4096}}"#;
        let chunk = AnthropicNativeProvider::parse_anthropic_sse("message_delta", data)
            .unwrap()
            .unwrap();
        assert!(chunk.is_final);
        assert_eq!(chunk.finish_reason, Some("length".to_string()));
    }

    #[test]
    fn test_parse_sse_message_start_extracts_usage() {
        let data = r#"{"type":"message_start","message":{"id":"msg_01","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null,"usage":{"input_tokens":25,"output_tokens":1}}}"#;
        let result = AnthropicNativeProvider::parse_anthropic_sse("message_start", data).unwrap();
        let chunk = result.unwrap();
        assert!(!chunk.is_final);
        let usage = chunk.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 25);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.cache_read_tokens, 0);
        assert_eq!(usage.cache_write_tokens, 0);
    }

    #[test]
    fn test_parse_sse_message_start_with_cache_usage() {
        let data = r#"{"type":"message_start","message":{"id":"msg_01","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null,"usage":{"input_tokens":100,"output_tokens":1,"cache_read_input_tokens":80,"cache_creation_input_tokens":15}}}"#;
        let result = AnthropicNativeProvider::parse_anthropic_sse("message_start", data).unwrap();
        let chunk = result.unwrap();
        let usage = chunk.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.cache_read_tokens, 80);
        assert_eq!(usage.cache_write_tokens, 15);
    }

    #[test]
    fn test_parse_sse_message_start_without_usage_returns_none() {
        let data = r#"{"type":"message_start","message":{"id":"msg_01","type":"message","role":"assistant","content":[],"model":"claude-sonnet-4-20250514","stop_reason":null}}"#;
        let result = AnthropicNativeProvider::parse_anthropic_sse("message_start", data).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_content_block_stop_returns_none() {
        let data = r#"{"type":"content_block_stop","index":0}"#;
        let result =
            AnthropicNativeProvider::parse_anthropic_sse("content_block_stop", data).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_message_stop_returns_none() {
        let data = r#"{"type":"message_stop"}"#;
        let result = AnthropicNativeProvider::parse_anthropic_sse("message_stop", data).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_ping_returns_none() {
        let data = r#"{"type":"ping"}"#;
        let result = AnthropicNativeProvider::parse_anthropic_sse("ping", data).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_error_event() {
        let data = r#"{"type":"error","error":{"type":"overloaded_error","message":"Overloaded"}}"#;
        let result = AnthropicNativeProvider::parse_anthropic_sse("error", data);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Overloaded"));
    }

    #[test]
    fn test_parse_sse_text_block_start_returns_none() {
        let data =
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#;
        let result =
            AnthropicNativeProvider::parse_anthropic_sse("content_block_start", data).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_sse_invalid_json() {
        let result =
            AnthropicNativeProvider::parse_anthropic_sse("content_block_delta", "not json");
        assert!(result.is_err());
    }

    // --- G-48: API version configurability tests ---

    #[test]
    fn test_default_api_version() {
        let provider = test_provider();
        // When env var is not set, defaults to "2023-06-01"
        // (env var may or may not be set in test environment, so test with_api_version)
        let provider = provider.with_api_version("2023-06-01");
        assert_eq!(provider.api_version, "2023-06-01");
    }

    #[test]
    fn test_custom_api_version() {
        let provider = AnthropicNativeProvider::new(
            Secret::new("test-key".to_string()),
            "https://api.anthropic.com".to_string(),
            "claude-sonnet-4-20250514".to_string(),
        )
        .with_api_version("2024-01-01");
        assert_eq!(provider.api_version, "2024-01-01");
    }

    // --- G-24: Structured output tests ---

    #[test]
    fn test_apply_response_format_text_is_noop() {
        let mut body = json!({"model": "claude-sonnet-4-20250514"});
        let mut tools = vec![];
        AnthropicNativeProvider::apply_response_format(
            &mut body,
            &ResponseFormat::Text,
            &mut tools,
        );
        assert!(body.get("tool_choice").is_none());
        assert!(tools.is_empty());
    }

    #[test]
    fn test_apply_response_format_json_object_adds_system_instruction() {
        let mut body = json!({"model": "claude-sonnet-4-20250514"});
        let mut tools = vec![];
        AnthropicNativeProvider::apply_response_format(
            &mut body,
            &ResponseFormat::JsonObject,
            &mut tools,
        );
        let system = body["system"].as_array().unwrap();
        assert_eq!(system.len(), 1);
        assert!(system[0]["text"].as_str().unwrap().contains("valid JSON"));
    }

    #[test]
    fn test_apply_response_format_json_object_prepends_to_existing_system() {
        let mut body = json!({
            "model": "claude-sonnet-4-20250514",
            "system": [{"type": "text", "text": "You are a helpful assistant."}]
        });
        let mut tools = vec![];
        AnthropicNativeProvider::apply_response_format(
            &mut body,
            &ResponseFormat::JsonObject,
            &mut tools,
        );
        let system = body["system"].as_array().unwrap();
        assert_eq!(system.len(), 2);
        // JSON instruction is first
        assert!(system[0]["text"].as_str().unwrap().contains("valid JSON"));
        // Original system prompt is second
        assert_eq!(system[1]["text"], "You are a helpful assistant.");
    }

    #[test]
    fn test_apply_response_format_json_schema_injects_tool() {
        let schema = json!({
            "type": "object",
            "properties": {"name": {"type": "string"}},
            "required": ["name"]
        });
        let mut body = json!({"model": "claude-sonnet-4-20250514"});
        let mut tools = vec![];
        AnthropicNativeProvider::apply_response_format(
            &mut body,
            &ResponseFormat::JsonSchema {
                name: "person".to_string(),
                schema: schema.clone(),
            },
            &mut tools,
        );

        // Tool was injected
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "person");
        assert_eq!(tools[0]["input_schema"], schema);

        // tool_choice forces the synthetic tool
        assert_eq!(body["tool_choice"]["type"], "tool");
        assert_eq!(body["tool_choice"]["name"], "person");

        // tools array in body is updated
        assert_eq!(body["tools"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_apply_response_format_json_schema_appends_to_existing_tools() {
        let schema = json!({"type": "object", "properties": {}});
        let mut body = json!({"model": "claude-sonnet-4-20250514"});
        let mut tools = vec![json!({"name": "existing_tool", "input_schema": {}})];
        AnthropicNativeProvider::apply_response_format(
            &mut body,
            &ResponseFormat::JsonSchema {
                name: "output".to_string(),
                schema,
            },
            &mut tools,
        );

        // Both the existing tool and the synthetic tool are present
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "existing_tool");
        assert_eq!(tools[1]["name"], "output");

        // tool_choice forces the synthetic tool
        assert_eq!(body["tool_choice"]["name"], "output");
    }

    mod resolve_breakpoints_tests {
        use super::*;

        fn sys(text: &str) -> Message {
            Message::System {
                content: text.to_string(),
            }
        }

        fn user(text: &str) -> Message {
            Message::user(text)
        }

        fn assistant(text: &str) -> Message {
            Message::Assistant {
                content: Some(text.to_string()),
                tool_calls: None,
                reasoning_content: None,
            }
        }

        #[test]
        fn resolve_last_system_finds_last_system_block() {
            let messages = vec![sys("first"), sys("second"), user("hi")];
            let bps = vec![CacheBreakpoint {
                anchor: CacheAnchor::LastSystem,
                ttl: CacheTtl::Persistent,
            }];
            let resolved = resolve_breakpoints(&messages, None, &bps);
            assert_eq!(resolved.len(), 1);
            assert_eq!(resolved[0].section, SECTION_SYSTEM);
            assert_eq!(resolved[0].index, 1);
            assert_eq!(resolved[0].ttl, CacheTtl::Persistent);
        }

        #[test]
        fn resolve_last_system_skips_when_no_system_messages() {
            let messages = vec![user("hi")];
            let bps = vec![CacheBreakpoint {
                anchor: CacheAnchor::LastSystem,
                ttl: CacheTtl::Ephemeral,
            }];
            let resolved = resolve_breakpoints(&messages, None, &bps);
            assert!(resolved.is_empty());
        }

        #[test]
        fn resolve_last_tool_marks_last_tool_index() {
            let tools = vec![
                serde_json::json!({"name": "a"}),
                serde_json::json!({"name": "b"}),
            ];
            let bps = vec![CacheBreakpoint {
                anchor: CacheAnchor::LastTool,
                ttl: CacheTtl::Persistent,
            }];
            let resolved = resolve_breakpoints(&[], Some(&tools), &bps);
            assert_eq!(resolved.len(), 1);
            assert_eq!(resolved[0].section, SECTION_TOOLS);
            assert_eq!(resolved[0].index, 1);
        }

        #[test]
        fn resolve_last_tool_skips_when_no_tools() {
            let bps = vec![CacheBreakpoint {
                anchor: CacheAnchor::LastTool,
                ttl: CacheTtl::Persistent,
            }];
            let resolved = resolve_breakpoints(&[], None, &bps);
            assert!(resolved.is_empty());

            let resolved2 = resolve_breakpoints(&[], Some(&[]), &bps);
            assert!(resolved2.is_empty());
        }

        #[test]
        fn resolve_message_index_in_range() {
            let messages = vec![sys("s"), user("u"), assistant("a")];
            let bps = vec![CacheBreakpoint {
                anchor: CacheAnchor::MessageIndex(2),
                ttl: CacheTtl::Ephemeral,
            }];
            let resolved = resolve_breakpoints(&messages, None, &bps);
            assert_eq!(resolved.len(), 1);
            assert_eq!(resolved[0].section, SECTION_MESSAGES);
            // Original index 2, 1 system message before it → wire index 1
            assert_eq!(resolved[0].index, 1);
        }

        #[test]
        fn resolve_message_index_out_of_range_skipped() {
            let messages = vec![sys("s"), user("u")];
            let bps = vec![CacheBreakpoint {
                anchor: CacheAnchor::MessageIndex(99),
                ttl: CacheTtl::Ephemeral,
            }];
            let resolved = resolve_breakpoints(&messages, None, &bps);
            assert!(resolved.is_empty());
        }

        #[test]
        fn resolve_sorts_and_keeps_trailing_four() {
            let messages = vec![
                sys("s"),
                user("u0"),
                user("u1"),
                user("u2"),
                user("u3"),
                user("u4"),
            ];
            let tools = vec![serde_json::json!({"name": "t"})];
            let bps = vec![
                CacheBreakpoint {
                    anchor: CacheAnchor::LastSystem,
                    ttl: CacheTtl::Persistent,
                },
                CacheBreakpoint {
                    anchor: CacheAnchor::LastTool,
                    ttl: CacheTtl::Persistent,
                },
                CacheBreakpoint {
                    anchor: CacheAnchor::MessageIndex(1),
                    ttl: CacheTtl::Ephemeral,
                },
                CacheBreakpoint {
                    anchor: CacheAnchor::MessageIndex(2),
                    ttl: CacheTtl::Ephemeral,
                },
                CacheBreakpoint {
                    anchor: CacheAnchor::MessageIndex(3),
                    ttl: CacheTtl::Ephemeral,
                },
            ];
            let resolved = resolve_breakpoints(&messages, Some(&tools), &bps);
            // 5 inputs → keep trailing 4
            assert_eq!(resolved.len(), 4);
            // Earliest one (LastSystem) should have been dropped
            assert!(!resolved.iter().any(|r| r.section == SECTION_SYSTEM));
            // Verify ascending order
            let positions: Vec<(u8, usize)> =
                resolved.iter().map(|r| (r.section, r.index)).collect();
            let mut sorted = positions.clone();
            sorted.sort();
            assert_eq!(positions, sorted);
        }

        #[test]
        fn persistent_breakpoint_triggers_extended_cache_ttl_header() {
            let resolved = vec![ResolvedMarker {
                section: SECTION_SYSTEM,
                index: 0,
                ttl: CacheTtl::Persistent,
            }];
            assert!(needs_extended_cache_ttl_header(&resolved));

            let resolved = vec![ResolvedMarker {
                section: SECTION_SYSTEM,
                index: 0,
                ttl: CacheTtl::Ephemeral,
            }];
            assert!(!needs_extended_cache_ttl_header(&resolved));

            let resolved: Vec<ResolvedMarker> = vec![];
            assert!(!needs_extended_cache_ttl_header(&resolved));
        }
    }

    mod build_request_body_cache_tests {
        use super::*;

        fn ttl_value(block: &Value) -> Option<&str> {
            block
                .get("cache_control")
                .and_then(|c| c.get("ttl"))
                .and_then(|t| t.as_str())
        }

        fn has_cache_control(block: &Value) -> bool {
            block.get("cache_control").is_some()
        }

        #[test]
        fn explicit_breakpoint_overrides_legacy_flag() {
            let provider = AnthropicNativeProvider::new_for_test(true);
            let messages = vec![
                Message::System {
                    content: "sys".into(),
                },
                Message::user("hi"),
            ];
            let bps = vec![CacheBreakpoint {
                anchor: CacheAnchor::LastSystem,
                ttl: CacheTtl::Persistent,
            }];
            let body = provider.build_request_body(
                &messages,
                None,
                &ChatParams::new("claude-3-5-sonnet"),
                false,
                &bps,
            );
            let system_blocks = body.get("system").unwrap().as_array().unwrap();
            assert!(has_cache_control(&system_blocks[0]));
            assert_eq!(ttl_value(&system_blocks[0]), Some("1h"));
        }

        #[test]
        fn empty_breakpoints_with_legacy_flag_synthesizes_fallback() {
            let provider = AnthropicNativeProvider::new_for_test(true);
            let messages = vec![Message::System {
                content: "sys".into(),
            }];
            let body = provider.build_request_body(
                &messages,
                None,
                &ChatParams::new("claude-3-5-sonnet"),
                false,
                &[],
            );
            let system_blocks = body.get("system").unwrap().as_array().unwrap();
            assert!(has_cache_control(&system_blocks[0]));
            assert_eq!(ttl_value(&system_blocks[0]), None);
        }

        #[test]
        fn empty_breakpoints_without_legacy_flag_no_marker() {
            let provider = AnthropicNativeProvider::new_for_test(false);
            let messages = vec![Message::System {
                content: "sys".into(),
            }];
            let body = provider.build_request_body(
                &messages,
                None,
                &ChatParams::new("claude-3-5-sonnet"),
                false,
                &[],
            );
            let system_blocks = body.get("system").unwrap().as_array().unwrap();
            assert!(!has_cache_control(&system_blocks[0]));
        }
    }
}
