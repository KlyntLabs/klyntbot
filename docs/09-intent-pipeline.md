# Intent Pipeline

The intent pipeline is the decision-making core of the agent. It receives a user message, classifies the intent, selects an execution strategy, runs the appropriate engine, validates the response, and records costs -- all in a single `process_message()` call.

## Section 1: Narrative Overview

### What it replaced

The intent pipeline (introduced in Phase 5, v0.4.0) replaced three separate systems:

- **Orchestrator** -- manually routed messages to engines based on simple heuristics
- **EngineDispatch** -- dispatched to Direct/Reactive/Planned engines with no unified interface
- **AgentPipeline** -- glued the above together with ad-hoc wiring

The new pipeline collapses all three into a single, five-stage flow with structured types, automatic escalation, and strategy feedback.

### Pipeline flow

```
User Message
     │
     ▼
┌─────────────────┐   Stage 1: Classify intent (heuristic → LLM)
│  IntentAnalyzer  │   Returns: ExecutionMode, ComplexitySignals, confidence, tool groups
└────────┬────────┘
         │
         ▼
┌─────────────────┐   Stage 2: Allocate token budget, assemble history
│  ContextEngine   │   Maps ExecutionMode → ExecutionStrategy for budget sizing
└────────┬────────┘
         │
         ▼
┌─────────────────┐   Stage 3: Filter tools, dispatch to engine, handle escalation
│ ExecutionRouter  │   Engines: DirectEngine | ReactiveEngine | PlannedEngine
└────────┬────────┘
         │
         ▼
┌─────────────────┐   Stage 4: Safety + quality checks
│ResponseValidator │   Truncation, system-prompt leak detection, quality flags
└────────┬────────┘
         │
         ▼
┌─────────────────┐   Stage 5: Record token usage + strategy outcome
│   CostTracker    │   Persisted to SQLite for reporting
└────────┬────────┘
         │
         ▼
    PipelineResult
```

**Source:** `crates/agent/src/intent_pipeline/pipeline.rs` (lines 68-261)

### Two-stage analysis

Intent analysis uses a two-stage design to minimize latency and LLM cost.

**Stage 1 -- Heuristic classification (zero cost, sub-millisecond).**
The `analyze_heuristic()` function pattern-matches the user message against keyword lists. It handles six categories in priority order:

1. **Greetings** -- "hi", "hello", "good morning" --> `Direct` (confidence 0.95)
2. **Short non-keyword messages** -- under 20 chars, 4 words, no action keywords --> `Direct` (0.85)
3. **Task management CRUD** -- "create a task", "add a todo" --> `Reactive` with max 5 iterations (0.90)
4. **Direct questions** -- "what is", "explain", "tell me about" --> `Direct` (0.90), unless conflicting action keywords are present (defers to LLM)
5. **Explicit plan keywords** -- "create a plan", "plan and implement" --> `Planned` (0.85)
6. **Structural complexity analysis** -- counts tool indicators, detects sequential language, assesses failure risk. Routes to `Reactive` or `Planned` based on a 0-7 complexity score.

If heuristics produce a result with confidence >= the configured threshold (default 0.85), the LLM is never called.

**Stage 2 -- LLM classifier (when heuristics are ambiguous).**
A lightweight LLM call with a structured JSON prompt classifies the message and returns `ComplexitySignals`. The classifier has a configurable timeout (default 2000ms). On timeout, parse error, or low confidence (< 0.5), the pipeline falls back to `Reactive { max_iterations: 10 }`.

**Source:** `crates/agent/src/intent_pipeline/analyzer.rs` (lines 22-131), `heuristics.rs` (lines 14-134), `classifier.rs` (lines 18-156)

### ExecutionMode

The pipeline selects one of three modes, each with different cost and capability profiles:

| Mode | When used | LLM calls | Tools | Max iterations |
|------|-----------|-----------|-------|----------------|
| **Direct** | Greetings, factual Q&A, short messages | 1 | None | 1 |
| **Reactive** | Task CRUD, searches, single-shot tool use | 1-N | Yes | 5-10 (configurable) |
| **Planned** | Multi-step workflows with dependencies | N+1 (plan generation + per-step) | Yes | 10-15 steps |

