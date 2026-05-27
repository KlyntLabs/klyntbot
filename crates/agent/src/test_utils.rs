//! Shared test utilities for the agent crate.

use std::sync::Mutex;

use providers::{
    CacheBreakpoint, ChatParams, LlmProvider, LlmResponse, LlmStream, LlmStreamChunk, Message,
    ProviderCapabilities, ProviderHealth, ToolCall, ToolCallDelta, Usage,
};
use serde_json::Value;

/// Unified mock LLM provider for unit tests.
///
/// Supports queued responses, error injection, tool calls, and configurable
/// provider metadata.
pub struct MockProvider {
    responses: Mutex<Vec<std::result::Result<LlmResponse, String>>>,
    streams: Mutex<Vec<Vec<LlmStreamChunk>>>,
    context_window: usize,
    capabilities: ProviderCapabilities,
    health: ProviderHealth,
}

impl MockProvider {
    /// Create a mock that returns a single text response.
    pub fn with_text(text: &str) -> Self {
        Self::with_response(LlmResponse {
            content: Some(text.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        })
    }

    /// Create a mock that returns a single tool call response.
    pub fn with_tool_call(name: &str, args: Value) -> Self {
        Self::with_response(LlmResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_1".to_string(),
                name: name.to_string(),
                arguments: args,
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        })
    }

    /// Create a mock that returns a single LLM response.
    pub fn with_response(response: LlmResponse) -> Self {
        Self {
            responses: Mutex::new(vec![Ok(response)]),
            streams: Mutex::new(Vec::new()),
            context_window: providers::DEFAULT_CONTEXT_WINDOW,
            capabilities: ProviderCapabilities::default(),
            health: ProviderHealth::Healthy,
        }
    }

    /// Create a mock that returns an error on every call.
    pub fn with_error(msg: &str) -> Self {
        Self {
            responses: Mutex::new(vec![Err(msg.to_string())]),
            streams: Mutex::new(Vec::new()),
            context_window: providers::DEFAULT_CONTEXT_WINDOW,
            capabilities: ProviderCapabilities::default(),
            health: ProviderHealth::Healthy,
        }
    }

    /// Create a mock with a queue of responses (or errors).
    /// Each call to `chat()` pops the next item from the queue.
    pub fn with_responses(responses: Vec<std::result::Result<LlmResponse, String>>) -> Self {
        Self {
            responses: Mutex::new(responses),
            streams: Mutex::new(Vec::new()),
            context_window: providers::DEFAULT_CONTEXT_WINDOW,
            capabilities: ProviderCapabilities::default(),
            health: ProviderHealth::Healthy,
        }
    }

    /// Create a mock that replays a scripted stream per `chat_stream()` call.
    /// Each call pops the next scripted `Vec<LlmStreamChunk>` from the queue.
    pub fn with_streams(streams: Vec<Vec<LlmStreamChunk>>) -> Self {
        Self {
            responses: Mutex::new(Vec::new()),
            streams: Mutex::new(streams),
            context_window: providers::DEFAULT_CONTEXT_WINDOW,
            capabilities: ProviderCapabilities::default(),
            health: ProviderHealth::Healthy,
        }
    }

    /// Set the context window size.
    pub fn context_window(mut self, window: usize) -> Self {
        self.context_window = window;
        self
    }

    /// Set provider capabilities.
    pub fn capabilities(mut self, caps: ProviderCapabilities) -> Self {
        self.capabilities = caps;
        self
    }

