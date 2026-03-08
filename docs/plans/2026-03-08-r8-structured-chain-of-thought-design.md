# R8: Structured Chain-of-Thought for Complex Tasks

> Approved: 2026-03-08 | Approach: B (Plan-Aware Engine)

## Problem

The agent has no explicit planning step. For complex tasks (complexity score >= 5), the ReAct loop jumps straight into tool execution without articulating a strategy. This leads to lower success rates on multi-step tasks with sequential dependencies.

## Solution

Add a structured planning phase inside the ReactiveEngine for complex tasks. The plan is structured data tracked in the Scratchpad, visible in chat, and advisory (the LLM is free to deviate).

## Trigger

- `analysis.signals.complexity_score() >= 5` (constant: `COT_COMPLEXITY_THRESHOLD`)
- Only when `ExecutionMode::Reactive`
- Gate checked in `runtime.rs` Step 7c, between tool filtering and router execution

## Data Structures

### PlanStep (new, in scratchpad.rs)

```rust
pub struct PlanStep {
    pub index: usize,
    pub description: String,
    pub expected_tool: Option<String>,
    pub completed: bool,
}
```

### ExecutionPlan (new, in scratchpad.rs)

```rust
pub struct ExecutionPlan {
    pub steps: Vec<PlanStep>,
    pub raw_text: String,
}
```

### ReasoningTrace (extended)

New field: `plan_step_index: Option<usize>` — links a trace to its corresponding plan step.

### Scratchpad (extended)

New field: `plan: Option<ExecutionPlan>`
New methods: `set_plan()`, `plan()`, `mark_step_completed(tool_name)`, `plan_progress() -> Option<(usize, usize)>`

### ExecutionParams (extended)

New field: `planning_prompt: Option<String>`
New builder: `with_planning_prompt(prompt)`

## Planning Prompt

Built in `runtime.rs` via `build_planning_prompt(message, tools)`:

```
This is a complex request. Before executing, create a step-by-step plan.

User request: {user_message}
Available tools: [{tool_names}]

Format each step as:
1. <description> [tool: <tool_name>]
2. <description> [tool: <tool_name>]
...

Keep the plan concise (3-7 steps). Then execute step 1.
```

The "Then execute step 1" instruction avoids wasting an iteration on plan-only output.

## ReactiveEngine Flow

1. If `params.planning_prompt` is Some, inject as `Message::user` before iteration 1
2. Iteration 1: LLM produces plan text + optionally starts executing. `parse_plan()` extracts steps.
3. Iterations 2..N: Normal ReAct loop. After each `ToolsExecuted`, call `scratchpad.mark_step_completed(tool_name)` and emit `PlanStepCompleted` event.
4. Synthesis (max iterations): Include plan progress in the synthesis prompt ("Plan progress: 3/5 steps completed. Remaining: ...").

## Plan Parsing

Lenient regex matching `^\d+\.\s+(?P<desc>.+?)(?:\[tool:\s*(?P<tool>\w+)\])?$`. Unmatched lines are ignored. Empty plan = no tracking (graceful degradation).

## Chat Integration

- Plan text included in the assistant's response content (visible in chat history)
- `AgentEvent::PlanGenerated { steps, complexity_score, raw_plan }` for real-time UI
- `AgentEvent::PlanStepCompleted { step_index, description, tool_name }` as steps finish
- `AgentEvent::PlanningStarted { complexity_score }` when planning begins
- Plan stored in `session_messages` via normal response flow

## Key Principle

The plan is **advisory, not prescriptive**. The engine tracks completion but never blocks or redirects the LLM. Deviation is logged, not prevented. The LLM often discovers better approaches during execution.

## Files Changed

| File | Change |
|------|--------|
| `crates/agent/src/execution/scratchpad.rs` | `PlanStep`, `ExecutionPlan`, extend `ReasoningTrace`, plan methods on `Scratchpad` |
| `crates/agent/src/execution/types.rs` | `planning_prompt` field on `ExecutionParams` |
| `crates/agent/src/intent_pipeline/engines/reactive.rs` | Planning injection, `parse_plan()`, plan-step tracking, enhanced synthesis |
| `crates/agent/src/agent_runtime/runtime.rs` | Step 7c: complexity gate, `build_planning_prompt()`, wire into params |
| `crates/agent/src/events.rs` | Three new `AgentEvent` variants |

~235 lines across 5 files. No new crates or dependencies.

## Future Extensions (not in scope)

- Use `classifier_provider` for cheaper planning calls
- Re-planning on tool failure ("revise your plan given this failure")
- Plan quality scoring fed into learning system
- Plan templates for common task patterns
