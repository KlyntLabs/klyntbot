# Subsystem 03 — Providers (LLM)

> **Status:** 🟢 Stable
> **Status last verified:** 2026-05-16
> **Crates:** `providers`
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

`providers` is the single seam between the rest of the workspace and any LLM API. Every LLM call goes through `Box<dyn LlmProvider>` (`DynProvider`). Three real adapters ship today (`AnthropicNativeProvider`, `OpenAiCompatProvider`, `TranscriptionProvider`) plus a `NoopProvider` for tests. `ProviderManager` wraps a provider with **circuit breaker + failover + degradation tracking**. The `factory` module turns `Config` into ready-to-use providers, including separate per-role providers for the cognitive layer (Distiller, Reforge synthesis, Reforge rules).

If you're adding a new model or a new API, this is where you do it. If you want to swap the active provider, change `config.providerManager.primary`; the factory rebuilds providers on next reload.

---

## Architecture diagram

```mermaid
flowchart TB
    classDef trait fill:#e3f2fd,stroke:#1976d2,color:#0d47a1
    classDef adapter fill:#bbdefb,stroke:#1976d2,color:#0d47a1
    classDef mgr fill:#c5cae9,stroke:#3949ab,color:#1a237e
    classDef fac fill:#d1c4e9,stroke:#512da8,color:#311b92
    classDef ext fill:#fff,stroke:#999,stroke-dasharray:5

    LP[LlmProvider trait<br/><i>+ DynProvider alias</i>]:::trait

    A1[AnthropicNativeProvider<br/><i>native Anthropic API + cache breakpoints</i>]:::adapter
    A2[OpenAiCompatProvider<br/><i>OpenAI + any compatible<br/>(local MLX, Together, Groq, …)</i>]:::adapter
    A3[TranscriptionProvider<br/><i>STT via OpenAI-compatible</i>]:::adapter
    A4[NoopProvider<br/><i>tests</i>]:::adapter

    LP --- A1
    LP --- A2
    LP --- A3
    LP --- A4

    PM[ProviderManager<br/><i>CircuitBreaker · Degradation · OnOpen/Degraded hooks</i>]:::mgr
    LP --> PM

    FAC[Factory<br/><i>create_provider<br/>create_provider_with_failover<br/>create_cognitive_provider</i>]:::fac
    PM --> FAC

    REG[ProviderRegistry<br/><i>ProviderSpec catalogue</i>]:::trait
    FAC <--> REG

    CFG[Config]:::ext
    CFG --> FAC

    AGT[Agent runtime<br/>cognitive<br/>reforge]:::ext
    FAC --> AGT
```

---

## Mental model

`providers` is a **boring abstraction layer with one strong invariant**: nothing else in the workspace `use reqwest`-es an LLM endpoint directly. Every LLM call, transcription call, or embedding call goes through `LlmProvider`. This buys three things:

1. **Swap providers in config, not code.** Anthropic → OpenAI → local Llama via MLX is a config-file change.
2. **Uniform observability.** Token counts, latency, costs, circuit-breaker state — all measured at one chokepoint.
3. **Failover with intent.** `ProviderManager` knows about a primary + ordered fallback chain. Circuit-breaker trips automatically.

### Three things make this subsystem interesting

- **Per-role providers.** Cognitive doesn't always want the same model as the agent. `ProviderRole` (`Distiller`, `ReforgeSynth`, `ReforgeRules`) selects different configurations per role. `create_cognitive_provider(role)` is the factory entry.
- **Anthropic cache breakpoints.** Anthropic's prompt-cache requires the caller to mark `cache_control` blocks. `CacheBreakpoint`, `CacheAnchor`, `CacheTtl` model this. `AnthropicNativeProvider` synthesizes a legacy `LastSystem/Ephemeral` fallback if the caller passes none and the legacy `cache_system_prompt` flag is on (see `crates/providers/src/adapters/anthropic_native.rs:192-206`).
- **Streaming is first-class.** `LlmStream` is a `Stream<Item = Result<LlmStreamChunk>>`. Chunks carry partial text or `ToolCallDelta`. The agent runtime is built around this stream.

---

## Reference

### `providers` — file map

