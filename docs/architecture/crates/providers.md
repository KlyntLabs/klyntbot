# Crate: `providers`

> **Status:** 🟢 Stable
> **Subsystem:** [03 — Providers (LLM)](../subsystems/03-providers.md)
> **Status last verified:** 2026-05-16
> **One-liner:** The single seam between the workspace and any LLM API

---

## TL;DR

`providers` is the only crate in the workspace that talks to LLM endpoints. Every `chat_completion`, `embed`, and `transcribe` call goes through `LlmProvider`. Four concrete adapters ship today (`AnthropicNativeProvider`, `OpenAiCompatProvider`, `TranscriptionProvider`, `NoopProvider`). `ProviderManager` wraps any provider with circuit-breaker + failover + degradation tracking. `factory` translates `Config` into ready-to-use providers including per-role variants for cognitive (`Distiller`, `ReforgeSynth`, `ReforgeRules`).

If you're adding a new model, a new API, or per-role routing, this is the file you'll edit.

---

## Module map

```
crates/providers/src/
├── lib.rs              ← Re-exports + ProviderRole enum
├── types.rs            ← Trait + ~25 types (LlmProvider, Message, ChatParams, CacheBreakpoint, ...)
├── adapters/
│   ├── mod.rs          ← Re-exports
│   ├── anthropic_native.rs  ← AnthropicNativeProvider (cache breakpoints, beta headers)
│   ├── openai_compat.rs     ← OpenAiCompatProvider (also handles Together, Groq, MLX, Mimo)
│   ├── transcription.rs     ← TranscriptionProvider (Whisper-shaped STT)
│   └── noop.rs              ← NoopProvider (tests only)
├── manager.rs          ← ProviderManager + CircuitBreakerConfig + DegradationLevel
├── registry.rs         ← ProviderRegistry + ProviderSpec + static PROVIDERS catalogue
├── catalogue.rs        ← Model catalogue helpers
├── factory.rs          ← create_provider + create_cognitive_provider + cognitive_chat_params
├── streaming.rs        ← Stream plumbing internals (pub(crate))
└── testing.rs          ← Test helpers + Noop builders
```

---

## Public API surface

### `LlmProvider` trait

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    async fn health(&self) -> ProviderHealth;

    async fn chat_completion(
        &self,
        messages: Vec<Message>,
        params: ChatParams,
    ) -> Result<LlmResponse, ProviderError>;

    fn chat_completion_stream(
        &self,
        messages: Vec<Message>,
        params: ChatParams,
    ) -> LlmStream;

    /// Optional — defaults to "not supported"
    async fn embed(&self, _text: &str) -> Result<Vec<f32>, ProviderError> {
        Err(ProviderError::Unsupported("embed".into()))
    }
}

pub type DynProvider = Box<dyn LlmProvider>;
```

### `ProviderRole`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRole {
    Distiller,          // Per-turn Distiller (Phase 3+) — cognitive
    ReforgeSynth,       // Reforge Phase 2.5 — Coding Synthesis
    ReforgeRules,       // Reforge Phase 3.5 — Rule Artifact Generation
}
```

### Wire types

#### `Message`

```rust
pub enum Message {
    System(String),
    User(UserContent),
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,                  // Plain String — no image-bearing schema today
    },
    ContextUpdate(String),                // Injected by LiveContextRefresher
}

pub enum UserContent {
    Text(String),
    Multi(Vec<ContentPart>),              // Multimodal
}

pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    // ContentPart::ImageData planned for Computer Use; not in code today
}

pub struct ImageUrl {
    pub url: String,
    pub detail: Option<String>,           // "low" | "high" | "auto" (OpenAI)
}
```

#### `ChatParams`

```rust
pub struct ChatParams {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<ToolChoice>,
    pub response_format: Option<ResponseFormat>,
    pub cache_breakpoints: Vec<CacheBreakpoint>,
    pub cache_system_prompt: bool,        // legacy fallback flag
    pub stop_sequences: Vec<String>,
    pub reasoning_effort: Option<ReasoningEffort>,  // for reasoning models
    pub session_id: Option<String>,       // for sampling delegation
    pub trace_id: Option<String>,
    pub stream_id: Option<String>,
    pub turn_id: Option<String>,
}

pub enum ResponseFormat {
    Text,
    JsonObject,
    JsonSchema { schema: Value },
}

pub enum ToolChoice {
    Auto,
    None,
    Required,
    Function(String),                     // force a specific tool
}

pub enum ReasoningEffort { Low, Medium, High }
```

