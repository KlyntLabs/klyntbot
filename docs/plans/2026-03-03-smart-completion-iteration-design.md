# Smart Completion: Agent Iteration System Redesign

**Date:** 2026-03-03
**Status:** Approved

## Problem

The agent response stops prematurely after executing tool calls. Root causes:

1. **Low iteration budget**: Heuristic assigns `max_iterations=5` for task CRUD. Escalation threshold at `ceil(5*0.8)=4` triggers after 4 iterations. Router has nowhere to escalate from reactive mode and returns a generic incomplete message.
2. **Content dropped**: When LLM returns text AND tool calls, `run_cycle` discards the text content. `assistant_with_tools` sets `content: None`, losing the LLM's reasoning across iterations.
3. **Duplicate React keys**: `SegmentedMessage.tsx` generates keys as `tool-${name}-${durationMs}`. Two tools with same name+duration collide.

## Design

### Iteration Budget Formula

Replace hardcoded tiers with dynamic computation:

```
max_iterations = min(max(estimated_tool_calls * 3, 10) + 5, 30)
```

- `estimated_tool_calls * 3`: headroom per tool (call + reflection + planning)
- Floor of 10: even simple requests get enough room
- `+ 5` buffer: synthesis and unexpected detours
- Ceiling of 30: safety net against bad estimates

### Remove Escalation Mechanism

The escalation chain (Direct -> Reactive -> give up) was designed for multi-tenant cost control. For a personal agent, it causes premature termination without benefit.

Changes:
- Remove `EngineResult::Escalate` variant
- Remove `EscalationContext`, `CompletedStep`, `incomplete_result()` from router
- Simplify router to direct dispatch (no escalation loop)
- ReactiveEngine loop exits only on: `FinalResponse`, fabrication retries exhausted, cancellation, or `max_iterations`

### Graceful Synthesis at Max-Iterations

When `max_iterations` reached without `FinalResponse`:

1. Inject synthesis prompt: "You've used all available iterations. Based on the work completed so far, provide a complete response to the user's original request."
2. One final LLM call with no tools (forces text response)
3. Return as `FinalResponse`

### Preserve Content Alongside Tool Calls

- `run_cycle` (`core.rs`): when response has both `content` and `tool_calls`, preserve content in the assistant message and stream it to UI
- `Message::assistant_with_tools` (`types.rs`): accept `Option<String>` content parameter
- Content is included in conversation history so the LLM maintains coherence

### Heuristic Simplification

- Remove hardcoded `max_iterations` from `reactive_analysis()` calls
- Add `compute_iteration_budget(signals: &ComplexitySignals) -> u32` function
- All reactive paths delegate to this function
- `is_task_management` contributes to `estimated_tool_calls` via `count_tool_indicators` instead of hardcoding `1`

### Fix React Key Collision

Change key from `tool-${seg.name}-${seg.durationMs}` to `tool-${i}-${seg.name}`. Index guarantees uniqueness; name adds semantic stability.

## Files Changed

| File | Change |
|------|--------|
| `crates/agent/src/intent_pipeline/analysis.rs` | Replace hardcoded iterations with `compute_iteration_budget()` |
| `crates/agent/src/intent_pipeline/types.rs` | Keep `ExecutionMode::Reactive { max_iterations }` but compute dynamically |
| `crates/agent/src/intent_pipeline/engines/reactive.rs` | Remove escalation threshold, add synthesis at max |
| `crates/agent/src/intent_pipeline/engines/mod.rs` | Remove `EngineResult::Escalate` |
| `crates/agent/src/intent_pipeline/router.rs` | Remove escalation chain, simplify dispatch |
| `crates/agent/src/execution/core.rs` | Preserve content when tool calls present |
| `crates/providers/src/types.rs` | `assistant_with_tools` accepts optional content |
| `desktop-ui/src/components/chat/SegmentedMessage.tsx` | Fix key collision |

## Testing

- Existing tests updated to remove escalation assertions
- New test: `reactive_synthesizes_at_max_iterations` — verifies synthesis prompt injected
- New test: `compute_iteration_budget_examples` — verifies formula
- New test: `content_preserved_with_tool_calls` — verifies content not dropped
- Existing `reactive_escalates_on_complexity` test updated or removed
