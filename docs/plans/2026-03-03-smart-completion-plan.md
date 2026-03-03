# Smart Completion Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix premature agent response termination by removing escalation, using dynamic iteration budgets, preserving LLM content alongside tool calls, and fixing React key collisions.

**Architecture:** Remove the escalation chain from the execution router. Replace hardcoded iteration tiers with a formula-based budget (`max(estimated_tool_calls * 3, 10) + 5`, capped at 30). Add graceful synthesis when max iterations are reached. Fix `assistant_with_tools` to preserve text content.

**Tech Stack:** Rust (agent/providers crates), TypeScript/React (desktop-ui)

---

### Task 1: Add `compute_iteration_budget` and update heuristic analysis

**Files:**
- Modify: `crates/agent/src/intent_pipeline/analysis.rs`

**Step 1: Write the failing tests**

Add these tests to the existing `mod tests` block at the bottom of `analysis.rs`:

```rust
#[test]
fn compute_budget_single_tool_call() {
    let signals = ComplexitySignals {
        estimated_tool_calls: 1,
        has_sequential_deps: false,
        failure_risk: FailureRisk::Low,
        requires_state_tracking: false,
        requires_retries: false,
    };
    assert_eq!(compute_iteration_budget(&signals), 15);
}

#[test]
fn compute_budget_many_tool_calls() {
    let signals = ComplexitySignals {
        estimated_tool_calls: 8,
        has_sequential_deps: false,
        failure_risk: FailureRisk::Low,
        requires_state_tracking: false,
        requires_retries: false,
    };
    assert_eq!(compute_iteration_budget(&signals), 29);
}

#[test]
fn compute_budget_capped_at_30() {
    let signals = ComplexitySignals {
        estimated_tool_calls: 20,
        has_sequential_deps: false,
        failure_risk: FailureRisk::Low,
        requires_state_tracking: false,
        requires_retries: false,
    };
    assert_eq!(compute_iteration_budget(&signals), 30);
}

#[test]
fn compute_budget_zero_tool_calls_gets_floor() {
    let signals = ComplexitySignals {
        estimated_tool_calls: 0,
        has_sequential_deps: false,
        failure_risk: FailureRisk::Low,
        requires_state_tracking: false,
        requires_retries: false,
    };
    assert_eq!(compute_iteration_budget(&signals), 15);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(compute_budget)'`
Expected: FAIL — `compute_iteration_budget` function doesn't exist yet.

**Step 3: Implement `compute_iteration_budget`**

Add this public function above the `direct_analysis` function (around line 352):

```rust
/// Compute iteration budget from complexity signals.
///
/// Formula: min(max(estimated_tool_calls * 3, 10) + 5, 30)
/// - Multiplier 3: headroom per tool (call + reflection + planning)
/// - Floor 10: even simple requests get enough room
/// - Buffer +5: synthesis and unexpected detours
/// - Ceiling 30: safety net against bad estimates
pub fn compute_iteration_budget(signals: &ComplexitySignals) -> u32 {
    let base = (signals.estimated_tool_calls as u32 * 3).max(10);
    (base + 5).min(30)
}
```

**Step 4: Update all `reactive_analysis` call sites to use the formula**

Replace the hardcoded `max_iterations` values in `analyze_heuristic`:

1. Task management (line 56-68): Change `reactive_analysis(5, ...)` to `reactive_analysis(compute_iteration_budget(&signals), ...)` — but first you need to compute signals before calling `reactive_analysis`. Restructure to:

```rust
// 3. Task management patterns
if is_task_management(&msg) {
    let signals = ComplexitySignals {
        estimated_tool_calls: count_tool_indicators(&msg).max(1),
        has_sequential_deps: false,
        failure_risk: FailureRisk::Low,
        requires_state_tracking: false,
        requires_retries: false,
    };
    let budget = compute_iteration_budget(&signals);
    return Some(reactive_analysis(
        budget,
        "Task management CRUD operation",
        0.90,
        signals,
        vec![ToolGroup::TaskManagement, ToolGroup::Search],
    ));
}
```