#### `CacheBreakpoint` (Anthropic-specific)

```rust
pub struct CacheBreakpoint {
    pub anchor: CacheAnchor,
    pub ttl: CacheTtl,
}

pub enum CacheAnchor {
    LastSystem,                           // mark cache_control after last system message
    LastUser,                             // mark after last user message
    LastTool,                             // mark after last tool result
    LastN(usize),                         // mark N messages from end
}

pub enum CacheTtl {
    Ephemeral,                            // 5-minute Anthropic cache
    Persistent,                           // 1-hour cache (when supported)
}
```

Up to 4 breakpoints per request (Anthropic limit).

#### Tool-calling shapes

```rust
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments_partial: Option<String>,   // streamed JSON fragment
}

pub struct ToolCallMessage {
    pub tool_call_id: String,
    pub content: ToolContent,
}

pub enum ToolContent {
    Text(String),
    Parts(Vec<ToolContentPart>),
}

pub enum ToolContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

pub struct FunctionCall {
    pub name: String,
    pub arguments: String,                // raw JSON string (OpenAI format)
}

pub fn tool_calls_to_messages(
    tool_calls: &[ToolCall],
    tool_call_id_to_result: &HashMap<String, ToolContent>,
) -> Vec<Message>;
```

#### Response shapes

```rust
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub reasoning: Option<String>,
    pub usage: Usage,
    pub finish_reason: FinishReason,
    pub model: String,
    pub cache_info: Option<CacheInfo>,
}

pub enum FinishReason {
    Stop, MaxTokens, ToolUse, ContentFilter, Other(String),
}

pub struct CacheInfo {
    pub creation_input_tokens: u32,
    pub read_input_tokens: u32,
}

pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub reasoning_tokens: Option<u32>,
    pub cache_input_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,
}

pub type LlmStream = Pin<Box<dyn Stream<Item = Result<LlmStreamChunk, ProviderError>> + Send>>;

pub enum LlmStreamChunk {
    Text(String),                         // partial response text
    ToolCallDelta(ToolCallDelta),         // partial tool call (id/name/args)
    Usage(Usage),                         // mid-stream usage update (Anthropic emits these)
    Reasoning(String),                    // reasoning tokens (for reasoning models)
    Done(LlmResponse),                    // terminal — full final response
    Error(ProviderError),                 // terminal — failure
}
```

#### Provider capability + health

```rust
pub struct ProviderCapabilities {
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
    pub supports_caching: bool,
    pub supports_reasoning: bool,
    pub supports_embeddings: bool,
    pub supports_json_mode: bool,
    pub max_context: usize,
    pub max_output: usize,
}

pub enum ProviderHealth {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String, since: Timestamp },
}

pub struct ProviderModel {
    pub id: String,
    pub max_context: usize,
    pub max_output: usize,
    pub price_in: f64,                    // USD per million input tokens
    pub price_out: f64,
}

pub const DEFAULT_CONTEXT_WINDOW: usize = 200_000;

pub fn id_implies_reasoning(model_id: &str) -> bool;
```

### `ProviderManager` (failover + circuit breaker)

```rust
pub struct ProviderManager { /* opaque */ }

impl ProviderManager {
    pub fn new(
        primary: DynProvider,
        fallback: Vec<DynProvider>,
        config: CircuitBreakerConfig,
    ) -> Self;

    pub fn with_on_circuit_open(self, hook: OnCircuitOpen) -> Self;
    pub fn with_on_degraded(self, hook: OnProviderDegraded) -> Self;

    // Inherits LlmProvider trait — usable as a DynProvider
}

pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,           // default: 5
    pub cooldown: Duration,                // default: 60s
    pub probe_timeout: Duration,           // default: 10s
}

pub enum DegradationLevel {
    Healthy,                              // use primary
    Slow,                                  // primary in use; latency warning emitted
    FailingOver,                          // switched to first fallback
    Exhausted,                            // all providers tripped; fail fast
}

pub type OnCircuitOpen = Arc<dyn Fn(&str /*provider_name*/, &ProviderError) + Send + Sync>;
pub type OnProviderDegraded = Arc<dyn Fn(&str, DegradationLevel) + Send + Sync>;
```

### `ProviderRegistry` / `ProviderSpec`

