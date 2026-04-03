# SimulatedAgentMode — Design Spec

## Purpose

A full end-to-end integration test mode for the simulator that runs messages through the real `AgentRuntime` pipeline — intent classification, skill routing, tool selection, ReAct execution, and response generation — against real tools and a real database. Serves as a pre-release validation gate that finds breakpoints across the entire stack, from user message to DB mutation.

## Architecture

**Dual-path execution:** Every simulated message goes through both paths:

- **Heuristic path** (existing) — `ActionExecutor` + cognitive pipeline. Preserves all 19+ existing metrics for regression detection. Continues to write to the shared DB.
- **Agent path** (new) — `AgentRuntime.process_message()` with `SimulationProvider` returning topic-keyed tool-call JSON. Real tools from production registration pattern. Both paths write to the same shared in-memory SQLite DB.

The agent path is the PRIMARY path — it proves the system works end-to-end. The heuristic path remains for backward-compatible metrics. Both paths write to the same DB; duplicates are accepted because the point is to verify real tool execution like a staging environment.

**Provider strategy:** A new `SimulationProvider` wraps keyword-to-tool-call logic. For each topic, it returns structured tool-call JSON that drives the ReAct loop through real tool execution. For topics without tool mappings (chat, coaching introspection), it returns plain text, falling through to Direct mode.

## Components

### 1. SimulationProvider

A new `LlmProvider` implementation at `crates/simulator/src/providers/simulation_provider.rs`.

- Returns topic-appropriate tool-call JSON based on keywords in the user message.
- For messages that don't match any keyword, returns a plain text response (Direct mode).
- Returns realistic `Usage` stats for cost tracking.
- Deterministic — seeded RNG for response selection when multiple actions are possible.
- Implements the full `LlmProvider` trait (chat, chat_stream, count_tokens, etc.).

### 2. AgentHarness

A new struct at `crates/simulator/src/agent_harness.rs` that owns the `AgentRuntime` and its dependencies.

**Holds:**
- `Arc<AgentRuntime>`
- `Arc<RwLock<ToolRegistry>>` (with 12 real domain tools registered)
- Reference to shared `sqlx::SqlitePool`

**Constructed from** existing `SimulationHarness` fields:
- `inner_pool` — shared DB for tool execution
- `bus` — DomainEventBus
- `context_queue` — ContextUpdateQueue
- `skill_catalog` / `skill_router` — promoted to `Arc<RwLock<>>` wrappers
- `embedding_engine` — for semantic routing

**Single method:**
```rust
pub async fn process(&self, msg: &AnnotatedMessage) -> AgentResult
```
Calls `AgentRuntime::process_message()` and captures metrics + breakpoints.

### 3. AgentResult

Return type from each agent-path message processing:

```rust
pub struct AgentResult {
    pub selected_skill: String,
    pub mode_used: String,        // "direct" or "reactive"
    pub tool_calls: Vec<String>,  // tool names called during ReAct
    pub iterations: u32,          // ReAct loop iterations
    pub response: String,         // final agent response text
    pub error: Option<String>,    // pipeline failure message
    pub breakpoints: Vec<AgentBreakpoint>,
}
```

### 4. AgentBreakpoint

Structured failure record:

```rust
pub struct AgentBreakpoint {
    pub kind: BreakpointKind,
    pub message_content: String,
    pub details: String,
    pub day: u32,
    pub phase: String,
}

pub enum BreakpointKind {
    RoutingMismatch,
    ToolExecutionFailed,
    ToolSelectionWrong,
    LoopTimeout,
    FabricationDetected,
    ClassificationLowConfidence,
    ResponseEmpty,
}
```

## Tool Registration

The `AgentHarness` constructs a `ToolRegistry` mirroring the production registration in `agent_loop/builder.rs`. 12 domain tools:

| Tool | Crate | Registration |
|---|---|---|
| `tasks` (TaskTool) | feature-tasks | Direct wiring with repos |
| `okr` (OkrTool) | tools | Direct with objective/key_result repos |
| `area` (AreaTool) | tools | Direct with area repo |
| `project` (ProjectTool) | tools | Direct with project/task repos |
| `finance` | feature-finance | FinanceFeature::tools() |
| `notes` | feature-notes | NotesFeature::tools() |
| `productivity` | feature-productivity | ProductivityFeature::tools() |
| `memory` | tools | Direct with cognitive repos |
| `annotate` | tools | Direct with annotation repo |
| `work_context` | activity-log | Direct with storage pool |
| `learning` (LearningTool) | feature-learning | Direct wiring |
| `cron` (CronTool) | tools | Via adapter |

All tools execute against the shared in-memory SQLite pool — same DB as the heuristic path.

## SimulationProvider Tool-Call Mappings

| Message topic | Tool | Actions |
|---|---|---|
| `tasks` (create) | `tasks` | `{"action": "create", "title": "...", "project": "main"}` |
| `tasks` (list/status) | `tasks` | `{"action": "list"}` |
| `tasks` (complete) | `tasks` | `{"action": "complete", "task_ref": "..."}` |
| `finance` | `finance` | `{"action": "record", "amount": N, "category": "..."}` |
| `notes` (create) | `notes` | `{"action": "create", "title": "...", "content": "..."}` |
| `notes` (query) | `notes` | `{"action": "search", "query": "..."}` |
| `productivity` | `productivity` | `{"action": "start_focus", "duration_mins": 25}` |
| `learning` | `learning` | `{"action": "create_flashcard", "front": "...", "back": "..."}` |
| `automation` | `cron` | `{"action": "create", "expression": "...", "command": "..."}` |
| `coaching` | `memory` | `{"action": "search", "query": "..."}` |
| `insights` | `work_context` | `{"action": "query"}` |
| `chat` | *(none — plain text)* | Falls through to Direct mode |

