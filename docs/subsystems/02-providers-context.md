# Subsystem Analysis: Providers, Context Engine & Session (Layers 2-3)

## 1. Crate Overview

| Crate | Layer | Files | LoC (approx) | Dependencies |
|-------|-------|-------|--------------|--------------|
| `providers` | 2 | 7 source files | ~1,300 | common, config, async-trait, reqwest, serde, serde_json, futures-util, tokio, tracing, base64 |
| `context_engine` | 2 | 4 source files | ~310 | common, providers, serde, serde_json, tokio, tracing, chrono, async-trait |
| `session` | 2 | 2 source files | ~920 | common, storage, serde, serde_json, chrono, tokio, tracing, uuid |

---

## 2. LLM Provider System (`providers`)

### 2.1 LlmProvider Trait (`types.rs:87-148`)

The core abstraction for all LLM backends. Defined as an async trait requiring `Send + Sync`:

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: &[Message], tools: Option<&[Value]>, params: &ChatParams) -> Result<LlmResponse>;
    async fn chat_stream(&self, messages: &[Message], tools: Option<&[Value]>, params: &ChatParams) -> Result<LlmStream>;
    fn supports_streaming(&self) -> bool;
    fn default_model(&self) -> &str;
    fn name(&self) -> &str;
    async fn count_tokens(&self, messages: &[Message], tools: Option<&[Value]>) -> Result<usize>;
    fn capabilities(&self) -> ProviderCapabilities;
    fn context_window(&self) -> usize;
}
```

**Key design decisions:**
- `chat_stream()` has a **default implementation** that calls `chat()` and wraps the result in a single-element stream. Providers that don't support streaming work out of the box.
- `count_tokens()` default is a **character-based estimation** (`json.len() / 4`). Only `AnthropicNativeProvider` overrides with a real API call.
- `capabilities()` default returns a struct with `vision: true`, `streaming: true`, `parallel_tool_calls: true` as defaults; everything else false.
- `context_window()` defaults to `DEFAULT_CONTEXT_WINDOW = 128,000`.

**Type alias:** `DynProvider = Arc<dyn LlmProvider>` — used everywhere for dynamic dispatch.

### 2.2 Message Types (`types.rs:254-378`)

Messages are a tagged enum with serde `#[serde(tag = "role", rename_all = "lowercase")]`:

| Variant | Fields | Notes |
|---------|--------|-------|
| `System` | `content: String` | |
| `User` | `content: UserContent` | Text or MultiPart (vision) |
| `Assistant` | `content: Option<String>`, `tool_calls: Option<Vec<ToolCallMessage>>`, `reasoning_content: Option<String>` | Supports thinking models |
| `Tool` | `tool_call_id`, `name`, `content` | Tool result |

`UserContent` is untagged: either `Text(String)` or `MultiPart(Vec<ContentPart>)`.

`ContentPart` supports `Text` and `ImageUrl` variants (vision).

**Convenience constructors:** `Message::system()`, `Message::user()`, `Message::user_multipart()`, `Message::assistant()`, `Message::assistant_with_tools()`, `Message::tool()`.

### 2.3 ChatParams (`types.rs:59-84`)

Builder-style parameters:
- `model: String` (required)
- `temperature: Option<f32>`
- `max_tokens: Option<u32>`

### 2.4 LlmResponse (`types.rs:154-172`)

```rust
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: String,
    pub usage: Usage,
    pub reasoning_content: Option<String>,  // DeepSeek-R1, Claude thinking
}
```

### 2.5 Usage (`types.rs:235-251`)

Tracks token usage including **cache metrics** (Anthropic prompt caching):
- `prompt_tokens`, `completion_tokens`, `total_tokens`
- `cache_read_tokens`, `cache_write_tokens` (both default to 0)

### 2.6 ProviderCapabilities (`types.rs:207-232`)

8 boolean capability flags for adaptive orchestration:
- `extended_thinking`, `structured_outputs`, `prompt_caching`
- `native_token_counting`, `vision`, `streaming`
- `tool_choice_required`, `parallel_tool_calls`

### 2.7 Streaming Types (`types.rs:28-56`)