```rust
pub struct ProviderRegistry { /* HashMap<String, ProviderSpec> */ }
impl ProviderRegistry {
    pub fn new() -> Self;
    pub fn register(&mut self, spec: ProviderSpec);
    pub fn get(&self, name: &str) -> Option<&ProviderSpec>;
    pub fn names(&self) -> Vec<&str>;
}

pub struct ProviderSpec {
    pub name: String,
    pub adapter: &'static str,            // "anthropic_native" | "openai_compat" | …
    pub base_url: Option<String>,
    pub models: Vec<ProviderModel>,
    pub default_model: String,
    pub capabilities: ProviderCapabilities,
}

/// Static catalogue of known providers (anthropic, openai, openrouter, mlx, …)
pub static PROVIDERS: Lazy<HashMap<&'static str, ProviderSpec>> = ...;
```

### Factory functions

```rust
/// Build a single provider (no failover)
pub fn create_provider(
    spec: &ProviderSpec,
    config: &Config,
) -> Result<DynProvider, ProviderError>;

/// Build with explicit failover chain
pub fn create_provider_with_failover(
    primary: &ProviderSpec,
    fallback: &[ProviderSpec],
    config: &Config,
) -> Result<DynProvider, ProviderError>;

pub fn create_provider_with_failover_full(
    primary: &ProviderSpec,
    fallback: &[ProviderSpec],
    breaker: CircuitBreakerConfig,
    config: &Config,
) -> Result<DynProvider, ProviderError>;

/// Build for a cognitive role
pub fn create_cognitive_provider(
    role: ProviderRole,
    config: &Config,
) -> Result<DynProvider, ProviderError>;

/// Standardized ChatParams for cognitive calls (T=0.2, max_tokens=4096)
pub fn cognitive_chat_params(role: ProviderRole) -> ChatParams;
```

### Adapter constructors (selected)

```rust
// crates/providers/src/adapters/anthropic_native.rs
impl AnthropicNativeProvider {
    pub fn new(
        api_key: Secret<String>,
        model: String,
        base_url: Option<String>,
        beta_headers: Vec<String>,        // e.g. "computer-use-2025-11-24"
    ) -> Self;
}

// crates/providers/src/adapters/openai_compat.rs
impl OpenAiCompatProvider {
    pub fn new(
        api_key: Secret<String>,
        base_url: String,                 // "https://api.openai.com/v1" or custom
        model: String,
        capabilities: ProviderCapabilities,
    ) -> Self;
}

// crates/providers/src/adapters/transcription.rs
impl TranscriptionProvider {
    pub fn new(
        api_key: Secret<String>,
        base_url: String,                 // OpenAI /audio/transcriptions endpoint
        model: String,                     // e.g. "whisper-1"
    ) -> Self;

    pub async fn transcribe(
        &self,
        audio_bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<String, ProviderError>;
}

// crates/providers/src/adapters/noop.rs
impl NoopProvider {
    pub fn new() -> Self;
    pub fn with_response(self, response: LlmResponse) -> Self;
    pub fn with_error(self, error: ProviderError) -> Self;
}
```

---

## Internals

### Cache breakpoint synthesis (legacy)

```rust
// crates/providers/src/adapters/anthropic_native.rs:192-206
fn resolve_breakpoints(&self, params: &ChatParams) -> Vec<CacheBreakpoint> {
    if !params.cache_breakpoints.is_empty() {
        return params.cache_breakpoints.clone();
    }
    if params.cache_system_prompt {
        tracing::warn!("no explicit cache_breakpoints; synthesizing legacy LastSystem/Ephemeral fallback");
        return vec![CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Ephemeral,
        }];
    }
    vec![]
}
```

`cache_system_prompt` is transitional — once all call sites set `cache_breakpoints` explicitly, the flag can be deleted.

### Anthropic streaming

The adapter parses SSE chunks into `LlmStreamChunk`. Key events:
- `message_start` → init usage
- `content_block_start { tool_use }` → start `ToolCallDelta`
- `content_block_delta { text_delta }` → emit `Text`
- `content_block_delta { input_json_delta }` → emit `ToolCallDelta` with `arguments_partial`
- `message_delta { usage }` → emit `Usage`
- `message_stop` → emit `Done(LlmResponse)` with accumulated state

### OpenAI-compatible streaming

Parses standard chat-completion SSE. Tool calls arrive in `choices[0].delta.tool_calls[]` with `index`, `id?`, `function.name?`, `function.arguments?`. Accumulator assembles by `index`.

### Circuit breaker state machine

```
[Closed] ──failure×N─→ [Open]
   ▲                      │
   │                      │ cooldown elapsed
   │                      ▼
   └────success───── [HalfOpen]
                          │ failure
                          ▼
                       [Open]
```

