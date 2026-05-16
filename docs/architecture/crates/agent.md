# Crate: `agent`

> **Status:** 🟡 In Progress (flat-runtime migration; vestigial `intent_summary`)
> **Subsystem:** [04 — Agent Runtime](../subsystems/04-agent-runtime.md)
> **Status last verified:** 2026-05-16
> **One-liner:** The runtime that turns "a message arrived" into "a final response"

---

## TL;DR

The most central crate in the workspace. Owns `AgentLoop` (long-lived message-bus listener with focus-session deferral + correction detection), `AgentRuntime` (per-turn 3-phase pipeline + KCA Phase-4 retry), `ExecutionCore` (single LLM cycle), `execute_loop` (the ReAct iteration with cancellation, loop detection, mid-loop compression, live context refresh), and `SubagentRuntime` (spawn/resume/kill with `DEFAULT_TURN_CAP = 500`).

Every constant that governs runtime behavior lives here: `MAX_CONCURRENT_TOOLS = 10`, `MAX_TOOL_RESULT_LENGTH = 50_000`, `LONG_RUNNING_TOOL_TIMEOUT = 600s`, `COMPRESSION_THRESHOLD = 0.70`, `MIN_RECENT_MESSAGES = 8`, `DEFAULT_TURN_CAP = 500` (subagents only — **main agent has no turn cap**), plus the `KCA_*` env flags for Letta-style memory-refusal recovery and the disable-compression escape hatch.

If you're touching this crate, you're touching the central path of every message in the system. Every change should be measured against the chat perf gates ([`subsystems/14-validation.md`](../subsystems/14-validation.md)).

---

## Module map

```
crates/agent/src/
├── lib.rs                  ← Re-exports
├── events.rs               ← AgentEvent enum (~64 variants — every streaming event)
├── subagent.rs             ← run_subagent_loop + per-invocation AgentTaskTool clone
├── subagent_runtime.rs     ← SubagentRuntime + ActiveSubagentRegistry + DEFAULT_TURN_CAP
├── subagent_events.rs      ← SubagentLifecycleEvent broadcast types
├── notes_integration_tests.rs  ← inline tests
│
├── agent_loop/
│   ├── mod.rs              ← AgentLoop — bus listener, focus deferral, correction detection
│   └── builder.rs          ← AgentLoopBuilder — wires every crate together at startup
│
├── agent_runtime/
│   ├── runtime.rs          ← AgentRuntime — Prepare→Execute→Record, KCA Phase-4 retry
│   └── scenario.rs         ← Scenario helpers
│
├── agent_profile/
│   ├── manager.rs          ← Per-session profile state
│   ├── skill_loader.rs     ← Skill file loading bridge
│   └── types.rs
│
├── execution/
│   ├── core.rs             ← ExecutionCore — single LLM cycle, streaming, dedup, fabrication detection
│   ├── execute_loop.rs     ← execute_loop — ReAct loop with cancellation
│   ├── budget.rs           ← SafetyCap + DepthMode (Normal/DeepThink/Ultra)
│   ├── types.rs            ← ExecutionParams, CycleOutcome, ToolExecutionResult, LoopFinishReason
│   ├── mid_loop_compressor.rs  ← MidLoopCompressor — extractive compression of stale tool results
│   ├── live_context_refresher.rs ← LiveContextRefresher — drains ContextUpdateQueue
│   ├── loop_detector.rs    ← LoopDetector — Warning@3, HardStop@5
│   ├── scratchpad.rs
│   └── cache_policy.rs     ← Cache breakpoint placement strategy
│
├── context_sources/
│   └── *.rs                ← ~20 ContextSource impls (identity, area, session, productivity, …)
│
├── adapters/
│   ├── mod.rs
│   ├── llm_summary.rs      ← LlmSummaryProvider — batch LLM abstractive compression
│   ├── cognitive_handlers.rs  ← LlmQueryPredictorHandler (KCA Track 7 predictive cache) + bridges
│   └── autotuner_bridge.rs ← Autotuner hook bridge
│
├── confidence/
│   ├── evaluator.rs        ← ConfidenceEvaluator
│   └── decision_logger.rs  ← DecisionLogger
│
├── learning/
│   ├── service.rs          ← LearningService
│   ├── interaction_recorder.rs
│   ├── outcome_recorder.rs
│   └── pattern_analyzer.rs
│
├── output/
│   ├── cost_tracker.rs     ← Per-model cost accounting, session ceiling check
│   └── validator.rs        ← ResponseValidator — length truncation, system-leak detection, detect_memory_refusal
│
├── autotuner/              ← AutoTuner metric hooks and shadow retriever
│
└── services/
    ├── memory_maintenance.rs
    ├── session_cleanup.rs
    └── recurring_tasks.rs
```

