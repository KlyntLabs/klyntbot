# Intent Pipeline

## Purpose

The intent pipeline is the central message-processing system that replaces the former Orchestrator + EngineDispatch + AgentPipeline. It receives a user message, classifies the intent, assembles context under a token budget, routes execution to the appropriate engine, validates the response, and records usage and strategy outcomes. The full flow is:

```
IntentAnalyzer -> ContextEngine -> ExecutionRouter -> ResponseValidator -> CostTracker
```

All modules live under `crates/agent/src/intent_pipeline/`.

## Key Types

### ExecutionMode

Defined in `types.rs`. Determines how a message is handled:

- **Direct** -- Single LLM call with no tools. Used for greetings, factual questions, acknowledgments, and short non-action messages.
- **Reactive { max_iterations }** -- ReAct loop with tool calls. Used for single-shot tasks like search, CRUD operations, and lookups. The `max_iterations` field caps the number of LLM-tool cycles.
- **Planned { visibility, max_steps }** -- Multi-step plan generation and sequential execution. Used for complex multi-tool workflows with dependencies. Carries a `PlanVisibility` and a step cap.

Each mode converts to a `context_engine::ExecutionStrategy` via a `From` impl, so the context engine can allocate token budgets without knowing about the pipeline's mode enum.

### ComplexitySignals

A structured assessment of request complexity, scored 0-7:

- `estimated_tool_calls` (u8) -- how many tools the message likely needs
- `has_sequential_deps` (bool) -- whether steps depend on each other (+2 to score)
- `failure_risk` (FailureRisk: Low/Medium/High) -- risk of operations failing
- `requires_state_tracking` (bool) -- whether results need comparison across steps
- `requires_retries` (bool) -- whether flaky operations are expected

### IntentAnalysis

The output of classification. Contains the chosen `ExecutionMode`, the `ComplexitySignals`, a confidence score (0.0-1.0), the `AnalysisSource` (Heuristic, LlmClassifier, or MidExecutionEscalation), reasoning text, and a list of `ToolGroup` values that narrow the tool set exposed to the LLM.

### ToolGroup

Controls which tools the LLM sees. Groups include None, TaskManagement (`todo`, `goal`, `plan`), Search (file and web tools), Calendar, Finance, Communication, Automation (`cron`, `spawn`), and Full (all tools). The `ask_user` tool is always included regardless of group selection.

### EngineResult

Returned by every execution engine. Two variants:

- **Complete** -- execution finished with content, usage, iteration count, reasoning traces, and an optional tool name.
- **Escalate** -- the engine could not handle the request. Carries a reason string, an `EscalationContext` (messages + completed work), and accumulated usage. The router uses this to try the next engine in the chain.

### ExecutionEngine trait

The unified async trait that all engines implement:

```rust
async fn execute(
    messages: Vec<Message>,
    tools: &[Value],
    params: &ExecutionParams,
    ctx: &RoutingContext,
    event_tx: Option<Sender<AgentEvent>>,
) -> Result<EngineResult>;

fn mode(&self) -> &str;
```

## How It Works

### Two-Stage Analysis

The `IntentAnalyzer` in `analysis.rs` classifies every incoming message in two stages.

**Stage 1: Heuristic classification (zero-cost).** The `analyze_heuristic()` function runs keyword and pattern matching against the lowercased message. It checks in order:

1. Greetings (`hi`, `hello`, `good morning`, etc.) -- returns Direct at 0.95 confidence.
2. Very short messages (under 20 chars, 4 words or fewer) with no action keywords -- returns Direct at 0.85.
3. Task management phrases (`create a task`, `add a todo`, `new task`, etc.) -- returns Reactive(5) at 0.90. This check runs before code/action keywords so that "create a task: implement auth" is correctly routed as task CRUD rather than code work.
4. Direct questions (`what is`, `explain`, `tell me about`) without conflicting action signals -- returns Direct at 0.90.
5. Explicit plan keywords (`create a plan`, `plan and implement`, `design and build`) -- returns Planned at 0.85.
6. Structural complexity analysis for remaining messages. If sequential language is detected (`first...then`, `after that`, `step 1`), the function defers to the LLM (returns None). Simple tool keywords with low complexity score route to Reactive. Action keywords with moderate complexity route to Reactive. High complexity (score >= 4) routes to Planned.
7. If none of the above matches, returns None to defer to Stage 2.

