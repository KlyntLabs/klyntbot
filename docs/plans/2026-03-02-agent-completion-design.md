# Agent Loop & Intent Pipeline 100% Completion Design

**Date:** 2026-03-02
**Approach:** Refactor + Fix (grouped by subsystem)
**Scope:** 13 gaps across agent_loop, intent_pipeline, and broader agent crate

## Problem Statement

The agent loop and intent pipeline are ~75-85% feature-complete. The core execution path (receive → classify → context → execute → validate → respond) works, but 13 gaps exist where data is computed then discarded, features are built but never wired, or configuration doesn't flow from analysis to execution.

## Gap Inventory

| # | Gap | Severity | Location |
|---|-----|----------|----------|
| 1 | `cancel_token` created but never observed by spawned task | High | `mod.rs:L556` |
| 2 | Streaming calls record `Usage::default()` (zero tokens) | High | `core.rs:L148-L230` |
| 3 | `max_iterations` from classifier ignored by router/engine | High | `router.rs:L100` |
| 4 | `ConfidenceEvaluator` built but never injected into pipeline | High | `builder.rs:L564` |
| 5 | `complexity_signals` always `Value::Null` in strategy records | Medium | `pipeline.rs:L309` |
| 6 | `EscalationContext.original_message` always empty | Medium | `direct.rs:L56`, `reactive.rs:L216` |
| 7 | `PipelineConfig.channel` fixed to "unknown" | Medium | `builder.rs:L704` |
| 8 | Reactive exhaustion returns raw error string to user | Medium | `router.rs:L179-L196` |
| 9 | `user_satisfaction` always None on strategy records | Medium | `pipeline.rs:L303` |
| 10 | Oversized messages silently dropped | Low | `mod.rs:L216` |
| 11 | Reaction satisfaction window hardcoded to 5 minutes | Low | `mod.rs:L87` |
| 12 | `PageContextSource`/`PersonaContextSource` exported but unused | Low | `builder.rs:L161` |
| 13 | `SubagentHandle.profile` stored but never read | Low | `subagent.rs:L78` |

Additional improvements:
- Fabrication retry budget hardcoded to 1
- ToolGroup name matching brittle (exact string match)
- Scratchpad traces computed but discarded above ReactiveEngine

## Subsystem 1: Execution Control

**Gaps:** #1 (cancel_token), #3 (max_iterations), fabrication retry

### Design

Introduce `ExecutionParams` — a per-request config bundle passed from router to engines:

```rust
pub struct ExecutionParams {
    pub max_iterations: usize,
    pub max_fabrication_retries: usize,
    pub cancel_token: Option<CancellationToken>,
    pub original_message: String,
}
```

- **Router propagates:** `ExecutionRouter::execute()` constructs `ExecutionParams` from `ExecutionMode` + optional cancel token from `RoutingContext`
- **ReactiveEngine reads from params:** Uses `params.max_iterations` instead of `self.max_iterations`. Same for fabrication retry budget.
- **Cancel token observed:** Reactive loop checks `cancel_token.is_cancelled()` at top of each iteration. Streaming path passes token into `RoutingContext`.
- **Config:** Add `maxFabricationRetries` (default: 2) to `OrchestratorConfig`

### Files Modified
- `intent_pipeline/types.rs` — add `ExecutionParams` struct
- `intent_pipeline/router.rs` — construct params, pass to engines
- `intent_pipeline/engines/reactive.rs` — read from params instead of self
- `intent_pipeline/engines/direct.rs` — receive params
- `intent_pipeline/pipeline.rs` — pass original message through
- `agent_loop/mod.rs` — pass cancel_token into RoutingContext for streaming
- `config` crate — add `maxFabricationRetries` to OrchestratorConfig

## Subsystem 2: Observability & Analytics

**Gaps:** #2 (streaming costs), #5 (complexity_signals), #7 (channel), #9 (user_satisfaction), #11 (reaction window), traces

### Design

**Streaming token estimation:** Add `TokenEstimator` utility — counts accumulated streamed text bytes, applies model-specific chars-per-token ratio (configurable, default ~4). Called in `call_provider_streaming()` after stream completes.