---

## Public API surface

### `AgentLoop`

```rust
pub struct AgentLoop { /* opaque */ }

impl AgentLoop {
    // Accessors
    pub fn tool_registry(&self) -> Arc<RwLock<tools::registry::ToolRegistry>>;
    pub fn runtime(&self) -> Arc<AgentRuntime>;
    pub fn skill_store(&self) -> Arc<RwLock<skill_system::SkillStore>>;
    pub fn hot_config(&self) -> Arc<RwLock<config::HotConfig>>;
    pub fn subagent_manager(&self) -> Option<Arc<SubagentManager>>;
    pub fn shutdown_flag(&self) -> Arc<AtomicBool>;
    pub fn model_name(&self) -> &str;

    // Configuration
    pub fn set_approval_suggester(&self, suggester: Arc<dyn approval::ApprovalSuggester>);
    pub fn set_subagent_tool_kit(&self, kit: Arc<klynt_core::ToolKitBuilder>);
    pub fn set_subagent_hook_engine(&self, engine: Arc<klynt_hooks::HookEngine>);
    pub fn set_subagent_event_sender(&self, tx: broadcast::Sender<SubagentLifecycleEvent>);

    // Lifecycle
    pub async fn reload_agents(&self) -> Result<()>;
    pub fn take_inbound_rx(&mut self) -> Option<mpsc::Receiver<InboundMessage>>;
    pub async fn run_with_rx(&self, inbound_rx: mpsc::Receiver<InboundMessage>) -> Result<()>;
    pub async fn run(&mut self) -> Result<()>;
    pub async fn stop(&self);
    pub async fn shutdown(&self) -> Result<()>;

    // MCP runtime control
    pub async fn reconnect_mcp_server(&self, server_def: &config::McpServerDef);
    pub async fn disconnect_mcp_server(&self, server_name: &str);

    // Direct invocation (bypasses message bus)
    pub async fn process_direct(
        &self,
        content: String,
        session_key: String,
    ) -> Result<String>;

    pub async fn process_direct_streaming(
        self: &Arc<Self>,
        content: String,
        session_key: String,
        mode: Option<String>,
    ) -> Result<StreamingHandle>;

    // Discovery
    pub async fn list_tools(&self) -> Arc<Vec<serde_json::Value>>;
    pub async fn tool_names(&self) -> Vec<String>;
}

pub struct StreamingHandle {
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub interaction_rx: mpsc::Receiver<tools::InteractionBundle>,
    pub cancel_token: CancellationToken,
    pub handle: JoinHandle<Result<String>>,
}
```

### `AgentRuntime`