If heuristic confidence meets or exceeds the `heuristic_confidence_threshold` config value (default 0.9), the LLM classifier is skipped entirely.

**Stage 2: LLM classifier.** The `IntentClassifier` sends the user message to a lightweight LLM call with a structured JSON prompt. The prompt asks for a mode (`direct`/`reactive`/`planned`), complexity signals, relevant tool names, confidence, and reasoning. The response is parsed from JSON (with `extract_json_object` to handle LLM preamble text). If the LLM returns confidence below 0.5, the result is overridden to Reactive(10) as a safe default. On timeout or error, the pipeline falls back to `IntentAnalysis::fallback()` which is Reactive(10) with Full tool access at 0.5 confidence.

The analyzer optionally consults a `StrategyRepo` to build historical strategy performance context (last 30 days of accuracy data), which is appended to the LLM classifier prompt. This context is cached with a 60-second TTL to avoid repeated database queries.

### IntentPipeline.process_message()

The `IntentPipeline` struct in `pipeline.rs` wires the full flow. Its `process_message()` method executes seven steps:

1. **Classify intent** -- calls `IntentAnalyzer::analyze()`, emits a `ClassificationComplete` agent event with the strategy, confidence, source, and duration.
2. **Assemble context** -- builds a `ContextRequest` with the message, history, system prompt, strategy (converted from the execution mode), tool definitions, and context window size. The `ContextEngine` allocates token budgets and returns assembled messages with a token count. Emits a `ContextAssembled` event.
3. **Filter tools** -- if the `IntentAnalysis` specifies tool groups (not Full), filters the tool definitions down to only those matching the allowed names. This narrows the LLM's action space to reduce hallucinated tool calls.
4. **Execute via router** -- passes the mode, assembled messages, filtered tools, execution params, and routing context to the `ExecutionRouter`. The router handles automatic escalation between engines.
5. **Validate response** -- runs the `ResponseValidator` on the output content. Checks for empty responses and generates validation warnings.
6. **Record usage** -- sends token usage to the `CostTracker` (best-effort, logged on failure).
7. **Record strategy outcome** -- writes a `StrategyRecordRow` to the `StrategyRepo` with the predicted vs actual strategy, escalation count, iterations used, success flag, response time, and tool information. This feeds back into the classifier's historical context.

Returns a `PipelineResult` with the final content, mode used, classification, escalation count, and validation result.

### Three Execution Engines

#### DirectEngine

Defined in `engines/direct.rs`. Makes a single LLM call with no tools (passes an empty tool slice to `ExecutionCore::run_cycle`). If the LLM returns a text response, the engine completes. If the LLM returns tool calls despite being given no tools, the engine escalates with an `EscalationContext` carrying the message history. Empty responses complete with empty content.

#### ReactiveEngine

Defined in `engines/reactive.rs`. Runs a ReAct loop up to `max_iterations` cycles. Each iteration calls `ExecutionCore::run_cycle` with tool definitions. Key behaviors:

- **Tool execution tracking**: Successful tool results are recorded as `CompletedStep` entries for potential escalation handoff.
- **Fabrication detection**: If the LLM returns a text response when tools are available (a "fabricated" response suggesting it described what it would do instead of actually calling tools), the engine injects a force-retry prompt once. On the second fabrication, it accepts the response.
- **Duplicate detection**: Tracks tool call signatures via a `HashSet`. If all tool calls in an iteration are duplicates of previous calls, they are blocked and a corrective prompt is injected.
- **Failure reflection**: When tool calls fail, a reflection prompt is injected asking the LLM to analyze what went wrong.
- **Escalation threshold**: When the iteration count reaches 80% of `max_iterations` (calculated as `ceil(max_iterations * 0.8)`), the engine escalates with the accumulated completed work and message history.
- **Scratchpad**: Maintains `ReasoningTrace` entries for each cycle, recording thoughts, planned actions, actual actions, and reflections.

