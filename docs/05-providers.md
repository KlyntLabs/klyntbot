# Providers

## Purpose

The `providers` crate (Layer 2) is the LLM abstraction layer. It defines a single `LlmProvider` trait and ships two concrete implementations -- `AnthropicNativeProvider` and `OpenAiCompatProvider` -- that together cover 12+ LLM services. A `ProviderManager` wrapper adds retry, failover, and circuit-breaker logic on top of any provider. The crate also includes a `TranscriptionProvider` for speech-to-text via Groq Whisper and a `ProviderRegistry` that maps model names to provider metadata at zero runtime cost.

Everything above Layer 2 (tools, agent, channels) interacts with providers exclusively through `DynProvider`, a `Arc<dyn LlmProvider>` alias, so the rest of the codebase is provider-agnostic.

## Key Types

### Traits

**`LlmProvider`** -- the central async trait. Required methods: `chat()` (non-streaming completion), `default_model()`, `name()`. Default-implemented methods: `chat_stream()` (falls back to a single-chunk wrapper around `chat()`), `count_tokens()` (character-based estimate at 4 chars per token), `capabilities()`, `context_window()` (128K default), `supports_streaming()`, `health_check()`.

### Structs and Enums

| Type | Role |
|------|------|
| `DynProvider` | Type alias `Arc<dyn LlmProvider>` -- the handle every consumer holds. |
| `Message` | Tagged enum with four variants: `System`, `User`, `Assistant`, `Tool`. Serialized with `#[serde(tag = "role", rename_all = "lowercase")]` so it maps directly to OpenAI/Anthropic wire format. |
| `UserContent` | `Text(String)` or `MultiPart(Vec<ContentPart>)` for vision payloads. |
| `ChatParams` | Per-request knobs: `model`, `temperature`, `max_tokens`, `response_format`. Builder pattern via `with_*` methods. |
| `ResponseFormat` | `Text`, `JsonObject`, or `JsonSchema { name, schema }` for structured output. |
| `LlmResponse` | Returned by `chat()`: `content`, `tool_calls`, `finish_reason`, `usage`, optional `reasoning_content`. |
| `LlmStreamChunk` | Incremental delta for streaming: content delta, tool-call delta, `is_final` flag, finish reason, reasoning content. |
| `LlmStream` | `Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>`. |
| `ToolCall` / `ToolCallMessage` / `FunctionCall` | Represent an LLM-requested tool invocation and its serialization for the wire. |
| `Usage` | Token counts: `prompt_tokens`, `completion_tokens`, `total_tokens`, plus `cache_read_tokens` and `cache_write_tokens` for Anthropic prompt caching. |
| `ProviderCapabilities` | Capability flags: `extended_thinking`, `structured_outputs`, `prompt_caching`, `native_token_counting`, `vision`, `streaming`, `tool_choice_required`, `parallel_tool_calls`. |
| `ProviderHealth` | Enum: `Healthy`, `Degraded(String)`, `Unhealthy(String)`, `Unknown`. |
| `ProviderSpec` | Static metadata for a registered provider: name, keywords, API base, env key, prefix rules, gateway flags, model overrides. |
| `ProviderRegistry` | Zero-allocation lookup functions over the static `PROVIDERS` slice. |
| `CircuitBreakerConfig` | `failure_threshold` (default 5) and `reset_timeout_secs` (default 60). |
| `ProviderManager` | Wraps primary + optional fallback + optional classifier provider with retry/failover/circuit-breaker. |
| `AnthropicNativeProvider` | Anthropic Messages API client with prompt caching, extended thinking, and native token counting. |
| `OpenAiCompatProvider` | Generic OpenAI chat-completions client that serves OpenAI, DeepSeek, Gemini, Groq, Zhipu, DashScope, Moonshot, MiniMax, AiHubMix, vLLM, and OpenRouter. |
| `TranscriptionProvider` | Groq Whisper client for audio-to-text. |

## How It Works

### Provider Registry and Auto-Detection

The registry is a static `&[ProviderSpec]` array (`PROVIDERS`) containing 12 entries organized into four tiers:

1. **Gateways** -- OpenRouter (detected by `sk-or-` key prefix or `openrouter` in the base URL) and AiHubMix (detected by `aihubmix` in the base URL). Gateways can route any model, so they are never matched by model-name keywords.
2. **Standard providers** -- Anthropic (`claude`), OpenAI (`gpt`), DeepSeek, Gemini, Zhipu, DashScope, Moonshot, MiniMax. Matched by substring keywords in the model name.
3. **Local** -- vLLM / OpenAI-compatible local servers. Matched by explicit config name.
4. **Auxiliary** -- Groq (mainly for Whisper transcription, also usable for LLM). Matched by keyword.

