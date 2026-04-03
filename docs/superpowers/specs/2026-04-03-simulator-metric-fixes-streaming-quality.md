# Fix: Default chat_stream Tool Call Loss & Agent Response Quality Metric

## Problem

Two simulator metrics report incorrect values after the SimulatedAgentMode implementation:

1. **`tool_selection: 0.000`** — The default `LlmProvider::chat_stream()` fallback (used by all non-streaming providers) drops `response.tool_calls` when converting to a single `LlmStreamChunk`. Since `ExecutionCore` always uses `chat_stream()` when an event channel is provided, the reactive engine never sees tool calls, never executes tools, and never emits `ToolStart` events. This is a **platform-level data-loss bug** — any non-streaming provider returning tool calls is affected.

2. **`agent_response_quality: 0.0`** — Hardcoded placeholder in `MetricCollector::snapshot()`. The infrastructure to score it (embedding engine, reference embeddings, `score_response_quality()`) is already present in the harness run loop.

## Fix 1: Default `chat_stream()` Tool Call Forwarding

**File:** `crates/providers/src/types.rs` (lines 163-185)

**Change:** Convert `response.tool_calls` into `ToolCallDelta` chunks before building the stream. Emit one delta per tool call (complete — `id`, `name`, `arguments` all set), followed by the final content chunk.

**Current code:**
```rust
async fn chat_stream(...) -> Result<LlmStream> {
    let response = self.chat(messages, tools, params).await?;
    let chunk = LlmStreamChunk {
        content: response.content,
        tool_call_delta: None,  // ← BUG: drops tool_calls
        is_final: true,
        ...
    };
    Ok(Box::pin(futures_util::stream::once(async move { Ok(chunk) })))
}
```

**Fixed code:** Build a `Vec<Result<LlmStreamChunk>>` with one chunk per tool call (each containing a `ToolCallDelta`), then a final chunk with content/usage/finish_reason. Use `futures_util::stream::iter` instead of `stream::once`.

**Compatibility:** `call_provider_streaming` in `core.rs:216-234` reconstructs tool calls from deltas by accumulating `id`, `name`, and `arguments` strings per index. A single complete delta per tool call (not incremental) is fully compatible — the reconstruction logic uses `push_str` which works fine with a single complete value.

**Impact:** All non-streaming providers benefit (SimulationProvider, ScriptedProvider, any future test providers). Real streaming providers (OpenAI, Anthropic, etc.) override `chat_stream()` entirely, so they're unaffected.

## Fix 2: Agent Response Quality Scoring

**Files:** `crates/simulator/src/metrics/mod.rs`, `crates/simulator/src/harness.rs`

**Change:** After the agent processes each message, score the agent's actual response (`agent_result.response`) against the expected response (`msg.ground_truth.expected_response`) using the existing `score_response_quality()` function with cached reference embeddings. Accumulate in two new `EpochAccumulator` fields, compute the average in `snapshot()`.

**Pattern:** Identical to the existing heuristic-path `response_quality` measurement (harness.rs lines 666-691), but scoring the agent's real response instead of the user's message.

## Verification

After both fixes:
- `tool_selection` should be > 0.0 (SimulationProvider topic-keyed tool calls match expected tools)
- `agent_response_quality` should be > 0.0 (embedding similarity of agent responses to reference answers)
- `react_convergence_rate` may drop below 1.0 (real tool execution can fail)
- `avg_react_iterations` should be > 1.0 (multiple iterations: tool call → execution → synthesis)
- All existing tests must pass unchanged
- The `agent_breakpoint_threshold` may need adjustment based on new metric values

## Scope

- Two files modified for Fix 1 (providers/types.rs, simulation_provider.rs cleanup)
- Two files modified for Fix 2 (metrics/mod.rs, harness.rs)
- No new files, no architectural changes
- Run full simulation suite to capture new metrics
