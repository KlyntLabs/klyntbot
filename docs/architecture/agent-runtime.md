# Agent Runtime & Execution Engine

The agent runtime is Klyntbot's central nervous system. It processes every user message through a unified pipeline: assembling context, driving LLM-tool cycles within a token/turn budget, and recording outcomes for continuous learning. This is not a thin wrapper around an LLM API — it is a budget-aware, streaming, self-correcting execution engine with fabrication detection, mid-loop compression, and live context injection.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [AgentLoop — The Entry Point](#agentloop--the-entry-point)
- [Three-Phase Pipeline](#three-phase-pipeline)
- [Execution Budget & Depth Modes](#execution-budget--depth-modes)
- [The Execute Loop](#the-execute-loop)
- [ExecutionCore — Single LLM Cycle](#executioncore--single-llm-cycle)
- [Mid-Loop Compression](#mid-loop-compression)
- [Live Context Refresh](#live-context-refresh)
- [Fabrication Detection](#fabrication-detection)
- [Streaming Events](#streaming-events)
- [LLM Provider Abstraction](#llm-provider-abstraction)
- [Configuration Reference](#configuration-reference)
- [Key Files](#key-files)

---

## Architecture Overview

```
AgentLoop (entry point)
  │
  ├── SessionManager ─── Session persistence (LRU + SQL)
  ├── ToolRegistry ───── 20+ tools, cached schemas
  ├── SkillStore ──────── Skill activation & authorization
  ├── HotConfig ──────── Live-reloadable settings
  └── AgentRuntime ───── The execution pipeline
       │
       ├── ContextEngine ── Context assembly & budget
       ├── ExecutionCore ── LLM↔tool cycle driver
       ├── CostTracker ──── Spending limits
       └── ResponseValidator ── Output quality/safety
```

The `AgentLoop` owns the message bus subscription and session lifecycle. For each incoming message, it delegates to `AgentRuntime` which runs the three-phase pipeline. The runtime is stateless per-request — all state lives in the session and the assembled context.

## AgentLoop — The Entry Point

**Location**: `crates/agent/src/agent_loop/mod.rs`

```rust
pub struct AgentLoop {
    pub bus: Arc<MessageBus>,
    pub session_manager: SessionManager,
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
    pub runtime: Arc<AgentRuntime>,
    pub skill_store: Arc<RwLock<SkillStore>>,
    pub hot_config: Arc<RwLock<HotConfig>>,
    pub mcp_manager: Arc<tokio::sync::Mutex<Option<McpManager>>>,
    pub cognitive_bg_service: tokio::sync::Mutex<Option<BackgroundConsolidationService>>,
}
```

The `AgentLoop` is constructed via a fluent builder (`AgentLoopBuilder`) that wires all dependencies:

```rust
AgentLoop::builder(bus, provider, config)
    .with_pool(db)
    .with_cron_service(cron)
    .with_notification_handle(notifs)
    .with_domain_bus(event_bus)
    .with_embedding_engine(embeddings)
    .build()
    .await?
```

The builder registers tools, context sources, cognitive services, and MCP connections. It returns a fully wired `AgentLoop` ready to process messages.

### Streaming Output

Every message processed returns a `StreamingHandle` for real-time UI updates:

```rust
pub struct StreamingHandle {
    pub event_rx: mpsc::Receiver<AgentEvent>,           // Content chunks, tool status
    pub interaction_rx: mpsc::Receiver<InteractionBundle>, // ask_user tool requests
    pub cancel_token: CancellationToken,                  // User cancellation
    pub handle: JoinHandle<Result<String>>,                // Final result
}
```

The frontend consumes `event_rx` for streaming text, tool call progress, budget updates, and compression notifications. `interaction_rx` handles interactive tools that pause execution to wait for user input (e.g., confirmation dialogs).

---

## Three-Phase Pipeline

Every message flows through three phases inside `AgentRuntime`:

```
┌─────────────────────────────────────────────────────────────┐
│                    AgentRuntime.process()                     │
│                                                               │
│  Phase 1: PREPARE        Phase 2: EXECUTE       Phase 3: RECORD│
│  ┌──────────────┐       ┌──────────────┐       ┌────────────┐│
│  │ Build context │──────▶│ LLM↔tool loop│──────▶│ Persist    ││
│  │ Retrieve memory│      │ Budget-gated  │      │ Learn      ││
│  │ Assemble prompt│      │ Streaming     │      │ Tune       ││
│  └──────────────┘       └──────────────┘       └────────────┘│
└─────────────────────────────────────────────────────────────┘
```

### Phase 1: Prepare

1. Build `RetrievalContext` from conversation history
2. Assemble system prompt via pluggable `ContextSource` implementations
3. Retrieve semantic and episodic memories (optional prefetch)
4. Build `ContextRequest` with all inputs
5. Call `ContextEngine.assemble()` → `AssembledContext` (system prompt + messages + token count)

See [Context Engine](context-engine.md) for the full assembly pipeline.

### Phase 2: Execute

1. Create `ExecutionBudget` from the selected depth mode
2. Create `ExecutionParams` (timeouts, max iterations, context window)
3. Enter the execute loop — the unified LLM↔tool cycle
4. Return `ExecuteLoopResult` (content + usage + turns + budget status)

### Phase 3: Record (fire-and-forget)

All recording happens asynchronously via `tokio::spawn` — the response is returned to the user immediately.

1. Record token usage for cost tracking
2. Record retrieval feedback for the memory system (which memories were useful)
3. Record interaction for the learning service
4. Trigger AutoTuner hook for A/B experiment tracking
5. Persist strategy record to database

```rust
pub struct RuntimeResult {
    pub content: String,
    pub mode_used: String,
    pub validation: ValidationResult,
    pub agent_name: String,
    pub turns: u32,
    pub budget_exhausted: bool,
    pub tool_calls: Vec<String>,
}
```

---

## Execution Budget & Depth Modes

**Location**: `crates/agent/src/execution/budget.rs`

The budget system is what separates Klyntbot from simple LLM wrappers. Every execution has a hard token and turn limit that governs how deep the agent can go. Users select a depth mode that trades off speed vs. cognitive depth:

| Mode | Token Limit | Turn Limit | Use Case |
|------|-------------|------------|----------|
| **Normal** | 60K | 15 | Fast, frictionless daily use |
| **DeepThink** | 90K (1.5x) | 22 | Adds mirror context, coaching, visible HUD |
| **Ultra** | 180K (3.0x) | Unlimited | Full cognitive partner, auto-save, FSRS atoms |

### Budget Lifecycle

```
Create(DepthMode)
  │
  ▼
┌──────────────────────────────────────────────┐
│              Execute Loop                      │
│  ┌─────────┐    ┌──────────┐    ┌──────────┐ │
│  │ LLM call │───▶│ deduct() │───▶│ tick()   │ │
│  └─────────┘    └──────────┘    └──────────┘ │
│       │                              │         │
│       │         ┌──────────────────┐ │         │
│       └────────▶│ should_wrap_up() │◀┘         │
│                 │ (at 85% usage)   │           │
│                 └──────────────────┘           │
│                        │                       │
│                 ┌──────────────────┐           │
│                 │ exhausted()      │           │
│                 │ → synthesis call │           │
│                 └──────────────────┘           │
└──────────────────────────────────────────────┘
```

**Key methods**:

```rust
pub fn remaining_pct(&self) -> f32       // 0.0–1.0, tighter of token/turn ratio
pub fn should_wrap_up(&self) -> bool     // True at 85% usage
pub fn exhausted(&self) -> bool          // True when max_tokens or max_turns reached
pub fn extend_turns(&mut self, n: u32)   // User taps "Extend" in UI HUD
```

**Wrap-up behavior**: At 85% budget consumption, the system injects a "wrap up" instruction into the next LLM call, nudging the model to synthesize results rather than starting new tool chains. If the budget fully exhausts, one final synthesis call is forced.

---

## The Execute Loop

**Location**: `crates/agent/src/execution/execute_loop.rs`

This is the core control loop. It replaces the old `DirectEngine`/`ReactiveEngine`/`ExecutionRouter` split with a single unified loop:

```rust
loop {
    // 1. Budget gate
    if budget.exhausted() {
        // Inject synthesis instruction + one final LLM call
        break;
    }

    // 2. Cancellation check
    if cancel_token.is_cancelled() {
        return partial_results;
    }

    // 3. Emit iteration start
    event_tx.send(IterationStart { iteration, max })?;

    // 4. Single LLM↔tool cycle
    let (outcome, usage) = core.run_cycle(messages, tools, params, ctx, event_tx)?;

    // 5. Budget accounting
    budget.deduct(&usage);
    budget.tick_turn();

    // 6. Handle outcome
    match outcome {
        CycleOutcome::FinalResponse { content } => return content,
        CycleOutcome::ToolsExecuted { results } => {
            // Record tool calls, continue loop
            for r in results { all_tool_calls.push(r.tool_name); }
        }
        CycleOutcome::EmptyResponse => { /* retry */ }
        CycleOutcome::FabricatedResponse { content } => { /* treat as final */ }
    }

    // 7. Mid-loop compression (if tokens > 70% of window)
    if let Some((before, after)) = compressor.compress_if_needed(messages) {
        event_tx.send(ContextCompressed { before, after })?;
    }

    // 8. Live context refresh (cognitive/productivity updates)
    if let Some(refresher) = &refresher {
        let updates = refresher.inject_pending(messages, context_window);
        event_tx.send(ContextReassembled { updates })?;
    }

    // 9. Emit budget update to UI
    event_tx.send(BudgetUpdate { tokens_remaining_pct, turns_used, max_turns, cost_usd, depth })?;
}
```

### Loop Termination

The loop terminates under four conditions:

1. **Natural completion** — Model returns content with no tool calls
2. **Budget exhausted** — Synthesis attempted, then forced exit
3. **User cancellation** — Partial results returned immediately
4. **Safety timeout** — Emergency exit (indicates a bug, not normal flow)

---

## ExecutionCore — Single LLM Cycle

**Location**: `crates/agent/src/execution/core.rs`

The `ExecutionCore` drives a single LLM call followed by tool execution:

```rust
pub async fn run_cycle(
    messages: &mut [Message],
    tools: &[Value],
    params: &ExecutionParams,
    ctx: &RoutingContext,
    event_tx: Option<&Sender<AgentEvent>>,
) -> Result<(CycleOutcome, Usage)>
```

### What happens in one cycle

1. **LLM call** with streaming — content chunks emitted in real-time via `ContentChunk` events
2. **Parse tool calls** from the LLM response
3. **Fabrication check** — detect when models skip tool calls and hallucinate results
4. **Tool call deduplication** — SHA-256 hash of arguments prevents duplicate execution
5. **Concurrent tool execution** — up to 10 tools in parallel (semaphore-controlled)
6. **Result sanitization** — strip control chars, truncate to 50KB
7. **Append results** as `Message::Tool` entries for the next cycle

```rust
pub enum CycleOutcome {
    ToolsExecuted { results: Vec<ToolExecutionResult> },
    FinalResponse { content: String },
    EmptyResponse,
    FabricatedResponse { content: String },
}

pub struct ToolExecutionResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub arguments: Value,
    pub result: String,
    pub duration_ms: u64,
    pub success: bool,
}
```

### Tool Timeouts

| Tool Type | Timeout | Reason |
|-----------|---------|--------|
| Standard | 30s | Most tools complete quickly |
| Interactive (`ask_user`) | 600s | Waits for human input |
| Custom (per-tool) | Varies | Some tools declare their own timeout |

---

## Mid-Loop Compression

**Location**: `crates/agent/src/execution/mid_loop_compressor.rs`

During long ReAct loops, tool results accumulate and can exhaust the context window. The mid-loop compressor activates when token usage exceeds **70% of the context window**:

```
Trigger: total_tokens > 0.70 * context_window

Strategy:
  1. Keep the most recent 8 messages verbatim (always)
  2. For older Message::Tool results:
     - Extract first 150 characters
     - Replace with: "{snippet}... [compressed {tool_name} result, originally {N} chars]"
  3. Skip messages under 50 tokens (not worth compressing)
```

This is extractive compression — no LLM calls, no latency. It preserves the most recent context where the agent is actively working while aggressively summarizing older tool outputs.

**Constants**:

| Parameter | Value |
|-----------|-------|
| Trigger threshold | 70% of context window |
| Min recent messages | 8 (always verbatim) |
| Min compressible size | 50 tokens |
| Summary snippet | 150 characters |

The compressor emits a `ContextCompressed { before_tokens, after_tokens }` event so the UI can show that compression occurred.

---

## Live Context Refresh

**Location**: `crates/agent/src/execution/live_context_refresher.rs`

Between loop iterations, the agent can receive mid-execution context updates from background systems (e.g., cognitive memory promotion, productivity state changes). The `LiveContextRefresher` drains a `ContextUpdateQueue` (from the `bus` crate) at each iteration boundary.

```rust
pub fn inject_pending(
    &self,
    messages: &mut Vec<Message>,
    context_window: usize,
) -> Vec<ContextReassembledUpdate>
```

Updates are injected as `Message::ContextUpdate { reason, content }` entries with XML-tagged content. Token budget is respected:

| Priority | Response Reserve |
|----------|-----------------|
| Standard | 20% reserved |
| High | 10% reserved |

**Frozen-context mode**: Set `pause_context_updates: true` on `ExecutionParams` to disable live refresh entirely. Useful for deterministic testing.

**Current producers**:
- Cognitive background service: pushes on memory promotion
- Productivity service: focus state changes

---

## Fabrication Detection

**Location**: `crates/agent/src/execution/core.rs`

Some models (DeepSeek-R1, Kimi, certain fine-tunes) occasionally skip tool calls and generate fake results inline. The fabrication detector catches this:

```
Detection signals:
  ├── Fake ID: 6+ hex characters in "id:" pattern (e.g., "9c4e5f3b")
  ├── Structured result indicators: "task created", "search results:", etc.
  ├── Multiple fields: 2+ field-like patterns (Priority:, Due Date:, Description:)
  └── Search with numbered list: "\n1.", "\n2." combined with fake ID

Verdict: fabricated if
  (structured_result AND (fake_id OR multiple_fields))
  OR
  (search_with_numbered_list AND fake_id)
```

When fabrication is detected, the response is treated as a `FabricatedResponse` — the content is used as-is (since it's often reasonable) but no tool calls are recorded. Retries are limited to `max_fabrication_retries` to prevent infinite loops.

---

## Streaming Events

**Location**: `crates/agent/src/events.rs`

The agent emits structured events throughout execution for real-time UI updates:

```rust
pub enum AgentEvent {
    PipelineStarted,
    ContentChunk { data: String },
    ToolStart { name, args, agent: Option<String> },
    ToolEnd { name, success, duration_ms, result: Option<String>, agent: Option<String> },
    IterationStart { iteration, max },
    ContextAssembled { total_tokens, budget, duration_ms },
    ExecutionStarted { engine, max_iterations },
    Done { content, message_id: Option<String> },
    ConfidenceAssessed { score, action },
    Error { message },
    EntityCreated(EntityCard),
    UsageReport { prompt_tokens, completion_tokens, cache_read_tokens, cache_write_tokens, estimated_cost_usd, model, response_time_ms },
}
```

The frontend uses these events to render:
- **Streaming text** — `ContentChunk` events append to the message bubble
- **Tool activity** — `ToolStart`/`ToolEnd` show which tools are running and their results
- **Completion** — `Done` carries the final content and persisted message ID
- **Cost tracking** — `UsageReport` shows token counts, cache hits, and estimated cost

---

## LLM Provider Abstraction

**Location**: `crates/providers/src/`

The runtime is provider-agnostic. All LLM interaction goes through the `LlmProvider` trait:

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(&self, messages: &[Message], tools: Option<&[Value]>, params: &ChatParams) -> Result<LlmResponse>;
    async fn chat_stream(&self, messages: &[Message], tools: Option<&[Value]>, params: &ChatParams) -> Result<LlmStream>;
    fn supports_streaming(&self) -> bool;
    fn default_model(&self) -> &str;
    fn name(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn context_window(&self) -> usize;
}
```

### Supported Providers

| Provider | Module | Features |
|----------|--------|----------|
| **Anthropic** | `anthropic_native.rs` | Extended thinking, streaming, vision, prompt caching, native token counting |
| **OpenAI-Compatible** | `openai_compat.rs` | GPT-4, DeepSeek-R1, Kimi, local llama.cpp, any OpenAI-compatible endpoint |
| **Noop** | `noop.rs` | Mock responses for testing |

### Message Types

```rust
pub enum Message {
    System { content: String },
    User { content: UserContent },             // Text or MultiPart (vision)
    Assistant { content, tool_calls, reasoning_content },
    Tool { tool_call_id, name, content },
    ContextUpdate { reason, content },          // Injected by LiveContextRefresher
}

pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub finish_reason: String,
    pub usage: Usage,
    pub reasoning_content: Option<String>,      // Extended thinking models
}
```

### Provider Capabilities

```rust
pub struct ProviderCapabilities {
    pub extended_thinking: bool,      // DeepSeek-R1, o1-preview
    pub structured_outputs: bool,     // Guided JSON generation
    pub prompt_caching: bool,         // Anthropic cache control
    pub native_token_counting: bool,  // Anthropic native counter
    pub vision: bool,                 // Image inputs
    pub streaming: bool,             // Real-time token emission
    pub parallel_tool_calls: bool,    // Multiple tools per LLM call
}
```

---

## Configuration Reference

| Component | Parameter | Default | Impact |
|-----------|-----------|---------|--------|
| **Budget** | Normal tokens | 60K | Daily use token limit |
| **Budget** | Normal turns | 15 | Max LLM-tool iterations |
| **Budget** | DeepThink multiplier | 1.5x | Token/turn boost |
| **Budget** | Ultra multiplier | 3.0x | Token boost, unlimited turns |
| **Budget** | Wrap-up threshold | 85% | When to nudge synthesis |
| **Context** | Response reserve | 15% | Tokens reserved for output |
| **Compression** | Trigger threshold | 70% | When mid-loop compression fires |
| **Compression** | Min recent messages | 8 | Always kept verbatim |
| **Compression** | Snippet length | 150 chars | Extractive summary length |
| **Execution** | Tool timeout | 30s | Standard tool timeout |
| **Execution** | Interactive timeout | 600s | ask_user tool timeout |
| **Execution** | Max tool result | 50KB | Tool output truncation |
| **Execution** | Max concurrent tools | 10 | Parallel tool execution |
| **Session** | In-memory trim | 60 → 40 | Auto-trim threshold |
| **Session** | SQL compact | 200 → 100 | Database compaction threshold |

---

## Key Files

| File | Purpose |
|------|---------|
| `crates/agent/src/agent_loop/mod.rs` | AgentLoop — message bus consumer, session lifecycle |
| `crates/agent/src/agent_loop/builder.rs` | Fluent builder for wiring all dependencies |
| `crates/agent/src/agent_runtime/runtime.rs` | AgentRuntime — 3-phase pipeline |
| `crates/agent/src/execution/execute_loop.rs` | Unified execute loop |
| `crates/agent/src/execution/core.rs` | ExecutionCore — single LLM↔tool cycle |
| `crates/agent/src/execution/budget.rs` | ExecutionBudget & depth modes |
| `crates/agent/src/execution/mid_loop_compressor.rs` | Mid-loop context compression |
| `crates/agent/src/execution/live_context_refresher.rs` | Live context injection |
| `crates/agent/src/events.rs` | AgentEvent streaming types |
| `crates/providers/src/types.rs` | LlmProvider trait, Message types |
| `crates/providers/src/adapters/` | Provider implementations |
| `crates/session/src/manager.rs` | SessionManager (LRU + SQL) |

---

*Related docs: [Context Engine](context-engine.md) | [Cognitive Memory](cognitive-memory.md) | [Core Infrastructure](core-infrastructure.md) | [Skill System](skill-system.md)*