```rust
pub struct AgentRuntime { /* opaque */ }

impl AgentRuntime {
    pub fn new(
        context_engine: Arc<ContextEngine>,
        core: Arc<ExecutionCore>,
        cost_tracker: Arc<CostTracker>,
        cfg: RuntimeConfig,
        hot_config: Arc<RwLock<config::HotConfig>>,
    ) -> Self;

    pub async fn process_message(
        &self,
        message: &str,
        history: Vec<Message>,
        tool_definitions: &[serde_json::Value],
        ctx: &RoutingContext,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
        cancel_token: Option<CancellationToken>,
        depth: DepthMode,
    ) -> Result<RuntimeResult>;

    // Configuration
    pub fn set_tool_kit(&self, kit: Arc<klynt_core::ToolKitBuilder>);
    pub fn set_hook_engine(&self, engine: Arc<klynt_hooks::HookEngine>);
    pub fn hook_engine(&self) -> Option<Arc<klynt_hooks::HookEngine>>;
    pub fn tool_registry(&self) -> Option<&Arc<RwLock<ToolRegistry>>>;

    // Builder methods
    pub fn with_interaction_recorder(self, recorder: Arc<InteractionRecorder>) -> Self;
    pub fn with_tool_registry(self, registry: Arc<RwLock<ToolRegistry>>) -> Self;
    pub fn with_memory_service(self, service: Arc<UnifiedMemoryService>) -> Self;
    // ... ~15 more
}

pub struct RuntimeConfig {
    pub execution_model: String,
    pub provider_name: String,
    pub context_window: usize,        // Replaces non-existent ANTHROPIC_CONTEXT_WINDOW
    pub max_response_tokens: usize,
    pub cache_enabled: bool,
}

pub struct RuntimeResult {
    pub content: String,
    pub mode_used: String,
    pub validation: ValidationResult,
    pub agent_name: String,
    pub turns: u32,
    pub safety_cap_hit: bool,
    pub tool_calls: Vec<String>,
}
```

### `ExecutionCore`

```rust
pub struct ExecutionCore {
    pub provider: DynProvider,
    pub tool_registry: Arc<RwLock<ToolRegistry>>,
    pub outcome_recorder: Option<Arc<OutcomeRecorder>>,
    pub domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    pub interceptor_chain: Option<Arc<tools_core::InterceptorChain>>,
    pub approval_gate: Option<Arc<approval::ApprovalGate>>,
}

impl ExecutionCore {
    pub fn new(provider: DynProvider, tool_registry: Arc<RwLock<ToolRegistry>>) -> Self;

    pub async fn run_cycle(
        &self,
        messages: &mut Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        routing_ctx: &RoutingContext,
        event_tx: Option<&mpsc::Sender<AgentEvent>>,
        seen_tool_calls: Option<&mut HashSet<String>>,
        cache_breakpoints: &[providers::CacheBreakpoint],
    ) -> Result<(CycleOutcome, Usage)>;
}
```

### `execute_loop`

```rust
pub async fn execute_loop(
    core: &ExecutionCore,
    messages: Vec<Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    cap: &mut SafetyCap,
    ctx: &RoutingContext,
    event_tx: Option<mpsc::Sender<AgentEvent>>,
) -> Result<ExecuteLoopResult>;

pub struct ExecuteLoopResult {
    pub messages: Vec<Message>,
    pub final_response: Option<String>,
    pub turns_used: u32,
    pub finish_reason: LoopFinishReason,
    pub total_usage: Usage,
    pub safety_cap_hit: bool,
}

pub enum LoopFinishReason {
    Completed,
    Cancelled,
    SafetyTurnLimit,
    TokenLimit,
    LoopDetected,
}
```

### Execution types

```rust
pub struct ExecutionParams {
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub max_iterations: u32,
    pub tool_timeout: Duration,       // default: 30s (NOT a const — set on params)
    pub pause_context_updates: bool,
    pub cancel_token: CancellationToken,
    // ...
}

pub enum CycleOutcome {
    FinalResponse { content: String },
    FabricatedResponse { content: String, reason: String },
    ToolsExecuted { count: u32 },
    EmptyResponse,
    Cancelled,
}

pub struct ToolExecutionResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub success: bool,
    pub output: String,
    pub duration_ms: u64,
}
```

### `SafetyCap` + `DepthMode`

