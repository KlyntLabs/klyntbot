# Providers Crate

**Crate**: `providers` (Layer 2)
**Path**: `crates/providers/`
**Dependencies**: `common`, `config`, `async-trait`, `reqwest`, `serde`, `serde_json`, `futures-util`, `tokio`, `tracing`, `base64`

---

## Section 1: Narrative Overview

### Purpose

The `providers` crate is the LLM abstraction layer for Klyntbot. It defines a unified `LlmProvider` trait that all LLM backends implement, plus a static provider registry that routes model names to the correct API endpoint. The crate ships two concrete implementations: `AnthropicNativeProvider` (Anthropic Messages API) and `OpenAiCompatProvider` (OpenAI chat completions format). A `ProviderManager` wraps any provider with retry, failover, and circuit breaker logic. A standalone `TranscriptionProvider` handles voice-to-text via Groq Whisper.

### The LlmProvider Trait

Defined in `crates/providers/src/types.rs:150`. Every LLM backend implements this `async_trait`:

- **`chat()`** -- non-streaming request, returns a complete `LlmResponse`.
- **`chat_stream()`** -- streaming request, returns an `LlmStream` (a `Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>`). Has a default implementation that calls `chat()` and wraps the result in a single-chunk stream.
- **`count_tokens()`** -- estimates input token count. Default uses character-based estimation (4 chars per token). The native Anthropic provider overrides this with the actual `/v1/messages/count_tokens` API.
- **`capabilities()`** -- returns `ProviderCapabilities` flags that the agent loop uses for adaptive behavior (e.g. whether extended thinking, prompt caching, or structured outputs are available).
- **`context_window()`** -- returns the model's context window size in tokens. Defaults to `DEFAULT_CONTEXT_WINDOW` (128,000).
- **`health_check()`** -- probes the provider endpoint. Default returns `ProviderHealth::Unknown`.

The type alias `DynProvider = Arc<dyn LlmProvider>` is the standard handle used throughout the codebase.

### Provider Auto-Detection

The `create_provider()` factory function in `crates/providers/src/lib.rs:37` resolves which backend to use via a four-priority cascade:

1. **Explicit provider field** -- `config.agents.defaults.provider`. If set and the provider has an API key, use it directly. If the configured model does not match the provider's keywords, the provider's own `default_model` is substituted.
2. **Model name matching** -- `ProviderRegistry::find_by_model()` scans the static `PROVIDERS` array for keyword matches (e.g. `"claude"` maps to Anthropic, `"gpt"` maps to OpenAI). Gateways and local providers are excluded from this scan.
3. **Gateway detection** -- `ProviderRegistry::find_gateway()` checks API key prefixes (e.g. `"sk-or-"` for OpenRouter) and `api_base` URL substrings (e.g. `"aihubmix"` for AiHubMix).
4. **First available key** -- iterates all 12 configured provider slots and uses the first one with a non-empty API key.

If no provider can be resolved, a `ConfigError::MissingField` is returned.

### Anthropic Native Client

**File**: `crates/providers/src/anthropic_native.rs`
**Struct**: `AnthropicNativeProvider`

Uses Anthropic's Messages API directly rather than the OpenAI-compatible format. This enables three exclusive features:

- **Prompt caching** -- when `cache_system_prompt` is `true` (the default), system prompts are wrapped with `"cache_control": {"type": "ephemeral"}`. The usage response exposes `cache_read_tokens` and `cache_write_tokens`.
- **Native token counting** -- overrides the default `count_tokens()` with a call to `/v1/messages/count_tokens`, falling back to character estimation on failure.
- **Extended thinking** -- when `ExtendedThinkingConfig` is provided and enabled, a `"thinking"` block is injected into the request body with the configured `budget_tokens`. Temperature is automatically removed (required by the API).

Message conversion (`convert_messages()`) translates the internal `Message` enum into Anthropic's content-block format. Notably, tool results are sent as `user`-role messages with `tool_result` content blocks, as required by the Anthropic API. Tool schemas are converted from OpenAI format (`function.parameters`) to Anthropic format (`input_schema`).

Structured output is handled by `apply_response_format()`: `JsonSchema` injects a synthetic tool with the desired schema and forces `tool_choice` to that tool; `JsonObject` prepends a JSON-only instruction to the system prompt.