- `LlmStreamChunk`: `content`, `tool_call_delta`, `is_final`, `finish_reason`, `reasoning_content`
- `ToolCallDelta`: `index`, `id`, `name`, `arguments` (all optional except index)
- `LlmStream = Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>`

### 2.8 HTTP Error Mapping (`types.rs:16-25`)

Centralized `map_http_error()` function used by all providers:
- 429 -> `ProviderError::RateLimited`
- 401/403 -> `ProviderError::AuthFailed`
- Other -> `ProviderError::InvalidResponse`

---

## 3. Provider Implementations

### 3.1 AnthropicNativeProvider (`anthropic_native.rs`)

Uses Anthropic Messages API directly (not OpenAI-compat). Key features:

**Construction:** `new(api_key: Secret<String>, base_url: String, model: String)` with 120s HTTP timeout.

**Message Conversion (`convert_messages`):**
- System messages: filtered out (handled separately via top-level `system` field)
- User messages: converted to `[{"type": "text", "text": ...}]` content blocks; multipart images use `{"type": "image", "source": {"type": "url", "url": ...}}`
- Assistant messages: text blocks + `tool_use` blocks with parsed input JSON
- Tool results: wrapped as `user` role messages with `tool_result` content blocks (Anthropic API requirement)

**Tool Schema Conversion (`convert_tools`):**
- OpenAI format: `{"type": "function", "function": {"name", "description", "parameters"}}`
- Anthropic format: `{"name", "description", "input_schema"}`

**Response Parsing (`parse_response`):**
- Handles `text`, `tool_use`, and `thinking` content blocks
- Maps `stop_reason`: `end_turn` -> `stop`, `tool_use` -> `tool_calls`, `max_tokens` -> `length`
- Extracts cache usage from `cache_read_input_tokens` and `cache_creation_input_tokens`

**Prompt Caching:** System prompt sent with `"cache_control": {"type": "ephemeral"}`.

**Token Counting:** Uses `/v1/messages/count_tokens` API endpoint. Falls back to character estimation on failure.

**Capabilities:**
- `extended_thinking: true`, `prompt_caching: true`, `native_token_counting: true`
- `vision: true`, `streaming: true`, `tool_choice_required: true`, `parallel_tool_calls: true`
- Context window: 200,000 tokens

**Note:** `chat_stream()` is NOT overridden — it uses the default fallback (single-chunk from `chat()`), despite `supports_streaming()` returning `true`. This is a **gap**.

### 3.2 OpenAiCompatProvider (`openai_compat.rs`)

Generic OpenAI-compatible provider for all non-native Anthropic backends.

**Construction:** `new(api_base, api_key, default_model)` with 120s timeout. Also `with_extra_headers()` for gateway-specific headers.

**Request Format:** Standard OpenAI chat completions API (`/chat/completions`). Messages sent directly as-is (serde serialization matches OpenAI format). Tools use `"tool_choice": "auto"`.

**Streaming (`chat_stream`):**
- Full SSE streaming implementation
- Uses `reqwest::bytes_stream()` -> `scan()` to accumulate line buffer -> parse `data: {...}` SSE events
- `parse_sse_chunk()` handles `[DONE]` marker, content deltas, reasoning_content deltas, tool_call deltas
- Stream errors are logged and recovered from (non-fatal per chunk)

**Response Parsing (`parse_response`):**
- Uses strongly-typed `ChatCompletionResponse` struct with serde deserialization
- Handles malformed tool arguments by wrapping in `{"raw": ...}` fallback
- Extracts `reasoning_content` for thinking models (DeepSeek-R1)

**Capabilities:** Uses trait defaults (no override) — `streaming: true`, `vision: true`, `parallel_tool_calls: true`; everything else false. Context window: default 128,000.

### 3.3 TranscriptionProvider (`transcription.rs`)

Separate from LlmProvider — voice-to-text only.

- Uses Groq Whisper API (`whisper-large-v3`)
- Endpoint: `https://api.groq.com/openai/v1/audio/transcriptions`
- Sends audio as multipart form with `audio/ogg` MIME type
- 60s HTTP timeout
- Returns plain text transcription

---

## 4. Provider Auto-Detection & Registry (`registry.rs`, `lib.rs`)

### 4.1 ProviderSpec

