# Agent Architecture Gap Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the 6 architectural gaps identified in the agent system audit — Anthropic system message loss, missing pipeline timeout, streaming token estimation, tool result sanitization, unused `triggers` field, and static MCP whitelist.

**Architecture:** Each fix is a targeted, isolated change to a specific crate. Fixes are ordered by criticality (critical → high → medium). All changes maintain backward compatibility and follow existing patterns.

**Tech Stack:** Rust, tokio, serde, Anthropic Messages API, SQLite

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/providers/src/adapters/anthropic_native.rs` | Modify | Fix `extract_system_prompt` to collect all system messages into content block array |
| `crates/providers/src/adapters/anthropic_native.rs` (tests) | Modify | Test multi-system-message merging |
| `crates/common/src/error.rs` | Modify | Add `Timeout(String)` variant to `KlyntbotError` |
| `crates/agent/src/execution/types.rs` | Modify | Add `pipeline_timeout: Option<Duration>` field |
| `crates/agent/src/intent_pipeline/types.rs` | Modify | Add `pipeline_timeout_secs` to `PipelineConfig` |
| `crates/agent/src/agent_runtime/runtime.rs` | Modify | Wrap `router.execute()` in `tokio::time::timeout` |
| `crates/config/src/schema/agents.rs` | Modify | Add `pipeline_timeout_secs` to `AgentDefaults` |
| `crates/agent/src/execution/core.rs` | Modify | Use `best_token_counter()` instead of `CharTokenCounter`; preserve streaming `usage` from final chunk |
| `crates/providers/src/types.rs` | Modify | Add `usage: Option<Usage>` to `LlmStreamChunk` |
| `crates/providers/src/adapters/anthropic_native.rs` (streaming) | Modify | Parse `message_delta.usage` from Anthropic SSE |
| `crates/providers/src/adapters/openai_compat.rs` (streaming) | Modify | Parse `usage` from final OpenAI SSE chunk |
| `crates/agent/src/execution/core.rs` (sanitization) | Modify | Add `sanitize_tool_result()` to filter tool outputs before message injection |
| `crates/skill-system/src/parser.rs` | Modify | Add `triggers: Vec<String>` to `RawKlyntbotMeta` |
| `crates/skill-system/src/types.rs` | Modify | Add `triggers: Vec<String>` to `KlyntbotMeta` |
| `crates/skill-system/src/router.rs` | Modify | Add trigger-based pre-check to `select_orchestrator_blended()` |
| `crates/klyntbot-server/src/bridge/registry.rs` | Modify | Replace static `HashSet` with dynamic registry read for whitelist |

---

### Task 1: Fix Anthropic Multi-System-Message Loss (CRITICAL)

**Context:** `ContextEngine::assemble()` produces 2–4 `Message::System` entries (base prompt, memory, summaries, inventory). `AnthropicNativeProvider::extract_system_prompt()` uses `find_map` and returns only the **first** one. Memory retrieval, history summaries, and inventory are silently dropped for all Anthropic model calls.

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs:76-82` (extract_system_prompt)
- Modify: `crates/providers/src/adapters/anthropic_native.rs:370-381` (build_request system field)
- Modify: `crates/providers/src/adapters/anthropic_native.rs:590-600` (count_tokens)

- [ ] **Step 1: Write test for multi-system-message extraction**

In the existing test module of `anthropic_native.rs`, add:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p providers -- extract_system_prompts`
Expected: FAIL — method `extract_system_prompts` does not exist (only `extract_system_prompt` singular)

- [ ] **Step 3: Replace `extract_system_prompt` with `extract_system_prompts`**

Replace the method at L76-82:

```rust
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
```

- [ ] **Step 4: Update `build_request` to use the new method**

Replace the system prompt injection block at L370-381.

**Important:** Anthropic's prompt caching caches everything up to and including the block with `cache_control`. The **last** system block is the optimal cache boundary (all blocks before it are included). Applying it to the first block would cache only the base prompt, missing memory/summaries.

Also update the docstring on `convert_messages` (L84-88) to reference `extract_system_prompts` (plural).

```rust
// System prompt — collect all system messages into content block array.
// Anthropic's API accepts `system` as an array of content blocks.
let system_prompts = Self::extract_system_prompts(messages);
if !system_prompts.is_empty() {
    let last_idx = system_prompts.len() - 1;
    if self.cache_system_prompt {
        let blocks: Vec<Value> = system_prompts
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let mut block = json!({"type": "text", "text": text});
                // Apply cache_control to the LAST block — Anthropic caches
                // everything up to and including this block.
                if i == last_idx {
                    block["cache_control"] = json!({"type": "ephemeral"});
                }
                block
            })
            .collect();
        body["system"] = json!(blocks);
    } else {
        let blocks: Vec<Value> = system_prompts
            .iter()
            .map(|text| json!({"type": "text", "text": text}))
            .collect();
        body["system"] = json!(blocks);
    }
}