2. Complex workflow (line 87-93): Change `reactive_analysis(20, ...)` to use `compute_iteration_budget(&signals)`.

3. Simple tool-assisted (line 108-114): Change `reactive_analysis(5, ...)` to `compute_iteration_budget(&signals)`.

4. Code/action keyword (line 119-125): Change `reactive_analysis(10, ...)` to `compute_iteration_budget(&signals)`.

5. High complexity (line 130-136): Change `reactive_analysis(20, ...)` to `compute_iteration_budget(&signals)`.

**Step 5: Update existing test expectations**

Update `task_mgmt_overrides_code_keywords` test (line 891-899): Change the assertion from `max_iterations: 5` to just `ExecutionMode::Reactive { .. }` (don't hardcode the exact number, since it depends on the message content).

**Step 6: Run all analysis tests**

Run: `cargo nextest run -p agent -E 'test(/analysis/)' --nocapture`
Expected: All PASS.

**Step 7: Commit**

```bash
git add crates/agent/src/intent_pipeline/analysis.rs
git commit -m "feat(agent): dynamic iteration budget from complexity signals"
```

---

### Task 2: Remove `EngineResult::Escalate` and simplify engines

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/mod.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/direct.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs`
- Modify: `crates/agent/src/subagent.rs`

**Step 1: Remove `Escalate` from `EngineResult` in `engines/mod.rs`**

Remove the import of `EscalationContext` (line 10) and the `Escalate` variant (lines 29-33). Remove the `Escalate` arm from the `Debug` impl (lines 71-73).

The file should become:

```rust
use async_trait::async_trait;
use providers::Usage;
use tools::RoutingContext;

use crate::execution::{ExecutionParams, ReasoningTrace};

pub mod direct;
pub mod reactive;
#[cfg(test)]
pub(crate) mod test_utils;

/// Result from an execution engine.
pub enum EngineResult {
    /// Execution completed successfully.
    Complete {
        content: String,
        usage: Usage,
        iterations: u32,
        traces: Vec<ReasoningTrace>,
        tool_name: Option<String>,
    },
}

// keep existing `complete()`, `empty()`, Debug impl — just remove Escalate arm
```

**Step 2: Update DirectEngine to re-execute with tools instead of escalating**

In `direct.rs`, when the LLM returns tool calls in Direct mode, instead of escalating, return a complete result with an explanation. Remove the `EscalationContext` import.

Replace the `ToolsExecuted` match arm (lines 49-59):

```rust
CycleOutcome::ToolsExecuted { .. } => {
    // LLM wanted tools despite Direct mode classification.
    // Return empty — the pipeline should have classified as Reactive.
    Ok(EngineResult::empty(usage, 1))
}
```

Remove `use crate::intent_pipeline::router::EscalationContext;` from the imports (line 15).

**Step 3: Update DirectEngine tests**

Update `escalates_when_tool_calls_present` test to expect `Complete` with empty content instead of `Escalate`:

```rust
#[tokio::test]
async fn returns_empty_when_tool_calls_in_direct_mode() {
    let engine = make_engine(MockSequenceProvider::with_tool_call("web_search"));

    let result = engine
        .execute(
            vec![Message::user("search for Rust docs")],
            &[],
            &default_params(),
            &routing_ctx(),
            None,
        )
        .await
        .unwrap();

    match result {
        EngineResult::Complete { content, .. } => {
            assert!(content.is_empty());
        }
    }
}
```

Remove the other test assertions that check for `EngineResult::Escalate` in `direct.rs` tests.

**Step 4: Remove escalation from ReactiveEngine**

In `reactive.rs`:
- Remove `use crate::intent_pipeline::router::{CompletedStep, EscalationContext};` (line 23)
- Remove `let escalation_threshold = ...` (line 58)
- Remove `let mut completed_work: Vec<CompletedStep> = Vec::new();` (line 64)
- Remove the `completed_work` tracking block inside `ToolsExecuted` (lines 168-176)
- Remove the escalation threshold check block (lines 226-242)

**Step 5: Add graceful synthesis at max-iterations in ReactiveEngine**

Replace the max-iterations fallback at the end of the loop (lines 258-265). Instead of returning empty content, inject a synthesis prompt and make one final LLM call:

```rust
// Max iterations reached — synthesize a response from completed work
debug!(
    "ReactiveEngine: max iterations ({}) reached, synthesizing final response",
    max_iterations
);

messages.push(Message::user(
    "You've used all available iterations. Based on the work completed so far, \
     provide a complete response to the user's original request. \
     Summarize what you accomplished and any remaining steps."
));

// One final LLM call with no tools — forces text response
let (synthesis_outcome, synthesis_usage) = self
    .core
    .run_cycle(&mut messages, &[], params, ctx, event_tx.as_ref(), None)
    .await?;
accumulate_usage(&mut accumulated_usage, &synthesis_usage);

let synthesis_content = match synthesis_outcome {
    CycleOutcome::FinalResponse { content } => content,
    CycleOutcome::FabricatedResponse { content } => content,
    _ => String::new(),
};

Ok(EngineResult::Complete {
    content: synthesis_content,
    usage: accumulated_usage,
    iterations: max_iterations,
    traces: scratchpad.traces().to_vec(),
    tool_name: last_tool_name,
})
```

**Step 6: Update ReactiveEngine tests**

Update `reactive_escalates_on_complexity` test — rename and change to expect synthesis:

```rust
#[tokio::test]
async fn reactive_synthesizes_at_max_iterations() {
    // With max_iterations=3, the engine should synthesize after 3 tool iterations
    let responses: Vec<_> = (0..5)
        .map(|_| tool_call_response("ok_tool"))
        .chain(std::iter::once(text_response("Here's what I did...")))
        .collect();
    let provider = MockSequenceProvider::new(responses);
    let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
    let engine = ReactiveEngine::new(core, 3);

    let result = engine
        .execute(
            vec![Message::user("complex task")],
            &[],
            &default_params(),
            &routing_ctx(),
            None,
        )
        .await
        .unwrap();

    match result {
        EngineResult::Complete { content, .. } => {
            assert!(
                !content.is_empty(),
                "Should have synthesized a response"
            );
        }
    }
}
```

Remove the `respects_per_request_max_iterations` test that checks for `Escalate` (or update it to check for synthesis).

**Step 7: Update subagent.rs**

In `subagent.rs` (line 491-496), remove the `Escalate` match arm:

```rust
match outcome {
    EngineResult::Complete { content, .. } => Ok(("ok".to_string(), content)),
}
```

**Step 8: Run all agent tests**

Run: `cargo nextest run -p agent --nocapture`
Expected: All PASS.

**Step 9: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/ crates/agent/src/subagent.rs
git commit -m "feat(agent): remove escalation mechanism, add synthesis at max iterations"
```

---

### Task 3: Simplify ExecutionRouter (remove escalation chain)

**Files:**
- Modify: `crates/agent/src/intent_pipeline/router.rs`

**Step 1: Simplify the router**

Remove `CompletedStep`, `EscalationContext`, `format_work_summary`, `incomplete_result`. Simplify `ExecutionRouter::execute` to direct dispatch without a loop.

The router should become:

```rust
use common::Result;
use providers::Usage;
use tools::RoutingContext;
use tracing::debug;

use super::engines::direct::DirectEngine;
use super::engines::reactive::ReactiveEngine;
use super::engines::EngineResult;
use super::types::ExecutionMode;
use crate::execution::ExecutionParams;

/// Result from the execution router.
#[derive(Debug)]
pub struct RouterResult {
    pub content: String,
    pub final_mode: String,
    pub escalation_count: u32,
    pub usage: Usage,
    pub iterations: u32,
    pub tool_name: Option<String>,
    pub traces: Vec<crate::execution::ReasoningTrace>,
}

pub struct ExecutionRouter {
    direct: DirectEngine,
    reactive: ReactiveEngine,
}

impl ExecutionRouter {
    pub fn new(direct: DirectEngine, reactive: ReactiveEngine) -> Self {
        Self { direct, reactive }
    }

    pub async fn execute(
        &self,
        mode: ExecutionMode,
        messages: Vec<providers::Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<RouterResult> {
        use super::engines::ExecutionEngine;

        let mode_name = mode.short_name();

        let result = match mode {
            ExecutionMode::Direct => {
                debug!("ExecutionRouter: executing with Direct mode");
                self.direct
                    .execute(messages, tools, params, ctx, event_tx)
                    .await?
            }
            ExecutionMode::Reactive { max_iterations } => {
                debug!(
                    "ExecutionRouter: executing with Reactive mode (max_iterations={})",
                    max_iterations
                );
                self.reactive
                    .execute(messages, tools, params, ctx, event_tx)
                    .await?
            }
        };

        match result {
            EngineResult::Complete {
                content,
                usage,
                iterations,
                tool_name,
                traces,
            } => Ok(RouterResult {
                content,
                final_mode: mode_name.to_string(),
                escalation_count: 0,
                usage,
                iterations,
                tool_name,
                traces,
            }),
        }
    }
}
```

**Step 2: Update router tests**

Remove tests that depend on escalation (`handles_escalation_from_direct_to_reactive`, `respects_max_escalation_limit`, `graceful_degradation_message_is_user_friendly`). Keep `routes_direct_to_direct_engine` and `routes_reactive_to_reactive_engine` — update `make_router_with_provider` to use the new 2-arg constructor:

```rust
fn make_router_with_provider(provider: providers::DynProvider) -> ExecutionRouter {
    let registry = make_registry();
    let core = Arc::new(ExecutionCore::new(provider, registry));
    let direct = DirectEngine::new(core.clone());
    let reactive = ReactiveEngine::new(core, 10);
    ExecutionRouter::new(direct, reactive)
}
```

Remove imports for `EscalationContext`, `CompletedStep`, `ToolCall`.

**Step 3: Run router tests**

Run: `cargo nextest run -p agent -E 'test(/router/)' --nocapture`
Expected: All PASS.

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/router.rs
git commit -m "refactor(agent): simplify router, remove escalation chain"
```

---

### Task 4: Update all callers of the simplified router

**Files:**
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs` (if `max_escalations` is passed to router constructor)

**Step 1: Check builder.rs for router construction**

Search for `ExecutionRouter::new` in `builder.rs` and update to remove the `max_escalations` argument.

**Step 2: Update pipeline.rs if needed**

The `RouterResult.escalation_count` field still exists (always 0 now) for backwards compat in strategy recording. No changes needed in pipeline.rs unless there are compile errors.

**Step 3: Build the full workspace**

Run: `cargo build --workspace`
Expected: Clean build, no errors.

**Step 4: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All PASS (some test adjustments may be needed — fix any stragglers).

**Step 5: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs crates/agent/src/intent_pipeline/pipeline.rs
git commit -m "refactor(agent): update callers for simplified router API"
```

---

### Task 5: Preserve content alongside tool calls

**Files:**
- Modify: `crates/providers/src/types.rs`
- Modify: `crates/agent/src/execution/core.rs`

**Step 1: Write the failing test in core.rs**

Add to the `mod tests` block in `core.rs`:

```rust
#[tokio::test]
async fn test_content_preserved_with_tool_calls() {
    // Provider returns BOTH content and tool calls
    let provider = Arc::new(MockProvider {
        responses: Mutex::new(vec![LlmResponse {
            content: Some("Let me search for that...".to_string()),
            tool_calls: vec![ToolCall {
                id: "call_1".to_string(),
                name: "echo".to_string(),
                arguments: serde_json::json!({}),
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }]),
    });
    let registry = make_registry_with(EchoTool);
    let core = ExecutionCore::new(provider, registry);

    let mut messages = vec![Message::user("search")];
    let params = ExecutionParams::new("mock");
    let tools = vec![];

    let (outcome, _usage) = core
        .run_cycle(&mut messages, &tools, &params, &routing_ctx(), None, None)
        .await
        .unwrap();

    assert!(matches!(outcome, CycleOutcome::ToolsExecuted { .. }));

    // The assistant message should contain BOTH content and tool calls
    let assistant_msg = &messages[1]; // user + assistant
    match assistant_msg {
        Message::Assistant { content, tool_calls, .. } => {
            assert!(content.is_some(), "Content should be preserved");
            assert_eq!(content.as_deref(), Some("Let me search for that..."));
            assert!(tool_calls.is_some(), "Tool calls should be present");
        }
        _ => panic!("Expected Assistant message"),
    }
}
```

**Step 2: Run to verify it fails**

Run: `cargo nextest run -p agent -E 'test(content_preserved_with_tool_calls)' --nocapture`
Expected: FAIL — content is `None` in the assistant message.

**Step 3: Update `assistant_with_tools` in `types.rs`**

Add a new constructor that accepts optional content:

```rust
/// Create an assistant message with tool calls and optional text content.
pub fn assistant_with_content_and_tools(
    content: Option<String>,
    tool_calls: Vec<ToolCallMessage>,
) -> Self {
    Self::Assistant {
        content,
        tool_calls: Some(tool_calls),
        reasoning_content: None,
    }
}
```

**Step 4: Update `run_cycle` in `core.rs` to preserve content**

In the `if !response.tool_calls.is_empty()` block (around line 327), change both places where `assistant_with_tools` is called.

For the duplicate-skip path (line 370):
```rust
messages.push(Message::assistant_with_content_and_tools(
    response.content.clone(),
    tool_call_msgs,
));
```

For the normal execution path (line 409):
```rust
let tool_call_msgs = tool_calls_to_messages(&response.tool_calls);
messages.push(Message::assistant_with_content_and_tools(
    response.content.clone(),
    tool_call_msgs,
));
```

Note: `response.content` needs to be cloned before the tool calls consume the response. Make sure to capture `let content = response.content.clone();` before the tool calls processing if needed.

**Step 5: Run the test**

Run: `cargo nextest run -p agent -E 'test(content_preserved_with_tool_calls)' --nocapture`
Expected: PASS.

**Step 6: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All PASS.

**Step 7: Commit**

```bash
git add crates/providers/src/types.rs crates/agent/src/execution/core.rs
git commit -m "fix(agent): preserve LLM content alongside tool calls"
```

---

### Task 6: Fix React key collision in SegmentedMessage

**Files:**
- Modify: `desktop-ui/src/components/chat/SegmentedMessage.tsx`

**Step 1: Fix the key generation**

Change line 83 from:
```typescript
const key = seg.type === 'tool' ? `tool-${seg.name}-${seg.durationMs}` : `text-${i}`;
```

To:
```typescript
const key = seg.type === 'tool' ? `tool-${i}-${seg.name}` : `text-${i}`;
```

**Step 2: Update the comment**

Change line 80-82 from:
```typescript
// Tool segments are stable once pushed; use name+duration as key.
// Text segments update in-place (only the last one mutates), so a
// type-prefixed index is safe and avoids spurious remounts.
```

To:
```typescript
// Use index + name as key for tools (index guarantees uniqueness,
// name adds semantic stability). Text segments use type-prefixed index
// since only the last one mutates during streaming.
```

**Step 3: Build the frontend**

Run: `cd desktop-ui && bun run build`
Expected: Clean build.

**Step 4: Commit**

```bash
git add desktop-ui/src/components/chat/SegmentedMessage.tsx
git commit -m "fix(desktop-ui): resolve duplicate React keys in SegmentedMessage"
```

---

### Task 7: Lint, test, and final verification

**Files:** None (verification only)

**Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings.

**Step 2: Check formatting**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

**Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All PASS.

**Step 4: Run doctests**

Run: `cargo test --workspace --doc`
Expected: All PASS.

**Step 5: Build frontend**

Run: `cd desktop-ui && bun run build`
Expected: Clean build.

**Step 6: Final commit (if any fixes needed)**

Only if lint/fmt required changes.
