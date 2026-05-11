//! Core execution engine that drives a single LLM→tool cycle.

use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Instant;

use futures_util::future::join_all;
use futures_util::StreamExt;
use tokio::sync::{RwLock, Semaphore};

use common::{helpers::tool_def_name, Result};
use context_engine::TokenCounter;
use klynt_core::tools::ask_user::ASK_USER_TOOL_NAME;
use providers::{tool_calls_to_messages, DynProvider, Message};
use tools::{registry::ToolRegistry, RoutingContext};
use tracing::debug;

use providers::Usage;

use crate::execution::types::{CycleOutcome, ExecutionParams, ToolExecutionResult};

/// Fan out a runtime AgentEvent to both the UI-streaming channel (single-
/// consumer mpsc::Sender) and the cognitive-ingest broadcast bus.
///
/// - `event_tx`: Optional. When `Some`, sends to the chat-streaming pipeline
///   that emits Tauri `agent:*` events. Drops are ignored (UI may close).
/// - `domain_bus`: Optional. Used by Distiller and Mirror subscribers.
///
/// This is the preferred replacement for direct `event_tx.send(evt).await`
/// at every existing emit site in `core.rs` and `execute_loop.rs`.
pub async fn fan_out_event(
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    domain_bus: Option<&Arc<bus::DomainEventBus>>,
    evt: crate::events::AgentEvent,
) {
    if let Some(tx) = event_tx {
        let _ = tx.send(evt.clone()).await;
    }
    if let Some(bus) = domain_bus {
        let payload =
            serde_json::to_value(&evt).unwrap_or_else(|_| serde_json::json!({"type": "unknown"}));
        bus.publish(bus::DomainEvent::Generic {
            kind: "agent_event".into(),
            payload,
        });
    }
}

/// Extended timeout for interactive tools (ask_user) and long-running tools
/// (shell / build / test) whose wall-time can legitimately exceed
/// `params.tool_timeout` (default 30 s). Tools opt in by returning
/// `Some(LONG_RUNNING_TOOL_TIMEOUT)` from `Tool::custom_timeout()`.
pub const LONG_RUNNING_TOOL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(600);

/// Finish reason emitted when the user cancels a streaming LLM call.
const FINISH_REASON_CANCELLED: &str = "cancelled";

/// Maximum number of tool calls that can execute concurrently within a single cycle.
const MAX_CONCURRENT_TOOLS: usize = 10;

/// Maximum length for a single tool result (50KB).
/// Tool results accumulate across 500+ messages per session, so this
/// limit directly bounds session cache footprint.
const MAX_TOOL_RESULT_LENGTH: usize = 50_000;
const TOOL_RESULT_TRUNCATION_POLICY: klynt_truncation::TruncationPolicy =
    klynt_truncation::TruncationPolicy::Bytes(MAX_TOOL_RESULT_LENGTH);

/// Partition tool calls by `Tool::is_concurrency_safe`.
///
/// Returns `(safe_calls, unsafe_calls)` where safe calls can be run in
/// parallel via `futures::future::join_all` and unsafe calls must run
/// sequentially to avoid side-effect races.
pub fn partition_by_concurrency_safety<'a>(
    tool_calls: &'a [providers::ToolCall],
    registry: &'a tools::registry::ToolRegistry,
) -> (Vec<&'a providers::ToolCall>, Vec<&'a providers::ToolCall>) {
    let mut safe = Vec::new();
    let mut unsafe_ = Vec::new();
    for tc in tool_calls {
        if let Some(tool) = registry.get(&tc.name) {
            if tool.is_concurrency_safe(&tc.arguments) {
                safe.push(tc);
            } else {
                unsafe_.push(tc);
            }
        } else {
            // Unknown tools default to unsafe.
            unsafe_.push(tc);
        }
    }
    (safe, unsafe_)
}

/// Sanitize tool result string before injecting into conversation messages.
///
/// - Strips control characters (except `\n`, `\t`, `\r`)
/// - Truncates to [`MAX_TOOL_RESULT_LENGTH`] with a notice (UTF-8 safe)
fn sanitize_tool_result(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .collect();
    if cleaned.len() <= MAX_TOOL_RESULT_LENGTH {
        return cleaned;
    }
    klynt_truncation::formatted_truncate_text(&cleaned, TOOL_RESULT_TRUNCATION_POLICY)
}

/// Hash a `serde_json::Value` without serializing to a string.
///
/// `Value` doesn't implement `Hash` (because of `f64`), so we walk the tree
/// and feed each element into a `DefaultHasher`. This is cheaper than
/// `serde_json::to_string()` which allocates a `String` for every tool call.
fn hash_json_value(value: &serde_json::Value) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hash_value_into(value, &mut hasher);
    hasher.finish()
}

fn hash_value_into(value: &serde_json::Value, hasher: &mut impl Hasher) {
    std::mem::discriminant(value).hash(hasher);
    match value {
        serde_json::Value::Null => {}
        serde_json::Value::Bool(b) => b.hash(hasher),
        serde_json::Value::Number(n) => {
            // Hash numeric representation directly to avoid string allocation.
            if let Some(i) = n.as_i64() {
                i.hash(hasher);
            } else if let Some(u) = n.as_u64() {
                u.hash(hasher);
            } else if let Some(f) = n.as_f64() {
                f.to_bits().hash(hasher);
            }
        }
        serde_json::Value::String(s) => s.hash(hasher),
        serde_json::Value::Array(arr) => {
            arr.len().hash(hasher);
            for item in arr {
                hash_value_into(item, hasher);
            }
        }
        serde_json::Value::Object(obj) => {
            obj.len().hash(hasher);
            for (k, v) in obj {
                k.hash(hasher);
                hash_value_into(v, hasher);
            }
        }
    }
}

