# Adaptive Orchestrator Agent Loop Redesign — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the monolithic ReAct agent loop with a 4-layer Adaptive Orchestrator architecture (Context Assembler → Orchestrator → Execution Engines → Output Pipeline) with native provider support, multi-axis learning, and chat-first optimizations.

**Architecture:** Layered pipeline where each message flows through: (1) token-budgeted context assembly with embedding-based memory retrieval, (2) intent classification selecting DirectResponse/ToolAssisted/AutonomousTask/Clarification, (3) strategy-specific execution engine with reasoning scratchpad and reflection, (4) output validation, cost tracking, and expanded learning. New `context_engine` crate at Layer 2. Refactored `agent` crate with orchestrator module and three execution engines replacing `agent_loop.rs`.

**Tech Stack:** Rust, tokio, fastembed (384-dim embeddings), tiktoken-rs (token counting), reqwest (HTTP), serde_json, async-trait. Native Anthropic `/v1/messages` API + OpenAI API alongside existing OpenAI-compat layer.

**Design Doc:** `docs/plans/2026-02-18-agent-loop-redesign-design.md`

---

## Phase Overview

| Phase | Name | Description | Dependencies |
|-------|------|-------------|--------------|
| 1 | Provider Layer Upgrade | Native Anthropic/OpenAI providers, ProviderManager with failover/retry, token counting | None |
| 2 | Context Engine | New crate: token budgets, memory retrieval, history compression | Phase 1 (token counting) |
| 3 | Shared Execution Core | Extract common LLM-call + tool-dispatch cycle from agent_loop.rs | Phase 1 |
| 4 | Orchestrator | Intent classification with heuristics + LLM classifier | Phase 2, 3 |
| 5 | Execution Engines | DirectResponse, ReAct+, PlanExecute engines | Phase 3, 4 |
| 6 | Output Pipeline | Response validator, cost tracker, expanded learning recorder | Phase 5 |
| 7 | Multi-Axis Learning | 6 axes of adaptation, behavioral signals, per-user profiles | Phase 6 |
| 8 | Chat-First Adaptations | Typing indicators, interrupt handling, channel formatting, pre-warming | Phase 5, 7 |
| 9 | Integration & Migration | Wire everything together, migrate agent_loop.rs, end-to-end tests | All |

---

## Phase 1: Provider Layer Upgrade

### Task 1.1: Extend LlmProvider Trait

**Files:**
- Modify: `crates/providers/src/types.rs`
- Test: `crates/providers/src/types.rs` (inline unit tests)

**Step 1: Write the failing test**

Add to `crates/providers/src/types.rs` inside `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_provider_capabilities_default() {
    let caps = ProviderCapabilities::default();
    assert!(!caps.extended_thinking);
    assert!(!caps.structured_outputs);
    assert!(!caps.prompt_caching);
    assert!(!caps.native_token_counting);
    assert!(caps.vision); // default true
    assert!(caps.streaming); // default true
    assert!(!caps.tool_choice_required);
}

#[test]
fn test_context_window_default() {
    // Verify the trait provides a default context window
    assert_eq!(128_000, DEFAULT_CONTEXT_WINDOW);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p providers -E 'test(provider_capabilities)' --no-capture`
Expected: FAIL — `ProviderCapabilities` not defined

**Step 3: Add ProviderCapabilities and extend LlmProvider trait**

In `crates/providers/src/types.rs`, add:

```rust
pub const DEFAULT_CONTEXT_WINDOW: usize = 128_000;

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

#[derive(Debug, Clone, Default)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cache_read_tokens: usize,
    pub cache_write_tokens: usize,
}

impl TokenUsage {
    pub fn zero() -> Self { Self::default() }
    pub fn total(&self) -> usize { self.input_tokens + self.output_tokens }
}

impl std::ops::AddAssign for TokenUsage {
    fn add_assign(&mut self, rhs: Self) {
        self.input_tokens += rhs.input_tokens;
        self.output_tokens += rhs.output_tokens;
        self.cache_read_tokens += rhs.cache_read_tokens;
        self.cache_write_tokens += rhs.cache_write_tokens;
    }
}
```

Add default methods to `LlmProvider` trait:

```rust
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
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p providers -E 'test(provider_capabilities)' --no-capture`
Expected: PASS

**Step 5: Run full providers test suite**

Run: `cargo nextest run -p providers`
Expected: All existing tests still pass

**Step 6: Commit**

```bash
git add crates/providers/src/types.rs
git commit -m "feat(providers): add ProviderCapabilities, TokenUsage, and extended LlmProvider trait"
```

---

### Task 1.2: Anthropic Native Provider

**Files:**
- Create: `crates/providers/src/anthropic_native.rs`
- Modify: `crates/providers/src/lib.rs` (add module)
- Test: `crates/providers/src/anthropic_native.rs` (inline tests)

**Step 1: Write the failing test**