#### PlannedEngine

Defined in `engines/planned.rs`. Decomposes a task into a persisted plan and executes each step sequentially. Two entry points:

- `execute()` / `execute_fresh()` -- generates steps from scratch via `generate_plan_steps()`, builds a Plan in Approved state, transitions to Executing, runs all steps, and synthesizes a summary.
- `execute_with_prior_work()` -- called during escalation from Reactive. Pre-fills completed steps from the escalation context, generates only the remaining steps with LLM awareness of prior work, and executes from the first incomplete step.

Step execution uses `plan_executor::run_step()` for multi-cycle LLM-tool loops per step (up to 5 cycles per step). On step failure, the engine retries up to `max_attempts` (default 3). If retries are exhausted, it backtracks by calling `plan_executor::regenerate_from()` to get replacement steps from the LLM. After `MAX_BACKTRACK_ATTEMPTS` (3) full backtrack events, the plan is marked Failed.

After all steps complete, the engine calls the LLM to synthesize a human-readable summary from the raw step outputs. If plan generation fails entirely (no steps produced), the engine falls back to a ReactiveEngine with 50 max iterations.

Plans are persisted to SQLite via `PlanRepo` at each state change. The `build_plan()` method accepts a visibility override so that user-requested plans (from heuristic classification) use Transparent visibility while auto-generated plans (from escalation) use the engine's configured default.

### ExecutionRouter and Escalation

The `ExecutionRouter` in `router.rs` dispatches to engines and manages the escalation chain. The chain is:

```
Direct -> Reactive -> Planned
```

Execution starts with the mode selected by classification. If an engine returns `EngineResult::Escalate`, the router moves to the next engine in the chain:

- **Direct escalates to Reactive** -- passes the carried messages directly.
- **Reactive escalates to Planned** -- calls `PlannedEngine::execute_with_prior_work()` with the escalation context (messages + completed tool work). If no PlannedEngine is configured, returns an error message.
- **Planned cannot escalate further** -- returns an error message with the failure reason.

The `max_escalations` parameter (configurable via `config.orchestrator.maxEscalations`, default 3) caps the total number of escalation transitions. When exceeded, the router returns an error message indicating the limit was reached.

The `EscalationContext` preserves work across transitions:
- `messages` -- the accumulated message history
- `completed_work` -- a list of `CompletedStep` entries (description, tool name, result) from successful tool calls
- `original_message` -- the user's original request text

### PlanCleanupService

Defined in `visibility.rs`. A background tokio task that runs every hour to delete stale plans based on their visibility:

- **Silent plans**: deleted 24 hours after reaching a terminal state (Completed, Failed, or Abandoned).
- **On_failure plans**: deleted 7 days after successful completion. Failed plans with on_failure visibility are retained for review.
- **Transparent plans**: never auto-deleted.

The service uses a `CancellationToken` for graceful shutdown. It calls `PlanRepo::delete_stale_plans()` with the age thresholds.

## Connections

- **ContextEngine** (`context_engine` crate, Layer 2): Allocates token budgets and assembles the message array. The pipeline converts `ExecutionMode` to `ExecutionStrategy` via a `From` impl.
- **ExecutionCore** (`agent::execution`): Shared by all three engines. Handles the low-level LLM call + tool dispatch cycle. Engines hold an `Arc<ExecutionCore>`.
- **ToolRegistry** (`tools` crate, Layer 3): Provides tool definitions for the LLM and executes tool calls. Accessed via `Arc<RwLock<ToolRegistry>>` in ExecutionCore.
- **StoragePool / Repos** (`storage` crate, Layer 1.5): `StrategyRepo` stores and retrieves classification outcomes for feedback. `PlanRepo` persists plans for the PlannedEngine. `UsageRepo` records token usage via CostTracker.
- **Config** (`config` crate, Layer 1): `OrchestratorConfig` provides `max_escalations`, `heuristic_confidence_threshold`, and `llm_classifier_timeout`.
- **AgentEvent** (`agent::events`): The pipeline emits events (`ClassificationComplete`, `ContextAssembled`, `ExecutionStarted`, `IterationStart`) for observability and dashboard updates.