/// Detect if a text response is actually a fabricated tool response.
///
/// Some LLMs (DeepSeek, Kimi, etc.) skip tool calls and generate text that
/// looks like a tool result. This function uses heuristics to detect that pattern.
///
/// Returns `true` if the text appears to be a fabricated tool execution.
/// Case-insensitive `find` for ASCII needles. Returns byte offset if found.
fn ifind(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }
    let mut offset = 0;
    let mut chars = haystack.chars();
    loop {
        if chars
            .clone()
            .zip(needle.chars())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b))
        {
            return Some(offset);
        }
        match chars.next() {
            Some(c) => offset += c.len_utf8(),
            None => break,
        }
    }
    None
}

/// Case-insensitive `contains` for ASCII needles.
fn icontains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut chars = haystack.chars();
    loop {
        if chars
            .clone()
            .zip(needle.chars())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b))
        {
            return true;
        }
        if chars.next().is_none() {
            break;
        }
    }
    false
}

fn is_fabricated_tool_response(text: &str, tool_names: &[&str]) -> bool {
    if tool_names.is_empty() {
        return false;
    }

    let has_todo = tool_names.contains(&"todo");
    let has_search = tool_names.iter().any(|t| t.contains("search"));

    // Pattern 1: Contains a fake ID pattern (hex ID like "9c4e5f3b" or "a1b2c3d4")
    let has_fake_id = {
        let mut found = false;
        for pattern in &["id:", "(id:"] {
            if let Some(pos) = ifind(text, pattern) {
                let after = &text[pos + pattern.len()..];
                let trimmed = after.trim_start();
                let hex_chars = trimmed
                    .chars()
                    .take(10)
                    .take_while(|c| c.is_ascii_hexdigit())
                    .count();
                if hex_chars >= 6 {
                    found = true;
                    break;
                }
            }
        }
        found
    };

    // Pattern 2: Context-aware structured result indicators
    let todo_indicators = ["task created", "task added", "i've created the task"];
    let search_indicators = [
        "i searched the web",
        "search results:",
        "here are the results",
        "i found these results",
    ];
    let has_todo_result = has_todo && todo_indicators.iter().any(|p| icontains(text, p));
    let has_search_result = has_search && search_indicators.iter().any(|p| icontains(text, p));
    let has_structured_result = has_todo_result || has_search_result;

    // Pattern 3: Has multiple field-like patterns (Priority:, Due Date:, Description:, Tags:)
    let field_patterns = [
        "priority:",
        "due date:",
        "description:",
        "tags:",
        "estimated time:",
    ];
    let field_count = field_patterns.iter().filter(|p| icontains(text, p)).count();
    let has_multiple_fields = field_count >= 2;

    // Pattern 4: Search with numbered list — requires fake ID for corroboration
    let has_search_with_list =
        has_search_result && has_fake_id && text.contains("\n1.") && text.contains("\n2.");

    // Decision: fabricated if structured result with (fake ID or multiple fields),
    // or search with numbered list and corroborating fake ID
    (has_structured_result && (has_fake_id || has_multiple_fields)) || has_search_with_list
}

/// Accumulated tool-call fragments from streaming deltas.
struct PartialToolCall {
    id: String,
    name: String,
    args: String,
}