```rust
pub struct SafetyCap {
    pub max_turns: u32,
    pub max_total_tokens: u32,
    pub used_turns: u32,
    pub used_tokens: u32,
}

impl SafetyCap {
    /// Main agent — ALWAYS sets max_turns = u32::MAX and normal_tokens = 0.
    pub fn new(depth: DepthMode) -> Self;

    /// Subagents/coding review — real cap.
    pub fn with_limits(turns: u32, tokens: u32, depth: DepthMode) -> Self;

    pub fn would_exceed_turns(&self, _: u32) -> bool;
    pub fn would_exceed_tokens(&self, additional: u32) -> bool;
    pub fn tick(&mut self, tokens: u32);
}

pub enum DepthMode { Normal, DeepThink, Ultra }
```

**The main agent has no turn cap.** Stops are: `LoopDetector::HardStop` (5 repeats), cancellation, budget exhaustion.

### `SubagentRuntime`

```rust
pub struct SubagentRuntime {
    pub repo: SubagentInstanceRepo,
    pub sessions: SessionRepo,
    pub active: ActiveSubagentRegistry,
    pub provider: DynProvider,
    // ... ~10 more
}

impl SubagentRuntime {
    pub async fn spawn(&self, p: SpawnParams) -> Result<SubagentRunResult>;
    pub async fn spawn_detached(&self, p: SpawnParams) -> Result<(String, String)>;
    pub async fn resume(&self, p: ResumeParams) -> Result<SubagentRunResult>;
    pub async fn kill(&self, agent_id: &str) -> Result<SubagentRunResult>;
    pub async fn list(
        &self,
        parent_agent_id: Option<&str>,
        status: Option<SubagentStatus>,
    ) -> Result<Vec<SubagentInstanceRow>>;
}

pub struct ActiveSubagentRegistry { /* DashMap<String, CancellationToken> */ }
impl ActiveSubagentRegistry {
    pub fn register(&self, agent_id: &str, token: CancellationToken);
    pub fn cancel(&self, agent_id: &str) -> bool;
    pub fn unregister(&self, agent_id: &str);
    pub fn is_active(&self, agent_id: &str) -> bool;
}

pub struct SpawnParams {
    pub task_id: String,
    pub task_description: String,
    pub parent_agent_id: Option<String>,
    pub agent_profile: String,
    pub workspace_cwd: PathBuf,
    pub depth: DepthMode,
    // ...
}

pub enum SubagentStatus { Pending, Running, Completed, Failed, Cancelled }
```

### `AgentEvent` (the streaming surface)

Major variants (selected):

```rust
pub enum AgentEvent {
    IterationStart { iteration: u32 },
    ContentChunk { text: String },
    ToolStart { name: String, args_preview: String, call_id: String },
    ToolEnd { call_id: String, success: bool, output_preview: String, duration_ms: u64 },
    ApprovalRequest { request_id: String, tool: String, class: ApprovalClass, summary: String },
    Reasoning { text: String },
    BudgetUpdate { tokens_in: u32, tokens_out: u32, cost_usd: f64 },
    UsageReport { usage: Usage },
    PreCompactionRun { before: u32, after: u32 },
    LoopWarning { tool: String, repeat_count: u32 },
    LoopHardStop { tool: String },
    FinalResponse { content: String },
    Error { message: String },
    Cancelled { partial: String },
    // ~20 more
}
```

---

## Internals

### Key constants (with file:line)

| Constant | Value | Location |
|---|---|---|
| `MAX_CONCURRENT_TOOLS` | `10` | `execution/core.rs:60` |
| `MAX_TOOL_RESULT_LENGTH` | `50_000` bytes | `execution/core.rs:65` |
| `LONG_RUNNING_TOOL_TIMEOUT` | `600 s` | `execution/core.rs:54` |
| Default `tool_timeout` (on `ExecutionParams`) | `30 s` | `execution/types.rs:74` |
| `COMPRESSION_THRESHOLD` | `0.70` | `execution/mid_loop_compressor.rs:15` |
| `MIN_RECENT_MESSAGES` | `8` | `execution/mid_loop_compressor.rs:18` |
| `MIN_COMPRESSIBLE_TOKENS` | `50` | `execution/mid_loop_compressor.rs:24` |
| `DEFAULT_TURN_CAP` (subagents) | `500` | `subagent_runtime.rs:21` |
| `CORRECTION_WINDOW_MINUTES` | `15` | `agent_loop/mod.rs:27` |
| `STREAM_GUARD_COUNTER` | `AtomicU64` | (`StreamGuard` in `app-core/runtime`) |