`Reactive` carries a `max_iterations` field that caps the ReAct loop. `Planned` carries `visibility` (controls cleanup behavior) and `max_steps`.

**Source:** `crates/agent/src/intent_pipeline/types.rs` (lines 8-19)

### ComplexitySignals

Structured analysis of request complexity, produced by both heuristics and the LLM classifier:

| Field | Type | Meaning |
|-------|------|---------|
| `estimated_tool_calls` | `u8` | Expected number of tool invocations (0-10) |
| `has_sequential_deps` | `bool` | Steps depend on each other's output ("first X, then Y") |
| `failure_risk` | `FailureRisk` | `Low`, `Medium` (API/network), or `High` (deploy/payment/migration) |
| `requires_state_tracking` | `bool` | Needs to compare, rank, or select across results |
| `requires_retries` | `bool` | Explicit retry/fallback language detected |

The `complexity_score()` method produces a 0-7 integer used for routing decisions:

```
+2  if estimated_tool_calls >= 3  (+1 if >= 2)
+2  if has_sequential_deps
+1  if failure_risk >= Medium
+1  if requires_state_tracking
+1  if requires_retries
```

**Source:** `crates/agent/src/intent_pipeline/types.rs` (lines 82-121)

### Tool groups

The pipeline narrows the tool action space based on classified intent, reducing hallucinated tool calls:

| Group | Tools | Use case |
|-------|-------|----------|
| `None` | (empty) | Greetings, factual Q&A |
| `TaskManagement` | todo, goal, plan | Task/todo CRUD |
| `Search` | grep, glob, read_file, list_dir, web_search, web_fetch, memory | File and web retrieval |
| `Calendar` | calendar, todo | Calendar operations |
| `Finance` | finance | Financial operations |
| `Communication` | message, ask_user | User interaction |
| `Automation` | cron, spawn | Background jobs |
| `Full` | (all tools) | Complex/ambiguous requests |

The `ask_user` tool is always included regardless of group selection.

**Source:** `crates/agent/src/intent_pipeline/types.rs` (lines 131-178)

### Engines

All three engines implement the `ExecutionEngine` trait and return `EngineResult` (either `Complete` or `Escalate`).

**DirectEngine** -- Makes a single LLM call with no tools. If the LLM generates tool calls despite being given none, the engine signals `Escalate` with the conversation context so no work is lost.

**Source:** `crates/agent/src/intent_pipeline/engines/direct.rs` (lines 17-78)

**ReactiveEngine** -- Runs a ReAct (Reason + Act) loop. On each iteration it calls the LLM, executes any requested tools, and feeds results back. Key behaviors:

- **Fabrication detection**: If the LLM returns a text response instead of calling a tool (fabricated tool output), the engine injects a force prompt and retries once.
- **Duplicate blocking**: Tracks seen tool call signatures via a `HashSet`. Duplicate calls are blocked with a redirect prompt.
- **Escalation threshold**: When 80% of `max_iterations` are consumed and the loop is still executing tools, the engine signals `Escalate` with all completed work packaged in an `EscalationContext`.
- **Failure reflection**: Tool failures trigger a reflection prompt asking the LLM to adjust its approach.

**Source:** `crates/agent/src/intent_pipeline/engines/reactive.rs` (lines 25-249)

**PlannedEngine** -- Decomposes a task into a multi-step plan via an LLM call, persists the plan to SQLite, and executes each step sequentially with retry and backtracking:

1. **Plan generation**: Calls the LLM with available tool names to generate a JSON array of step drafts.
2. **Persistence**: Saves the plan in `Approved` state, transitions to `Executing`.
3. **Step execution**: For each step, builds a context window (current + next 3 steps), calls the LLM, executes tool calls.
4. **Retry**: Failed steps are retried up to `max_attempts` (default 3).
5. **Backtracking**: After max retries, calls `regenerate_from()` to get replacement steps from the failure point. Up to 3 backtrack events before the plan is marked `Failed`.
6. **Response synthesis**: After all steps complete, an LLM call synthesizes a human-readable summary from raw step outputs.
7. **Reactive fallback**: If plan generation returns zero steps, falls back to a `ReactiveEngine` with 50 iterations.