/// Call the provider with streaming, emitting `ContentChunk` events per-token.
///
/// Consumes the stream and returns an `LlmResponse` equivalent to `provider.chat()`,
/// but with real-time content chunks sent to the UI as they arrive.
///
/// Token usage is taken from real provider data when available (via `LlmStreamChunk.usage`).
/// Falls back to `best_token_counter()` estimation when the provider sends no usage.
#[allow(clippy::too_many_arguments)]
async fn call_provider_streaming(
    provider: &dyn providers::LlmProvider,
    messages: &[Message],
    tools: &[serde_json::Value],
    params: &providers::ChatParams,
    event_tx: &tokio::sync::mpsc::Sender<crate::events::AgentEvent>,
    domain_bus: Option<&Arc<bus::DomainEventBus>>,
    cache_breakpoints: &[providers::CacheBreakpoint],
    cancel_token: &tokio_util::sync::CancellationToken,
) -> Result<providers::LlmResponse> {
    tracing::debug!(
        messages = messages.len(),
        tools = tools.len(),
        "call_provider_streaming: about to call provider.chat_stream"
    );
    let mut stream = provider
        .chat_stream(messages, Some(tools), params, cache_breakpoints)
        .await?;
    tracing::debug!("call_provider_streaming: chat_stream returned, awaiting first chunk");

    let mut content = String::new();
    let mut partials: Vec<PartialToolCall> = Vec::with_capacity(4);
    let mut finish_reason = String::new();
    let mut reasoning = String::new();
    let mut accumulated_usage = Usage::default();
    let mut has_real_usage = false;

    loop {
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                tracing::info!(
                    content_len = content.len(),
                    reasoning_len = reasoning.len(),
                    "call_provider_streaming: cancellation observed mid-stream"
                );
                fan_out_event(
                    Some(event_tx),
                    domain_bus,
                    crate::events::AgentEvent::Cancelled {
                        partial_content: content.clone(),
                        partial_reasoning: reasoning.clone(),
                    },
                )
                .await;
                finish_reason = FINISH_REASON_CANCELLED.to_string();
                break;
            }
            maybe_chunk = stream.next() => {
                match maybe_chunk {
                    None => break,
                    Some(result) => {
                        let chunk = result?;

                        // Consume content — avoids cloning on every token chunk.
                        if let Some(text) = chunk.content {
                            if !text.is_empty() {
                                content.push_str(&text);
                                fan_out_event(
                                    Some(event_tx),
                                    domain_bus,
                                    crate::events::AgentEvent::ContentChunk { data: text },
                                )
                                .await;
                            }
                        }

                        if let Some(r) = chunk.reasoning_content {
                            if !r.is_empty() {
                                reasoning.push_str(&r);
                                // Emit reasoning as a first-class event so the bridge can
                                // surface it to the FE as a `PartDelta::Reasoning` (rendered
                                // inline as italic "Thinking:" prose). Previously we silently
                                // buffered reasoning and only fell back to a synthetic
                                // ContentChunk if `content` was empty — meaning models that
                                // emit BOTH a reasoning channel and visible answer (most
                                // extended-thinking models) had their reasoning dropped.
                                fan_out_event(
                                    Some(event_tx),
                                    domain_bus,
                                    crate::events::AgentEvent::ReasoningChunk { data: r },
                                )
                                .await;
                            }
                        }

                        if let Some(delta) = chunk.tool_call_delta {
                            while partials.len() <= delta.index {
                                partials.push(PartialToolCall {
                                    id: String::new(),
                                    name: String::new(),
                                    args: String::new(),
                                });
                            }
                            let partial = &mut partials[delta.index];
                            if let Some(id) = delta.id {
                                partial.id = id;
                            }
                            if let Some(name) = delta.name {
                                partial.name.push_str(&name);
                            }
                            if let Some(args) = delta.arguments {
                                partial.args.push_str(&args);
                            }
                        }

                        if let Some(reason) = chunk.finish_reason {
                            finish_reason = reason;
                        }

                        if let Some(chunk_usage) = chunk.usage {
                            has_real_usage = true;
                            if chunk_usage.prompt_tokens > 0 {
                                accumulated_usage.prompt_tokens = chunk_usage.prompt_tokens;
                            }
                            if chunk_usage.completion_tokens > 0 {
                                accumulated_usage.completion_tokens = chunk_usage.completion_tokens;
                            }
                            if chunk_usage.cache_read_tokens > 0 {
                                accumulated_usage.cache_read_tokens = chunk_usage.cache_read_tokens;
                            }
                            if chunk_usage.cache_write_tokens > 0 {
                                accumulated_usage.cache_write_tokens = chunk_usage.cache_write_tokens;
                            }
                        }
                    }
                }
            }
        }
    }

    let tool_calls: Vec<providers::ToolCall> = partials
        .into_iter()
        .filter(|p| !p.id.is_empty() && !p.name.is_empty())
        .map(|p| {
            let arguments = serde_json::from_str(&p.args)
                .unwrap_or(serde_json::Value::Object(Default::default()));
            providers::ToolCall {
                id: p.id,
                name: p.name,
                arguments,
            }
        })
        .collect();

    // Use real usage from provider when available, fall back to estimation.
    let usage = if has_real_usage {
        accumulated_usage.total_tokens =
            accumulated_usage.prompt_tokens + accumulated_usage.completion_tokens;
        accumulated_usage
    } else {
        // Fallback when provider sends no usage
        let counter = context_engine::best_token_counter();
        let est_input: u32 = messages
            .iter()
            .map(|m| match m {
                Message::System { content: c } => counter.estimate_text(c),
                Message::User { content: c } => match c {
                    providers::UserContent::Text(t) => counter.estimate_text(t),
                    providers::UserContent::MultiPart(parts) => parts
                        .iter()
                        .map(|p| match p {
                            providers::ContentPart::Text { text } => counter.estimate_text(text),
                            _ => 0,
                        })
                        .sum(),
                },
                Message::Assistant {
                    content: c,
                    reasoning_content: r,
                    ..
                } => {
                    c.as_deref().map_or(0, |t| counter.estimate_text(t))
                        + r.as_deref().map_or(0, |t| counter.estimate_text(t))
                }
                Message::Tool { content: c, .. } => {
                    counter.estimate_text(&c.as_text()) + c.image_part_count() * 1024
                }
                Message::ContextUpdate { content: c, .. } => counter.estimate_text(c),
            })
            .sum::<usize>() as u32;
        let est_output = counter.estimate_text(&content) as u32;
        Usage {
            prompt_tokens: est_input,
            completion_tokens: est_output,
            total_tokens: est_input + est_output,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        }
    };

    // Reasoning-model fallback: when streamed `content` is empty but the
    // model emitted `reasoning_content` (Mimo, deepseek-r1, qwq, glm-zero),
    // promote the reasoning to be the answer. Without this, finish_reason=
    // length truncations mid-thought produce silent empty output across the
    // whole agent. We must ALSO fan out a synthetic ContentChunk event for
    // any downstream consumer that scrapes the live event stream (the bench
    // harness, streaming UIs) since no ContentChunk was ever emitted for
    // reasoning chunks during the loop.
    //
    // Skip the fallback when tool_calls are present — the model already
    // expressed itself via the tool call, and duplicating the reasoning
    // into a synthetic ContentChunk would render the SAME text twice
    // (once as the italic Thinking block, once as plain answer body).
    let resolved_content = if content.trim().is_empty() {
        if reasoning.trim().is_empty() || !tool_calls.is_empty() {
            None
        } else {
            fan_out_event(
                Some(event_tx),
                domain_bus,
                crate::events::AgentEvent::ContentChunk {
                    data: reasoning.clone(),
                },
            )
            .await;
            Some(reasoning.clone())
        }
    } else {
        Some(content)
    };

    Ok(providers::LlmResponse {
        content: resolved_content,
        tool_calls,
        finish_reason,
        usage,
        reasoning_content: if reasoning.is_empty() {
            None
        } else {
            Some(reasoning)
        },
    })
}

/// Drives a single LLM-tool execution cycle.
///
/// Given a provider and tool registry, `run_cycle` sends messages to the LLM,
/// executes any requested tool calls in parallel (with per-tool timeout),
/// appends results to the message history, and returns the cycle outcome.
pub struct ExecutionCore {
    pub provider: DynProvider,
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
    pub outcome_recorder: Option<Arc<crate::learning::recorder::OutcomeRecorder>>,
    pub domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    pub interceptor_chain: Option<Arc<tools_core::InterceptorChain>>,
    pub approval_gate: Option<Arc<approval::ApprovalGate>>,
    token_counter: Arc<dyn TokenCounter>,
    tool_semaphore: Arc<Semaphore>,
}