`ProviderManager` checks the primary's breaker before each call. If `Open`, skips to first fallback (whose breaker is checked in turn). If all breakers are `Open`, returns `Exhausted` immediately without making any network calls.

### Failover ordering preserves provider identity

The fallback list order matters. If `[a, b, c]` and `a` is unhealthy, falls to `b`. If both `a` and `b` are unhealthy, falls to `c`. **Never reorders** based on observed latency — strict declarative order.

### `id_implies_reasoning` classifier

```rust
pub fn id_implies_reasoning(model_id: &str) -> bool {
    let id = model_id.to_lowercase();
    id.starts_with("o1") || id.starts_with("o3") || id.starts_with("o4")
        || id.starts_with("gpt-5")
        || id.contains("reasoner")
        || id.contains("thinking")
}
```

Used to enable reasoning-token accounting and reasoning UI badges. Heuristic only.

---

## Workflows

### Chat completion (streaming)

```rust
let provider: DynProvider = create_provider_with_failover(
    &PROVIDERS["anthropic"],
    &[PROVIDERS["openai"].clone()],
    &config,
)?;

let mut stream = provider.chat_completion_stream(
    vec![Message::System("...".into()), Message::User(UserContent::Text("...".into()))],
    ChatParams {
        model: "claude-opus-4-7".into(),
        temperature: 0.7,
        max_tokens: 4096,
        cache_breakpoints: vec![CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Ephemeral,
        }],
        ..Default::default()
    },
);

while let Some(chunk) = stream.next().await {
    match chunk? {
        LlmStreamChunk::Text(t)            => print!("{t}"),
        LlmStreamChunk::ToolCallDelta(d)   => collect_tool_call(d),
        LlmStreamChunk::Reasoning(r)       => log_reasoning(&r),
        LlmStreamChunk::Usage(u)           => emit_budget_update(u),
        LlmStreamChunk::Done(resp)         => return Ok(resp),
        LlmStreamChunk::Error(e)           => return Err(e.into()),
    }
}
```

### Cognitive provider per role

```rust
// Reforge Phase 2 — Synthesize (LLM call #1)
let synth_provider = create_cognitive_provider(ProviderRole::ReforgeSynth, &config)?;
let synth_params = cognitive_chat_params(ProviderRole::ReforgeSynth);
let response = synth_provider.chat_completion(messages, synth_params).await?;
```

Each call constructs a fresh `Box<dyn LlmProvider>` rather than holding a long-lived `Arc`. The cost is negligible vs. the latency of the call; the simplicity is worth it.

### Failover detection

```rust
let mgr = ProviderManager::new(primary, fallback, CircuitBreakerConfig::default())
    .with_on_circuit_open(Arc::new(|name, err| {
        tracing::warn!(provider=name, error=%err, "circuit opened");
        // emit Toast in UI
    }))
    .with_on_degraded(Arc::new(|name, level| {
        tracing::info!(provider=name, level=?level, "degradation");
    }));
```

`on_circuit_open` fires each time a breaker transitions to `Open`. `on_degraded` fires on `Slow` (latency spike) and `FailingOver` (active failover).

---

## Testing approach

### `NoopProvider` with canned responses

```rust
let provider: DynProvider = Box::new(
    NoopProvider::new().with_response(LlmResponse {
        content: Some("hello".into()),
        tool_calls: vec![],
        ..Default::default()
    })
);

let resp = provider.chat_completion(messages, params).await.unwrap();
assert_eq!(resp.content.as_deref(), Some("hello"));
```

### Force errors

```rust
let provider = NoopProvider::new()
    .with_error(ProviderError::Http("503 Service Unavailable".into()));
```

### Test circuit-breaker behavior

```rust
let breaker_config = CircuitBreakerConfig {
    failure_threshold: 2,
    cooldown: Duration::from_millis(100),
    probe_timeout: Duration::from_millis(50),
};

let primary = NoopProvider::new().with_error(/* fail twice */);
let fallback = NoopProvider::new().with_response(/* succeed */);

let mgr = ProviderManager::new(Box::new(primary), vec![Box::new(fallback)], breaker_config);

// First 2 calls fail on primary, succeed via fallback
// Breaker opens after threshold; subsequent calls skip primary
```

### Mock HTTP responses

For adapter-level testing, use `wiremock`:

```rust
let mock_server = MockServer::start().await;
Mock::given(method("POST")).and(path("/v1/messages"))
    .respond_with(ResponseTemplate::new(200).set_body_json(...))
    .mount(&mock_server).await;

let provider = AnthropicNativeProvider::new(
    Secret::new("test".into()),
    "claude-test".into(),
    Some(mock_server.uri()),
    vec![],
);
```