Static metadata struct for each provider:
- `name`, `keywords`, `env_key`, `display_name`
- `prefix`, `skip_prefixes` (for model name routing)
- `is_gateway`, `is_local`
- `detect_by_key_prefix`, `detect_by_base_keyword` (auto-detection)
- `default_api_base`
- `strip_model_prefix` (for gateways like AiHubMix)
- `model_overrides` (per-model parameter overrides, e.g., Kimi K2.5 requires temp >= 1.0)

### 4.2 Registered Providers (PROVIDERS static array)

12 providers in 4 categories:

| Category | Providers |
|----------|-----------|
| **Gateways** | OpenRouter (`sk-or-` key prefix), AiHubMix (URL keyword detection) |
| **Standard** | Anthropic (`claude`), OpenAI (`gpt`), DeepSeek (`deepseek`), Gemini (`gemini`), Zhipu (`glm`/`zai`), DashScope (`qwen`), Moonshot (`kimi`), MiniMax |
| **Local** | vLLM (`http://localhost:8000/v1`) |
| **Auxiliary** | Groq (mainly for transcription) |

### 4.3 ProviderRegistry Lookup Methods

| Method | Purpose | Notes |
|--------|---------|-------|
| `find_by_model(model)` | Match by keyword in model name | Skips gateways and local providers |
| `find_by_name(name)` | Exact match on config field name | Returns any provider type |
| `find_gateway(name, key, base)` | Detect gateway/local provider | Priority: name > key prefix > base URL keyword |
| `resolve_model(model, gateway)` | Apply model name prefixing | Handles strip+re-prefix for gateways |
| `get_model_overrides(model)` | Get per-model param overrides | Currently only Kimi K2.5 temperature |

### 4.4 create_provider() Factory (`lib.rs:36-114`)

4-priority resolution chain:

1. **Explicit provider field** (`config.agents.defaults.provider`) — if set, try that specific provider
2. **Model name matching** — route `claude-*` to Anthropic, `gpt-*` to OpenAI, etc.
3. **Gateway detection** — check all provider configs for API key prefix or base URL keywords
4. **Fallback** — first provider with any non-empty API key

**Native provider toggle:** When `pc.native == true` and provider is `"anthropic"`, creates `AnthropicNativeProvider` instead of `OpenAiCompatProvider`. This is the only provider-specific branch.

### 4.5 try_create_from_spec() (`lib.rs:118-162`)

Internal helper that maps `ProviderSpec.name` to the correct config field (`config.providers.anthropic`, etc.) and creates the provider. Uses a match on 12 provider names.

---

## 5. Circuit Breaker & ProviderManager (`manager.rs`)

### 5.1 Architecture

`ProviderManager` wraps primary + optional fallback providers with resilience patterns. It itself implements `LlmProvider`, making it composable.

**Fields:**
- `primary: DynProvider`
- `fallback: Option<DynProvider>`
- `classifier_provider: Option<DynProvider>` (public, for complexity classifier)
- `failure_count: Arc<AtomicU32>` (consecutive failures)
- `circuit_open_until: Arc<RwLock<Option<Instant>>>` (circuit breaker state)
- `circuit_config: CircuitBreakerConfig`

### 5.2 CircuitBreakerConfig

- `failure_threshold: u32` (default: 5) — consecutive failures to open circuit
- `reset_timeout_secs: u64` (default: 60) — seconds before circuit half-opens

### 5.3 Retry Logic (`try_primary_with_retry`)

- **3 attempts max** with exponential backoff: 500ms -> 1s -> 2s
- **Only retries `RateLimited`** errors — all other errors fail fast immediately
- On success: resets failure counter
- On exhausted retries: records failure (contributes to circuit breaker)

### 5.4 Circuit Breaker State Machine

```
Closed (normal) --[N consecutive failures]--> Open (bypass primary)
Open --[timeout expires]--> Half-Open (try primary again)
Half-Open --[success]--> Closed
Half-Open --[failure]--> Open (re-trips)
```

**Implementation detail:** When circuit opens, `failure_count` is reset to 0. The circuit reopens if the threshold is hit again after timeout. There's no explicit "half-open" state — the `is_circuit_open()` check simply returns `false` once the timeout expires, and the normal try-primary path executes.

### 5.5 Failover Behavior