impl ExecutionCore {
    pub fn new(provider: DynProvider, tool_registry: Arc<RwLock<ToolRegistry>>) -> Self {
        Self {
            provider,
            tool_registry,
            outcome_recorder: None,
            domain_event_bus: None,
            interceptor_chain: None,
            approval_gate: None,
            token_counter: Arc::new(context_engine::CharTokenCounter),
            tool_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_TOOLS)),
        }
    }

    /// Set the token counter for mid-loop compression.
    pub fn with_token_counter(mut self, counter: Arc<dyn TokenCounter>) -> Self {
        self.token_counter = counter;
        self
    }

    pub fn token_counter(&self) -> &Arc<dyn TokenCounter> {
        &self.token_counter
    }

    /// Set the domain event bus for publishing tool execution events.
    pub fn with_domain_bus(mut self, bus: Arc<bus::DomainEventBus>) -> Self {
        self.domain_event_bus = Some(bus);
        self
    }

    /// Set the outcome recorder for per-tool learning.
    pub fn with_outcome_recorder(
        mut self,
        recorder: Arc<crate::learning::recorder::OutcomeRecorder>,
    ) -> Self {
        self.outcome_recorder = Some(recorder);
        self
    }

    /// Set the interceptor chain for pre-execution validation hooks.
    ///
    /// Interceptors run after `prepare()` (which releases the registry read lock)
    /// and before `execute()`. They should validate tool arguments only — not
    /// rely on registry state, which may change between `prepare()` and `check()`.
    pub fn with_interceptor_chain(mut self, chain: Arc<tools_core::InterceptorChain>) -> Self {
        self.interceptor_chain = Some(chain);
        self
    }

    /// Set the approval gate for pre-execution permission checks.
    pub fn with_approval_gate(mut self, gate: Arc<approval::ApprovalGate>) -> Self {
        self.approval_gate = Some(gate);
        self
    }

    pub fn set_approval_suggester(&self, suggester: Arc<dyn approval::ApprovalSuggester>) {
        if let Some(ref gate) = self.approval_gate {
            gate.set_suggester(suggester);
        }
    }

    /// Run a single LLM-tool cycle:
    /// 1. Call provider.chat() with the current messages and tool definitions
    /// 2. If tool calls returned: execute all in parallel with timeout → ToolsExecuted
    /// 3. If text response: FinalResponse
    /// 4. Otherwise: EmptyResponse
    pub async fn run_cycle(
        &self,
        messages: &mut Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        routing_ctx: &RoutingContext,
        event_tx: Option<&tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
        seen_tool_calls: Option<&mut HashSet<String>>,
        cache_breakpoints: &[providers::CacheBreakpoint],
    ) -> Result<(CycleOutcome, Usage)> {
        // Pre-compute tool names once for fabrication detection.
        // Only meaningful in assistant mode; coding mode skips the check.
        let tool_names: Vec<&str> = tools.iter().filter_map(tool_def_name).collect();
        let in_coding = routing_ctx.channel.as_str() == common::tool_channel::CODING_CHANNEL;

        // When an event channel is available, stream tokens so the UI updates
        // in real-time. Non-streaming providers already have a default
        // `chat_stream()` impl that wraps `chat()` into a single-chunk stream,
        // so this works uniformly for all providers.

        // Build a never-cancelled token if no cancel_token was supplied so
        // call_provider_streaming always has a valid reference.
        let cancel_ref = params
            .cancel_token
            .clone()
            .unwrap_or_else(tokio_util::sync::CancellationToken::new);

        let response = if let Some(tx) = event_tx {
            call_provider_streaming(
                &*self.provider,
                messages,
                tools,
                &params.chat_params,
                tx,
                self.domain_event_bus.as_ref(),
                cache_breakpoints,
                &cancel_ref,
            )
            .await?
        } else {
            self.provider
                .chat(
                    messages,
                    Some(tools),
                    &params.chat_params,
                    cache_breakpoints,
                )
                .await?
        };

        let usage = response.usage.clone();

        if response.finish_reason == FINISH_REASON_CANCELLED {
            return Ok((
                CycleOutcome::Cancelled {
                    partial_content: response.content.clone().unwrap_or_default(),
                    partial_reasoning: response.reasoning_content.clone().unwrap_or_default(),
                },
                usage,
            ));
        }

        if usage.prompt_tokens > 0 {
            let hit_rate = usage.cache_read_tokens as f64 / usage.prompt_tokens as f64;
            tracing::info!(
                target: "klynt::execution::cache",
                cache_read = usage.cache_read_tokens,
                cache_write = usage.cache_write_tokens,
                prompt = usage.prompt_tokens,
                hit_rate,
                "cache hit ratio for this call"
            );
        }

        tracing::debug!(
            content_len = response.content.as_deref().map(|s| s.len()).unwrap_or(0),
            tool_calls = response.tool_calls.len(),
            "run_cycle: LLM response decoded"
        );

        if !response.tool_calls.is_empty() {
            debug!(
                "ExecutionCore: LLM returned {} tool calls: {:?}",
                response.tool_calls.len(),
                response
                    .tool_calls
                    .iter()
                    .map(|tc| &tc.name)
                    .collect::<Vec<_>>()
            );

            // ── Duplicate tool call prevention ─────────────────────
            // Only build signature keys when dedup tracking is enabled.
            let (current_keys, all_duplicates) = if seen_tool_calls.is_some() {
                let keys: Vec<String> = response
                    .tool_calls
                    .iter()
                    .map(|tc| {
                        let args_hash = hash_json_value(&tc.arguments);
                        format!("{}|{:x}", tc.name, args_hash)
                    })
                    .collect();
                let all_dup = !keys.is_empty()
                    && keys
                        .iter()
                        .all(|k| seen_tool_calls.as_ref().unwrap().contains(k));
                (Some(keys), all_dup)
            } else {
                (None, false)
            };

            if all_duplicates {
                debug!(
                    "ExecutionCore: skipping duplicate tool calls: {:?}",
                    response
                        .tool_calls
                        .iter()
                        .map(|tc| &tc.name)
                        .collect::<Vec<_>>()
                );

                // Append assistant message so the conversation stays coherent.
                // Carry reasoning_content forward — providers with extended
                // thinking enabled (Anthropic, Kimi compat, etc.) reject
                // follow-up requests that omit the original thinking block
                // alongside replayed tool_use messages.
                let tool_call_msgs = tool_calls_to_messages(&response.tool_calls);
                messages.push(Message::assistant_with_content_tools_and_reasoning(
                    response.content.clone(),
                    tool_call_msgs,
                    response.reasoning_content.clone(),
                ));

                // Append synthetic "already called" tool results
                let mut results = Vec::new();
                for tc in &response.tool_calls {
                    let skip_msg = format!(
                        "Skipped: '{}' was already called with identical arguments. \
                         The results are already in this conversation — use them instead of calling again.",
                        tc.name
                    );
                    messages.push(Message::tool(
                        tc.id.clone(),
                        tc.name.clone(),
                        skip_msg.clone(),
                    ));
                    results.push(ToolExecutionResult {
                        tool_call_id: tc.id.clone(),
                        tool_name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        result: skip_msg,
                        duration_ms: 0,
                        success: false,
                    });
                }

                return Ok((CycleOutcome::ToolsExecuted { results }, usage));
            }

            // Note: dedup registration is intentionally deferred until after
            // execution (see post-`join_all` block below). Registering before
            // execution made failed calls indistinguishable from successful
            // ones, so a transient `write` failure poisoned the dedup set and
            // the model could never retry with identical args. Now we register
            // only on success — failed calls remain retryable.

            // Append assistant message with tool calls (preserving any text
            // content AND reasoning_content for thinking-enabled providers).
            let tool_call_msgs = tool_calls_to_messages(&response.tool_calls);
            messages.push(Message::assistant_with_content_tools_and_reasoning(
                response.content.clone(),
                tool_call_msgs,
                response.reasoning_content.clone(),
            ));

            // Execute all tools in parallel, each with a timeout.
            // ToolStart/ToolEnd events are emitted inside each future so
            // the spinner updates in real-time as tools run.

            // Pre-resolve per-tool custom timeouts before spawning futures.
            // This acquires the registry read lock once, avoiding holding it
            // inside each async future.
            let custom_timeouts: Vec<Option<std::time::Duration>> = {
                let reg = self.tool_registry.read().await;
                response
                    .tool_calls
                    .iter()
                    .map(|tc| reg.get(&tc.name).and_then(|t| t.custom_timeout()))
                    .collect()
            };

            // Create entity card channel for this batch of tool calls
            let (entity_tx, mut entity_rx) = tokio::sync::mpsc::channel::<common::EntityCard>(16);

            // Clone once for the entire batch (not per-tool-call inside .map())
            let interceptor_ref = self.interceptor_chain.clone();

            let futures: Vec<_> = response
                .tool_calls
                .iter()
                .enumerate()
                .map(|(i, tc)| {
                    let registry = self.tool_registry.clone();
                    let semaphore = self.tool_semaphore.clone();
                    let interceptor_chain = interceptor_ref.clone();
                    let approval_gate = self.approval_gate.clone();
                    let name = tc.name.clone();
                    let args = tc.arguments.clone();
                    let mut ctx = routing_ctx.clone();
                    ctx.entity_tx = Some(entity_tx.clone());
                    ctx.cancel_token = params.cancel_token.clone();
                    let id = tc.id.clone();
                    let timeout_dur = if name == ASK_USER_TOOL_NAME {
                        LONG_RUNNING_TOOL_TIMEOUT
                    } else {
                        custom_timeouts[i].unwrap_or(params.tool_timeout)
                    };
                    let tx = event_tx.cloned();

                    async move {
                        // Pre-flight: look up tool and run permission checks before
                        // acquiring the semaphore so that interactive approval prompts
                        // don't starve concurrent tool slots.
                        let preflight = async {
                            let tool = {
                                let reg = registry.read().await;
                                reg.prepare(&name, &args, &ctx)?
                            };
                            if let Some(ref chain) = interceptor_chain {
                                chain.check(&name, &args, None).await?;
                            }
                            if let Some(ref gate) = approval_gate {
                                let req = approval::ApprovalRequest {
                                    tool_name: name.clone(),
                                    action: args
                                        .get("action")
                                        .and_then(|v| v.as_str())
                                        .map(String::from),
                                    args: args.clone(),
                                    class: tool.approval_class(&args),
                                    scope: tool.approval_scope(&args),
                                    ctx: approval::ApprovalContext {
                                        mode: ctx.session_mode,
                                        channel: approval::ChannelKind::from(ctx.channel.as_str()),
                                        session_id: ctx.chat_id.to_string(),
                                        user_id: None,
                                        cwd: std::env::current_dir()
                                            .unwrap_or_else(|_| std::path::PathBuf::from(".")),
                                    },
                                    preview: None,
                                    suggested_grant: None,
                                };
                                let approval_cancel_ref = ctx
                                    .cancel_token
                                    .clone()
                                    .unwrap_or_else(tokio_util::sync::CancellationToken::new);
                                match gate.check(req, &approval_cancel_ref).await? {
                                    approval::GateOutcome::Allow => {}
                                    approval::GateOutcome::Deny { reason } => {
                                        return Err(common::KlyntbotError::PermissionDenied(
                                            reason,
                                        ));
                                    }
                                    approval::GateOutcome::Cancel => {
                                        return Err(common::KlyntbotError::Cancelled(
                                            "user cancelled approval".into(),
                                        ));
                                    }
                                }
                            }
                            Ok(tool)
                        }
                        .await;

                        let args_snapshot = args.clone();

                        // Limit concurrent tool executions to prevent runaway parallelism.
                        let _permit = semaphore.acquire().await.expect("semaphore closed");

                        // Emit ToolStart BEFORE executing
                        fan_out_event(
                            tx.as_ref(),
                            self.domain_event_bus.as_ref(),
                            crate::events::AgentEvent::ToolStart {
                                name: name.clone(),
                                args: args.clone(),
                                agent: None,
                                call_id: Some(id.clone()),
                            },
                        )
                        .await;

                        let start = Instant::now();
                        let exec_result = match preflight {
                            Ok(tool) => {
                                tokio::time::timeout(timeout_dur, async {
                                    tool.execute(args, &ctx).await
                                })
                                .await
                            }
                            Err(e) => Ok(Err(e)),
                        };
                        let duration_ms = start.elapsed().as_millis() as u64;

                        let (result_str, success) = match exec_result {
                            Ok(Ok(s)) => (s, true),
                            Ok(Err(e)) => (format!("Error: {}", e), false),
                            Err(_) => (
                                format!(
                                    "Tool '{}' timed out after {}s",
                                    name,
                                    timeout_dur.as_secs()
                                ),
                                false,
                            ),
                        };

                        // Emit ToolEnd AFTER executing
                        // Truncate result to 2KB to avoid huge WebSocket payloads
                        let truncated = Some(klynt_truncation::truncate_text(
                            &result_str,
                            klynt_truncation::TruncationPolicy::Bytes(2048),
                        ));
                        fan_out_event(
                            tx.as_ref(),
                            self.domain_event_bus.as_ref(),
                            crate::events::AgentEvent::ToolEnd {
                                name: name.clone(),
                                success,
                                duration_ms,
                                result: truncated,
                                agent: None,
                                call_id: Some(id.clone()),
                            },
                        )
                        .await;

                        ToolExecutionResult {
                            tool_call_id: id,
                            tool_name: name,
                            arguments: args_snapshot,
                            result: result_str,
                            duration_ms,
                            success,
                        }
                    }
                })
                .collect();

            drop(entity_tx);

            let results = join_all(futures).await;

            // Register dedup keys for *successful* tool calls only. A failed
            // call (e.g. `write` to a non-existent dir) must remain retryable
            // with identical args. `current_keys` and `results` are both
            // index-aligned with `response.tool_calls`, so the zip is 1:1.
            if let Some(seen) = seen_tool_calls {
                if let Some(keys) = current_keys.as_ref() {
                    for (key, r) in keys.iter().zip(results.iter()) {
                        if r.success {
                            seen.insert(key.clone());
                        }
                    }
                }
            }

            // Emit EntityCreated events for any entities tools created
            while let Ok(card) = entity_rx.try_recv() {
                fan_out_event(
                    event_tx,
                    self.domain_event_bus.as_ref(),
                    crate::events::AgentEvent::EntityCreated(card),
                )
                .await;
            }

            // Record tool outcomes for learning (best-effort)
            if let Some(ref recorder) = self.outcome_recorder {
                let session_key = common::SessionKey::from_parts(
                    routing_ctx.channel.as_str(),
                    routing_ctx.chat_id.as_str(),
                );
                for r in &results {
                    let error_cat = if r.success {
                        None
                    } else {
                        Some(crate::learning::recorder::categorize_error(&r.result))
                    };
                    recorder
                        .record_tool_outcome(
                            &r.tool_name,
                            r.success,
                            error_cat,
                            r.duration_ms,
                            None,
                            session_key.as_str(),
                        )
                        .await;
                }
            }

            // Publish tool execution events to domain bus for unified activity log
            if let Some(ref domain_bus) = self.domain_event_bus {
                for r in &results {
                    domain_bus.publish(bus::DomainEvent::ToolCallExecuted {
                        tool_name: r.tool_name.clone(),
                        args_preview: {
                            let full = r.arguments.to_string();
                            Some(if full.len() > 200 {
                                let end = full.floor_char_boundary(200);
                                format!("{}...", &full[..end])
                            } else {
                                full
                            })
                        },
                        session_key: Some(
                            common::SessionKey::from_parts(
                                routing_ctx.channel.as_str(),
                                routing_ctx.chat_id.as_str(),
                            )
                            .to_string(),
                        ),
                        duration_ms: Some(r.duration_ms as i64),
                    });
                }
            }

            // Append tool result messages (sanitized to strip control chars / cap length)
            for r in &results {
                messages.push(Message::tool(
                    r.tool_call_id.clone(),
                    r.tool_name.clone(),
                    sanitize_tool_result(&r.result),
                ));
            }

            return Ok((CycleOutcome::ToolsExecuted { results }, usage));
        }

        // No tool calls — check for text content.
        tracing::debug!("run_cycle: no tool calls, checking content");
        if let Some(content) = response.content {
            if !content.trim().is_empty() {
                if Self::check_fabrication(&content, &tool_names, in_coding) {
                    tracing::debug!("run_cycle: returning FabricatedResponse");
                    return Ok((CycleOutcome::FabricatedResponse { content }, usage));
                }

                tracing::debug!(
                    content_len = content.len(),
                    "run_cycle: returning FinalResponse"
                );
                return Ok((CycleOutcome::FinalResponse { content }, usage));
            }
        }

        tracing::debug!("run_cycle: returning EmptyResponse");
        Ok((CycleOutcome::EmptyResponse, usage))
    }

    /// Returns true if the content looks like a fabricated tool response.
    /// Skipped in coding mode where real tool execution is used.
    fn check_fabrication(content: &str, tool_names: &[&str], in_coding: bool) -> bool {
        if in_coding || tool_names.is_empty() {
            return false;
        }
        is_fabricated_tool_response(content, tool_names)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::MockProvider;
    use async_trait::async_trait;
    use providers::{LlmResponse, ToolCall, Usage};
    use serde_json::Value;
    use std::time::Duration;
    use tools::Tool;

    // ── Mock tool (fast) ─────────────────────────────────────────

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn name(&self) -> &str {
            "echo"
        }
        fn description(&self) -> &str {
            "Echoes input"
        }
        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, args: Value, _ctx: &RoutingContext) -> common::Result<String> {
            Ok(format!("echoed: {}", args))
        }
    }

    // ── Mock tool (slow, for timeout test) ───────────────────────

    struct SlowTool;

    #[async_trait]
    impl Tool for SlowTool {
        fn name(&self) -> &str {
            "slow_tool"
        }
        fn description(&self) -> &str {
            "Sleeps forever"
        }
        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> common::Result<String> {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok("should not reach".to_string())
        }
    }

    // ── Helpers ──────────────────────────────────────────────────

    fn make_registry_with(tool: impl Tool + 'static) -> Arc<RwLock<ToolRegistry>> {
        let mut reg = ToolRegistry::new();
        reg.register(tool);
        Arc::new(RwLock::new(reg))
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    // ── Tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_cycle_final_response() {
        let provider = Arc::new(MockProvider::with_text("Hello world"));
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("hi")];
        let params = ExecutionParams::new("mock", 128_000);
        let tools = vec![];

        let (outcome, _usage) = core
            .run_cycle(
                &mut messages,
                &tools,
                &params,
                &routing_ctx(),
                None,
                None,
                &[],
            )
            .await
            .unwrap();

        match outcome {
            CycleOutcome::FinalResponse { content } => {
                assert_eq!(content, "Hello world");
            }
            other => panic!("Expected FinalResponse, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_cycle_tool_execution() {
        let provider = Arc::new(MockProvider::with_tool_call(
            "echo",
            serde_json::json!({"msg": "test"}),
        ));
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("use echo")];
        let params = ExecutionParams::new("mock", 128_000);
        let tools = vec![];

        let (outcome, _usage) = core
            .run_cycle(
                &mut messages,
                &tools,
                &params,
                &routing_ctx(),
                None,
                None,
                &[],
            )
            .await
            .unwrap();

        match outcome {
            CycleOutcome::ToolsExecuted { results } => {
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].tool_name, "echo");
                assert!(results[0].success);
                assert!(results[0].result.contains("echoed"));
            }
            other => panic!("Expected ToolsExecuted, got {:?}", other),
        }

        // Messages should have assistant + tool result appended
        assert!(messages.len() >= 3); // user + assistant(tool_calls) + tool result
    }

    #[tokio::test]
    async fn test_tool_timeout() {
        let provider = Arc::new(MockProvider::with_tool_call(
            "slow_tool",
            serde_json::json!({}),
        ));
        let registry = make_registry_with(SlowTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("run slow")];
        let params = ExecutionParams::new("mock", 128_000).with_timeout(Duration::from_millis(100));
        let tools = vec![];

        let (outcome, _usage) = core
            .run_cycle(
                &mut messages,
                &tools,
                &params,
                &routing_ctx(),
                None,
                None,
                &[],
            )
            .await
            .unwrap();

        match outcome {
            CycleOutcome::ToolsExecuted { results } => {
                assert_eq!(results.len(), 1);
                assert!(!results[0].success);
                assert!(results[0].result.contains("timed out"));
            }
            other => panic!("Expected ToolsExecuted with timeout, got {:?}", other),
        }
    }

    // ── Fabrication detection tests ─────────────────────────────

    #[test]
    fn test_detects_fabricated_todo_response() {
        let tool_names = vec!["todo", "calendar", "web_search"];
        let fabricated = "I've created the task for you:\n\n**Task Created:** Buy groceries (ID: 9c4e5f3b)\n- **Description:** Weekly shopping\n- **Priority:** P3 (Medium)\n- **Due Date:** Tomorrow";
        assert!(is_fabricated_tool_response(fabricated, &tool_names));
    }

    #[test]
    fn test_detects_fabricated_search_response() {
        let tool_names = vec!["todo", "calendar", "web_search"];
        // Search fabrication with fake ID for corroboration
        let fabricated = "I searched the web for you (ID: abcdef12) and found these results:\n1. Rust programming language\n2. Rust game";
        assert!(is_fabricated_tool_response(fabricated, &tool_names));
    }

    #[test]
    fn test_does_not_flag_normal_response() {
        let tool_names = vec!["todo", "calendar", "web_search"];
        let normal = "Sure! I'd be happy to help you create a task. What would you like the task to be about?";
        assert!(!is_fabricated_tool_response(normal, &tool_names));
    }

    #[test]
    fn test_does_not_flag_explanation_about_tools() {
        let tool_names = vec!["todo", "calendar", "web_search"];
        let explanation = "I have access to a todo tool that can help you manage tasks. Would you like me to create one?";
        assert!(!is_fabricated_tool_response(explanation, &tool_names));
    }

    #[test]
    fn test_detects_fabricated_with_fake_id() {
        let tool_names = vec!["todo"];
        let fabricated = "Task added! ID: a1b2c3d4. Title: Buy milk. Priority: High.";
        assert!(is_fabricated_tool_response(fabricated, &tool_names));
    }

    #[test]
    fn test_no_tools_means_no_fabrication() {
        let tool_names: Vec<&str> = vec![];
        let text = "Task Created: Buy groceries (ID: 9c4e5f3b)";
        assert!(!is_fabricated_tool_response(text, &tool_names));
    }

    #[test]
    fn test_task_fabrication_not_flagged_without_todo_tool() {
        // "web_search" is available but NOT "todo" — task patterns should NOT trigger
        let tool_names = vec!["web_search", "calendar"];
        let fabricated = "I've created the task for you:\n\n**Task Created:** Buy groceries (ID: 9c4e5f3b)\n- **Description:** Weekly shopping\n- **Priority:** P3 (Medium)\n- **Due Date:** Tomorrow";
        assert!(!is_fabricated_tool_response(fabricated, &tool_names));
    }

    #[test]
    fn test_search_fabrication_not_flagged_without_search_tool() {
        // "todo" is available but NOT "web_search" — search patterns should NOT trigger
        let tool_names = vec!["todo", "calendar"];
        let fabricated = "I searched the web for you and found these results:\n1. Rust programming language\n2. Rust game";
        assert!(!is_fabricated_tool_response(fabricated, &tool_names));
    }

    #[test]
    fn test_calendar_fabrication_not_flagged_without_calendar_tool() {
        // Only "todo" available — calendar patterns should NOT trigger
        let tool_names = vec!["todo"];
        let fabricated =
            "Event created: Team meeting (ID: abcdef12)\n- Priority: High\n- Due Date: Tomorrow";
        assert!(!is_fabricated_tool_response(fabricated, &tool_names));
    }

    #[test]
    fn test_search_with_list_requires_fake_id() {
        // Search with numbered list but NO fake ID — should NOT trigger
        let tool_names = vec!["web_search"];
        let text = "I searched the web for you and found these results:\n1. Rust programming language\n2. Rust game";
        assert!(!is_fabricated_tool_response(text, &tool_names));
    }

    #[test]
    fn test_search_with_list_and_fake_id_triggers() {
        // Search with numbered list AND fake ID — should trigger
        let tool_names = vec!["web_search"];
        let text = "I searched the web for you (ID: abcdef12) and found these results:\n1. Rust programming language\n2. Rust game";
        assert!(is_fabricated_tool_response(text, &tool_names));
    }

    #[tokio::test]
    async fn test_cycle_empty_response() {
        let provider = Arc::new(MockProvider::with_response(LlmResponse {
            content: Some("".to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }));
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("empty")];
        let params = ExecutionParams::new("mock", 128_000);
        let tools = vec![];

        let (outcome, _usage) = core
            .run_cycle(
                &mut messages,
                &tools,
                &params,
                &routing_ctx(),
                None,
                None,
                &[],
            )
            .await
            .unwrap();

        assert!(matches!(outcome, CycleOutcome::EmptyResponse));
    }

    // ── Fabrication detection integration tests ──────────────────

    #[tokio::test]
    async fn test_cycle_detects_fabricated_response() {
        // Provider returns text that looks like a fabricated todo result
        let provider = Arc::new(MockProvider::with_response(LlmResponse {
            content: Some(
                "I've created the task for you:\n\n**Task Created:** Buy groceries (ID: 9c4e5f3b)\n- **Priority:** P3\n- **Due Date:** Tomorrow".to_string()
            ),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }));
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("create task: buy")];
        let params = ExecutionParams::new("mock", 128_000);
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "todo",
                "description": "Manage tasks",
                "parameters": {"type": "object", "properties": {}}
            }
        })];

        let (outcome, _usage) = core
            .run_cycle(
                &mut messages,
                &tools,
                &params,
                &routing_ctx(),
                None,
                None,
                &[],
            )
            .await
            .unwrap();

        assert!(matches!(outcome, CycleOutcome::FabricatedResponse { .. }));
    }

    #[tokio::test]
    async fn test_cycle_normal_text_not_flagged() {
        let provider = Arc::new(MockProvider::with_text(
            "Sure, I can help you create a task. What would you like?",
        ));
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("create task: buy")];
        let params = ExecutionParams::new("mock", 128_000);
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "todo",
                "description": "Manage tasks",
                "parameters": {"type": "object", "properties": {}}
            }
        })];

        let (outcome, _usage) = core
            .run_cycle(
                &mut messages,
                &tools,
                &params,
                &routing_ctx(),
                None,
                None,
                &[],
            )
            .await
            .unwrap();

        assert!(matches!(outcome, CycleOutcome::FinalResponse { .. }));
    }

    // ── Hash-based dedup key tests ──────────────────────────────

    #[test]
    fn test_hash_json_value_same_values_same_hash() {
        let v1 = serde_json::json!({"action": "search", "query": "rust"});
        let v2 = serde_json::json!({"action": "search", "query": "rust"});
        assert_eq!(hash_json_value(&v1), hash_json_value(&v2));
    }

    #[test]
    fn test_hash_json_value_different_values_different_hash() {
        let v1 = serde_json::json!({"action": "search"});
        let v2 = serde_json::json!({"action": "add"});
        assert_ne!(hash_json_value(&v1), hash_json_value(&v2));
    }

    #[test]
    fn test_hash_json_value_handles_nested() {
        let v1 = serde_json::json!({"a": {"b": [1, 2, 3]}, "c": true});
        let v2 = serde_json::json!({"a": {"b": [1, 2, 3]}, "c": true});
        assert_eq!(hash_json_value(&v1), hash_json_value(&v2));
    }

    #[test]
    fn test_hash_json_value_null_vs_empty() {
        let null = serde_json::json!(null);
        let empty_obj = serde_json::json!({});
        assert_ne!(hash_json_value(&null), hash_json_value(&empty_obj));
    }

    #[tokio::test]
    async fn test_content_preserved_with_tool_calls() {
        // Provider returns BOTH content and tool calls
        let provider = Arc::new(MockProvider::with_response(LlmResponse {
            content: Some("Let me search for that...".to_string()),
            tool_calls: vec![ToolCall {
                id: "call_1".to_string(),
                name: "echo".to_string(),
                arguments: serde_json::json!({}),
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }));
        let registry = make_registry_with(EchoTool);
        let core = ExecutionCore::new(provider, registry);

        let mut messages = vec![Message::user("search")];
        let params = ExecutionParams::new("mock", 128_000);
        let tools = vec![];

        let (outcome, _usage) = core
            .run_cycle(
                &mut messages,
                &tools,
                &params,
                &routing_ctx(),
                None,
                None,
                &[],
            )
            .await
            .unwrap();

        assert!(matches!(outcome, CycleOutcome::ToolsExecuted { .. }));

        // The assistant message should contain BOTH content and tool calls
        let assistant_msg = &messages[1]; // user + assistant
        match assistant_msg {
            Message::Assistant {
                content,
                tool_calls,
                ..
            } => {
                assert!(content.is_some(), "Content should be preserved");
                assert_eq!(content.as_deref(), Some("Let me search for that..."));
                assert!(tool_calls.is_some(), "Tool calls should be present");
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    // ── sanitize_tool_result tests ───────────────────────────────

    #[test]
    fn sanitize_tool_result_strips_control_chars() {
        let input = "Normal text\x00\x01\x02with control chars\nand newlines";
        let result = sanitize_tool_result(input);
        assert!(!result.contains('\x00'));
        assert!(!result.contains('\x01'));
        assert!(result.contains('\n'));
        assert!(result.contains("Normal text"));
    }

    #[test]
    fn sanitize_tool_result_caps_length() {
        let long = "x".repeat(200_000);
        let result = sanitize_tool_result(&long);
        assert!(
            result.len() <= MAX_TOOL_RESULT_LENGTH + 200,
            "result.len() = {}",
            result.len()
        );
    }

    #[test]
    fn sanitize_tool_result_preserves_normal_content() {
        let input = "Task created: 'Buy groceries' with priority High";
        assert_eq!(sanitize_tool_result(input), input);
    }
}