**Not constants — provider-supplied:**
- Context window comes from `RuntimeConfig.context_window` (NOT a named `ANTHROPIC_CONTEXT_WINDOW`).

### KCA env-var feature flags

| Flag | Effect | Location |
|---|---|---|
| `KCA_DISABLE_COMPRESSION=1` | Disables `TieredHistoryCompressor` — returns history verbatim (Letta benchmark mode citing 74% LoCoMo) | `context_engine` (called from `AgentRuntime`) |
| `KCA_PHASE_4=1` + `KCA_PHASE_4_LEGACY_NUDGE=1` | Activates legacy text-nudge memory-refusal retry | `agent_runtime/runtime.rs:575-586` |
| `KCA_PHASE_4_TOOL_DRIVEN=1` | Activates tool-call memory-refusal retry (model nudged to call `memory` tool with entities) | `agent_runtime/runtime.rs:573` |

All three default to off. See [`subsystems/04-agent-runtime.md`](../subsystems/04-agent-runtime.md) for the full KCA flag table including cognitive-side flags.

### The ReAct loop (inside `execute_loop`)

```
loop {
    1. Cancellation token check
    2. SafetyCap gate (turn cap + token cap)
    3. Emit AgentEvent::IterationStart
    4. Compute cache breakpoints via cache_policy::compression_aware_default
    5. ExecutionCore::run_cycle:
       a. Stream LLM response chunks
       b. On tool_use blocks: parallel execute (MAX_CONCURRENT_TOOLS=10),
          partitioned by is_concurrency_safe — safe in join_all, unsafe sequentially
       c. Approval gate per tool call (if ApprovalGate present)
       d. Dedup tool calls via seen_tool_calls HashSet
       e. Fabrication detection (skipped in coding mode)
    6. Accumulate usage, tick turn, call on_iteration callback (subagent heartbeat)
    7. Match CycleOutcome:
       - FinalResponse / FabricatedResponse → return
       - ToolsExecuted → LoopDetector.check (Warning@3, HardStop@5)
       - EmptyResponse → treat as self-stop
       - Cancelled → return partial
    8. MidLoopCompressor.compress_if_needed (fires PreCompact/PostCompact hooks)
    9. LiveContextRefresher.inject_pending_with_ctx
       (drains ContextUpdateQueue + InjectorRegistry::collect_all)
   10. Emit AgentEvent::BudgetUpdate
}
```

### `LoopDetector`

Tracks `(tool_name, args_hash)` tuples per iteration:
- 3 repeats → emit `Warning`
- 5 repeats → emit `HardStop` (breaks loop with `LoopFinishReason::LoopDetected`)

### `MidLoopCompressor`

Fires when total message tokens exceed `COMPRESSION_THRESHOLD = 0.70` of `context_window`. Strategy:
- Preserve `system_count` prefix + `MIN_RECENT_MESSAGES = 8` tail verbatim
- Replace older `Message::Tool` entries exceeding `MIN_COMPRESSIBLE_TOKENS = 50` with a 150-char extractive snippet
- **Image parts dropped** on compression (one-way, lossy)
- Returns `Some((before, after, compacted_count))` when applied, `None` otherwise

### `LiveContextRefresher`

Called at each iteration boundary in `execute_loop` (after mid-loop compression). Drains `ContextUpdateQueue` and calls `InjectorRegistry::collect_all(ctx)` for dynamic injectors. Injects as `Message::ContextUpdate`. High-priority updates get 10% response reserve; standard gets 20%. Drops updates that exceed token budget with a warning.

### KCA Phase-4 — Letta-style memory-refusal recovery