**Persist complexity_signals:** Serialize `analysis.signals` in pipeline.rs:
```rust
complexity_signals: serde_json::to_value(&analysis.signals).unwrap_or_default(),
```

**Wire user_satisfaction:** Connect `handle_reaction()` → update most recent `StrategyRecordRow` for that chat. Extend `StrategyRepo` to update `user_satisfaction` on matching record. Make window configurable: `OrchestratorConfig::satisfaction_window_minutes` (default: 15).

**Per-request channel:** Move `channel` from `PipelineConfig` (build-time) to `RoutingContext` (per-request). `CostTracker::record()` reads from `RoutingContext.channel_name`.

**Persist traces:** Add `traces: Vec<String>` to `RouterResult`. Pipeline persists to strategy record as `execution_traces` JSON.

### Files Modified
- `execution/core.rs` — add TokenEstimator, use in call_provider_streaming
- `intent_pipeline/pipeline.rs` — serialize complexity_signals, read channel from RoutingContext, persist traces
- `intent_pipeline/router.rs` — carry traces from EngineResult to RouterResult
- `agent_loop/mod.rs` — configurable reaction window, update strategy record on reaction
- `storage` crate — add execution_traces column to strategy migration, extend StrategyRepo
- `config` crate — add satisfactionWindowMinutes, tokenEstimation config

## Subsystem 3: Escalation & Graceful Degradation

**Gaps:** #6 (original_message), #8 (raw error on exhaustion), #10 (silent drop)

### Design

**Populate original_message:** Pipeline passes `message: &str` through to router. Router stores it in `ExecutionParams.original_message` (from Subsystem 1). Engines inject into `EscalationContext`.

**Graceful degradation:** When Reactive exhaustion triggers and there's nowhere to escalate:
- Assemble partial-result summary from `EscalationContext.completed_work`
- Return user-friendly message with what was accomplished
- Add `PartialResult` variant to `EngineResult`

**Oversized message feedback:** Replace silent drop with outbound error message including length and max.

### Files Modified
- `intent_pipeline/engines/direct.rs` — populate original_message from params
- `intent_pipeline/engines/reactive.rs` — populate original_message, assemble partial results
- `intent_pipeline/router.rs` — handle PartialResult, format graceful degradation
- `intent_pipeline/types.rs` — add PartialResult to EngineResult
- `agent_loop/mod.rs` — send error reply on oversized messages

## Subsystem 4: Intelligence & Adaptive Learning

**Gaps:** #4 (ConfidenceEvaluator), #12 (context sources), #13 (SubagentHandle.profile), ToolGroup matching

### Design

**Wire ConfidenceEvaluator:** Add `confidence_evaluator: Option<Arc<ConfidenceEvaluator>>` to `IntentPipeline`. After analysis, if confidence < threshold, call `evaluator.decide()` → may trigger `ask_user` for clarification → re-analyze → route.

**Register context sources:** Add `PersonaContextSource` and `PageContextSource` to builder sources vector. Gate behind config flags (`context.persona.enabled`, `context.page.enabled`).

**Fix SubagentHandle.profile:** Remove `#[allow(dead_code)]`. Use profile in `list_active()` for status output.

**Robust ToolGroup matching:** Each tool declares group membership via `tool_group: Option<ToolGroup>` on tool definition (set via derive macro attribute). Pipeline filters by `tool.group == allowed_group` instead of name substring matching.

### Files Modified
- `intent_pipeline/pipeline.rs` — add confidence_evaluator field, check after analysis
- `agent_loop/builder.rs` — inject evaluator into pipeline, register context sources
- `subagent.rs` — remove allow(dead_code), use profile in list_active
- `tools_core` crate — add tool_group field to ToolDefinition
- `tools_core_macros` crate — support #[tool_group = "..."] attribute
- `intent_pipeline/types.rs` — update ToolGroup filtering to use definition field

## Estimated Completeness After All Fixes

| Component | Before | After |
|-----------|--------|-------|
| Agent Loop | 75% | 98% |
| Intent Pipeline | 85% | 98% |
| Broader Agent Crate | 80% | 95% |
| **Overall** | **~80%** | **~97%** |

Remaining ~3% would be: edge cases discovered during implementation, additional test coverage, and potential new requirements from wiring ConfidenceEvaluator.