| Path | Purpose |
|---|---|
| `src/lib.rs` | Module declarations, `ProviderRole`, re-exports |
| `src/types.rs` | `LlmProvider` trait, `DynProvider`, `Message`, `ContentPart`, `UserContent`, `ChatParams`, `LlmResponse`, `LlmStream`, `LlmStreamChunk`, `ToolCall`, `ToolCallDelta`, `ToolCallMessage`, `ToolContent`, `ToolContentPart`, `FunctionCall`, `CacheAnchor`, `CacheBreakpoint`, `CacheTtl`, `ImageUrl`, `ResponseFormat`, `Usage`, `ProviderCapabilities`, `ProviderHealth`, `ProviderModel`, `DEFAULT_CONTEXT_WINDOW`, `id_implies_reasoning`, `tool_calls_to_messages` |
| `src/adapters/mod.rs` | Re-export of adapter implementations |
| `src/adapters/anthropic_native.rs` | `AnthropicNativeProvider` — official Anthropic API with cache breakpoints |
| `src/adapters/openai_compat.rs` | `OpenAiCompatProvider` — OpenAI API + every compatible endpoint |
| `src/adapters/transcription.rs` | `TranscriptionProvider` — OpenAI-compatible STT (Whisper-shaped) |
| `src/adapters/noop.rs` | `NoopProvider` — tests |
| `src/manager.rs` | `ProviderManager`, `CircuitBreakerConfig`, `DegradationLevel`, `OnCircuitOpen`, `OnProviderDegraded` |
| `src/registry.rs` | `ProviderRegistry`, `ProviderSpec`, `PROVIDERS` static (catalogue of known model IDs + capabilities) |
| `src/factory.rs` | `create_provider`, `create_provider_with_failover`, `create_provider_with_failover_full`, `create_cognitive_provider`, `cognitive_chat_params` |
| `src/catalogue.rs` | Model catalogue helpers (used by `registry`) |
| `src/streaming.rs` | Stream plumbing internals |
| `src/testing.rs` | Test helpers and `Noop` builders |

### `LlmProvider` trait (the seam)

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

    // Optional — defaults to "not supported"
    async fn embed(&self, text: &str) -> Result<Vec<f32>, ProviderError> { ... }
}

pub type DynProvider = Box<dyn LlmProvider>;
```

### `ProviderRole` (per-role wiring)

```rust
pub enum ProviderRole {
    Distiller,         // Per-turn distillation (cognitive Phase 3+)
    ReforgeSynth,      // Reforge Phase 2.5 — Coding Synthesis
    ReforgeRules,      // Reforge Phase 3.5 — Rule Artifact Generation
}
```

Each role can have a distinct provider + model + temperature. Configured via `config.cognitive.provider` (single provider today; multi-role expansion is in the spec at `docs/superpowers/specs/2026-05-07-provider-router-multi-role-design.md`).

### `Message` and content shapes

```rust
pub enum Message {
    System(String),
    User(UserContent),                    // String or Vec<ContentPart> (multimodal)
    Assistant {
        content: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    Tool {
        tool_call_id: String,
        content: String,                  // Plain String today — no image-bearing schema (see overview's Computer Use notes)
    },
    ContextUpdate(String),                // Injected by LiveContextRefresher
}

pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
    // ContentPart::ImageData planned for Computer Use; not in code today
}
```

### `ChatParams`

```rust
pub struct ChatParams {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub tools: Option<Vec<Tool>>,
    pub tool_choice: Option<ToolChoice>,
    pub response_format: Option<ResponseFormat>,
    pub cache_breakpoints: Vec<CacheBreakpoint>,
    pub cache_system_prompt: bool,        // legacy flag — see synthesis logic
    pub stop_sequences: Vec<String>,
    pub reasoning_effort: Option<ReasoningEffort>,  // for reasoning models
    // ...
}
```

### Cache breakpoints (Anthropic only)

```rust
pub struct CacheBreakpoint {
    pub anchor: CacheAnchor,    // LastSystem | LastUser | LastTool | LastN(usize)
    pub ttl: CacheTtl,          // Ephemeral (5min) | Persistent (1h)
}
```

The provider walks the message list and inserts `cache_control` markers at the right offsets. Up to 4 breakpoints per request. If none provided and `cache_system_prompt` legacy flag is on, the adapter synthesizes a `LastSystem` + `Ephemeral` breakpoint with a warning log (`crates/providers/src/adapters/anthropic_native.rs:206`).

### Tool calling shapes

```rust
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub struct ToolCallDelta {       // Streaming variant
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments_partial: Option<String>,
}