Create `crates/providers/src/anthropic_native.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_messages_to_anthropic_format() {
        let provider = AnthropicNativeProvider::new(
            Secret::new("test-key".to_string()),
            "https://api.anthropic.com".to_string(),
            "claude-sonnet-4-20250514".to_string(),
        );
        let messages = vec![
            Message::user("Hello"),
            Message::assistant("Hi there"),
        ];
        let result = provider.convert_messages(&messages);
        // Anthropic format: content is array of content blocks
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"][0]["type"], "text");
        assert_eq!(result[0]["content"][0]["text"], "Hello");
    }

    #[test]
    fn test_convert_tool_schema_to_anthropic_format() {
        let provider = AnthropicNativeProvider::new(
            Secret::new("test-key".to_string()),
            "https://api.anthropic.com".to_string(),
            "claude-sonnet-4-20250514".to_string(),
        );
        let openai_tool = serde_json::json!({
            "type": "function",
            "function": {
                "name": "get_weather",
                "description": "Get weather for a location",
                "parameters": {
                    "type": "object",
                    "properties": { "city": { "type": "string" } },
                    "required": ["city"]
                }
            }
        });
        let result = provider.convert_tools(&[openai_tool]);
        assert_eq!(result[0]["name"], "get_weather");
        assert_eq!(result[0]["description"], "Get weather for a location");
        assert!(result[0]["input_schema"].is_object());
    }

    #[test]
    fn test_capabilities() {
        let provider = AnthropicNativeProvider::new(
            Secret::new("test-key".to_string()),
            "https://api.anthropic.com".to_string(),
            "claude-sonnet-4-20250514".to_string(),
        );
        let caps = provider.capabilities();
        assert!(caps.extended_thinking);
        assert!(caps.prompt_caching);
        assert!(caps.native_token_counting);
    }

    #[test]
    fn test_context_window_by_model() {
        let provider = AnthropicNativeProvider::new(
            Secret::new("test-key".to_string()),
            "https://api.anthropic.com".to_string(),
            "claude-sonnet-4-20250514".to_string(),
        );
        assert_eq!(provider.context_window(), 200_000);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p providers -E 'test(convert_messages_to_anthropic)' --no-capture`
Expected: FAIL — module not found

**Step 3: Implement AnthropicNativeProvider**

In `crates/providers/src/anthropic_native.rs`:

```rust
use async_trait::async_trait;
use config::Secret;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::{debug, warn};

use crate::types::*;
use common::Result;

pub struct AnthropicNativeProvider {
    client: Client,
    api_key: Secret<String>,
    base_url: String,
    model: String,
}

impl AnthropicNativeProvider {
    pub fn new(api_key: Secret<String>, base_url: String, model: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Failed to build HTTP client");
        Self { client, api_key, base_url, model }
    }

    pub fn convert_messages(&self, messages: &[Message]) -> Vec<Value> {
        messages.iter().filter_map(|msg| {
            match msg {
                Message::System { .. } => None, // system handled separately
                Message::User { content } => {
                    let content_blocks = match content {
                        UserContent::Text(text) => json!([{"type": "text", "text": text}]),
                        UserContent::MultiPart(parts) => {
                            let blocks: Vec<Value> = parts.iter().map(|p| match p {
                                ContentPart::Text { text } => json!({"type": "text", "text": text}),
                                ContentPart::ImageUrl { image_url } => json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": image_url.detail.as_deref().unwrap_or("image/png"),
                                        "data": image_url.url.trim_start_matches("data:image/png;base64,")
                                    }
                                }),
                            }).collect();
                            Value::Array(blocks)
                        }
                    };
                    Some(json!({"role": "user", "content": content_blocks}))
                }
                Message::Assistant { content, tool_calls, .. } => {
                    let mut blocks = Vec::new();
                    if let Some(text) = content {
                        if !text.is_empty() {
                            blocks.push(json!({"type": "text", "text": text}));
                        }
                    }
                    if let Some(calls) = tool_calls {
                        for call in calls {
                            blocks.push(json!({
                                "type": "tool_use",
                                "id": call.id,
                                "name": call.function.name,
                                "input": serde_json::from_str::<Value>(&call.function.arguments)
                                    .unwrap_or(json!({}))
                            }));
                        }
                    }
                    Some(json!({"role": "assistant", "content": blocks}))
                }
                Message::Tool { tool_call_id, content, .. } => {
                    Some(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": tool_call_id,
                            "content": content
                        }]
                    }))
                }
            }
        }).collect()
    }

    pub fn convert_tools(&self, openai_tools: &[Value]) -> Vec<Value> {
        openai_tools.iter().filter_map(|tool| {
            let func = tool.get("function")?;
            Some(json!({
                "name": func["name"],
                "description": func["description"],
                "input_schema": func["parameters"]
            }))
        }).collect()
    }

    fn extract_system_prompt(messages: &[Message]) -> Option<String> {
        messages.iter().find_map(|m| {
            if let Message::System { content } = m { Some(content.clone()) } else { None }
        })
    }

    fn model_context_window(model: &str) -> usize {
        if model.contains("opus") { 200_000 }
        else if model.contains("sonnet") { 200_000 }
        else if model.contains("haiku") { 200_000 }
        else { 200_000 }
    }

    fn parse_response(&self, body: Value) -> Result<LlmResponse> {
        let content_blocks = body["content"].as_array();
        let mut text_content = String::new();
        let mut tool_calls = Vec::new();

        if let Some(blocks) = content_blocks {
            for block in blocks {
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
                            name: name.clone(),
                            arguments: input,
                        });
                    }
                    _ => {}
                }
            }
        }

        let usage = body.get("usage").map(|u| Usage {
            prompt_tokens: u["input_tokens"].as_u64().unwrap_or(0) as usize,
            completion_tokens: u["output_tokens"].as_u64().unwrap_or(0) as usize,
            total_tokens: (u["input_tokens"].as_u64().unwrap_or(0)
                + u["output_tokens"].as_u64().unwrap_or(0)) as usize,
        });

        let finish_reason = body["stop_reason"].as_str().map(|r| match r {
            "end_turn" => "stop".to_string(),
            "tool_use" => "tool_calls".to_string(),
            other => other.to_string(),
        });

        Ok(LlmResponse {
            content: if text_content.is_empty() { None } else { Some(text_content) },
            tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
            finish_reason,
            usage,
            reasoning_content: None, // TODO: extract from thinking blocks
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
        let system_prompt = Self::extract_system_prompt(messages);
        let anthropic_messages = self.convert_messages(messages);

        let mut request = json!({
            "model": params.model.as_deref().unwrap_or(&self.model),
            "messages": anthropic_messages,
            "max_tokens": params.max_tokens.unwrap_or(4096),
        });

        if let Some(system) = &system_prompt {
            request["system"] = json!([{
                "type": "text",
                "text": system,
                "cache_control": {"type": "ephemeral"}
            }]);
        }

        if let Some(temp) = params.temperature {
            request["temperature"] = json!(temp);
        }

        if let Some(tools) = tools {
            if !tools.is_empty() {
                request["tools"] = json!(self.convert_tools(tools));
                request["tool_choice"] = json!({"type": "auto"});
            }
        }

        let response = self.client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| common::KlyntbotError::Provider(
                common::ProviderError::Http(e.to_string())
            ))?;

        let status = response.status();
        if status == 429 {
            return Err(common::KlyntbotError::Provider(common::ProviderError::RateLimited));
        }
        if status == 401 || status == 403 {
            return Err(common::KlyntbotError::Provider(common::ProviderError::AuthFailed));
        }

        let body: Value = response.json().await
            .map_err(|e| common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(e.to_string())
            ))?;

        if !status.is_success() {
            let error_msg = body["error"]["message"].as_str()
                .unwrap_or("Unknown error").to_string();
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(error_msg)
            ));
        }

        self.parse_response(body)
    }

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
    ) -> Result<LlmStream> {
        // TODO: implement SSE streaming for Anthropic native
        // For now, fall back to non-streaming
        let response = self.chat(messages, tools, params).await?;
        let chunk = LlmStreamChunk {
            content: response.content,
            tool_call_delta: None,
            is_final: true,
            finish_reason: response.finish_reason,
            reasoning_content: response.reasoning_content,
        };
        let stream = futures_util::stream::once(async { Ok(Some(chunk)) });
        Ok(Box::pin(stream))
    }

    fn supports_streaming(&self) -> bool {
        true // Will be native once SSE is implemented
    }

    fn default_model(&self) -> &str {
        &self.model
    }

    fn name(&self) -> &str {
        "anthropic-native"
    }

    async fn count_tokens(&self, messages: &[Message], tools: Option<&[Value]>) -> Result<usize> {
        let system_prompt = Self::extract_system_prompt(messages);
        let anthropic_messages = self.convert_messages(messages);

        let mut request = json!({
            "model": self.model,
            "messages": anthropic_messages,
        });

        if let Some(system) = &system_prompt {
            request["system"] = json!(system);
        }

        if let Some(tools) = tools {
            if !tools.is_empty() {
                request["tools"] = json!(self.convert_tools(tools));
            }
        }

        let response = self.client
            .post(format!("{}/v1/messages/count_tokens", self.base_url))
            .header("x-api-key", self.api_key.expose())
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| common::KlyntbotError::Provider(
                common::ProviderError::Http(e.to_string())
            ))?;

        let body: Value = response.json().await
            .map_err(|e| common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(e.to_string())
            ))?;

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
        Self::model_context_window(&self.model)
    }
}
```