Authentication uses the `x-api-key` header with `anthropic-version` set to `"2023-06-01"` by default (overridable via `ANTHROPIC_API_VERSION` env var). The HTTP client has a 120-second timeout.

Health checks send a minimal messages request and interpret the status: 200 = Healthy, 401/403 = Unhealthy (auth), 429 = Degraded (rate limited), 529 = Unhealthy (overloaded), other 4xx = Healthy (API reachable).

Context window is hardcoded at 200,000 tokens.

### OpenAI-Compatible Client

**File**: `crates/providers/src/openai_compat.rs`
**Struct**: `OpenAiCompatProvider`

A generic client that speaks the OpenAI `/chat/completions` protocol. Used for OpenAI, DeepSeek, Gemini, Groq, Zhipu, DashScope, Moonshot, MiniMax, vLLM, OpenRouter, AiHubMix, and any other provider exposing an OpenAI-compatible endpoint.

Key behaviors:

- **Model context window lookup** -- `model_context_window()` uses prefix matching against known model families (GPT-4, GPT-4o, o-series, GPT-3.5-turbo) to return accurate context window sizes. Unrecognized models default to 128K.
- **Model overrides** -- before each request, `ProviderRegistry::get_model_overrides()` checks for per-model parameter requirements (e.g. Kimi K2.5 requires temperature >= 1.0). Overrides are applied when the user does not set the parameter explicitly.
- **Extra headers** -- the `with_extra_headers()` builder method supports provider-specific headers (e.g. `APP-Code` for AiHubMix).
- **Structured output** -- `serialize_response_format()` maps `ResponseFormat::JsonSchema` to OpenAI's `json_schema` format with `strict: true`.

Authentication uses the standard `Bearer` token in the `Authorization` header. HTTP timeout is 120 seconds.

Health checks hit `/models` with a 5-second timeout client.

Capabilities report `structured_outputs: true` and default values for everything else. The provider name reported is `"openai-compat"`.

### Streaming vs Non-Streaming

Both providers implement true SSE (Server-Sent Events) streaming:

- **OpenAI-compat** -- parses standard `data: {...}` lines. The `[DONE]` marker signals end of stream. Uses `reqwest::Response::bytes_stream()` with `futures_util::StreamExt::scan()` to buffer partial lines and emit parsed `LlmStreamChunk` items.
- **Anthropic native** -- parses Anthropic's event-typed SSE format with both `event:` and `data:` lines. Event types include `content_block_start`, `content_block_delta` (with sub-types `text_delta`, `input_json_delta`, `thinking_delta`), `message_delta` (stop reason), and `error`. Stop reasons are normalized to OpenAI-style finish reasons (`end_turn` -> `"stop"`, `tool_use` -> `"tool_calls"`, `max_tokens` -> `"length"`).

Providers that do not support streaming get a default `chat_stream()` implementation that calls `chat()` and wraps the result in a one-element stream.

### Provider Registry

**File**: `crates/providers/src/registry.rs`
**Static**: `PROVIDERS: &[ProviderSpec]` (13 entries)

The registry is a compile-time array of `ProviderSpec` structs. Each entry defines a provider's name, model-name keywords, environment variable, API base URL, default model, prefix rules, gateway/local flags, and auto-detection hints.

Provider categories:

| Category | Providers |
|----------|-----------|
| Gateways | OpenRouter, AiHubMix |
| Standard | Anthropic, OpenAI, DeepSeek, Gemini, Zhipu, DashScope, Moonshot, MiniMax |
| Local | vLLM |
| Auxiliary | Groq |

Model name resolution (`resolve_model()`) handles prefixing logic for providers that require it (e.g. `deepseek-chat` -> `deepseek/deepseek-chat`), skips prefixing when the model already has the prefix, and applies gateway-specific stripping and re-prefixing (e.g. AiHubMix strips `anthropic/` and prepends `openai/`).

### Failover Strategy (ProviderManager)

**File**: `crates/providers/src/manager.rs`
**Struct**: `ProviderManager`

`ProviderManager` implements `LlmProvider` itself, wrapping a primary provider, an optional fallback provider, and an optional classifier provider. It provides three resilience mechanisms:

1. **Exponential backoff retry** -- rate-limited errors (HTTP 429) are retried up to 3 times with delays of 500ms, 1s, 2s. Non-retryable errors (auth failures, etc.) fail fast without retry.
2. **Automatic failover** -- when the primary exhausts retries or fails with a non-retryable error, the request is forwarded to the fallback provider (if configured).
3. **Circuit breaker** -- after `failure_threshold` (default: 5) consecutive failures, the circuit opens for `reset_timeout_secs` (default: 60). While open, all requests bypass the primary and go directly to the fallback. After the timeout, the circuit closes (half-open) and the primary is tried again. A single success resets the failure counter.

Created via `create_provider_with_failover()` in `crates/providers/src/lib.rs:141`. Only activates when `config.provider_manager.fallback` is set to a non-empty provider name. Otherwise, returns a plain `DynProvider` identical to `create_provider()`.

The manager also supports a `classifier_provider` field for routing complexity classification requests to a separate (potentially cheaper/faster) model.

### Token Counting

Two strategies:

1. **Character estimation** (default) -- `json.len() / 4`, used by `OpenAiCompatProvider` and as a fallback for all providers. Defined in the default `count_tokens()` implementation at `crates/providers/src/types.rs:196`.
2. **Native API counting** -- `AnthropicNativeProvider` calls `/v1/messages/count_tokens` for exact counts. On failure (non-2xx), it falls back to character estimation with a warning log.

### Transcription Service

**File**: `crates/providers/src/transcription.rs`
**Struct**: `TranscriptionProvider`

A standalone service for voice-to-text using Groq's Whisper API. Sends audio files as multipart form data to `/audio/transcriptions` with the `whisper-large-v3` model. Supports custom API base URLs via `with_api_base()`. HTTP timeout is 60 seconds. Error mapping uses the shared `map_http_error()` utility.

### HTTP Error Mapping

The shared function `map_http_error()` in `crates/providers/src/types.rs:18` centralizes HTTP status-to-error translation:

| HTTP Status | Error Variant |
|-------------|---------------|
| 429 | `ProviderError::RateLimited` (extracts `retry_after` from JSON body if present) |
| 401, 403 | `ProviderError::AuthFailed` |
| Other | `ProviderError::InvalidResponse` |

---

## Section 2: API Reference

### LlmProvider Trait

**File**: `crates/providers/src/types.rs:150`

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
    async fn health_check(&self) -> Result<ProviderHealth>;
}
```

Methods with default implementations: `chat_stream` (wraps `chat()` in single-chunk stream), `supports_streaming` (returns `false`), `count_tokens` (character estimation), `capabilities` (defaults), `context_window` (128K), `health_check` (returns `Unknown`).

### ProviderRegistry

**File**: `crates/providers/src/registry.rs:300`

| Method | Signature | Description |
|--------|-----------|-------------|
| `find_by_model` | `fn find_by_model(model: &str) -> Option<&'static ProviderSpec>` | Case-insensitive keyword match against model name. Skips gateways and local providers. |
| `find_by_name` | `fn find_by_name(name: &str) -> Option<&'static ProviderSpec>` | Exact match against provider config field name. |
| `find_gateway` | `fn find_gateway(provider_name: Option<&str>, api_key: Option<&str>, api_base: Option<&str>) -> Option<&'static ProviderSpec>` | Finds gateway/local providers by name, then key prefix, then base URL keyword. |
| `resolve_model` | `fn resolve_model(model: &str, gateway: Option<&ProviderSpec>) -> String` | Applies provider-specific model name prefixing. Gateway mode strips then re-prefixes. |
| `get_model_overrides` | `fn get_model_overrides(model: &str) -> HashMap<String, Value>` | Returns per-model parameter overrides (e.g. forced temperature for Kimi K2.5). |

### ProviderSpec

**File**: `crates/providers/src/registry.rs:10`