pub struct ToolCallMessage {     // For pushing tool results back
    pub tool_call_id: String,
    pub content: ToolContent,
}
```

### `ProviderManager` (failover + circuit breaker)

```rust
pub struct ProviderManager {
    primary: DynProvider,
    fallback: Vec<DynProvider>,
    breaker_config: CircuitBreakerConfig,
    on_circuit_open: Option<OnCircuitOpen>,
    on_degraded: Option<OnProviderDegraded>,
}
```

`ProviderManager` itself implements `LlmProvider`, so it's swappable into any consumer that takes a `DynProvider`.

**Degradation levels:**
- `DegradationLevel::Healthy` → use primary
- `DegradationLevel::Slow` → primary still in use, latency warning emitted
- `DegradationLevel::FailingOver` → switched to first fallback
- `DegradationLevel::Exhausted` → all providers tripped; subsequent calls fail fast

### Factory entry points

```rust
// Build a single provider (no failover)
pub fn create_provider(spec: &ProviderSpec, config: &Config) -> Result<DynProvider, ProviderError>;

// Build with explicit failover chain
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

// Build for a cognitive role (Distiller / ReforgeSynth / ReforgeRules)
pub fn create_cognitive_provider(
    role: ProviderRole,
    config: &Config,
) -> Result<DynProvider, ProviderError>;

// Standard ChatParams for a cognitive call (temperature 0.2, max_tokens 4096)
pub fn cognitive_chat_params(role: ProviderRole) -> ChatParams;
```

### `ProviderRegistry` / `PROVIDERS`

Static catalogue of known providers and their model lists. Used by the registry to resolve `"anthropic"` → `AnthropicNativeProvider` + `claude-opus-4-7` default model. Custom providers can be added but only the four shipped adapters are exercised in production.

---

## Workflows

### Chat completion (streaming)

```
1. AgentRuntime calls ProviderRouter.chat_completion_stream(messages, params)
   ↓
2. ProviderManager checks circuit-breaker state
   - Healthy → use primary
   - Open → skip to first fallback
   ↓
3. Selected provider's chat_completion_stream(messages, params)
   ↓
4. Adapter:
   a. Translate Vec<Message> + ChatParams to the provider's wire format
   b. (Anthropic) Apply cache breakpoints; insert cache_control markers
   c. POST to API endpoint (SSE / chunked streaming)
   d. Parse chunks into LlmStreamChunk
      - Each chunk is text fragment | tool_use_delta | usage_update | done
   ↓
5. ProviderManager observes:
   - Success → reset breaker
   - Failure → increment failure count; trip breaker if threshold; emit OnCircuitOpen
   - Slow → emit OnProviderDegraded
   ↓
6. Caller consumes LlmStream chunk-by-chunk
```

### Failover

```
Call A → primary returns 5xx
   ↓
ProviderManager increments primary's failure count
   ↓
If count ≥ threshold (CircuitBreakerConfig.failure_threshold) → open breaker for primary
   ↓
Retry on first fallback (call B)
   ↓
If fallback succeeds → return result; breaker stays open for primary
   ↓
After cooldown (CircuitBreakerConfig.cooldown), half-open primary; next call probes
   ↓