`ProviderRegistry` provides three lookup methods: `find_by_model()` (keyword match, skips gateways), `find_by_name()` (exact config key), and `find_gateway()` (priority: explicit name, then API key prefix, then base URL keyword). It also handles model-name prefixing via `resolve_model()` (e.g., `deepseek-chat` becomes `deepseek/deepseek-chat` for DeepSeek's API) and per-model parameter overrides via `get_model_overrides()` (e.g., Kimi K2.5 forces `temperature >= 1.0`).

### Provider Creation (`create_provider`)

Resolution follows a strict priority:

1. **Explicit provider field** -- if `config.agents.defaults.provider` is set, look it up by name. If the configured model does not match the provider's keywords, use the provider's own default model instead.
2. **Model-name match** -- scan the registry for a standard provider whose keywords match the model name.
3. **Gateway detection** -- iterate all configured provider entries looking for API-key prefix or base-URL substring matches against known gateways.
4. **Fallback** -- use the first provider with a non-empty API key.

When the matched `ProviderSpec` is Anthropic with `native: true` in config, the factory creates an `AnthropicNativeProvider`; otherwise it creates an `OpenAiCompatProvider` pointed at the appropriate API base.

### Provider Creation with Failover (`create_provider_with_failover`)

Calls `create_provider()` for the primary, then checks `config.provider_manager.fallback`. If a fallback name is set, it creates a second provider and wraps both in a `ProviderManager`. An optional `classifier_model` config field creates a third lightweight provider used by the intent classifier so classification calls do not consume primary-provider quota.

### AnthropicNativeProvider

Uses Anthropic's Messages API directly (`/v1/messages`) rather than the OpenAI-compatible endpoint, unlocking three native features:

- **Prompt caching** -- when `cache_system_prompt` is enabled (default), the system prompt is sent as a content block with `cache_control: { type: "ephemeral" }`. Token usage is reported back via `cache_read_tokens` and `cache_write_tokens` in the `Usage` struct.
- **Extended thinking** -- when configured, adds a `thinking` block type to the request with a budget token limit. Thinking content is extracted from the response and returned in `LlmResponse.reasoning_content`.
- **Native token counting** -- calls Anthropic's `/v1/messages/count_tokens` endpoint for exact counts instead of the character-based estimate.

The provider converts Klyntbot's `Message` enum to Anthropic's format: system prompts become the top-level `system` field, tool results are wrapped as `tool_result` content blocks inside `user` role turns, and assistant messages map to `assistant` blocks with optional `tool_use` entries.

Streaming is implemented via SSE parsing of Anthropic's event types (`message_start`, `content_block_start`, `content_block_delta`, `message_delta`, `message_stop`), with special handling for `thinking` blocks. The context window is 200K tokens.

### OpenAiCompatProvider

A general-purpose client that speaks the OpenAI `POST /chat/completions` protocol. It serves as the universal adapter for every non-Anthropic provider. Key behaviors:

- **Request building** -- `build_request_body()` assembles the JSON payload, applying per-model overrides from the registry (e.g., minimum temperature for Kimi K2.5) and serializing `ResponseFormat` into OpenAI's `response_format` field with strict JSON schema support.
- **Streaming** -- parses SSE lines (`data: {...}`, `data: [DONE]`) via a `scan`-based stream adapter, accumulating partial lines across byte boundaries. Emits `LlmStreamChunk` instances with content deltas, tool-call deltas, and reasoning content (for DeepSeek-R1 style models).
- **Context window lookup** -- a built-in table maps model name prefixes to context window sizes (GPT-4 base at 8K, GPT-4 32K at 32K, GPT-4 Turbo/GPT-4o at 128K, o1-mini/o1-preview at 128K, o1/o3/o4 series at 200K, GPT-3.5 Turbo at 16K). Unknown models default to 128K.
- **Health check** -- `GET /models` with a 5-second timeout. Returns `Healthy`, `Degraded` (timeout), or `Unhealthy` (error status).

### ProviderManager (Failover, Retry, Circuit Breaker)

Wraps a primary and optional fallback `DynProvider` behind the same `LlmProvider` trait, so consumers are unaware of the resilience layer. The three mechanisms:

1. **Retry with exponential backoff** -- on rate-limit errors (HTTP 429), retries up to 3 times with delays of 500ms, 1s, and 2s. Non-rate-limit errors fail fast without retrying.
2. **Failover** -- if the primary (after retries) still fails, the request is forwarded to the fallback provider. If no fallback is configured, the error propagates.
3. **Circuit breaker** -- after `failure_threshold` consecutive non-rate-limit failures (default 5), the circuit opens for `reset_timeout_secs` (default 60). While open, all requests bypass the primary entirely and go straight to the fallback. After the timeout, the circuit enters half-open state and the primary is tried again.

The `ProviderManager` also exposes `check_health()` which calls `health_check()` on both primary and fallback providers, and holds an optional `classifier_provider` field for the intent pipeline's complexity classifier.

### TranscriptionProvider

A standalone client for the Groq Whisper API (`/audio/transcriptions`). Reads an audio file from disk, sends it as a multipart form upload with the `whisper-large-v3` model, and returns the transcribed text. Supports custom API base URLs for alternative Whisper-compatible endpoints.

## Connections

**Depends on:**
- `common` (Layer 0) -- error types (`KlyntbotError`, `ProviderError`, `ConfigError`), `MessageRole`
- `config` (Layer 1) -- `Config` struct for reading provider settings, `Secret<String>` for API key wrapping, `ExtendedThinkingConfig`

**Depended on by:**
- `agent` (Layer 5) -- creates providers via `create_provider_with_failover()`, holds `DynProvider` for LLM calls in the agent loop, intent pipeline, and plan executor
- `tools` (Layer 3) -- some tools use `DynProvider` for AI-powered operations (e.g., enrichment)
- `context_engine` (Layer 2) -- uses `count_tokens()` for token budget allocation
- `session` (Layer 2) -- uses `Message` type for conversation history