```
1. After first response, ResponseValidator::detect_memory_refusal scans for refusal phrases
2. If detected AND (KCA_PHASE_4_TOOL_DRIVEN OR KCA_PHASE_4 + LEGACY_NUDGE both set):
   - Tool-driven path: append user msg nudging model to call `memory` tool with named entities
   - Legacy path: append user msg "re-search with broader effort"
3. Re-run execute_loop once more
4. Accept retry result ONLY IF detect_memory_refusal does NOT fire again
   (double-refusal → keep original)
5. Both retries spent → return original response
```

### Subagent `spawn` vs `spawn_detached`

| Method | Behavior |
|---|---|
| `spawn` | Synchronous — blocks until loop completes; returns `SubagentRunResult` |
| `spawn_detached` | Async setup (insert DB rows, register cancel token, emit `Spawned`); `tokio::spawn` the loop; returns `(agent_id, session_id)` immediately |

Both use `execute_loop` with `SafetyCap::with_limits(DEFAULT_TURN_CAP = 500)`. Both clone the cached base tool registry and append a fresh `AgentTaskTool` (the only per-invocation tool — see `subagent.rs:800,819`).

### Predictive cache warming (KCA Track 7)

After each completed turn, `AgentRuntime` fires a detached `tokio::spawn` that:
1. Calls `LlmQueryPredictorHandler::predict_next` to generate `predictions_per_turn` (default 3) follow-up queries.
2. Pre-retrieves memories for each predicted query.
3. Stores in `PredictiveCache`.
4. Cache hits on the next actual turn skip the full retrieval.

### Focus-session message deferral

`AgentLoop::run_with_rx` listens on `DomainEventBus` for `FocusSessionStarted` / `FocusSessionEnded`. While focus session is active:
- Inbound messages buffered in `deferred_messages: Vec<InboundMessage>`
- Single auto-reply per `(channel, sender)` pair (configurable text, deduped per session)
- On `FocusSessionEnded`, messages drain in order

### Correction rate-limiter

`session.correction_cooldown: u32` is set to 3 on first keyword-correction emission; decremented per message. Prevents correction-signal spam. Symmetric in `process_message` and `process_direct_streaming`.

### `CORRECTION_WINDOW_MINUTES = 15`

When a reaction-based correction is emitted, the trial repo retroactively marks shadow log entries from the past 15 minutes as corrected. Autotuner feedback loop.

### Fabrication detection (skipped in coding mode)

`ExecutionCore::check_fabrication` heuristic — checks for fake hex IDs, context-aware structured-result phrases (todo/search/calendar), and multiple field patterns. **Skipped entirely when `channel == CODING_CHANNEL`** because coding-mode legitimately produces output that looks like fabricated structured data. `FabricatedResponse` is treated identically to `FinalResponse` by the loop — distinction visible only in the event stream.

### `LlmSummaryProvider` (in `adapters/llm_summary.rs`)

Bridges `SummaryProvider` trait to a real LLM. Sends up to 5 conversation segments per LLM call (parallel sub-batches via `join_all`). Returns a JSON array of summaries. Handles reasoning-model prose wrapping by preferring the *last* balanced JSON array (`extract_last_json_array`) before falling back to first match.

---

## Workflows

See `subsystems/04-agent-runtime.md` for the full end-to-end traces. Crate-level workflow summary:

| Workflow | Entry point | Returns |
|---|---|---|
| Bus-driven turn (e.g. Telegram) | `AgentLoop::run_with_rx` | runs until shutdown |
| Direct turn (Tauri command) | `AgentLoop::process_direct_streaming` | `StreamingHandle` |
| Subagent spawn (sync) | `SubagentRuntime::spawn` | `SubagentRunResult` |
| Subagent spawn (async) | `SubagentRuntime::spawn_detached` | `(agent_id, session_id)` |
| Subagent cancel | `ActiveSubagentRegistry::cancel(agent_id)` | `bool` (was active?) |

---

## Testing approach

### Use `NoopProvider`