    /// Set the health check response.
    pub fn health(mut self, health: ProviderHealth) -> Self {
        self.health = health;
        self
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    async fn chat(
        &self,
        _messages: &[Message],
        _tools: Option<&[Value]>,
        _params: &ChatParams,
        _cache_breakpoints: &[CacheBreakpoint],
    ) -> common::Result<LlmResponse> {
        let mut responses = self.responses.lock().unwrap();
        if responses.is_empty() {
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(
                    "MockProvider: no more responses in queue".to_string(),
                ),
            ));
        }
        match responses.remove(0) {
            Ok(r) => Ok(r),
            Err(e) => Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(e),
            )),
        }
    }

    fn default_model(&self) -> &str {
        "mock"
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.capabilities.clone()
    }

    fn context_window(&self) -> usize {
        self.context_window
    }

    async fn health_check(&self) -> common::Result<ProviderHealth> {
        Ok(self.health.clone())
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> common::Result<LlmStream> {
        let scripted = {
            let mut s = self.streams.lock().unwrap();
            if s.is_empty() {
                None
            } else {
                Some(s.remove(0))
            }
        };

        let chunks: Vec<common::Result<LlmStreamChunk>> = match scripted {
            // Replay the scripted stream verbatim.
            Some(chunks) => chunks.into_iter().map(Ok).collect(),
            // No script queued: wrap chat() exactly like the trait default,
            // so non-streaming mocks keep working when driven via chat_stream.
            None => {
                let response = self
                    .chat(messages, tools, params, cache_breakpoints)
                    .await?;
                let mut out = Vec::with_capacity(response.tool_calls.len() + 1);
                for (i, tc) in response.tool_calls.iter().enumerate() {
                    out.push(Ok(LlmStreamChunk {
                        content: None,
                        tool_call_delta: Some(ToolCallDelta {
                            index: i,
                            id: Some(tc.id.clone()),
                            name: Some(tc.name.clone()),
                            arguments: Some(
                                serde_json::to_string(&tc.arguments).unwrap_or_default(),
                            ),
                        }),
                        is_final: false,
                        finish_reason: None,
                        reasoning_content: None,
                        usage: None,
                    }));
                }
                out.push(Ok(LlmStreamChunk {
                    content: response.content,
                    tool_call_delta: None,
                    is_final: true,
                    finish_reason: Some(response.finish_reason),
                    reasoning_content: response.reasoning_content,
                    usage: Some(response.usage),
                }));
                out
            }
        };

        Ok(Box::pin(futures_util::stream::iter(chunks)))
    }
}

/// Builds a scripted `Vec<LlmStreamChunk>` for `MockProvider::with_streams`.
///
/// `tool_call` splits a call's JSON arguments across multiple deltas at the
/// same index (id+name only on the first), which is what forces
/// `PartialToolCall` in the execution core to concatenate fragments.
pub struct StreamScript {
    chunks: Vec<LlmStreamChunk>,
    next_tool_index: usize,
}

impl Default for StreamScript {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamScript {
    pub fn new() -> Self {
        Self {
            chunks: Vec::new(),
            next_tool_index: 0,
        }
    }

    /// Append a visible-content (text) delta.
    pub fn text(mut self, s: &str) -> Self {
        self.chunks.push(LlmStreamChunk {
            content: Some(s.to_string()),
            tool_call_delta: None,
            is_final: false,
            finish_reason: None,
            reasoning_content: None,
            usage: None,
        });
        self
    }

    /// Append a reasoning (extended-thinking) delta.
    pub fn reasoning(mut self, s: &str) -> Self {
        self.chunks.push(LlmStreamChunk {
            content: None,
            tool_call_delta: None,
            is_final: false,
            finish_reason: None,
            reasoning_content: Some(s.to_string()),
            usage: None,
        });
        self
    }

    /// Append a tool call whose `arguments` arrive fragmented across same-index
    /// deltas. `id`/`name` are sent only on the first fragment.
    pub fn tool_call(mut self, id: &str, name: &str, arg_fragments: &[&str]) -> Self {
        let index = self.next_tool_index;
        self.next_tool_index += 1;

        if arg_fragments.is_empty() {
            self.chunks.push(LlmStreamChunk {
                content: None,
                tool_call_delta: Some(ToolCallDelta {
                    index,
                    id: Some(id.to_string()),
                    name: Some(name.to_string()),
                    arguments: Some(String::new()),
                }),
                is_final: false,
                finish_reason: None,
                reasoning_content: None,
                usage: None,
            });
            return self;
        }

        for (i, frag) in arg_fragments.iter().enumerate() {
            let first = i == 0;
            self.chunks.push(LlmStreamChunk {
                content: None,
                tool_call_delta: Some(ToolCallDelta {
                    index,
                    id: if first { Some(id.to_string()) } else { None },
                    name: if first { Some(name.to_string()) } else { None },
                    arguments: Some(frag.to_string()),
                }),
                is_final: false,
                finish_reason: None,
                reasoning_content: None,
                usage: None,
            });
        }
        self
    }

    /// Append a usage-only chunk (mirrors message_start / message_delta).
    pub fn usage(mut self, usage: Usage) -> Self {
        self.chunks.push(LlmStreamChunk {
            content: None,
            tool_call_delta: None,
            is_final: false,
            finish_reason: None,
            reasoning_content: None,
            usage: Some(usage),
        });
        self
    }

    /// Terminal chunk carrying the finish reason. Consumes the builder.
    pub fn finish(mut self, reason: &str) -> Vec<LlmStreamChunk> {
        self.chunks.push(LlmStreamChunk {
            content: None,
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(reason.to_string()),
            reasoning_content: None,
            usage: None,
        });
        self.chunks
    }
}