The engine also supports **escalation takeover** via `execute_with_prior_work()`, which accepts an `EscalationContext` from the ReactiveEngine, pre-fills completed steps, and generates only the remaining steps.

**Source:** `crates/agent/src/intent_pipeline/engines/planned.rs` (lines 28-476)

### Router

The `ExecutionRouter` maps `ExecutionMode` to the appropriate engine and manages the escalation chain. It holds instances of `DirectEngine`, `ReactiveEngine`, and optionally `PlannedEngine`.

```
ExecutionMode::Direct   → DirectEngine.execute()
ExecutionMode::Reactive → ReactiveEngine.execute()
ExecutionMode::Planned  → PlannedEngine.execute_with_visibility()
```

If no `PlannedEngine` is configured and `Planned` mode is requested, the router falls back to `ReactiveEngine`.

**Source:** `crates/agent/src/intent_pipeline/router.rs` (lines 39-247)

### Escalation chain

When an engine returns `EngineResult::Escalate`, the router automatically upgrades to the next mode:

```
Direct ──escalate──▶ Reactive ──escalate──▶ Planned ──(terminal)
```

Each escalation carries an `EscalationContext` containing:

- `messages` -- Full conversation history accumulated so far
- `completed_work` -- `Vec<CompletedStep>` with tool name, description, and result for each successful tool call
- `original_message` -- The user's original request

This ensures no work is repeated. The `PlannedEngine` receives the completed work as pre-filled completed steps in the generated plan.

Escalation is bounded by `max_escalations` (configurable, default 1). When the limit is reached, the router returns an error message explaining the task exceeded the escalation limit.

Triggers for escalation:
- **Direct --> Reactive**: LLM generates tool calls in direct mode (it needed tools despite classification saying otherwise)
- **Reactive --> Planned**: ReactiveEngine consumed 80% of its iteration budget while still executing tools (task is more complex than expected)

**Source:** `crates/agent/src/intent_pipeline/router.rs` (lines 116-243), `escalation.rs` (lines 1-46)

### Visibility service

The `PlanCleanupService` runs as a background `tokio::spawn` task on an hourly interval, deleting stale plans based on their visibility level:

| Visibility | Behavior | Cleanup rule |
|------------|----------|-------------|
| `transparent` | Always visible in dashboard | Never auto-deleted |
| `on_failure` | Only surfaced if the plan fails | Successful plans deleted after 7 days |
| `silent` | Never shown to user | Deleted 24 hours after reaching terminal state |

The service accepts a `CancellationToken` for graceful shutdown.

**Source:** `crates/agent/src/intent_pipeline/visibility.rs` (lines 19-72)

### Configuration

All settings live under the `orchestrator` key in `~/.klyntbot/config.json`:

```json
{
  "orchestrator": {
    "heuristicConfidenceThreshold": 0.85,
    "llmClassifierTimeout": 2000,
    "llmClassifierModel": null,
    "defaultPlanVisibility": "on_failure",
    "planComplexityThreshold": 3,
    "maxEscalations": 1
  }
}
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `heuristicConfidenceThreshold` | `f32` | `0.85` | Minimum confidence for heuristic result to be accepted without LLM fallback |
| `llmClassifierTimeout` | `u64` (ms) | `2000` | Timeout for the LLM classifier call |
| `llmClassifierModel` | `Option<String>` | `null` | Override model for classifier (uses default agent model if null) |
| `defaultPlanVisibility` | `String` | `"on_failure"` | Default visibility for auto-generated plans |
| `planComplexityThreshold` | `u8` | `3` | Complexity score threshold for triggering planned execution |
| `maxEscalations` | `u32` | `1` | Maximum escalations per request |

**Source:** `crates/config/src/schema/orchestrator.rs` (lines 6-61)

### Strategy feedback loop

The pipeline records every execution outcome to a `StrategyRepo` in SQLite. Each record captures:

- Predicted strategy (from classifier) vs. actual strategy (after escalation)
- Escalation count, iterations used, response time
- Success (validation passed), tool name and duration

The `IntentAnalyzer` loads the last 30 days of strategy summaries (cached for 60 seconds) and feeds them to the LLM classifier as context. This creates a feedback loop: the classifier learns from historical accuracy to make better routing decisions over time.

**Source:** `crates/agent/src/intent_pipeline/pipeline.rs` (lines 279-314), `mod.rs` (lines 21-38)

---

## Section 2: API Reference

### IntentPipeline

**File:** `crates/agent/src/intent_pipeline/pipeline.rs` (line 70)

```rust
pub struct IntentPipeline {
    analyzer: IntentAnalyzer,
    context_engine: Arc<ContextEngine>,
    router: ExecutionRouter,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    config: PipelineConfig,
    strategy_repo: Option<storage::StrategyRepo>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(IntentAnalyzer, Arc<ContextEngine>, ExecutionRouter, Arc<CostTracker>, PipelineConfig) -> Self` | Construct the pipeline with all dependencies |
| `with_strategy_repo` | `(self, StrategyRepo) -> Self` | Attach strategy recording (builder pattern) |
| `process_message` | `(&self, message, history, tool_defs, tool_names, ctx, system_prompt, event_tx) -> Result<PipelineResult>` | Full pipeline execution; see below |

**`process_message` parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `message` | `&str` | The user's message text |
| `history` | `Vec<Message>` | Conversation history |
| `tool_definitions` | `&[serde_json::Value]` | JSON tool definitions for the LLM |
| `tool_names` | `&[&str]` | Tool name strings for the classifier |
| `ctx` | `&RoutingContext` | Channel and chat ID context |
| `system_prompt` | `Option<&str>` | Override system prompt (falls back to config default) |
| `event_tx` | `Option<Sender<AgentEvent>>` | Optional channel for streaming pipeline events |

### PipelineResult

**File:** `crates/agent/src/intent_pipeline/pipeline.rs` (line 26)

```rust
pub struct PipelineResult {
    pub content: String,
    pub mode_used: String,
    pub classification: IntentAnalysis,
    pub escalations: u32,
    pub validation: ValidationResult,
}
```

### PipelineConfig

**File:** `crates/agent/src/intent_pipeline/pipeline.rs` (line 40)

```rust
pub struct PipelineConfig {
    pub execution_model: String,
    pub system_prompt: String,
    pub context_window: usize,
    pub max_response_tokens: usize,
    pub channel: String,
    pub provider_name: String,
}
```

### IntentAnalyzer

**File:** `crates/agent/src/intent_pipeline/analyzer.rs` (line 22)

```rust
pub struct IntentAnalyzer {
    classifier: IntentClassifier,
    classifier_params: ChatParams,
    strategy_repo: Option<storage::StrategyRepo>,
    config: OrchestratorConfig,
    strategy_cache: Mutex<Option<(Instant, Option<String>)>>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(DynProvider, &str, &OrchestratorConfig) -> Self` | Create analyzer with LLM provider, model name, and config |
| `with_strategy_repo` | `(self, StrategyRepo) -> Self` | Attach strategy feedback repository |
| `analyze` | `(&self, message, tool_names) -> IntentAnalysis` | Two-stage classification: heuristic then LLM |

### IntentAnalysis

**File:** `crates/agent/src/intent_pipeline/types.rs` (line 182)

```rust
pub struct IntentAnalysis {
    pub mode: ExecutionMode,
    pub signals: ComplexitySignals,
    pub confidence: f32,
    pub source: AnalysisSource,
    pub reasoning: String,
    pub tool_groups: Vec<ToolGroup>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `fallback` | `() -> Self` | Fallback analysis: `Reactive { max_iterations: 10 }`, confidence 0.5, `ToolGroup::Full` |
| `allowed_tool_names` | `(&self) -> Option<HashSet<&'static str>>` | Collect allowed tools from groups; returns `None` if `Full` is present |

### Heuristics

**File:** `crates/agent/src/intent_pipeline/heuristics.rs`

| Function | Line | Signature | Description |
|----------|------|-----------|-------------|
| `analyze_heuristic` | 14 | `(message: &str) -> Option<IntentAnalysis>` | Main entry point. Returns `Some` for clear-cut intents, `None` for ambiguous messages |
| `is_greeting` | 140 | `(msg: &str) -> bool` | Matches greeting patterns ("hi", "hello", "good morning", etc.) |
| `is_task_management` | 157 | `(msg: &str) -> bool` | Matches task CRUD patterns ("create a task", "add a todo", etc.) |
| `is_direct_question` | 171 | `(msg: &str) -> bool` | Matches question/explanation patterns ("what is", "explain", etc.) |
| `has_tool_keyword` | 187 | `(msg: &str) -> bool` | Detects tool-assisted operation keywords ("search", "find", "list", etc.) |
| `has_action_keyword` | 205 | `(msg: &str) -> bool` | Detects code/action keywords ("fix", "build", "implement", etc.) |
| `has_plan_keyword` | 231 | `(msg: &str) -> bool` | Detects explicit planning language ("create a plan", "plan and implement") |
| `analyze_complexity` | 333 | `(msg: &str) -> ComplexitySignals` | Builds full complexity signals from message analysis |
| `detect_sequential_language` | 259 | `(msg: &str) -> bool` | Detects ordering language ("first...then", "after that"); requires 2+ indicators |
| `assess_failure_risk` | 279 | `(msg: &str) -> FailureRisk` | Keyword-based risk assessment (deploy/production = High, API/network = Medium) |
| `infer_tool_groups` | 389 | `(msg: &str) -> Vec<ToolGroup>` | Maps message keywords to relevant tool groups |

### IntentClassifier (LLM)

**File:** `crates/agent/src/intent_pipeline/classifier.rs` (line 18)

```rust
pub struct IntentClassifier {
    provider: DynProvider,
    timeout: Duration,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(DynProvider, Duration) -> Self` | Create classifier with LLM provider and timeout |
| `classify` | `(&self, message, tool_names, params, strategy_context) -> Result<IntentAnalysis>` | Single LLM call returning structured JSON classification |

The classifier sends a structured prompt requesting JSON with fields: `mode`, `estimated_tool_calls`, `has_sequential_deps`, `failure_risk`, `requires_state_tracking`, `requires_retries`, `relevant_tools`, `confidence`, `reasoning`. The response is parsed by `parse_classification_json()` (line 88) which extracts JSON from arbitrary surrounding text.

### ExecutionMode

**File:** `crates/agent/src/intent_pipeline/types.rs` (line 9)

```rust
pub enum ExecutionMode {
    Direct,
    Reactive { max_iterations: u32 },
    Planned { visibility: PlanVisibility, max_steps: u8 },
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `short_name` | `(&self) -> &'static str` | Returns `"direct"`, `"reactive"`, or `"planned"` |
| `max_iterations` | `(&self) -> u32` | Returns 1 for Direct, `max_iterations` for Reactive, `max_steps` for Planned |

Implements `From<&ExecutionMode>` for `context_engine::ExecutionStrategy`:
- `Direct` --> `DirectResponse`
- `Reactive` --> `ToolAssisted { max_iterations }`
- `Planned` --> `AutonomousTask { max_iterations: 50 }`

### ComplexitySignals

**File:** `crates/agent/src/intent_pipeline/types.rs` (line 83)

```rust
pub struct ComplexitySignals {
    pub estimated_tool_calls: u8,
    pub has_sequential_deps: bool,
    pub failure_risk: FailureRisk,
    pub requires_state_tracking: bool,
    pub requires_retries: bool,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `complexity_score` | `(&self) -> u8` | Weighted 0-7 score used for routing decisions |

### FailureRisk

**File:** `crates/agent/src/intent_pipeline/types.rs` (line 74)

```rust
pub enum FailureRisk { Low, Medium, High }
```

Implements `PartialOrd`/`Ord` so `Low < Medium < High`.

### AnalysisSource

**File:** `crates/agent/src/intent_pipeline/types.rs` (line 124)

```rust
pub enum AnalysisSource {
    Heuristic,
    LlmClassifier,
    MidExecutionEscalation,
}
```

### ToolGroup

**File:** `crates/agent/src/intent_pipeline/types.rs` (line 137)

```rust
pub enum ToolGroup { None, TaskManagement, Search, Calendar, Finance, Communication, Automation, Full }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `tool_names` | `(&self) -> &'static [&'static str]` | Returns the tool name strings for this group; `Full` returns empty (special: means all) |

### ExecutionEngine trait

**File:** `crates/agent/src/intent_pipeline/engines/mod.rs` (line 37)

```rust
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    async fn execute(
        &self,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<Sender<AgentEvent>>,
    ) -> Result<EngineResult>;

    fn mode(&self) -> &str;
}
```

### EngineResult

**File:** `crates/agent/src/intent_pipeline/engines/mod.rs` (line 18)

```rust
pub enum EngineResult {
    Complete {
        content: String,
        usage: Usage,
        iterations: u32,
        traces: Vec<ReasoningTrace>,
        tool_name: Option<String>,
    },
    Escalate {
        reason: String,
        carried_context: EscalationContext,
        usage: Usage,
    },
}
```

### DirectEngine

**File:** `crates/agent/src/intent_pipeline/engines/direct.rs` (line 18)

```rust
pub struct DirectEngine { core: Arc<ExecutionCore> }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(Arc<ExecutionCore>) -> Self` | Construct with shared execution core |
| `execute` | (trait impl) | Single LLM call; escalates on tool calls |
| `mode` | (trait impl) | Returns `"direct"` |

### ReactiveEngine

**File:** `crates/agent/src/intent_pipeline/engines/reactive.rs` (line 26)

```rust
pub struct ReactiveEngine { core: Arc<ExecutionCore>, max_iterations: u32 }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(Arc<ExecutionCore>, u32) -> Self` | Construct with execution core and iteration cap |
| `execute` | (trait impl) | ReAct loop with fabrication detection, duplicate blocking, escalation at 80% |
| `mode` | (trait impl) | Returns `"reactive"` |

### PlannedEngine

**File:** `crates/agent/src/intent_pipeline/engines/planned.rs` (line 28)

```rust
pub struct PlannedEngine {
    core: Arc<ExecutionCore>,
    plan_repo: storage::PlanRepo,
    provider: DynProvider,
    model: String,
    default_visibility: PlanVisibility,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(Arc<ExecutionCore>, PlanRepo, DynProvider, String, PlanVisibility) -> Self` | Construct with execution core, plan storage, LLM provider, model, and default visibility |
| `execute` | (trait impl) | Generate plan from scratch, persist, execute steps |
| `execute_with_visibility` | `(&self, messages, tools, params, ctx, event_tx, PlanVisibility) -> Result<EngineResult>` | Execute with a specific visibility override from the classifier |
| `execute_with_prior_work` | `(&self, EscalationContext, tools, params, ctx, event_tx) -> Result<EngineResult>` | Escalation takeover: pre-fills completed steps from prior reactive work |
| `mode` | (trait impl) | Returns `"planned"` |

### EscalationContext

**File:** `crates/agent/src/intent_pipeline/escalation.rs` (line 19)

```rust
pub struct EscalationContext {
    pub messages: Vec<Message>,
    pub completed_work: Vec<CompletedStep>,
    pub original_message: String,
}
```

### CompletedStep

**File:** `crates/agent/src/intent_pipeline/escalation.rs` (line 10)

```rust
pub struct CompletedStep {
    pub description: String,
    pub tool_name: String,
    pub result: String,
}
```

### ExecutionRouter

**File:** `crates/agent/src/intent_pipeline/router.rs` (line 39)

```rust
pub struct ExecutionRouter {
    direct: DirectEngine,
    reactive: ReactiveEngine,
    planned: Option<PlannedEngine>,
    max_escalations: u32,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(DirectEngine, ReactiveEngine, Option<PlannedEngine>, u32) -> Self` | Construct with engines and max escalation count |
| `execute` | `(&self, ExecutionMode, Vec<Message>, tools, params, ctx, event_tx) -> Result<RouterResult>` | Dispatch to engine with automatic escalation loop |

### RouterResult

**File:** `crates/agent/src/intent_pipeline/router.rs` (line 22)

```rust
pub struct RouterResult {
    pub content: String,
    pub final_mode: String,
    pub escalation_count: u32,
    pub usage: Usage,
    pub iterations: u32,
    pub tool_name: Option<String>,
}
```

### PlanCleanupService

**File:** `crates/agent/src/intent_pipeline/visibility.rs` (line 20)

```rust
pub struct PlanCleanupService {
    plan_repo: storage::PlanRepo,
    cancel: CancellationToken,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(PlanRepo, CancellationToken) -> Self` | Construct with plan repository and cancellation token |
| `spawn` | `(self)` | Spawns the cleanup loop as a background tokio task |

Cleanup runs every 3600 seconds (1 hour). Calls `PlanRepo::delete_stale_plans(24, 168)` to remove silent plans older than 24 hours and successful on_failure plans older than 7 days (168 hours).

### ResponseValidator

**File:** `crates/agent/src/output/validator.rs` (line 11)

```rust
pub struct ResponseValidator {
    max_response_chars: usize,
    check_leaked_system_prompt: bool,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `(max_response_tokens: usize) -> Self` | Construct with max tokens (chars = tokens * 4) |
| `with_system_leak_check` | `(self, bool) -> Self` | Enable/disable system prompt leak detection |
| `validate` | `(&self, content: &str) -> ValidationResult` | Run all validation checks |

Checks performed:
1. **Length truncation** -- Truncates at word boundary, appends ellipsis
2. **System prompt leak detection** -- Scans for 11 known patterns (e.g., "you are klyntbot", "`<system>`"), redacts matches
3. **Quality checks** -- Flags empty responses (invalid) and extremely short responses (warning)

### ValidationResult

**File:** `crates/agent/src/output/validator.rs` (line 19)

```rust
pub struct ValidationResult {
    pub is_valid: bool,
    pub warnings: Vec<ValidationWarning>,
    pub filtered_content: String,
}
```

### CostTracker

**File:** `crates/agent/src/output/cost_tracker.rs` (line 40)

```rust
pub struct CostTracker { sql_repo: storage::UsageRepo }
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `from_repo` | `(UsageRepo) -> Self` | Construct from SQL repository |
| `record` | `(&self, usage, model, provider, strategy, channel) -> Result<()>` | Record a single usage entry with estimated cost |
| `report` | `(&self, days: u32) -> Result<UsageReport>` | Generate aggregated usage report for the last N days |

### OrchestratorConfig

**File:** `crates/config/src/schema/orchestrator.rs` (line 8)

```rust
pub struct OrchestratorConfig {
    pub heuristic_confidence_threshold: f32,  // default: 0.85
    pub llm_classifier_timeout: u64,          // default: 2000 (ms)
    pub llm_classifier_model: Option<String>, // default: None
    pub default_plan_visibility: String,      // default: "on_failure"
    pub plan_complexity_threshold: u8,        // default: 3
    pub max_escalations: u32,                 // default: 1
}
```

### Module re-exports

**File:** `crates/agent/src/intent_pipeline/mod.rs` (lines 17-18)

```rust
pub use pipeline::IntentPipeline;
pub use types::{AnalysisSource, ComplexitySignals, ExecutionMode, FailureRisk, IntentAnalysis};
```

### Helper: format_strategy_context

**File:** `crates/agent/src/intent_pipeline/mod.rs` (line 21)

```rust
pub(crate) fn format_strategy_context(summaries: &[StrategySummaryRow]) -> String
```

Formats historical strategy performance (accuracy %, sample count, average escalations) into a human-readable string appended to the LLM classifier prompt.