---

## Extension points

### Add a new provider adapter

1. Create `crates/providers/src/adapters/my_provider.rs`.
2. Implement `LlmProvider`:
   ```rust
   #[async_trait]
   impl LlmProvider for MyProvider {
       fn name(&self) -> &str { "my_provider" }
       fn model(&self) -> &str { &self.model }
       fn capabilities(&self) -> ProviderCapabilities { /* ... */ }
       async fn health(&self) -> ProviderHealth { /* probe */ }
       async fn chat_completion(&self, messages, params) -> Result<LlmResponse, ProviderError> { /* ... */ }
       fn chat_completion_stream(&self, messages, params) -> LlmStream { /* ... */ }
       async fn embed(&self, text) -> Result<Vec<f32>, ProviderError> { /* ... */ }
   }
   ```
3. Re-export from `adapters/mod.rs`.
4. Add to static `PROVIDERS` catalogue in `registry.rs`.
5. Add factory branch in `factory::create_provider`.
6. Add config sub-struct in `crates/config/src/schema/providers.rs` if needed.

### Add a new `ProviderRole`

1. Add variant to `ProviderRole` enum in `lib.rs`.
2. Add factory branch in `factory::create_cognitive_provider`.
3. Define default `ChatParams` in `factory::cognitive_chat_params`.
4. Update consumers (typically a new reforge phase).

### Add a new `LlmStreamChunk` variant

⚠️ Cross-cutting. Every adapter must produce it; every consumer (esp. `agent::ExecutionCore`) must handle it. Coordinate carefully or hide behind an existing variant first.

### Add a Beta Anthropic feature

`AnthropicNativeProvider::new` takes a `beta_headers: Vec<String>` parameter. Add the header to enable beta features:

```rust
let provider = AnthropicNativeProvider::new(
    api_key,
    "claude-opus-4-7".into(),
    None,
    vec!["computer-use-2025-11-24".into()],
);
```

This sets the `anthropic-beta` HTTP header.

---

## Key constants

| Constant | Value | Location |
|---|---|---|
| `DEFAULT_CONTEXT_WINDOW` | `200_000` | `types.rs` |
| `CircuitBreakerConfig::failure_threshold` (default) | `5` | `manager.rs` |
| `CircuitBreakerConfig::cooldown` (default) | `60s` | `manager.rs` |
| `CircuitBreakerConfig::probe_timeout` (default) | `10s` | `manager.rs` |
| Anthropic max cache breakpoints | `4` | enforced by adapter |
| `cognitive_chat_params(*)::temperature` | `0.2` | `factory.rs` |
| `cognitive_chat_params(*)::max_tokens` | `4096` | `factory.rs` |

---

## Open questions

- **`Message::Tool.content` is plain `String`** — no image-bearing schema. Blocks Computer Use spec which needs `ContentPart::ImageData`.
- **`CacheBreakpoint` leaks Anthropic-specific semantics through the generic trait.** Acceptable today; footgun if a second provider adds prompt caching with a different model.
- **`cache_system_prompt` legacy flag** should be deleted once all call sites set `cache_breakpoints` explicitly.
- **Multi-role provider router design** exists at `docs/superpowers/specs/2026-05-07-provider-router-multi-role-design.md` (spec file present). Not implemented.
- **`LlmProvider::embed`** is rarely used — cognitive layer goes through `fastembed` locally instead. Trait method exists but is mostly unused.
- **No structured per-call cost emission** — `Usage` returned but cost translation (via `common::pricing`) happens in callers. Could centralize.
- **No retry inside adapters** — failover is via `ProviderManager`. If a single transient error happens, the call fails entirely (no per-adapter retry). Acceptable today; would matter for high-latency APIs.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #1 + #3 for specifics.

---

## Cross-references

- [Subsystem 03 — Providers (LLM)](../subsystems/03-providers.md) (parent)
- [Subsystem 04 — Agent Runtime](../subsystems/04-agent-runtime.md) (the main consumer)
- [Subsystem 05 — Cognitive Memory](../subsystems/05-cognitive-memory.md) (per-role usage)
- [Subsystem 11 — Channels, MCP](../subsystems/11-channels-mcp.md) (MCP sampling delegation via providers)
- [`crates/agent.md`](./agent.md) *(planned)* — `ExecutionCore` consumes `DynProvider`
- [`crates/cognitive.md`](./cognitive.md) *(planned)* — `Reforge` consumes per-role providers