Probe success → close primary breaker
Probe failure → re-open
```

`OnCircuitOpen` callback fires each time the breaker opens — used by app-core to emit a toast/banner to the UI ("Primary provider unhealthy; using fallback").

### Cognitive role lookup (reforge)

```
Reforge Phase 2 (Synthesize):
   1. Call cognitive_chat_params(ReforgeSynth) → standardized ChatParams
   2. Call create_cognitive_provider(ReforgeSynth, &config) → DynProvider
   3. Build prompt + call provider.chat_completion(messages, params)
   4. Parse JSON response
   5. Free provider (it's a Box; dropped at end of phase)
```

Each reforge LLM call constructs its own provider via the factory rather than holding a long-lived `Arc`. This is intentional — the reforge cycle is rare (nightly) and the construction cost is negligible vs. the latency of the call.

---

## Internals

### Cache breakpoint synthesis (legacy compatibility)

```rust
// crates/providers/src/adapters/anthropic_native.rs:192-206
fn resolve_breakpoints(&self, params: &ChatParams) -> Vec<CacheBreakpoint> {
    if !params.cache_breakpoints.is_empty() {
        return params.cache_breakpoints.clone();
    }
    if params.cache_system_prompt {
        tracing::warn!(
            "no explicit cache_breakpoints; synthesizing legacy LastSystem/Ephemeral fallback"
        );
        return vec![CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Ephemeral,
        }];
    }
    vec![]
}
```

This is a transitional API — once all callers explicitly set `cache_breakpoints`, the `cache_system_prompt` flag can be deleted.

### Circuit breaker thresholds (defaults)

```rust
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,   // default: 5
    pub cooldown: Duration,        // default: 60s
    pub probe_timeout: Duration,   // default: 10s
}
```

### Streaming chunk shape

```rust
pub enum LlmStreamChunk {
    Text(String),                          // partial response text
    ToolCallDelta(ToolCallDelta),          // partial tool call (id/name/args)
    Usage(Usage),                          // mid-stream token usage update (Anthropic emits these)
    Reasoning(String),                     // reasoning tokens (for reasoning models)
    Done(LlmResponse),                     // terminal — full final response
    Error(ProviderError),                  // terminal — failure
}
```

### `id_implies_reasoning`

Helper that classifies model IDs into "reasoning model" buckets (gpt-5, o3, o4-mini, etc.) for reasoning-token accounting and UI treatment.

### `DEFAULT_CONTEXT_WINDOW`

Constant (`200_000` for Anthropic). Used as a fallback when a model's window isn't in the catalogue.

---

## Dependencies & extension points

### Upstream deps

- `reqwest` (HTTP)
- `eventsource-stream` (SSE parsing for streaming)
- `tokio` (async runtime)
- `serde` / `serde_json` (wire types)
- `tracing` (logging)
- `common` (`Secret<String>`, errors)
- `config` (Config schema)
- `async-trait`, `async-stream`

### Downstream consumers

- `agent` — every chat turn
- `cognitive` — per-turn distiller, reforge phases (2, 4, 6)
- `app-core` — auto-title generation (still TODO at `title_service.rs:50`), etc.

### Adding a new provider adapter

1. Create `crates/providers/src/adapters/my_provider.rs`.
2. Implement `LlmProvider`. Required methods: `name`, `model`, `capabilities`, `chat_completion`, `chat_completion_stream`. Optional: `embed`.
3. Add to the `adapters` re-export in `mod.rs`.
4. Update `ProviderRegistry::PROVIDERS` if you want it factory-resolvable by name.
5. Add to `factory::create_provider` match arm — translate `ProviderSpec` to your constructor.
6. Add config sub-struct in `crates/config/src/schema/providers.rs` if your provider needs credentials / endpoint URL.
7. Cover with tests in `crates/providers/src/testing.rs` patterns + an integration test against a mock server.

### Adding a new `ProviderRole`

1. Add a variant to `ProviderRole` in `crates/providers/src/lib.rs`.
2. Add a default config + factory branch in `factory.rs::create_cognitive_provider`.
3. Wire the call site in the consumer (e.g., a new reforge phase).
4. Document the role's purpose in `docs/architecture/subsystems/05-cognitive-memory.md`.

### Adding a new `LlmStreamChunk` variant

⚠️ Cross-cutting change. Every adapter must produce it; every consumer (esp. `agent::ExecutionCore`) must handle it. Coordinate carefully or hide behind an existing variant first.

---

## Open questions & debt

- **`Message::Tool.content` is plain `String`** — image-bearing tool results have no schema. Blocking item for Computer Use; spec describes `ContentPart::ImageData` as needed.
- **Anthropic-specific `cache_control` semantics leak through the trait** — `CacheBreakpoint` is in the generic trait API but only Anthropic uses it. Acceptable today, but if a second provider adds prompt-caching with a different model, this becomes a footgun.
- **`cache_system_prompt` legacy flag** — should be deleted once all call sites set `cache_breakpoints` explicitly.
- **Auto-title generation is a stub** — `app-core::title_service.rs:50` has `// TODO: LLM call`. The infrastructure (cognitive provider) is ready; the wiring isn't.
- **Multi-role provider router design exists** at `docs/superpowers/specs/2026-05-07-provider-router-multi-role-design.md`. Today the cognitive layer uses a single `config.cognitive.provider`; the spec proposes separate providers per `ProviderRole`. Not implemented.
- **Embedding via `LlmProvider::embed`** — only Anthropic doesn't offer it, OpenAI-compat does, but the cognitive layer goes through `fastembed` locally instead. The trait method exists but is mostly unused.
- **No structured per-call cost emission** — `Usage` is returned but cost translation (via `common::pricing`) happens in callers. Could centralize in `ProviderManager`.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #1 (TODOs), #3 (legacy), #5 (doc drift) for specifics.

---

## Cross-references

- [`01-foundations.md`](./01-foundations.md) — `Secret<String>`, `ProviderError`
- [`04-agent-runtime.md`](./04-agent-runtime.md) — the main consumer
- [`05-cognitive-memory.md`](./05-cognitive-memory.md) — reforge, distiller, per-role provider usage
- [`11-channels-mcp.md`](./11-channels-mcp.md) — MCP server can perform sampling delegation (LLM-to-LLM through providers)
- [`crates/providers.md`](../crates/providers.md) — *(planned)* the deep crate-level reference