| Field | Type | Description |
|-------|------|-------------|
| `name` | `&'static str` | Config field name (e.g. `"anthropic"`) |
| `keywords` | `&'static [&'static str]` | Model-name keywords for matching (lowercase) |
| `env_key` | `&'static str` | Environment variable for API key |
| `display_name` | `&'static str` | Human-readable label |
| `prefix` | `&'static str` | Model prefix for routing |
| `skip_prefixes` | `&'static [&'static str]` | Skip prefixing if model already starts with these |
| `env_extras` | `&'static [(&'static str, &'static str)]` | Extra env vars to set |
| `is_gateway` | `bool` | Can route any model |
| `is_local` | `bool` | Local deployment |
| `detect_by_key_prefix` | `&'static str` | API key prefix for auto-detection |
| `detect_by_base_keyword` | `&'static str` | URL substring for auto-detection |
| `default_api_base` | `&'static str` | Default API base URL |
| `default_model` | `&'static str` | Default model when explicitly selected |
| `strip_model_prefix` | `bool` | Strip existing prefix before re-prefixing (gateways) |
| `model_overrides` | `&'static [(&'static str, &'static [(&'static str, &'static str)])]` | Per-model parameter overrides |

Methods:

| Method | Signature | Description |
|--------|-----------|-------------|
| `label` | `fn label(&self) -> &str` | Returns `display_name` if non-empty, otherwise `name` |

### AnthropicNativeProvider

**File**: `crates/providers/src/anthropic_native.rs:26`

**Construction**:

```rust
pub fn new(api_key: Secret<String>, base_url: String, model: String) -> Self
```

Builder methods (all consume and return `Self`):

| Method | Signature | Description |
|--------|-----------|-------------|
| `with_api_version` | `fn with_api_version(self, version: impl Into<String>) -> Self` | Override API version (default: `"2023-06-01"`) |
| `with_cache_system_prompt` | `fn with_cache_system_prompt(self, enabled: bool) -> Self` | Enable/disable prompt caching (default: `true`) |
| `with_extended_thinking` | `fn with_extended_thinking(self, config: Option<ExtendedThinkingConfig>) -> Self` | Configure extended thinking |

Public conversion methods:

| Method | Signature | Description |
|--------|-----------|-------------|
| `convert_messages` | `fn convert_messages(&self, messages: &[Message]) -> Vec<Value>` | Convert internal messages to Anthropic content-block format |
| `convert_tools` | `fn convert_tools(&self, openai_tools: &[Value]) -> Vec<Value>` | Convert OpenAI tool schemas to Anthropic `input_schema` format |

LlmProvider trait overrides: `chat`, `chat_stream`, `count_tokens` (native API), `capabilities` (all flags `true`), `context_window` (200,000), `supports_streaming` (`true`), `default_model`, `name` (`"anthropic-native"`), `health_check`.

### OpenAiCompatProvider

**File**: `crates/providers/src/openai_compat.rs:24`

**Construction**:

```rust
pub fn new(api_base: impl Into<String>, api_key: impl Into<String>, default_model: impl Into<String>) -> Result<Self>
```

Builder methods:

| Method | Signature | Description |
|--------|-----------|-------------|
| `with_extra_headers` | `fn with_extra_headers(self, headers: Vec<(String, String)>) -> Self` | Add provider-specific HTTP headers |

LlmProvider trait overrides: `chat`, `chat_stream`, `supports_streaming` (`true`), `default_model`, `name` (`"openai-compat"`), `capabilities` (`structured_outputs: true`), `context_window` (model-dependent lookup), `health_check` (GET `/models`).

### ProviderManager

**File**: `crates/providers/src/manager.rs:34`

**Construction**:

```rust
pub fn new(primary: DynProvider, fallback: Option<DynProvider>, classifier_provider: Option<DynProvider>) -> Self
pub fn with_config(primary: DynProvider, fallback: Option<DynProvider>, classifier_provider: Option<DynProvider>, circuit_config: CircuitBreakerConfig) -> Self
```

| Field | Type | Description |
|-------|------|-------------|
| `classifier_provider` | `Option<DynProvider>` | Public field; dedicated provider for complexity classification |

| Method | Signature | Description |
|--------|-----------|-------------|
| `check_health` | `async fn check_health(&self) -> (ProviderHealth, Option<ProviderHealth>)` | Returns health of primary and fallback |