```rust
let provider: DynProvider = Box::new(
    NoopProvider::new().with_response(LlmResponse {
        content: Some("hello".into()),
        ..Default::default()
    })
);
let core = Arc::new(ExecutionCore::new(provider, registry));
```

### `process_direct` is the easiest entry point for integration tests

```rust
let result = agent_loop.process_direct(
    "Hello".to_string(),
    "test:session-1".to_string(),
).await.unwrap();
```

### Streaming test pattern

```rust
let handle = agent_loop.process_direct_streaming(
    content, session_key, None,
).await.unwrap();

let mut events = Vec::new();
while let Some(event) = handle.event_rx.recv().await {
    events.push(event);
}
let final_text = handle.handle.await.unwrap().unwrap();
```

### Force cancellation

```rust
let handle = agent_loop.process_direct_streaming(...).await.unwrap();
handle.cancel_token.cancel();
let result = handle.handle.await.unwrap();
assert!(matches!(result, Err(KlyntbotError::Cancelled(_))));
```

### Test compression behavior

Use `MidLoopCompressor::compress_if_needed` directly with a constructed message list above the threshold. The compressor is pure — no I/O.

---

## Extension points

### Add a `ContextSource`

Implement the trait (`crates/context_engine/src/source.rs`); register via `ContextEngine::register_source` (typically in `app-core::init`). See [`crates/context_engine.md`](./context_engine.md) for the trait + priority semantics.

### Add an `AgentEvent` variant

⚠️ Cross-cutting. Every consumer that pattern-matches `AgentEvent` (frontend store, MCP relay, etc.) must handle it. Coordinate or hide behind an existing variant.

### Add a `ClassifyHook` (approval)

Lives in the `approval` crate. See [`subsystems/10-sandboxing-security.md`](../subsystems/10-sandboxing-security.md).

### Add a tool

Four wiring paths exist (FeaturePackage / agent::builder / app-core::init / subagent). See [`subsystems/07-tools-framework.md`](../subsystems/07-tools-framework.md#the-four-wiring-paths).

### Add a KCA env flag

1. Read at startup or per-call via `std::env::var(...).is_ok_and(|v| matches!(v.as_str(), "1"|"true"|"yes"))`.
2. Document in the [Key constants](#key-constants-with-fileline) table.
3. Use sparingly — config-driven is preferred for non-experimental flags.

### Add a subagent profile

Edit `agent_profile/skill_loader.rs` to add the profile definition. The runtime injects the profile-specific tools/skills at spawn time.

---

## Open questions

- **`intent_pipeline` is vestigial.** `SourceContext::intent_summary` always `None`; runtime is flat. Decide: delete or repurpose.
- **Main agent has no turn cap.** Deliberate; revisit if observed loops waste tokens.
- **MidLoopCompressor + TieredHistoryCompressor have overlapping concerns.** Both can fire on same turn. Document the interaction more carefully (in `04-agent-runtime.md`).
- **`KCA_*` flags are env-only.** Migrate experimental ones to config when they stabilize.
- **Predictive cache warming is undocumented** at the user-facing level. Worth a config flag to disable for diagnostic runs.
- **Subagent registry uses `DashMap`** — consider whether `RwLock<HashMap>` would be sufficient (it's not hot enough to need shards).
- **`AgentEvent` is large** (~30 variants). Some are coding-specific; consider splitting into `AssistantEvent` + `CodingEvent` for type clarity.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #3 + #5 + #9 for specifics.

---

## Cross-references

- [Subsystem 04 — Agent Runtime](../subsystems/04-agent-runtime.md) (parent)
- [`crates/providers.md`](./providers.md) (consumed `DynProvider`)
- [`crates/tools-core.md`](./tools-core.md) (`Tool`, `ToolRegistry`, `RoutingContext`)
- [`crates/context_engine.md`](./context_engine.md) (consumed `ContextEngine`)
- [`crates/cognitive.md`](./cognitive.md) *(planned)* (consumed memory services)
- [`crates/app-core.md`](./app-core.md) (constructs `AgentLoop`; owns `ThreadRuntime`)