// Note: apply_response_format() at L327 calls body["system"].as_array_mut()
// which now always finds an array — this is correct since we always use
// content block array format.
```

- [ ] **Step 5: Update `count_tokens` to use the new method**

Replace L598-600:

```rust
let system_prompts = Self::extract_system_prompts(messages);
if !system_prompts.is_empty() {
    let blocks: Vec<Value> = system_prompts
        .iter()
        .map(|text| json!({"type": "text", "text": text}))
        .collect();
    body["system"] = json!(blocks);
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo nextest run -p providers`
Expected: All tests pass, including the new `extract_system_prompts_*` tests

- [ ] **Step 7: Run full workspace build check**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings. Confirm no other code references `extract_system_prompt` (singular).

- [ ] **Step 8: Commit**

```bash
git add crates/providers/src/adapters/anthropic_native.rs
git commit -m "fix(providers): collect all system messages for Anthropic API calls

extract_system_prompt used find_map which took only the first System message.
Memory retrieval, history summaries, and inventory messages were silently
dropped. Now collects all into a content block array per Anthropic's API spec."
```

---

### Task 2: Add Pipeline-Level Timeout (HIGH)

**Context:** No wall-clock timeout wraps the execution pipeline. `CancellationToken` is only polled at iteration boundaries (not mid-LLM-call). Theoretical worst case: 30s/tool × 10 parallel × 30 iterations = ~15 minutes with no abort. This adds a configurable pipeline timeout using `tokio::time::timeout` around `router.execute()`.

**Files:**
- Modify: `crates/common/src/error.rs` (add Timeout variant)
- Modify: `crates/config/src/schema/agents.rs:27-50` (AgentDefaults)
- Modify: `crates/agent/src/execution/types.rs:9-36` (ExecutionParams)
- Modify: `crates/agent/src/intent_pipeline/types.rs:134-149` (PipelineConfig)
- Modify: `crates/agent/src/agent_runtime/runtime.rs:523-546` (process_message)

**Important context:** `AgentRuntime` holds `self.config: PipelineConfig` (defined at `crates/agent/src/intent_pipeline/types.rs:134`), **not** the top-level `config::Config`. The timeout value must be threaded through `PipelineConfig`, not accessed via `self.config.agents.defaults`.

- [ ] **Step 1: Write test for pipeline timeout**

In `crates/agent/src/execution/types.rs` test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_params_pipeline_timeout_builder() {
        let params = ExecutionParams::new("test-model")
            .with_pipeline_timeout(Duration::from_secs(120));
        assert_eq!(params.pipeline_timeout, Some(Duration::from_secs(120)));
    }

    #[test]
    fn execution_params_default_no_pipeline_timeout() {
        let params = ExecutionParams::new("test-model");
        assert!(params.pipeline_timeout.is_none());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -- execution_params_pipeline_timeout`
Expected: FAIL — `pipeline_timeout` field and `with_pipeline_timeout` don't exist

- [ ] **Step 3: Add `Timeout` variant to `KlyntbotError`**

In `crates/common/src/error.rs`, add to the `KlyntbotError` enum:

```rust
#[error("Timeout: {0}")]
Timeout(String),
```

- [ ] **Step 4: Add `pipeline_timeout_secs` to config**

In `crates/config/src/schema/agents.rs`, add to `AgentDefaults`:

```rust
/// Maximum wall-clock time for a single pipeline execution (seconds).
/// Covers the entire router.execute() call including all iterations.
/// Default: 300 (5 minutes). Set to 0 to disable.
#[serde(default = "default_pipeline_timeout_secs")]
pub pipeline_timeout_secs: u64,
```

Add the default function and update the `Default` impl:

```rust
fn default_pipeline_timeout_secs() -> u64 {
    300
}
```

- [ ] **Step 5: Add `pipeline_timeout` to `PipelineConfig`**

In `crates/agent/src/intent_pipeline/types.rs`, add to `PipelineConfig`:

```rust
pub struct PipelineConfig {
    // ... existing fields ...
    /// Pipeline-level wall-clock timeout (0 = disabled).
    pub pipeline_timeout_secs: u64,
}
```

Update `PipelineConfig::default()` to include `pipeline_timeout_secs: 300`.

Update wherever `PipelineConfig` is constructed (search for `PipelineConfig {` in the builder) to pass through `config.agents.defaults.pipeline_timeout_secs`.

- [ ] **Step 6: Add `pipeline_timeout` field to `ExecutionParams`**

In `crates/agent/src/execution/types.rs`, add field and builder:

```rust
pub struct ExecutionParams {
    // ... existing fields ...
    /// Wall-clock timeout for the entire pipeline execution.
    pub pipeline_timeout: Option<Duration>,
}

impl ExecutionParams {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            // ... existing defaults ...
            pipeline_timeout: None,
        }
    }

    pub fn with_pipeline_timeout(mut self, dur: Duration) -> Self {
        self.pipeline_timeout = Some(dur);
        self
    }
}
```

- [ ] **Step 7: Wire pipeline timeout in `runtime.rs`**

In `crates/agent/src/agent_runtime/runtime.rs`, after building `ExecutionParams` at ~L523:

```rust
// Pipeline timeout from config (PipelineConfig, not top-level Config)
let timeout_secs = self.config.pipeline_timeout_secs;
if timeout_secs > 0 {
    params = params.with_pipeline_timeout(Duration::from_secs(timeout_secs));
}
```

Then wrap the `router.execute()` call at ~L536 with `tokio::time::timeout`:

```rust
let pipeline_future = self.router.execute(
    analysis.mode.clone(),
    assembled.messages,
    &filtered_tools,
    &params,
    ctx,
    event_tx.clone(),
);

let router_result = if let Some(timeout_dur) = params.pipeline_timeout {
    match tokio::time::timeout(timeout_dur, pipeline_future).await {
        Ok(result) => result?,
        Err(_) => {
            warn!("Pipeline execution timed out after {:?}", timeout_dur);
            if let Some(ref tx) = event_tx {
                let _ = tx.send(crate::events::AgentEvent::Error {
                    message: format!("Execution timed out after {}s", timeout_dur.as_secs()),
                }).await;
            }
            return Err(common::KlyntbotError::Timeout(format!(
                "Pipeline execution exceeded {}s limit",
                timeout_dur.as_secs()
            )));
        }
    }
} else {
    pipeline_future.await?
};
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo nextest run -p agent -- execution_params_pipeline_timeout`
Expected: PASS

- [ ] **Step 9: Run full build check**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 10: Commit**

```bash
git add crates/common/src/error.rs crates/config/src/schema/agents.rs crates/agent/src/execution/types.rs crates/agent/src/intent_pipeline/types.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): add configurable pipeline-level timeout

Wraps router.execute() in tokio::time::timeout. Defaults to 300s (5min).
Prevents unbounded execution when LLM calls or tool chains run long.
Adds Timeout variant to KlyntbotError. Threads config through PipelineConfig.
Configurable via config.agents.defaults.pipelineTimeoutSecs."
```

---

### Task 3: Fix Streaming Token Estimation (HIGH)

**Context:** Streaming responses use `CharTokenCounter` (4 chars ≈ 1 token), always zero-out cache tokens, and ignore actual `usage` data that providers send in the final SSE chunk. Non-streaming paths get real `Usage` from the API. This means cost tracking, budget warnings, and strategy recording are all inaccurate for streaming calls.

**Files:**
- Modify: `crates/providers/src/types.rs` (LlmStreamChunk — add usage field)
- Modify: `crates/providers/src/adapters/anthropic_native.rs` (parse message_delta.usage)
- Modify: `crates/providers/src/adapters/openai_compat.rs` (parse final chunk usage)
- Modify: `crates/agent/src/execution/core.rs:156-280` (call_provider_streaming — prefer real usage)

- [ ] **Step 1: Write test for LlmStreamChunk with usage**

In `crates/providers/src/types.rs` test module:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p providers -- stream_chunk_with_usage`
Expected: FAIL — `usage` field doesn't exist on `LlmStreamChunk`

- [ ] **Step 3: Add `usage` field to `LlmStreamChunk`**

In `crates/providers/src/types.rs`, add the field to the existing struct. **Keep all existing fields including `is_final: bool`:**

```rust
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
    /// Accumulated usage stats from the provider (populated across multiple SSE events).
    pub usage: Option<Usage>,
}
```

Update all sites that construct `LlmStreamChunk` to include `usage: None`. Search for `LlmStreamChunk {` across:
- `anthropic_native.rs` — ~6 construction sites in `parse_anthropic_sse()`
- `openai_compat.rs` — 1 construction site in `parse_sse_chunk()`
- `types.rs` — 1 in the default `chat_stream` fallback impl

- [ ] **Step 4: Parse usage from Anthropic streaming events**

**Key insight:** Anthropic streaming sends usage across TWO different SSE events:
- `message_start` → contains `usage.input_tokens`, `usage.cache_read_input_tokens`, `usage.cache_creation_input_tokens`
- `message_delta` → contains `usage.output_tokens` only

Currently `message_start` returns `Ok(None)` (the catch-all branch at L311). We need to emit a chunk from `message_start` to capture input/cache tokens, and extend `message_delta` to capture output tokens.

**Step 4a: Handle `message_start` (input + cache tokens)**

In `parse_anthropic_sse()`, add a new match arm BEFORE the catch-all `_ => Ok(None)`:

```rust
"message_start" => {
    // Extract input token counts (including cache) from the initial message event
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
                completion_tokens: 0, // not available yet
                total_tokens: input_tokens as u32,
                cache_read_tokens: usage_val
                    .get("cache_read_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                cache_write_tokens: usage_val
                    .get("cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            }),
        }))
    } else {
        Ok(None)
    }
}
```

**Step 4b: Handle `message_delta` (output tokens)**

In the existing `"message_delta"` arm (L283-299), add usage extraction:

```rust
"message_delta" => {
    let stop_reason = value["delta"]["stop_reason"].as_str();
    let finish_reason = match stop_reason {
        Some("end_turn") => Some("stop".to_string()),
        Some("tool_use") => Some("tool_calls".to_string()),
        Some("max_tokens") => Some("length".to_string()),
        Some(other) => Some(other.to_string()),
        None => None,
    };
    // message_delta carries output_tokens only
    let usage = value.get("usage").and_then(|u| {
        u.get("output_tokens").and_then(|v| v.as_u64()).map(|output| Usage {
            prompt_tokens: 0, // already captured in message_start
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
```

**Note:** The consumer in `call_provider_streaming` must **merge** usage from both events (add `prompt_tokens` from `message_start` to `completion_tokens` from `message_delta`). See Step 6.

- [ ] **Step 5: Parse usage from OpenAI streaming + enable `stream_options`**

**Step 5a:** In `openai_compat.rs` `build_request_body()`, when `stream` is true, add:

```rust
if stream {
    body["stream"] = json!(true);
    body["stream_options"] = json!({"include_usage": true});
}
```

**Step 5b:** In `parse_sse_chunk()`, after building the `LlmStreamChunk`, extract usage from the top-level `value`:

```rust
// OpenAI sends usage in a final chunk with choices=[] when stream_options.include_usage is true
let usage = value.get("usage").and_then(|u| {
    Some(Usage {
        prompt_tokens: u.get("prompt_tokens")?.as_u64()? as u32,
        completion_tokens: u.get("completion_tokens")?.as_u64()? as u32,
        total_tokens: u.get("total_tokens")?.as_u64()? as u32,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    })
});
```

**Step 5c:** Handle the usage-only final chunk. OpenAI sends a chunk with `choices: []` and `usage: {...}` at the very end. The current parser returns `None` when `choices` is empty (L231-233). Add a check before the early return:

```rust
if choices.is_empty() {
    // OpenAI sends a final chunk with usage and empty choices
    let usage = value.get("usage").and_then(|u| {
        Some(Usage {
            prompt_tokens: u.get("prompt_tokens")?.as_u64()? as u32,
            completion_tokens: u.get("completion_tokens")?.as_u64()? as u32,
            total_tokens: u.get("total_tokens")?.as_u64()? as u32,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        })
    });
    if usage.is_some() {
        return Ok(Some(LlmStreamChunk {
            content: None,
            tool_call_delta: None,
            is_final: true,
            finish_reason: None,
            reasoning_content: None,
            usage,
        }));
    }
    return Ok(None);
}
```

- [ ] **Step 6: Update `call_provider_streaming` to merge real usage from chunks**

In `crates/agent/src/execution/core.rs`, modify `call_provider_streaming` (L156-280).

**Important:** Anthropic sends usage across 2 SSE events (`message_start` for input, `message_delta` for output). We must **merge** usage objects, not just take the last one. Also, `content` is a local `String` used both during streaming and in the final `LlmResponse` — avoid shadowing.

At the top of the function (after existing `let mut` declarations at L165-168), add:

```rust
let mut accumulated_usage = Usage::default();
let mut has_real_usage = false;
```

Inside the `while let Some(result) = stream.next().await` loop, after existing processing (after `finish_reason` handling at L208-209), add:

```rust
if let Some(chunk_usage) = chunk.usage {
    has_real_usage = true;
    // Merge: add non-zero fields (Anthropic splits across events)
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
```

**Remove the entire estimation block** (L226-L273 — the `let counter = context_engine::CharTokenCounter` through the `Usage { ... }` struct). Replace with:

```rust
let usage = if has_real_usage {
    // Recompute total from merged components
    accumulated_usage.total_tokens =
        accumulated_usage.prompt_tokens + accumulated_usage.completion_tokens;
    accumulated_usage
} else {
    // Fallback: estimate when provider sends no usage in stream
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
            Message::Tool { content: c, .. } => counter.estimate_text(c),
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
```

**Note:** The match arm bindings use `c` / `r` / `t` instead of `content` / `reasoning_content` to avoid shadowing the outer `content: String` variable that's used in `LlmResponse` construction at L260.

This merges real provider usage across multiple SSE events when available, falling back to estimation (using `best_token_counter()` for consistency with context assembly).

- [ ] **Step 7: Run tests**

Run: `cargo nextest run -p providers && cargo nextest run -p agent`
Expected: All pass

- [ ] **Step 8: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 9: Commit**

```bash
git add crates/providers/src/types.rs crates/providers/src/adapters/anthropic_native.rs crates/providers/src/adapters/openai_compat.rs crates/agent/src/execution/core.rs
git commit -m "fix(providers): use real token counts from streaming responses

LlmStreamChunk now carries optional Usage. Anthropic message_delta and
OpenAI final chunk usage are parsed. call_provider_streaming prefers
real counts over estimation. Fallback uses best_token_counter instead of
CharTokenCounter. Cache tokens now tracked for streaming Anthropic calls."
```

---

### Task 4: Add Tool Result Sanitization (HIGH)

**Context:** Tool result strings are injected directly into `Message::Tool` with no filtering. External MCP server responses (`McpTool`) pass through unfiltered. A compromised MCP server could inject prompt-like content. An existing `sanitize_input()` function in `mcp::server::security` strips control characters and caps length — but it's never called on tool results.

**Files:**
- Modify: `crates/agent/src/execution/core.rs:632-638` (tool result injection point)

> Note: `McpTool` (`crates/mcp/src/client/tool_adapter.rs`) is not separately modified — its results flow through `core.rs`'s injection point, so sanitization there covers all tool types including MCP.

- [ ] **Step 1: Write test for tool result sanitization**

In `crates/agent/src/execution/` test module:

```rust
#[test]
fn sanitize_tool_result_strips_control_chars() {
    let input = "Normal text\x00\x01\x02with control chars\nand newlines";
    let result = sanitize_tool_result(input);
    assert!(!result.contains('\x00'));
    assert!(!result.contains('\x01'));
    assert!(result.contains('\n')); // newlines preserved
    assert!(result.contains("Normal text"));
}

#[test]
fn sanitize_tool_result_caps_length() {
    let long = "x".repeat(200_000);
    let result = sanitize_tool_result(&long);
    assert!(result.len() <= MAX_TOOL_RESULT_LENGTH + 50); // allow for truncation message
}

#[test]
fn sanitize_tool_result_preserves_normal_content() {
    let input = "Task created: 'Buy groceries' with priority High";
    assert_eq!(sanitize_tool_result(input), input);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -- sanitize_tool_result`
Expected: FAIL — function doesn't exist

- [ ] **Step 3: Implement `sanitize_tool_result` in execution core**

Add to `crates/agent/src/execution/core.rs`:

```rust
/// Maximum length for a single tool result (100KB).
const MAX_TOOL_RESULT_LENGTH: usize = 100_000;

/// Sanitize tool result string before injecting into conversation messages.
///
/// - Strips control characters (except \n, \t, \r)
/// - Truncates to MAX_TOOL_RESULT_LENGTH with a notice (UTF-8 safe)
fn sanitize_tool_result(input: &str) -> String {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .collect();

    if cleaned.len() > MAX_TOOL_RESULT_LENGTH {
        // Find a valid UTF-8 char boundary at or before MAX_TOOL_RESULT_LENGTH.
        // Walk backwards from the limit until we find a char boundary.
        let mut truncate_at = MAX_TOOL_RESULT_LENGTH;
        while truncate_at > 0 && !cleaned.is_char_boundary(truncate_at) {
            truncate_at -= 1;
        }
        let mut truncated = cleaned[..truncate_at].to_string();
        truncated.push_str("\n[truncated - result exceeded 100KB]");
        truncated
    } else {
        cleaned
    }
}
```

- [ ] **Step 4: Apply sanitization at the injection point**

At `core.rs:632-638`, replace:

```rust
for r in &results {
    messages.push(Message::tool(
        r.tool_call_id.clone(),
        r.tool_name.clone(),
        r.result.clone(),
    ));
}
```

With:

```rust
for r in &results {
    messages.push(Message::tool(
        r.tool_call_id.clone(),
        r.tool_name.clone(),
        sanitize_tool_result(&r.result),
    ));
}
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p agent -- sanitize_tool_result`
Expected: All pass

- [ ] **Step 6: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/execution/core.rs
git commit -m "fix(agent): sanitize tool results before message injection

Strips control characters and caps at 100KB. Defends against prompt
injection via compromised MCP servers or malformed tool output."
```

---

### Task 5: Parse and Consume `triggers` Field in Skill System (MEDIUM)

**Context:** All 5 built-in SKILL.md files declare `triggers:` arrays (50+ entries for task-management) in YAML frontmatter. `RawKlyntbotMeta` has no `triggers` field, so serde silently drops them. The router matches only on description text. Adding triggers dramatically improves routing accuracy for keyword-heavy queries.

**Files:**
- Modify: `crates/skill-system/src/parser.rs:22-38` (RawKlyntbotMeta)
- Modify: `crates/skill-system/src/types.rs:101-111` (KlyntbotMeta)
- Modify: `crates/skill-system/src/router.rs:26-47` (keyword_scores)

- [ ] **Step 1: Write test for triggers parsing**

In `crates/skill-system/src/parser.rs` test module:

```rust
#[test]
fn parse_skill_md_extracts_triggers() {
    let content = r#"---
name: test-skill
description: A test skill for managing tasks
metadata:
  klyntbot:
    type: orchestrator
    triggers:
      - "add task"
      - "create todo"
      - "show tasks"
---
Test body content.
"#;
    let pkg = parse_skill_md(content, PathBuf::from("/tmp/test"), SkillScope::BuiltIn).unwrap();
    let meta = pkg.metadata.klyntbot.unwrap();
    assert_eq!(meta.triggers.len(), 3);
    assert!(meta.triggers.contains(&"add task".to_string()));
}

#[test]
fn parse_skill_md_empty_triggers_is_ok() {
    let content = r#"---
name: test-skill
description: A test skill
---
Body.
"#;
    let pkg = parse_skill_md(content, PathBuf::from("/tmp/test"), SkillScope::BuiltIn).unwrap();
    let meta = pkg.metadata.klyntbot;
    // No klyntbot metadata block at all — triggers defaults to empty
    assert!(meta.is_none() || meta.unwrap().triggers.is_empty());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p skill-system -- parse_skill_md_extracts_triggers`
Expected: FAIL — triggers field not captured

- [ ] **Step 3: Add `triggers` to `RawKlyntbotMeta` and `KlyntbotMeta`**

In `crates/skill-system/src/parser.rs`, add to `RawKlyntbotMeta`:

```rust
#[derive(Deserialize, Default)]
struct RawKlyntbotMeta {
    // ... existing fields ...
    #[serde(default)]
    triggers: Vec<String>,
}
```

In `crates/skill-system/src/types.rs`, add to `KlyntbotMeta`:

```rust
#[derive(Debug, Clone, Default)]
pub struct KlyntbotMeta {
    // ... existing fields ...
    /// Trigger phrases that boost this skill during routing.
    pub triggers: Vec<String>,
}
```

Update the conversion from `RawKlyntbotMeta` → `KlyntbotMeta` in `parser.rs` (`parse_metadata_block`) to pass through the triggers field.

Add an accessor to `SkillPackage` in `types.rs`:

```rust
/// Trigger phrases for routing boost.
pub fn triggers(&self) -> &[String] {
    self.metadata
        .klyntbot
        .as_ref()
        .map(|k| k.triggers.as_slice())
        .unwrap_or(&[])
}
```

- [ ] **Step 4: Run parser tests to verify triggers are captured**

Run: `cargo nextest run -p skill-system -- parse_skill_md`
Expected: PASS

- [ ] **Step 5: Write test for trigger-boosted routing**

In `crates/skill-system/src/router.rs` test module. The test must construct real `SkillPackage` objects — `SkillCatalog::default()` produces an empty catalog.

```rust
#[test]
fn router_boosts_score_for_trigger_match() {
    use crate::discovery::SkillSource;

    // Build catalog via discover_sync (the standard test pattern in this crate)
    let skills = vec![
        (
            "task-management".to_string(),
            "---\nname: task-management\ndescription: Manage tasks and todos.\nmetadata:\n  klyntbot:\n    type: orchestrator\n    triggers:\n      - \"add task\"\n      - \"create todo\"\n---\nTask management body.".to_string(),
        ),
        (
            "general".to_string(),
            "---\nname: general\ndescription: General purpose assistant.\nmetadata:\n  klyntbot:\n    type: orchestrator\n---\nGeneral body.".to_string(),
        ),
    ];
    let source = SkillSource::BuiltIn(skills);
    let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
    let router = SkillRouter::new(&catalog);

    // "add task" trigger should boost task-management even if description
    // doesn't contain the word "add"
    let scores = router.keyword_scores("add task to my list", &catalog);
    let task_score = scores.get("task-management").copied().unwrap_or(0.0);
    assert!(task_score > 0.0, "trigger phrase should produce a score");
}
```

- [ ] **Step 6: Add trigger matching to `keyword_scores`**

In `crates/skill-system/src/router.rs`, modify `keyword_scores()` to also check triggers:

```rust
pub fn keyword_scores(&self, message: &str, catalog: &SkillCatalog) -> HashMap<String, f64> {
    let msg_lower = message.to_lowercase();
    let msg_tokens: Vec<String> = tokenize(message);
    let mut result = HashMap::new();

    for (name, desc_tokens) in &self.description_tokens {
        let pkg = match catalog.skills.get(name) {
            Some(p) => p,
            None => continue,
        };

        // Description keyword matching (existing logic)
        let mut hits = 0usize;
        for token in desc_tokens {
            if msg_tokens.contains(token) {
                hits += 1;
            }
        }

        // Trigger phrase matching — exact substring match (case-insensitive)
        let trigger_hits = pkg
            .triggers()
            .iter()
            .filter(|t| msg_lower.contains(&t.to_lowercase()))
            .count();

        if hits > 0 || trigger_hits > 0 {
            let normalizer = (desc_tokens.len() as f64 / 3.0).max(1.0);
            let desc_score = (hits as f64 / normalizer).min(1.0);
            // Each trigger match adds 0.3, capped at 1.0
            let trigger_score = (trigger_hits as f64 * 0.3).min(1.0);
            let score = (desc_score + trigger_score).min(1.0);
            result.insert(name.clone(), score);
        }
    }
    result
}
```

- [ ] **Step 7: Run all skill-system tests**

Run: `cargo nextest run -p skill-system`
Expected: All pass

- [ ] **Step 8: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 9: Commit**

```bash
git add crates/skill-system/src/parser.rs crates/skill-system/src/types.rs crates/skill-system/src/router.rs
git commit -m "feat(skill-system): parse and consume triggers field from SKILL.md

triggers: arrays in YAML frontmatter were silently dropped because
RawKlyntbotMeta had no triggers field. Now parsed and used as a routing
boost in SkillRouter::keyword_scores — exact phrase matches add 0.3
per trigger hit, improving skill selection for keyword-heavy queries."
```

---

### Task 6: Make MCP Whitelist Reactive (MEDIUM)

**Context:** `ToolRegistryBridge` holds a static `HashSet<String>` whitelist built once at startup. If tools are registered dynamically (MCP client reconnection, WASM plugin load), they don't appear in the MCP server's `list_tools` or `call_tool` whitelist without a restart. The fix: replace the static set with a dynamic read from config, or allow the whitelist to be refreshed.

**Files:**
- Modify: `crates/klyntbot-server/src/bridge/registry.rs` (ToolRegistryBridge)

- [ ] **Step 1: Write test for dynamic whitelist**

In `crates/klyntbot-server/src/bridge/registry.rs` test module:

```rust
#[test]
fn whitelist_reflects_updates_at_call_time() {
    let registry = Arc::new(tokio::sync::RwLock::new(tools_core::ToolRegistry::new()));
    let bridge = ToolRegistryBridge::new(registry, vec!["tasks".to_string()]);

    // Initially "notes" is not whitelisted
    assert!(!bridge.is_whitelisted("notes"));

    // Update whitelist to include "notes"
    bridge.update_whitelist(vec!["tasks".to_string(), "notes".to_string()]);

    // Now it should be whitelisted
    assert!(bridge.is_whitelisted("notes"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p klyntbot-server -- whitelist_reflects_updates`
Expected: FAIL — `is_whitelisted` and `update_whitelist` don't exist

- [ ] **Step 3: Replace static HashSet with `Arc<RwLock<HashSet<String>>>`**

In `crates/klyntbot-server/src/bridge/registry.rs`:

```rust
pub struct ToolRegistryBridge {
    registry: Arc<tokio::sync::RwLock<tools_core::ToolRegistry>>,
    whitelist: Arc<std::sync::RwLock<HashSet<String>>>,
}

impl ToolRegistryBridge {
    pub fn new(
        registry: Arc<tokio::sync::RwLock<tools_core::ToolRegistry>>,
        whitelist: Vec<String>,
    ) -> Self {
        Self {
            registry,
            whitelist: Arc::new(std::sync::RwLock::new(
                whitelist.into_iter().collect(),
            )),
        }
    }

    /// Update the whitelist at runtime (e.g., on config reload or tool registration).
    pub fn update_whitelist(&self, tools: Vec<String>) {
        let mut wl = self.whitelist.write().expect("whitelist lock");
        *wl = tools.into_iter().collect();
    }

    pub fn is_whitelisted(&self, name: &str) -> bool {
        self.whitelist.read().expect("whitelist lock").contains(name)
    }
}
```

Update all callsites that check `self.whitelist.contains(name)` to use `self.is_whitelisted(name)`.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p klyntbot-server`
Expected: All pass

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add crates/klyntbot-server/src/bridge/registry.rs
git commit -m "feat(mcp): make tool whitelist dynamically updatable

ToolRegistryBridge whitelist is now Arc<RwLock<HashSet>> instead of a
static HashSet. Exposes update_whitelist() for runtime changes when
tools are registered/unregistered or config is reloaded."
```

---

## Verification

After all tasks are complete, run the full verification suite:

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

All must pass with 0 errors and 0 clippy warnings.