LlmProvider trait: `chat` and `chat_stream` implement retry-with-backoff, failover, and circuit breaker logic. `name` returns `"provider-manager"`. `default_model`, `capabilities`, `context_window`, `supports_streaming` delegate to the primary. `health_check` delegates to primary via `check_health`.

### CircuitBreakerConfig

**File**: `crates/providers/src/manager.rs:17`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `failure_threshold` | `u32` | `5` | Consecutive failures before circuit opens |
| `reset_timeout_secs` | `u64` | `60` | Seconds before circuit resets to half-open |

### TranscriptionProvider

**File**: `crates/providers/src/transcription.rs:15`

**Construction**:

```rust
pub fn new(api_key: impl Into<String>) -> Result<Self>
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `with_api_base` | `fn with_api_base(self, api_base: impl Into<String>) -> Self` | Override API base (default: `https://api.groq.com/openai/v1`) |
| `transcribe` | `async fn transcribe(&self, audio_path: &str) -> Result<String>` | Transcribe audio file via Groq Whisper (`whisper-large-v3`). Returns transcribed text. |

### Core Types

#### Message

**File**: `crates/providers/src/types.rs:322`

Tagged enum serialized with `#[serde(tag = "role", rename_all = "lowercase")]`:

| Variant | Fields | Description |
|---------|--------|-------------|
| `System` | `content: String` | System prompt |
| `User` | `content: UserContent` | User message (text or multipart) |
| `Assistant` | `content: Option<String>`, `tool_calls: Option<Vec<ToolCallMessage>>`, `reasoning_content: Option<String>` | Assistant response |
| `Tool` | `tool_call_id: String`, `name: String`, `content: String` | Tool result |

Factory methods: `system()`, `user()`, `user_multipart()`, `assistant()`, `assistant_with_tools()`, `tool()`. Instance method: `role() -> MessageRole`.

#### UserContent

**File**: `crates/providers/src/types.rs:363`

Untagged enum:

| Variant | Type | Description |
|---------|------|-------------|
| `Text` | `String` | Plain text |
| `MultiPart` | `Vec<ContentPart>` | Mixed content blocks |

#### ContentPart

**File**: `crates/providers/src/types.rs:370`

Tagged enum (`#[serde(tag = "type", rename_all = "snake_case")]`):

| Variant | Fields | Description |
|---------|--------|-------------|
| `Text` | `text: String` | Text block |
| `ImageUrl` | `image_url: ImageUrl` | Image reference |

#### ImageUrl

**File**: `crates/providers/src/types.rs:378`

| Field | Type | Description |
|-------|------|-------------|
| `url` | `String` | Image URL (data URI or HTTPS) |

#### LlmResponse

**File**: `crates/providers/src/types.rs:222`

| Field | Type | Description |
|-------|------|-------------|
| `content` | `Option<String>` | Text response |
| `tool_calls` | `Vec<ToolCall>` | Tool calls requested by the LLM |
| `finish_reason` | `String` | `"stop"`, `"tool_calls"`, `"length"`, etc. |
| `usage` | `Usage` | Token counts |
| `reasoning_content` | `Option<String>` | Chain-of-thought (thinking models) |

#### ToolCall

**File**: `crates/providers/src/types.rs:243`

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Tool call ID |
| `name` | `String` | Tool name |
| `arguments` | `Value` | Parsed JSON arguments |

Method: `to_message() -> ToolCallMessage`.

#### ToolCallMessage

**File**: `crates/providers/src/types.rs:347`

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | Tool call ID |
| `type` | `String` | Always `"function"` |
| `function` | `FunctionCall` | Function name and arguments |

#### FunctionCall

**File**: `crates/providers/src/types.rs:355`

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Function name |
| `arguments` | `String` | JSON string of arguments |

#### Usage