**Step 4: Add module declaration**

In `crates/providers/src/lib.rs`, add:
```rust
pub mod anthropic_native;
```

**Step 5: Run tests**

Run: `cargo nextest run -p providers --no-capture`
Expected: All tests pass

**Step 6: Commit**

```bash
git add crates/providers/src/anthropic_native.rs crates/providers/src/lib.rs crates/providers/src/types.rs
git commit -m "feat(providers): add Anthropic native provider with prompt caching and token counting"
```

---

### Task 1.3: ProviderManager with Failover and Rate Limiting

**Files:**
- Create: `crates/providers/src/manager.rs`
- Modify: `crates/providers/src/lib.rs`
- Test: `crates/providers/src/manager.rs` (inline tests)

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingProvider {
        call_count: Arc<AtomicUsize>,
        should_fail: bool,
    }

    // ... implement LlmProvider for CountingProvider ...

    #[tokio::test]
    async fn test_primary_provider_used_first() {
        let primary_calls = Arc::new(AtomicUsize::new(0));
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let manager = ProviderManager::new(
            Arc::new(CountingProvider { call_count: primary_calls.clone(), should_fail: false }),
            Some(Arc::new(CountingProvider { call_count: fallback_calls.clone(), should_fail: false })),
            None,
        );
        manager.chat(&[], None, &ChatParams::default()).await.unwrap();
        assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_fallback_on_rate_limit() {
        let primary = Arc::new(CountingProvider { call_count: Arc::new(AtomicUsize::new(0)), should_fail: true });
        let fallback_calls = Arc::new(AtomicUsize::new(0));
        let fallback = Arc::new(CountingProvider { call_count: fallback_calls.clone(), should_fail: false });
        let manager = ProviderManager::new(primary, Some(fallback), None);
        manager.chat(&[], None, &ChatParams::default()).await.unwrap();
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_threshold() {
        // After 5 consecutive failures, circuit should open
        let manager = ProviderManager::with_config(/* ... */, CircuitBreakerConfig {
            failure_threshold: 5,
            reset_timeout_secs: 60,
        });
        // ... trigger 5 failures, verify circuit is open
    }

    #[tokio::test]
    async fn test_retry_with_backoff() {
        // Verify exponential backoff on rate limits
        // 1st call fails (rate limit), 2nd call succeeds
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p providers -E 'test(provider_manager)' --no-capture`

**Step 3: Implement ProviderManager**

Implement `ProviderManager` with:
- `primary: DynProvider`
- `fallback: Option<DynProvider>`
- `classifier_provider: Option<DynProvider>`
- Circuit breaker (AtomicU32 failure count + Instant last_failure)
- `retry_with_backoff()` — 500ms, 1s, 2s up to 3 attempts
- Delegate `LlmProvider` trait through with failover logic

**Step 4: Run tests, commit**

```bash
git commit -m "feat(providers): add ProviderManager with failover, retry, and circuit breaker"
```

---

### Task 1.4: Provider Config Extension

**Files:**
- Modify: `crates/config/src/schema/providers.rs`
- Modify: `crates/config/src/schema/core.rs`
- Test: inline unit tests

**Step 1: Add new config fields**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub api_key: Secret<String>,
    pub api_base: Option<String>,
    pub extra_headers: Option<HashMap<String, String>>,
    #[serde(default)]
    pub native: bool,                              // NEW
    #[serde(default)]
    pub cache_system_prompt: bool,                 // NEW
    pub extended_thinking: Option<ExtendedThinkingConfig>,  // NEW
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtendedThinkingConfig {
    pub enabled: bool,
    pub budget_tokens: usize,
    pub use_for: Vec<String>,  // ["planning", "reflection"]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderManagerConfig {
    pub primary: Option<String>,
    pub fallback: Option<String>,
    pub classifier_model: Option<String>,
    pub circuit_breaker: Option<CircuitBreakerConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub reset_timeout_secs: u64,
}
```

Add `provider_manager: ProviderManagerConfig` to `Config` struct.

**Step 2: Test deserialization, commit**

```bash
git commit -m "feat(config): add native provider, extended thinking, and provider manager config"
```

---

### Task 1.5: Wire ProviderManager into create_provider

**Files:**
- Modify: `crates/providers/src/lib.rs`
- Test: `tests/provider_tests.rs`

Update `create_provider()` to check `native: true` flag and instantiate `AnthropicNativeProvider` when appropriate. Create `create_provider_manager()` that builds the full `ProviderManager` with primary + fallback + classifier.

```bash
git commit -m "feat(providers): wire ProviderManager and native providers into create_provider"
```

---

## Phase 2: Context Engine (New Crate)

### Task 2.1: Create context_engine Crate

**Files:**
- Create: `crates/context_engine/Cargo.toml`
- Create: `crates/context_engine/src/lib.rs`
- Create: `crates/context_engine/src/budget.rs`
- Modify: `Cargo.toml` (workspace members)

**Step 1: Create crate skeleton**

`crates/context_engine/Cargo.toml`:
```toml
[package]
name = "context_engine"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { path = "../common" }
config = { path = "../config" }
providers = { path = "../providers" }
session = { path = "../session" }
tools = { path = "../tools" }
goal = { path = "../goal" }
plan = { path = "../plan" }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
async-trait = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

**Step 2: Write token budget tests**

In `crates/context_engine/src/budget.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_budget_allocation_standard() {
        let config = BudgetConfig::standard(128_000);
        assert_eq!(config.response_reserve(), 19_200);  // 15%
        assert_eq!(config.available_input(), 108_800);   // 85%
    }

    #[test]
    fn test_priority_waterfall_fits_within_budget() {
        let mut allocator = BudgetAllocator::new(BudgetConfig::standard(128_000));
        allocator.allocate(Priority::SystemIdentity, 500);
        allocator.allocate(Priority::ActiveTask, 2000);
        allocator.allocate(Priority::ToolDefinitions, 3000);
        allocator.allocate(Priority::RecentHistory, 10000);
        assert!(allocator.remaining() > 0);
        assert_eq!(allocator.total_allocated(), 15500);
    }

    #[test]
    fn test_overflow_truncates_lowest_priority() {
        let mut allocator = BudgetAllocator::new(BudgetConfig::standard(1000)); // tiny budget
        allocator.allocate(Priority::SystemIdentity, 500);
        allocator.allocate(Priority::ActiveTask, 300);
        // Only 50 tokens left (1000 * 0.85 - 800 = 50), skills won't fully fit
        let allocated = allocator.try_allocate(Priority::Skills, 200);
        assert!(allocated < 200);
    }
}
```

**Step 3: Implement BudgetAllocator**

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    SystemIdentity = 0,
    ActiveTask = 1,
    ToolDefinitions = 2,
    RecentHistory = 3,
    RetrievedMemory = 4,
    CompressedHistory = 5,
    BootstrapPersona = 6,
    Skills = 7,
}

pub struct BudgetConfig {
    pub total_context_window: usize,
    pub response_reserve_pct: f32,         // default 0.15
    pub priority_limits: HashMap<Priority, f32>, // max % per priority
}

pub struct BudgetAllocator {
    config: BudgetConfig,
    allocations: HashMap<Priority, usize>,
}

pub struct BudgetReport {
    pub total_window: usize,
    pub total_allocated: usize,
    pub remaining: usize,
    pub per_priority: Vec<(Priority, usize, f32)>, // priority, tokens, % used
}
```

**Step 4: Run tests, commit**

```bash
git commit -m "feat(context_engine): create crate with token budget allocator"
```

---

### Task 2.2: Memory Retriever

**Files:**
- Create: `crates/context_engine/src/memory_retriever.rs`
- Test: inline

**Step 1: Write tests for embedding-based memory retrieval**

```rust
#[tokio::test]
async fn test_retrieve_relevant_memories() {
    // Setup: create MemoryRetriever with mock embedding engine
    // Store 5 memories with different topics
    // Query: "login authentication bug"
    // Assert: returns memories about auth, not about weather
}

#[tokio::test]
async fn test_memory_retrieval_respects_budget() {
    // Setup: budget allows 500 tokens
    // Store memories totaling 2000 tokens
    // Assert: only top-k memories fitting within budget returned
}

#[tokio::test]
async fn test_chunked_memory_md() {
    // Setup: MEMORY.md with 3 paragraphs
    // Assert: chunked into 3 separate searchable entries
}
```

**Step 2: Implement MemoryRetriever**

```rust
pub struct MemoryRetriever {
    embedding_engine: Arc<EmbeddingEngine>,
    conversation_store: Option<Arc<RwLock<dyn ConversationEmbeddingHandler>>>,
    memory_store_path: PathBuf,
    cached_chunks: RwLock<Option<Vec<MemoryChunk>>>,
}

pub struct MemoryChunk {
    pub content: String,
    pub source: MemorySource,
    pub embedding: Vec<f32>,
    pub token_estimate: usize,
}

pub enum MemorySource {
    LongTerm,       // MEMORY.md chunk
    DailyNote,      // YYYY-MM-DD.md chunk
    Conversation,   // past conversation snippet
    TaskContext,     // related todo item
}

impl MemoryRetriever {
    pub async fn retrieve(
        &self,
        query_embedding: &[f32],
        budget_tokens: usize,
        threshold: f32,
    ) -> Vec<MemoryChunk>;
}
```

**Step 3: Run tests, commit**

```bash
git commit -m "feat(context_engine): add embedding-based memory retriever"
```

---

### Task 2.3: History Compressor

**Files:**
- Create: `crates/context_engine/src/history_compressor.rs`
- Test: inline

**Step 1: Write tests**

```rust
#[tokio::test]
async fn test_recent_messages_kept_verbatim() {
    let compressor = HistoryCompressor::new(/* budget: 5000 tokens */);
    let history = generate_test_history(20); // 20 messages
    let result = compressor.compress(&history, 5000).await;
    // Last 4 messages should be verbatim
    assert_eq!(result.recent_messages.len(), 4);
    assert!(result.recent_messages[0].content == history[16].content);
}

#[tokio::test]
async fn test_old_messages_summarized() {
    let compressor = HistoryCompressor::new(/* ... */);
    let history = generate_test_history(30);
    let result = compressor.compress(&history, 5000).await;
    assert!(!result.summaries.is_empty());
    assert!(result.summaries[0].content.starts_with("Earlier in this conversation:"));
}

#[tokio::test]
async fn test_summaries_cached() {
    // Compress once, then add 1 message, compress again
    // Only the new chunk should be summarized, not everything
}
```

**Step 2: Implement HistoryCompressor**

```rust
pub struct HistoryCompressor {
    provider: Option<DynProvider>,  // for LLM-powered summarization
    cached_summaries: RwLock<HashMap<String, CachedSummary>>,  // chunk_hash → summary
}

pub struct CompressedHistory {
    pub summaries: Vec<HistorySummary>,      // older turns, summarized
    pub recent_messages: Vec<SessionMessage>,  // recent turns, verbatim
    pub total_tokens: usize,
}

pub struct HistorySummary {
    pub content: String,       // "Earlier: user asked about X, agent found Y..."
    pub message_range: (usize, usize),  // which messages this covers
    pub token_count: usize,
}

impl HistoryCompressor {
    pub async fn compress(
        &self,
        history: &[SessionMessage],
        budget_tokens: usize,
    ) -> CompressedHistory;

    fn extractive_fallback(messages: &[SessionMessage]) -> String;
}
```

**Step 3: Run tests, commit**

```bash
git commit -m "feat(context_engine): add history compressor with incremental summarization"
```

---

### Task 2.4: ContextEngine Main Assembly

**Files:**
- Modify: `crates/context_engine/src/lib.rs`
- Create: `crates/context_engine/src/assembler.rs`
- Test: `tests/context_engine_integration.rs`

**Step 1: Write integration test**

```rust
#[tokio::test]
async fn test_assemble_direct_response_context() {
    // Minimal context for DirectResponse strategy
    let engine = create_test_context_engine();
    let result = engine.assemble(ContextRequest {
        execution_strategy: ExecutionStrategy::DirectResponse,
        // ...
    }).await.unwrap();
    // Should have: system prompt, last 4 messages, NO tool definitions
    assert!(result.token_count < 2000);
    assert!(result.budget_report.per_priority.iter()
        .find(|(p, _, _)| *p == Priority::ToolDefinitions)
        .map(|(_, tokens, _)| *tokens == 0)
        .unwrap_or(true));
}

#[tokio::test]
async fn test_assemble_tool_assisted_context() {
    // Standard context with tools and memory retrieval
    let engine = create_test_context_engine();
    let result = engine.assemble(ContextRequest {
        execution_strategy: ExecutionStrategy::ToolAssisted { /* ... */ },
        // ...
    }).await.unwrap();
    assert!(result.token_count > 2000);
    // Should include tool definitions
    assert!(result.messages.iter().any(|m| {
        if let Message::System { content } = m { content.contains("tools") } else { false }
    }));
}
```

**Step 2: Implement ContextEngine assembler**

The main `assemble()` method orchestrates budget allocation, memory retrieval, history compression, and message construction.

**Step 3: Run tests, commit**

```bash
git commit -m "feat(context_engine): implement main ContextEngine assembler with budget-aware assembly"
```

---

## Phase 3: Shared Execution Core

### Task 3.1: Extract ExecutionCore from agent_loop.rs

**Files:**
- Create: `crates/agent/src/execution/mod.rs`
- Create: `crates/agent/src/execution/core.rs`
- Create: `crates/agent/src/execution/types.rs`
- Modify: `crates/agent/src/lib.rs`
- Test: inline + `tests/execution_core_tests.rs`

**Step 1: Write tests for single cycle execution**

```rust
#[tokio::test]
async fn test_cycle_with_tool_calls() {
    let core = create_test_execution_core(mock_provider_with_tool_calls());
    let mut messages = vec![Message::user("test")];
    let outcome = core.run_cycle(&mut messages, &ToolFilter::all(), &ExecutionParams::default()).await.unwrap();
    assert!(matches!(outcome, CycleOutcome::ToolsExecuted { .. }));
}

#[tokio::test]
async fn test_cycle_with_final_response() {
    let core = create_test_execution_core(mock_provider_with_text_response());
    let mut messages = vec![Message::user("test")];
    let outcome = core.run_cycle(&mut messages, &ToolFilter::all(), &ExecutionParams::default()).await.unwrap();
    assert!(matches!(outcome, CycleOutcome::FinalResponse { .. }));
}

#[tokio::test]
async fn test_tool_timeout() {
    // Tool that takes 5s, timeout set to 1s
    let core = create_test_execution_core_with_slow_tool();
    let mut messages = vec![Message::user("test")];
    let params = ExecutionParams { tool_timeout: Duration::from_secs(1), ..Default::default() };
    let outcome = core.run_cycle(&mut messages, &ToolFilter::all(), &params).await.unwrap();
    // Tool should have timed out, error fed back as string
    if let CycleOutcome::ToolsExecuted { tool_results, .. } = outcome {
        assert!(tool_results[0].result.contains("timed out"));
    }
}

#[tokio::test]
async fn test_read_lock_for_tool_definitions() {
    // Verify we use read lock, not write lock, for get_definitions
}
```

**Step 2: Implement ExecutionCore**

Extract the common code from `run_standard_iteration()` (agent_loop.rs:L1418-L1593):
- LLM call
- Tool call parsing
- Confidence evaluation
- Parallel tool execution with `join_all` + per-tool `tokio::time::timeout`
- Error-to-string passthrough
- Token usage collection
- Outcome recording

```rust
pub struct ExecutionCore {
    pub provider: DynProvider,
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
    pub confidence_evaluator: Option<Arc<ConfidenceEvaluator>>,
    pub outcome_recorder: Option<Arc<OutcomeRecorder>>,
}

pub struct ExecutionParams {
    pub tool_timeout: Duration,        // default 30s
    pub include_reasoning_prompt: bool,
    pub chat_params: ChatParams,
}

pub struct ToolResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub result: String,
    pub duration_ms: u64,
    pub success: bool,
}
```

**Step 3: Run tests, commit**

```bash
git commit -m "feat(agent): extract shared ExecutionCore with tool timeout and read-lock"
```

---

### Task 3.2: Reasoning Scratchpad

**Files:**
- Create: `crates/agent/src/execution/scratchpad.rs`
- Test: inline

```rust
pub struct Scratchpad {
    traces: Vec<ReasoningTrace>,
}

pub struct ReasoningTrace {
    pub cycle: u32,
    pub thought: String,
    pub planned_actions: Vec<String>,
    pub actual_action: String,
    pub reflection: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl Scratchpad {
    pub fn new() -> Self;
    pub fn add(&mut self, trace: ReasoningTrace);
    pub fn summarize(&self) -> String;
    pub fn last_n(&self, n: usize) -> &[ReasoningTrace];
}
```

```bash
git commit -m "feat(agent): add reasoning scratchpad for execution trace persistence"
```

---

## Phase 4: Orchestrator

### Task 4.1: Heuristic Pre-filter

**Files:**
- Create: `crates/agent/src/orchestrator/mod.rs`
- Create: `crates/agent/src/orchestrator/heuristics.rs`
- Test: inline

**Step 1: Write tests**

```rust
#[test]
fn test_greeting_detected_as_direct() {
    assert_eq!(classify_heuristic("hi"), Some(ExecutionStrategy::direct()));
    assert_eq!(classify_heuristic("hello!"), Some(ExecutionStrategy::direct()));
    assert_eq!(classify_heuristic("hey there"), Some(ExecutionStrategy::direct()));
}

#[test]
fn test_code_keywords_hint_tool_assisted() {
    assert_eq!(classify_heuristic("fix the auth bug in login.rs"), Some(ExecutionStrategy::tool_assisted_default()));
}

#[test]
fn test_plan_command_detected_as_autonomous() {
    assert_eq!(classify_heuristic("create a plan for refactoring the database layer"),
        Some(ExecutionStrategy::autonomous_default()));
}

#[test]
fn test_complex_message_falls_through() {
    assert_eq!(classify_heuristic("what do you think about the new design?"), None);
}
```

**Step 2: Implement heuristic classifier**

Pattern matching on message length, keywords, and structural cues. Returns `Option<ExecutionStrategy>` — `None` means "proceed to LLM classification."

```bash
git commit -m "feat(orchestrator): add heuristic pre-filter for zero-cost intent classification"
```

---

### Task 4.2: LLM Classifier

**Files:**
- Create: `crates/agent/src/orchestrator/classifier.rs`
- Test: inline + mock provider tests

**Step 1: Write tests**

```rust
#[tokio::test]
async fn test_classifier_returns_valid_strategy() {
    let classifier = LlmClassifier::new(mock_provider_returning(json!({
        "strategy": "tool_assisted",
        "reasoning": "User wants to check tasks",
        "estimated_steps": 2,
        "tools_likely_needed": ["todo"],
        "confidence": 0.9
    })));
    let result = classifier.classify(&InboundMessage::test("show my tasks"), &[], &[]).await.unwrap();
    assert!(matches!(result.strategy, ExecutionStrategy::ToolAssisted { .. }));
}

#[tokio::test]
async fn test_classifier_timeout_returns_default() {
    let classifier = LlmClassifier::new(slow_mock_provider(Duration::from_secs(5)));
    // With 2s timeout, should fall back to ToolAssisted
    let result = classifier.classify_with_timeout(&msg, &[], &[], Duration::from_secs(2)).await;
    assert!(matches!(result.strategy, ExecutionStrategy::ToolAssisted { .. }));
}

#[tokio::test]
async fn test_classifier_invalid_json_returns_default() {
    let classifier = LlmClassifier::new(mock_provider_returning_text("I don't understand"));
    let result = classifier.classify(&msg, &[], &[]).await.unwrap();
    assert!(matches!(result.strategy, ExecutionStrategy::ToolAssisted { .. }));
}
```

**Step 2: Implement LLM classifier with minimal prompt (~300 tokens)**

```bash
git commit -m "feat(orchestrator): add LLM-based intent classifier with timeout fallback"
```

---

### Task 4.3: Orchestrator Main Module

**Files:**
- Modify: `crates/agent/src/orchestrator/mod.rs`
- Create: `crates/agent/src/orchestrator/escalation.rs`
- Test: inline + integration

Combines heuristic pre-filter + LLM classifier + confidence gate + tool filter generation.

```rust
pub struct Orchestrator {
    heuristic_rules: Vec<HeuristicRule>,
    classifier: LlmClassifier,
    escalation_policy: EscalationPolicy,
}

impl Orchestrator {
    pub async fn classify(&self, message: &InboundMessage, ...) -> ClassificationResult;
    pub fn handle_escalation(&self, signal: EscalationSignal, current: &ExecutionStrategy) -> ExecutionStrategy;
}
```

```bash
git commit -m "feat(orchestrator): implement full orchestrator with heuristics, LLM classifier, and escalation"
```

---

## Phase 5: Execution Engines

### Task 5.1: DirectResponse Engine

**Files:**
- Create: `crates/agent/src/execution/direct.rs`
- Test: inline

Single LLM call, no tools, escalation to ToolAssisted if LLM returns tool_calls.

```bash
git commit -m "feat(agent): add DirectResponse execution engine"
```

---

### Task 5.2: ReAct+ Engine

**Files:**
- Create: `crates/agent/src/execution/react_plus.rs`
- Test: inline + `tests/react_plus_tests.rs`

Enhanced ReAct loop with reasoning scratchpad, reflection checkpoints (OnFailure mode), and escalation threshold.

```bash
git commit -m "feat(agent): add ReAct+ execution engine with reasoning and reflection"
```

---

### Task 5.3: PlanExecute Engine (Redesigned)

**Files:**
- Create: `crates/agent/src/execution/plan_execute.rs`
- Modify: `crates/plan/src/types.rs` (add new PlanStep fields)
- Test: inline + `tests/plan_execute_tests.rs`

Key fixes:
1. Real parameter generation in planner prompt
2. Rich step context (memory, goals, history, accumulated results)
3. Step dependencies with topological sort for parallel execution
4. Checkpoint reflection every N steps
5. Progress file for long tasks

```bash
git commit -m "feat(agent): add PlanExecute engine with real params, DAG parallelism, and checkpoint reflection"
```

---

### Task 5.4: Engine Registry and Dispatch

**Files:**
- Create: `crates/agent/src/execution/dispatch.rs`
- Test: inline

Maps `ExecutionStrategy` → engine execution, handles escalation loop.

```bash
git commit -m "feat(agent): add engine dispatch with escalation handling"
```

---

## Phase 6: Output Pipeline

### Task 6.1: Response Validator

**Files:**
- Create: `crates/agent/src/output/mod.rs`
- Create: `crates/agent/src/output/validator.rs`
- Test: inline

Safety checks (leaked tokens, length limits) + quality checks (embedding similarity).

```bash
git commit -m "feat(agent): add response validator with safety and quality checks"
```

---

### Task 6.2: Cost & Usage Tracker

**Files:**
- Create: `crates/agent/src/output/cost_tracker.rs`
- Test: inline

Per-request recording to `~/.klyntbot/data/usage.jsonl`. Model pricing table.

```bash
git commit -m "feat(agent): add cost and usage tracker with per-request recording"
```

---

### Task 6.3: Expanded Learning Recorder

**Files:**
- Modify: `crates/agent/src/learning/recorder.rs`
- Modify: `crates/agent/src/learning/types.rs`
- Test: inline + `tests/learning_integration.rs`

Extend `OutcomeRecord` with strategy effectiveness, reasoning quality, context effectiveness, reflection outcomes.

```bash
git commit -m "feat(learning): expand outcome recording with strategy and reasoning metrics"
```

---

### Task 6.4: Session Persistence Upgrade

**Files:**
- Modify: `crates/session/src/manager.rs`
- Test: inline + `tests/session_persistence_tests.rs`

Atomic writes (temp + rename), file locking (flock), metadata on entries, auto-compaction.

```bash
git commit -m "feat(session): add atomic writes, file locking, and metadata on session entries"
```

---

## Phase 7: Multi-Axis Learning

### Task 7.1: Per-Tool Confidence Thresholds

**Files:**
- Modify: `crates/agent/src/learning/adaptive.rs`
- Modify: `crates/agent/src/confidence/evaluator.rs`
- Test: inline

Replace single `f32` with `HashMap<String, f32>` for per-tool thresholds.

```bash
git commit -m "feat(learning): add per-tool confidence thresholds"
```

---

### Task 7.2: Strategy Classification Tracking

**Files:**
- Create: `crates/agent/src/learning/strategy_tracker.rs`
- Test: inline

Track predicted vs actual strategy. Feed misclassifications back.

```bash
git commit -m "feat(learning): add strategy classification accuracy tracking"
```

---

### Task 7.3: Behavioral Signal Detection

**Files:**
- Create: `crates/agent/src/learning/behavioral_signals.rs`
- Test: inline

Analyze follow-up messages to infer response quality (thanks = good, rephrase = bad, etc.).

```bash
git commit -m "feat(learning): add behavioral signal detection for implicit quality feedback"
```

---

### Task 7.4: User & Channel Profiles

**Files:**
- Create: `crates/agent/src/learning/profiles.rs`
- Test: inline

`UserProfile` and `ChannelProfile` with response length preference, frequently used tools, topic clusters.

```bash
git commit -m "feat(learning): add per-user and per-channel learning profiles"
```

---

## Phase 8: Chat-First Adaptations

### Task 8.1: Typing Indicators & Progress Reporter

**Files:**
- Create: `crates/agent/src/chat/mod.rs`
- Create: `crates/agent/src/chat/progress.rs`
- Modify: `crates/bus/src/events.rs` (add OutboundMessage::Typing variant)
- Test: inline

```bash
git commit -m "feat(agent): add typing indicators and progress reporter for chat channels"
```

---

### Task 8.2: Interrupt Handling

**Files:**
- Create: `crates/agent/src/chat/interrupt.rs`
- Test: inline

Queue/CancelAndSwitch/Merge policies based on message timing and content.

```bash
git commit -m "feat(agent): add interrupt handling for follow-up messages during execution"
```

---

### Task 8.3: Channel-Aware Response Formatting

**Files:**
- Create: `crates/agent/src/chat/formatter.rs`
- Test: inline

Per-channel formatting (Telegram Markdown, Discord embeds, WhatsApp plain text, Slack mrkdwn).

```bash
git commit -m "feat(agent): add channel-aware response formatting"
```

---

### Task 8.4: Context Pre-Warming

**Files:**
- Create: `crates/agent/src/chat/pre_warm.rs`
- Test: inline

Pre-load session history, memories, and tool definitions when "user is typing" event received.

```bash
git commit -m "feat(agent): add context pre-warming on typing indicators"
```

---

## Phase 9: Integration & Migration

### Task 9.1: New Agent Pipeline (process_message_v2)

**Files:**
- Create: `crates/agent/src/pipeline.rs`
- Test: `tests/pipeline_integration.rs`

Wire together: ContextEngine → Orchestrator → Engine Dispatch → Output Pipeline. New `process_message_v2()` that calls through the full pipeline.

```bash
git commit -m "feat(agent): add process_message_v2 wiring full Adaptive Orchestrator pipeline"
```

---

### Task 9.2: Migrate agent_loop.rs

**Files:**
- Modify: `crates/agent/src/agent_loop.rs`
- Test: `tests/agent_loop_tests.rs`

Replace `process_message()` with `process_message_v2()`. Keep `run_agent_loop()` as fallback (feature-flagged) during transition. Run full test suite.

```bash
git commit -m "feat(agent): migrate agent_loop to Adaptive Orchestrator pipeline"
```

---

### Task 9.3: CLI Commands for New Features

**Files:**
- Create: `crates/cli/src/usage.rs`
- Create: `crates/cli/src/learning_cmd.rs`
- Modify: `crates/cli/src/commands.rs`
- Test: integration tests

Add: `klyntbot usage report`, `klyntbot learning status`, `klyntbot session inspect`, `klyntbot provider status`.

```bash
git commit -m "feat(cli): add usage, learning, session inspect, and provider status commands"
```

---

### Task 9.4: End-to-End Integration Tests

**Files:**
- Create: `tests/orchestrator_e2e.rs`
- Create: `tests/context_engine_e2e.rs`
- Create: `tests/pipeline_e2e.rs`

Full pipeline tests covering:
1. DirectResponse path (greeting → single LLM call → response)
2. ToolAssisted path (task query → classify → ReAct+ → tool calls → response)
3. AutonomousTask path (complex request → classify → plan → execute steps → response)
4. Escalation path (DirectResponse → ToolAssisted when LLM wants tools)
5. Clarification path (ambiguous request → ask user)
6. Context budget enforcement (large memory → truncation → still fits)
7. Provider failover (primary fails → fallback succeeds)

```bash
git commit -m "test: add end-to-end integration tests for Adaptive Orchestrator pipeline"
```

---

### Task 9.5: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

Update architecture section, add new crate to layer diagram, update CLI commands, add new config schema documentation.

```bash
git commit -m "docs: update CLAUDE.md with Adaptive Orchestrator architecture"
```

---

## Verification Checklist

Before marking the redesign complete:

- [ ] `cargo build --workspace` succeeds with zero warnings
- [ ] `cargo clippy --workspace --all-targets --all-features` — zero warnings
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo nextest run --workspace` — all 910+ existing tests pass
- [ ] New tests: 50+ tests across all new modules
- [ ] `cargo test --workspace --doc` — doctests pass
- [ ] `cargo build --no-default-features` — builds without email
- [ ] Manual test: `klyntbot chat "hello"` → DirectResponse path works
- [ ] Manual test: `klyntbot chat "show my tasks"` → ToolAssisted path works
- [ ] Manual test: `klyntbot plan create + execute` → PlanExecute path works
- [ ] `klyntbot usage report` → shows cost data
- [ ] `klyntbot learning status` → shows learning axes
- [ ] `klyntbot provider status` → shows primary/fallback state