The provider inspects keywords in the user message to decide the action variant (e.g., "Mark X as done" -> `tasks.complete`, "Create a task" -> `tasks.create`).

## New Metrics (5)

Added to `MetricSnapshot` alongside existing metrics:

| Metric | Formula | What it measures |
|---|---|---|
| `agent_routing_accuracy` | correct_skill / total_agent_calls | Did AgentRuntime pick the right orchestrator? |
| `agent_tool_selection` | correct_tool / total_tool_calls | Did the ReAct loop call the expected tool? |
| `agent_mode_distribution` | reactive_count / total_agent_calls | Fraction of messages using Reactive (vs Direct) mode |
| `react_convergence_rate` | converged / total_reactive | Fraction of Reactive executions that terminated normally (not timeout/fabrication) |
| `agent_response_quality` | embedding similarity of real agent response vs reference | Measures actual AI output quality (replaces the proxy metric) |

## Breakpoint Detection & Report

**BreakpointKind triggers:**

| Kind | Triggered when |
|---|---|
| `RoutingMismatch` | AgentRuntime picks skill X but ground truth says Y |
| `ToolExecutionFailed` | A real tool returns an error |
| `ToolSelectionWrong` | Agent calls tool X but topic maps to tool Y |
| `LoopTimeout` | ReAct loop hits max_iterations without converging |
| `FabricationDetected` | Agent returns prose instead of tool call in Reactive mode |
| `ClassificationLowConfidence` | IntentAnalyzer confidence below threshold |
| `ResponseEmpty` | Agent returns no content |

**Report integration:** `SimulationReport` gets a new `agent_summary: AgentSummary` field:

```rust
pub struct AgentSummary {
    pub total_agent_calls: u32,
    pub successful: u32,
    pub breakpoints: Vec<AgentBreakpoint>,
    pub breakpoints_by_kind: HashMap<String, u32>,
    pub agent_routing_accuracy: f64,
    pub agent_tool_selection: f64,
    pub react_convergence_rate: f64,
    pub avg_react_iterations: f64,
    pub mode_distribution: HashMap<String, u32>,
}
```

**CI gate:** `SimulationReport.passed()` additionally checks that the breakpoint rate is below a configurable threshold (default: 20% in `SimulationConfig`). CI fails if more than 1 in 5 messages produce agent-path errors.

## Harness Integration

In `SimulationHarness::run()`, the message processing loop adds an agent-path call after the existing heuristic-path processing:

```
for msg in messages:
    // EXISTING: heuristic path (ActionExecutor + cognitive pipeline + metrics)
    ...existing code unchanged...

    // NEW: agent path
    if let Some(ref agent) = self.agent_harness {
        let result = agent.process(msg).await;
        // Record agent metrics on accumulator
        // Collect breakpoints
    }
```

The `AgentHarness` is constructed in `SimulationHarness::new()` after all existing setup, using the already-available pool, bus, skills, and embedding engine.

## Configuration

New fields in `SimulationConfig` (scenario TOML):

```toml
[simulation]
agent_mode = true                    # Enable agent-path execution (default: false)
agent_breakpoint_threshold = 0.20    # Max breakpoint rate before CI failure
agent_max_iterations = 15            # ReAct loop iteration limit
```

When `agent_mode = false` (default), the agent path is skipped entirely — existing scenarios run unchanged with zero overhead.

## File Structure

```
crates/simulator/src/
  providers/
    simulation_provider.rs   (NEW — topic-keyed LlmProvider)
  agent_harness.rs           (NEW — AgentRuntime wrapper + tool registration)
  agent_types.rs             (NEW — AgentResult, AgentBreakpoint, BreakpointKind, AgentSummary)
  harness.rs                 (MODIFY — construct AgentHarness, call per message)
  metrics/mod.rs             (MODIFY — 5 new agent metric fields)
  scenario.rs                (MODIFY — agent_mode config, new MetricName variants)
  report.rs                  (MODIFY — AgentSummary in report, passed() gate)
  metrics/ground_truth.rs    (MODIFY — metric value mappings)
  lib.rs                     (MODIFY — export new modules)

crates/simulator/Cargo.toml  (MODIFY — add agent dependency)

tests/simulation/
  scenarios/
    software_engineer_12mo.toml  (MODIFY — add agent_mode = true)
  smoke.rs                       (MODIFY — agent metric assertions)
```

## Dependencies

The simulator crate needs new dependencies:
- `agent` — for `AgentRuntime`, `IntentAnalyzer`, `ExecutionRouter`, etc.
- `feature-learning` — for `LearningTool` registration
- `feature-coaching` — for coaching signal handling (if tools exist)
- `activity-log` — for `WorkContextTool`

Already available: `tools`, `cognitive`, `bus`, `storage`, `config`, `skill-system`, `providers`, `feature-tasks`, `feature-notes`, `feature-finance`, `feature-productivity`.