**File**: `crates/providers/src/types.rs:303`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prompt_tokens` | `u32` | `0` | Input tokens |
| `completion_tokens` | `u32` | `0` | Output tokens |
| `total_tokens` | `u32` | `0` | Total tokens |
| `cache_read_tokens` | `u32` | `0` | Tokens read from cache (Anthropic) |
| `cache_write_tokens` | `u32` | `0` | Tokens written to cache (Anthropic) |

Method: `total() -> u32`.

#### ChatParams

**File**: `crates/providers/src/types.rs:102`

| Field | Type | Description |
|-------|------|-------------|
| `model` | `String` | Model identifier |
| `temperature` | `Option<f32>` | Sampling temperature |
| `max_tokens` | `Option<u32>` | Maximum output tokens |
| `response_format` | `Option<ResponseFormat>` | Structured output format |

Builder: `ChatParams::new(model)`, `.with_temperature(f32)`, `.with_max_tokens(u32)`, `.with_response_format(ResponseFormat)`.

#### ResponseFormat

**File**: `crates/providers/src/types.rs:91`

| Variant | Fields | Description |
|---------|--------|-------------|
| `Text` | -- | Plain text (default) |
| `JsonObject` | -- | Model outputs valid JSON |
| `JsonSchema` | `name: String`, `schema: Value` | Model outputs JSON conforming to schema |

#### ProviderCapabilities

**File**: `crates/providers/src/types.rs:275`

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `extended_thinking` | `bool` | `false` | Supports chain-of-thought thinking blocks |
| `structured_outputs` | `bool` | `false` | Supports JSON schema structured output |
| `prompt_caching` | `bool` | `false` | Supports prompt caching |
| `native_token_counting` | `bool` | `false` | Has native token counting API |
| `vision` | `bool` | `true` | Supports image inputs |
| `streaming` | `bool` | `true` | Supports streaming responses |
| `tool_choice_required` | `bool` | `false` | Supports forced tool choice |
| `parallel_tool_calls` | `bool` | `true` | Supports parallel tool execution |

#### ProviderHealth

**File**: `crates/providers/src/types.rs:137`

| Variant | Fields | Description |
|---------|--------|-------------|
| `Healthy` | -- | Provider responding normally |
| `Degraded` | `String` | Responding with degraded performance |
| `Unhealthy` | `String` | Not responding or returning errors |
| `Unknown` | -- | No health check implemented |

#### LlmStreamChunk

**File**: `crates/providers/src/types.rs:60`

| Field | Type | Description |
|-------|------|-------------|
| `content` | `Option<String>` | Incremental text delta |
| `tool_call_delta` | `Option<ToolCallDelta>` | Tool call delta |
| `is_final` | `bool` | True if final chunk |
| `finish_reason` | `Option<String>` | Present only in final chunk |
| `reasoning_content` | `Option<String>` | Thinking delta |

#### ToolCallDelta

**File**: `crates/providers/src/types.rs:79`

| Field | Type | Description |
|-------|------|-------------|
| `index` | `usize` | Tool call index |
| `id` | `Option<String>` | Tool call ID (first chunk only) |
| `name` | `Option<String>` | Tool name (first chunk only) |
| `arguments` | `Option<String>` | Partial JSON arguments |

#### LlmStream

**File**: `crates/providers/src/types.rs:87`

```rust
pub type LlmStream = Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>;
```

#### DynProvider

**File**: `crates/providers/src/types.rs:218`

```rust
pub type DynProvider = Arc<dyn LlmProvider>;
```

### Factory Functions

**File**: `crates/providers/src/lib.rs`

| Function | Signature | Description |
|----------|-----------|-------------|
| `create_provider` | `fn create_provider(config: &Config) -> Result<(DynProvider, String)>` | Resolves and creates an LLM provider from config. Returns `(provider, resolved_model_name)`. |
| `create_provider_with_failover` | `fn create_provider_with_failover(config: &Config) -> Result<(DynProvider, String)>` | Like `create_provider` but wraps in `ProviderManager` when `config.provider_manager.fallback` is set. |

### Free Functions

| Function | Signature | File:Line | Description |
|----------|-----------|-----------|-------------|
| `tool_calls_to_messages` | `fn tool_calls_to_messages(tool_calls: &[ToolCall]) -> Vec<ToolCallMessage>` | `types.rs:269` | Batch-convert tool calls to message format |

### Constants

| Name | Value | File:Line | Description |
|------|-------|-----------|-------------|
| `DEFAULT_CONTEXT_WINDOW` | `128_000` | `types.rs:382` | Default context window for unrecognized models |
| `PROVIDERS` | `&[ProviderSpec]` (13 entries) | `registry.rs:70` | Static array of all registered provider specs |