| Scenario | Primary | Fallback | Behavior |
|----------|---------|----------|----------|
| Circuit closed, success | 1 call | 0 calls | Direct return |
| Circuit closed, rate-limited | 3 calls (retries) | 1 call | Retry then fallback |
| Circuit closed, auth error | 1 call | 1 call | Fail fast, fallback |
| Circuit open | 0 calls | 1 call | Skip primary entirely |
| Circuit closed, no fallback, error | 1+ calls | N/A | Return error |

### 5.6 Streaming Failover

`chat_stream()` does NOT use retry logic — it tries primary once, then falls back on any error. This is a simpler path than `chat()`.

### 5.7 Delegation

`ProviderManager.name()` returns `"provider-manager"` (not the primary's name). `default_model()`, `capabilities()`, `context_window()`, `supports_streaming()` all delegate to primary.

---

## 6. Context Engine (`context_engine`)

### 6.1 Architecture

Three components orchestrated by `ContextEngine`:
1. **BudgetAllocator** — waterfall token budget allocation
2. **HistoryCompressor** — extractive summarization of old messages
3. **ContextEngine (assembler)** — orchestrates assembly into `AssembledContext`

### 6.2 BudgetAllocator (`budget.rs`)

**Priority enum (8 levels, highest first):**
1. `SystemIdentity` — system prompt
2. `ActiveTask` — current task context
3. `ToolDefinitions` — tool JSON schemas
4. `RecentHistory` — verbatim recent messages
5. `RetrievedMemory` — embedding-based memory
6. `CompressedHistory` — summarized old messages
7. `BootstrapPersona` — persona/personality
8. `Skills` — loaded skill prompts

**BudgetConfig:**
- `total_context_window` — model's full window size
- `response_reserve_pct` — default 15% reserved for response generation
- `available_input()` = `total_context_window * 0.85`

**Allocator API:**
- `allocate(priority, tokens)` — allocate up to `tokens`, capped at remaining
- `try_allocate(priority, tokens)` -> `usize` — returns how many were actually allocated
- `remaining()` — tokens still available
- `total_allocated()` — sum of all allocations
- `report()` -> `BudgetReport` — summary with per-priority breakdown

**Key behavior:** Allocations are first-come-first-served, not priority-ordered. The caller (ContextEngine) is responsible for calling `allocate()` in the correct priority order.

### 6.3 HistoryCompressor (`history_compressor.rs`)

**Strategy:**
- Always keeps at least `min_recent_messages` (default: 4) verbatim from the end
- Uses up to half of remaining budget to expand the recent window further back
- Summarizes older messages in chunks of 5 using extractive summarization

**Extractive Summary (`extractive_summary`):**
- No LLM call — purely text-based
- Takes first 100 chars (at sentence boundary) from each User and Assistant message
- Format: `"Earlier in this conversation:\n- User: ...\n- Assistant: ..."`
- Ignores System and Tool messages

**Token Estimation:** `len() / 4` for text, `parts.len() * 10` for multipart, assistant gets +20 overhead, tool gets +10 overhead.

### 6.4 ContextEngine Assembler (`assembler.rs`)

**ExecutionStrategy enum:**
- `DirectResponse` — no tool use needed
- `ToolAssisted { max_iterations }` — may use tools
- `AutonomousTask { max_iterations }` — full autonomous execution
- `Clarification { reason }` — need more user info

**ContextRequest input:**
- `message_text`, `history`, `system_prompt`, `strategy`
- `tool_definitions`, `memory_path`, `context_window`

**Assembly pipeline (`assemble()`):**
1. Allocate system prompt tokens (Priority::SystemIdentity)
2. Allocate tool definition tokens (only for ToolAssisted/AutonomousTask strategies — DirectResponse/Clarification get 0)
3. Split remaining budget: 60% for recent history, 40% for compressed history
4. Compress history via HistoryCompressor
5. Build final message list: system prompt -> summaries (as system messages) -> recent messages

**Output (AssembledContext):** `messages: Vec<Message>`, `token_count: usize`, `budget_report: BudgetReport`

---

## 7. Session Management (`session`)

### 7.1 Session Struct (`manager.rs:19-84`)

```rust
pub struct Session {
    pub key: String,                              // e.g., "telegram:chat123"
    pub messages: Vec<SessionMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

**SessionMessage:**
- `id: String` (UUID v4, auto-generated)
- `role: String` (system/user/assistant/tool)
- `content: String`
- `timestamp: DateTime<Utc>`
- `request_id: Option<String>` (for message correlation)

**API:** `new(key)`, `add_message(role, content)`, `add_message_with_request_id(...)`, `get_history(max_messages)`, `clear()`.

**Note:** `Session.messages` uses string roles (not the `Message` enum from providers). This is a **separate representation** from `providers::Message` — conversion happens at the agent layer.

### 7.2 SessionManager (`manager.rs:115-515`)

Dual-backend session manager with LRU caching.

**Backends:**
1. **JSONL files** — default, one `.jsonl` file per session
2. **SQL (PostgreSQL)** — via `storage::SessionRepo`, optional

**Construction:**
- `new(sessions_dir)` — JSONL backend, 1000-session cache
- `with_capacity(sessions_dir, max_cache_size)` — JSONL with custom cache size
- `from_repo(repo: storage::SessionRepo)` — SQL backend

**LRU Cache:**
- `cache: HashMap<String, Session>` — in-memory sessions
- `lru_order: VecDeque<String>` — tracks access order
- On `get_or_create()`: updates LRU order, evicts oldest if over capacity
- Evicted sessions are saved before removal (errors logged, not propagated)

**CRUD operations:**

| Method | JSONL Path | SQL Path |
|--------|-----------|----------|
| `get_or_create(key)` | Load from `.jsonl` or create new | Query `SessionRepo.get_session()` or `create_session()` |
| `save(session)` | Atomic write (write to `.tmp`, rename) with compaction | Upsert session + insert messages (ignore duplicates) |
| `save_by_key(key)` | Save from cache if exists | Same |
| `delete(key)` | Remove file | `delete_session()` |
| `list()` | Read directory, parse all `.jsonl` files | `list_sessions()` (message_count always 0) |

### 7.3 JSONL Persistence Format

Each session is a single `.jsonl` file with:
- **Line 1:** Metadata record: `{"_type": "metadata", "key": "...", "created_at": "...", "updated_at": "...", "metadata": {...}}`
- **Lines 2+:** Message records: `{"id": "uuid", "role": "...", "content": "...", "timestamp": "...", "request_id": null}`

**Key sanitization:** `:`, `/`, `\` replaced with `_` for filesystem safety.

**Atomic writes:** Write to `.jsonl.tmp`, then `rename()` — prevents corruption on crash.

**Write mutex:** `tokio::sync::Mutex` prevents concurrent writes to the same session file.

### 7.4 Compaction

- **Threshold:** 1,000 messages triggers compaction
- **Keep:** Last 500 messages
- **Trigger:** On `save()`, before writing to disk
- Only applies to JSONL path (SQL path doesn't compact)

### 7.5 SQL Backend Behavior

When `sql_repo` is `Some`:
- Session metadata stored via `SessionRepo`
- Messages persisted individually with UUID primary key
- Duplicate message inserts silently ignored (ON CONFLICT)
- `list()` returns `message_count: 0` for all sessions (would require extra query)

---

## 8. Token Counting Accuracy

| Provider | Method | Accuracy |
|----------|--------|----------|
| AnthropicNativeProvider | `/v1/messages/count_tokens` API | **Exact** (with graceful fallback to estimation) |
| OpenAiCompatProvider | Default: `json.len() / 4` | **Rough estimate** (~25% error typical) |
| ContextEngine | `text.len() / 4` + overhead constants | **Rough estimate** |
| HistoryCompressor | `text.len() / 4` + overhead constants | **Rough estimate** |

**Impact:** Budget allocation in ContextEngine uses character-based estimation regardless of which provider is active. The native token counting API is only used at the provider level (for reporting/validation), not for context assembly decisions.

---

## 9. Gap Analysis

### 9.1 Critical Gaps

| # | Gap | Impact | Recommendation |
|---|-----|--------|----------------|
| G1 | **AnthropicNativeProvider lacks streaming implementation** | `supports_streaming()` returns `true` but `chat_stream()` uses default single-chunk fallback. Real-time UX is degraded for Anthropic native mode. | Implement SSE streaming similar to OpenAiCompatProvider, parsing Anthropic's `event: message_start/content_block_delta/message_stop` format. |
| G2 | **Context assembly uses character estimation, not provider token counting** | Budget allocation can be 25%+ off, leading to context truncation or wasted window. | Wire the active provider's `count_tokens()` into ContextEngine. Use char estimation as fallback only. |
| G3 | **No memory retrieval in ContextEngine.assemble()** | `ContextRequest.memory_path` is accepted but never used. `Priority::RetrievedMemory` and `Priority::BootstrapPersona` are defined but never allocated. | Implement embedding-based memory retrieval (via `storage::EmbeddingRepo`) and integrate into assembly pipeline. |
| G4 | **Session message format is decoupled from provider Message type** | Session stores `role: String` + `content: String`, losing tool calls, multipart content, reasoning content. Reloaded sessions can't reconstruct full LLM context. | Either store `providers::Message` directly, or add fields for tool_calls, content_parts, reasoning_content. |

### 9.2 Moderate Gaps

| # | Gap | Impact | Recommendation |
|---|-----|--------|----------------|
| G5 | **ProviderManager streaming has no retry** | Rate-limited streaming requests fail immediately with no backoff. | Add retry logic to `chat_stream()` matching `try_primary_with_retry()` behavior. |
| G6 | **No per-model context window mapping** | `OpenAiCompatProvider` returns default 128K for all models. GPT-4 (8K), GPT-4-turbo (128K), GPT-3.5 (16K) all get the same window. | Add a model -> context_window lookup table in the registry. |
| G7 | **SQL session list() always returns message_count: 0** | Session listing in SQL mode can't show message counts without N+1 queries. | Add a `COUNT(*)` join or subquery in `SessionRepo.list_sessions()`. |
| G8 | **No structured output support** | `ProviderCapabilities.structured_outputs` is always `false`. No JSON mode or response format parameter. | Add `response_format` to `ChatParams` and pass through for providers that support it (OpenAI, Anthropic). |
| G9 | **create_provider() doesn't create ProviderManager** | The factory returns a single provider, not a ProviderManager with failover. ProviderManager creation must happen at a higher layer. | Consider wiring primary+fallback provider creation into `create_provider()` based on config. |
| G10 | **Anthropic image handling uses URL source only** | No support for base64-encoded images in Anthropic native provider. | Add base64 source type for inline image content. |

### 9.3 Minor Gaps / Tech Debt

| # | Gap | Impact | Recommendation |
|---|-----|--------|----------------|
| G11 | **Hardcoded Groq endpoint in TranscriptionProvider** | Can't use alternative Whisper providers or self-hosted. | Make endpoint configurable. |
| G12 | **HistoryCompressor is purely extractive** | Summaries lose nuance — just first 100 chars of each message. | Consider LLM-based summarization for long conversations (opt-in, uses extra API calls). |
| G13 | **BudgetAllocator priority ordering is caller-enforced** | If caller allocates in wrong order, lower-priority content can starve higher-priority. | Consider sorting allocations by priority internally, or at minimum documenting the contract. |
| G14 | **No health check endpoint for providers** | No way to test if a provider is reachable before routing traffic. | Add `async fn health_check(&self) -> bool` to LlmProvider trait. |
| G15 | **Session compaction only on JSONL path** | SQL sessions grow unbounded. | Add SQL-side compaction (DELETE old messages beyond threshold). |
| G16 | **No model parameter overrides applied at request time** | `ProviderRegistry::get_model_overrides()` exists but is never called in the request path. | Wire overrides into `ChatParams` construction in the agent layer. |
| G17 | **AnthropicNativeProvider API version hardcoded** | `ANTHROPIC_VERSION = "2023-06-01"` — may need updates for new features. | Make configurable or update periodically. |
| G18 | **No request/response logging for debugging** | Provider calls are only logged at debug level with minimal info (model, message count). | Add optional verbose request/response logging (with PII redaction). |

---

## 10. Test Coverage Summary

| Module | Tests | Key Coverage |
|--------|-------|-------------|
| `types.rs` | 4 tests | Capabilities default, usage cache fields, context window constant |
| `anthropic_native.rs` | 11 tests | Message conversion (all variants), tool schema conversion, response parsing (text, tool_use, missing usage, max_tokens), capabilities, context window |
| `openai_compat.rs` | 0 tests | **No unit tests** — only integration-level coverage |
| `manager.rs` | 8 tests | Primary/fallback routing, rate-limit retry, non-retryable failover, no-fallback error, circuit breaker open/reset, retry backoff success, failure counter reset, delegation |
| `registry.rs` | 22 tests | Model lookup (5 providers), name lookup, gateway detection (name/key/base/priority), model resolution (prefix/skip/gateway/strip), overrides, validation (unique names, valid env keys, valid API bases) |
| `transcription.rs` | 0 tests | **No unit tests** |
| `assembler.rs` | 5 tests | Direct response (no tools), tool-assisted (with tools), context fits window, empty history, clarification (no tools) |
| `budget.rs` | 4 tests | Standard config, allocation within budget, try_allocate cap, report, allocate capped at available |
| `history_compressor.rs` | 5 tests | Recent kept verbatim, min_recent enforced, empty history, small history all verbatim, summary format |
| `session/manager.rs` | 15 tests | CRUD, save/load round-trip, path sanitization, delete, list, message IDs (unique, persisted), atomic writes, compaction (above/below threshold), request_id, eviction resilience |

**Notable gaps in test coverage:**
- OpenAiCompatProvider has zero unit tests (relies on integration tests)
- TranscriptionProvider has zero tests
- No tests for SQL session backend path
- No tests for `create_provider()` factory function
- No test for `ProviderManager.chat_stream()` failover behavior

---

## 11. Public API Surface

### providers crate exports (`lib.rs`)

```rust
// Types
pub use types::{ChatParams, ContentPart, DynProvider, FunctionCall, ImageUrl,
    LlmProvider, LlmResponse, LlmStream, LlmStreamChunk, Message,
    ProviderCapabilities, ToolCall, ToolCallDelta, ToolCallMessage,
    Usage, UserContent, DEFAULT_CONTEXT_WINDOW, tool_calls_to_messages};

// Providers
pub use anthropic_native::AnthropicNativeProvider;
pub use openai_compat::OpenAiCompatProvider;
pub use transcription::TranscriptionProvider;

// Registry & Manager
pub use registry::{ProviderRegistry, ProviderSpec, PROVIDERS};
pub use manager::{CircuitBreakerConfig, ProviderManager};

// Factory
pub fn create_provider(config: &Config) -> Result<DynProvider>;
```

### context_engine crate exports (`lib.rs`)

```rust
pub use assembler::{AssembledContext, ContextEngine, ContextRequest, ExecutionStrategy};
pub use budget::{BudgetAllocator, BudgetConfig, BudgetReport, Priority};
pub use history_compressor::{CompressedHistory, HistoryCompressor, HistorySummary};
```

### session crate exports (`lib.rs`)

```rust
pub use manager::{Session, SessionInfo, SessionManager, SessionMessage};
```

---

## 12. Architectural Observations

1. **Clean trait abstraction**: `LlmProvider` is well-designed with sensible defaults. New providers only need to implement `chat()`, `name()`, and `default_model()`.

2. **Provider registry is data-driven**: The static `PROVIDERS` array makes adding new providers trivial — just add a `ProviderSpec` entry. No code changes needed for the routing logic.

3. **ProviderManager is composable**: It implements `LlmProvider` itself, so it can be used anywhere a provider is expected. This enables layered resilience (wrap primary in manager, wrap manager in another manager if needed).

4. **Context engine is synchronous**: `ContextEngine::assemble()` is not async — it uses character-based estimation rather than calling the provider for token counts. This is a deliberate trade-off: fast assembly at the cost of accuracy.

5. **Session has dual persistence**: JSONL for simplicity/portability, SQL for production. The LRU cache ensures performance regardless of backend. The migration path from JSONL to SQL is clean.

6. **Anthropic native is a first-class citizen**: The native provider gets prompt caching, real token counting, and content block handling. All other providers go through OpenAI compat.

7. **Budget priorities are well-layered**: 8 priority levels allow fine-grained control over what gets context window space, but several levels (RetrievedMemory, BootstrapPersona, Skills) are defined but unused in the current assembly pipeline.
