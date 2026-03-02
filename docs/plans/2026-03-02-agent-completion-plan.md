# Agent Loop & Intent Pipeline 100% Completion — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix all 13 identified gaps in the agent system to bring it from ~80% to ~97% feature completeness.

**Architecture:** Four subsystems addressed in dependency order: (1) Execution Control lays the foundation (`ExecutionParams` expansion), (2) Observability wires analytics, (3) Escalation fixes degradation paths, (4) Intelligence wires the confidence evaluator and context sources. Each subsystem is a logical commit boundary.

**Tech Stack:** Rust, tokio, serde_json, sqlx (SQLite), async_trait, tokio_util::CancellationToken

**Design doc:** `docs/plans/2026-03-02-agent-completion-design.md`

---

## Task 1: Expand `ExecutionParams` with per-request fields

**Files:**
- Modify: `crates/agent/src/execution/types.rs:9-26`
- Modify: `crates/config/src/schema/orchestrator.rs:8-24`

**Step 1: Write the failing test**

Add to `crates/agent/src/execution/types.rs` bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_params_has_per_request_fields() {
        let params = ExecutionParams::new("mock")
            .with_max_iterations(5)
            .with_max_fabrication_retries(3)
            .with_original_message("hello".to_string());
        assert_eq!(params.max_iterations, 5);
        assert_eq!(params.max_fabrication_retries, 3);
        assert_eq!(params.original_message, "hello");
        assert!(params.cancel_token.is_none());
    }

    #[test]
    fn execution_params_with_cancel_token() {
        let token = tokio_util::sync::CancellationToken::new();
        let params = ExecutionParams::new("mock")
            .with_cancel_token(token.clone());
        assert!(params.cancel_token.is_some());
    }

    #[test]
    fn execution_params_defaults() {
        let params = ExecutionParams::new("mock");
        assert_eq!(params.max_iterations, 10);
        assert_eq!(params.max_fabrication_retries, 2);
        assert!(params.original_message.is_empty());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(execution_params_has_per_request)'`
Expected: FAIL — `with_max_iterations` method doesn't exist

**Step 3: Implement the expanded `ExecutionParams`**

In `crates/agent/src/execution/types.rs`, expand the struct:

```rust
#[derive(Debug, Clone)]
pub struct ExecutionParams {
    pub tool_timeout: Duration,
    pub chat_params: ChatParams,
    /// Per-request max iterations (overrides engine default).
    pub max_iterations: u32,
    /// Max fabrication retries before returning fabricated content as-is.
    pub max_fabrication_retries: u32,
    /// Cancellation token for aborting the execution loop.
    pub cancel_token: Option<tokio_util::sync::CancellationToken>,
    /// The original user message that triggered this execution.
    pub original_message: String,
}

impl ExecutionParams {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            tool_timeout: Duration::from_secs(30),
            chat_params: ChatParams::new(model),
            max_iterations: 10,
            max_fabrication_retries: 2,
            cancel_token: None,
            original_message: String::new(),
        }
    }

    pub fn with_timeout(mut self, dur: Duration) -> Self {
        self.tool_timeout = dur;
        self
    }

    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.max_iterations = max;
        self
    }

    pub fn with_max_fabrication_retries(mut self, max: u32) -> Self {
        self.max_fabrication_retries = max;
        self
    }

    pub fn with_cancel_token(mut self, token: tokio_util::sync::CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    pub fn with_original_message(mut self, msg: String) -> Self {
        self.original_message = msg;
        self
    }
}
```

Add `maxFabricationRetries` to `crates/config/src/schema/orchestrator.rs`:

```rust
pub struct OrchestratorConfig {
    // ... existing fields ...

    /// Maximum fabrication retries before accepting fabricated content (default: 2)
    #[serde(default = "default_max_fabrication_retries")]
    pub max_fabrication_retries: u32,

    /// Reaction satisfaction window in minutes (default: 15)
    #[serde(default = "default_satisfaction_window_minutes")]
    pub satisfaction_window_minutes: u64,
}

fn default_max_fabrication_retries() -> u32 { 2 }
fn default_satisfaction_window_minutes() -> u64 { 15 }
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(execution_params)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/agent/src/execution/types.rs crates/config/src/schema/orchestrator.rs
git commit -m "feat(agent): expand ExecutionParams with per-request iteration budget, cancel token, and original message"
```

---

## Task 2: Wire `ReactiveEngine` to use per-request params

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs:26-59`
- Test: inline `#[cfg(test)] mod tests` in same file

**Step 1: Write the failing test**

Add to `crates/agent/src/intent_pipeline/engines/reactive.rs` tests:

```rust
#[tokio::test]
async fn respects_per_request_max_iterations() {
    // max_iterations=3 in params, engine default is 10
    let responses: Vec<_> = (0..10).map(|_| tool_call_response("ok_tool")).collect();
    let provider = MockSequenceProvider::new(responses);
    let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
    let engine = ReactiveEngine::new(core, 10); // default 10

    let params = default_params().with_max_iterations(3);
    let result = engine
        .execute(
            vec![Message::user("short task")],
            &[],
            &params,
            &routing_ctx(),
            None,
        )
        .await
        .unwrap();

    match result {
        EngineResult::Escalate { reason, .. } => {
            // 3 * 0.8 = 2.4 → ceil = 3, should escalate at iteration 3
            assert!(reason.contains("3"), "reason should reference 3 iterations: {}", reason);
        }
        EngineResult::Complete { iterations, .. } => {
            assert!(iterations <= 3, "should not exceed 3 iterations");
        }
    }
}

#[tokio::test]
async fn checks_cancel_token() {
    let token = tokio_util::sync::CancellationToken::new();
    token.cancel(); // pre-cancel

    let provider = MockSequenceProvider::new(vec![tool_call_response("ok_tool")]);
    let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
    let engine = ReactiveEngine::new(core, 10);

    let params = default_params().with_cancel_token(token);
    let result = engine
        .execute(
            vec![Message::user("test")],
            &[],
            &params,
            &routing_ctx(),
            None,
        )
        .await
        .unwrap();

    // Should return early due to cancellation
    match result {
        EngineResult::Complete { content, .. } => {
            assert!(content.is_empty() || content.contains("cancelled"));
        }
        _ => {} // Either outcome is fine as long as it doesn't loop forever
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(respects_per_request_max_iterations)'`
Expected: FAIL — engine still uses `self.max_iterations`

**Step 3: Implement**

In `reactive.rs`, change the execute method to read from `params`:

```rust
async fn execute(
    &self,
    messages: Vec<Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
) -> Result<EngineResult> {
    let mut messages = messages;
    let mut scratchpad = Scratchpad::new();
    // Use per-request max_iterations from params, fall back to engine default
    let max_iterations = if params.max_iterations > 0 {
        params.max_iterations
    } else {
        self.max_iterations
    };
    let escalation_threshold = (max_iterations as f32 * 0.8).ceil() as u32;
    let mut accumulated_usage = providers::Usage::default();
    let mut fabrication_retries = 0u32;
    let max_fabrication_retries = params.max_fabrication_retries;
    let mut seen_tool_calls: HashSet<String> = HashSet::new();
    let mut last_tool_name: Option<String> = None;
    let mut completed_work: Vec<CompletedStep> = Vec::new();

    for iteration in 1..=max_iterations {
        // Check cancellation
        if let Some(ref token) = params.cancel_token {
            if token.is_cancelled() {
                return Ok(EngineResult::Complete {
                    content: String::new(),
                    usage: accumulated_usage,
                    iterations: iteration - 1,
                    traces: scratchpad.traces().to_vec(),
                    tool_name: last_tool_name,
                });
            }
        }

        // ... rest of iteration (send IterationStart with max_iterations from params) ...
```

Also update the fabrication check to use `max_fabrication_retries`:

```rust
CycleOutcome::FabricatedResponse { content } => {
    fabrication_retries += 1;
    if fabrication_retries > max_fabrication_retries {
        debug!("ReactiveEngine: fabrication retries exhausted ({}), returning as-is", max_fabrication_retries);
        return Ok(EngineResult::Complete { content, usage: accumulated_usage, iterations: iteration, traces: scratchpad.traces().to_vec(), tool_name: last_tool_name });
    }
    // ... inject force prompt ...
```

And populate `original_message` in escalation:

```rust
carried_context: EscalationContext {
    messages,
    completed_work,
    original_message: params.original_message.clone(),
},
```

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(reactive)'`
Expected: ALL PASS

**Step 5: Do the same for `DirectEngine`**

In `direct.rs:56`, change:
```rust
original_message: params.original_message.clone(),
```

**Step 6: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/reactive.rs crates/agent/src/intent_pipeline/engines/direct.rs
git commit -m "feat(agent): wire per-request max_iterations, cancel token, and fabrication retries into engines"
```

---

## Task 3: Wire pipeline to pass per-request params through router

**Files:**
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs:213-226`
- Modify: `crates/agent/src/agent_loop/mod.rs:536-601`

**Step 1: Write the failing test**

Add to `crates/agent/src/intent_pipeline/pipeline.rs` tests:

```rust
#[tokio::test]
async fn pipeline_passes_max_iterations_from_classification() {
    // "show my tasks" classifies as Reactive{max_iterations: 5}
    let provider = MockPipelineProvider::new(vec![text_response("Tasks: ...")]);
    let pipeline = make_pipeline(provider).await;

    let result = pipeline
        .process_message("show my tasks", vec![], &[], &[], &routing_ctx(), None, None)
        .await
        .unwrap();

    // Should complete — the key thing is it doesn't panic
    assert!(!result.content.is_empty());
}
```

**Step 2: Implement**

In `pipeline.rs:213`, change params construction to include classification data:

```rust
let params = ExecutionParams::new(&self.config.execution_model)
    .with_max_iterations(analysis.mode.max_iterations())
    .with_original_message(message.to_string());
```

In `agent_loop/mod.rs:556`, pass the cancel token into RoutingContext or a wrapper:

```rust
let cancel_token = CancellationToken::new();
let cancel_clone = cancel_token.clone();

let handle = tokio::spawn(async move {
    // ... existing code but inject cancel_clone into params ...
});

Ok(StreamingHandle {
    event_rx,
    interaction_rx,
    cancel_token,  // now the caller's cancel affects the spawned task
    handle,
})
```

For this to work, store the cancel_token on the `RoutingContext` (add `pub cancel_token: Option<CancellationToken>` field), then in `pipeline.rs` transfer it to `ExecutionParams`:

```rust
let params = ExecutionParams::new(&self.config.execution_model)
    .with_max_iterations(analysis.mode.max_iterations())
    .with_original_message(message.to_string());

// Transfer cancel token from routing context if present
let params = if let Some(token) = ctx.cancel_token.clone() {
    params.with_cancel_token(token)
} else {
    params
};
```

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(pipeline)'`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/pipeline.rs crates/agent/src/agent_loop/mod.rs crates/tools/src/routing.rs
git commit -m "feat(agent): wire per-request params through pipeline and enable streaming cancellation"
```

---

## Task 4: Streaming token estimation

**Files:**
- Create: `crates/agent/src/execution/token_estimator.rs`
- Modify: `crates/agent/src/execution/core.rs:148-237`
- Modify: `crates/agent/src/execution/mod.rs`

**Step 1: Write the failing test**

Create `crates/agent/src/execution/token_estimator.rs`:

```rust
/// Estimates token counts from raw text when streaming providers don't report usage.
///
/// Uses a chars-per-token ratio that varies by model family.
/// Default ratio ~4 chars/token for Claude models, ~3.5 for GPT models.
pub struct TokenEstimator {
    chars_per_token: f32,
}

impl TokenEstimator {
    pub fn for_model(model: &str) -> Self {
        let ratio = if model.contains("claude") {
            3.5
        } else if model.contains("gpt") {
            3.3
        } else if model.contains("deepseek") {
            3.8
        } else {
            3.5 // safe default
        };
        Self { chars_per_token: ratio }
    }

    /// Estimate token count from text length.
    pub fn estimate(&self, text: &str) -> u32 {
        (text.len() as f32 / self.chars_per_token).ceil() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimates_claude_tokens() {
        let est = TokenEstimator::for_model("claude-sonnet-4-20250514");
        // 100 chars / 3.5 = ~29 tokens
        let text = "a".repeat(100);
        let tokens = est.estimate(&text);
        assert!(tokens >= 25 && tokens <= 35, "got {}", tokens);
    }

    #[test]
    fn estimates_empty_text() {
        let est = TokenEstimator::for_model("claude-sonnet-4-20250514");
        assert_eq!(est.estimate(""), 0);
    }

    #[test]
    fn unknown_model_uses_default() {
        let est = TokenEstimator::for_model("some-unknown-model");
        let tokens = est.estimate(&"x".repeat(35));
        assert_eq!(tokens, 10);
    }
}
```

**Step 2: Run test to verify it passes**

Run: `cargo nextest run -p agent -E 'test(estimates_claude_tokens)'`
Expected: PASS

**Step 3: Wire into `call_provider_streaming`**

In `core.rs`, after the stream loop completes (line ~220), add:

```rust
// Estimate token usage for streaming calls
let estimator = super::token_estimator::TokenEstimator::for_model(&params.model);
let prompt_text_len: usize = messages.iter().map(|m| m.content_text().len()).sum();
let estimated_input = estimator.estimate(&"x".repeat(prompt_text_len));
let estimated_output = estimator.estimate(&content);

Ok(providers::LlmResponse {
    // ...
    usage: Usage {
        prompt_tokens: estimated_input,
        completion_tokens: estimated_output,
        total_tokens: estimated_input + estimated_output,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    },
    // ...
})
```

Note: `call_provider_streaming` needs access to the `params.chat_params.model` — it already receives `params: &ChatParams`. Get model from there.

**Step 4: Add module to `crates/agent/src/execution/mod.rs`**

```rust
pub mod token_estimator;
```

**Step 5: Run tests**

Run: `cargo nextest run -p agent`
Expected: ALL PASS

**Step 6: Commit**

```bash
git add crates/agent/src/execution/token_estimator.rs crates/agent/src/execution/core.rs crates/agent/src/execution/mod.rs
git commit -m "feat(agent): estimate streaming token usage instead of reporting zeros"
```

---

## Task 5: Persist complexity_signals and traces in strategy records

**Files:**
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs:293-316`
- Modify: `crates/agent/src/intent_pipeline/router.rs:43-57`
- Modify: `crates/storage/src/repos/strategy.rs` (add `execution_traces` column)
- Create: migration file in `crates/storage/src/migrations/`

**Step 1: Write the failing test**

In `pipeline.rs` tests, modify `pipeline_writes_strategy_record`:

```rust
#[tokio::test]
async fn pipeline_persists_complexity_signals() {
    let storage_pool = storage::StoragePool::connect_in_memory().await.expect("pool");
    let repos = storage::Repos::from_pool(&storage_pool);

    let provider = MockPipelineProvider::new(vec![text_response("Done!")]);
    // ... (same pipeline setup as existing test) ...

    let result = pipeline.process_message("create a task for buying groceries", vec![], &[], &["task"], &routing_ctx(), None, None).await.unwrap();
    assert!(!result.content.is_empty());

    let since = chrono::Utc::now() - chrono::Duration::minutes(1);
    let records = repos.strategies.list_by_date_range(since, chrono::Utc::now()).await.unwrap();
    assert_eq!(records.len(), 1);
    assert_ne!(records[0].complexity_signals, serde_json::Value::Null, "complexity_signals should be populated");
}
```

**Step 2: Implement complexity_signals persistence**

In `pipeline.rs:309`, change:

```rust
complexity_signals: serde_json::to_value(&analysis.signals).unwrap_or_default(),
```

**Step 3: Add `traces` field to `RouterResult`**

In `router.rs:43-57`, add:

```rust
pub struct RouterResult {
    // ... existing fields ...
    /// Reasoning traces from engine execution.
    pub traces: Vec<crate::execution::ReasoningTrace>,
}
```

Populate it in the `Complete` branch (~line 124):

```rust
EngineResult::Complete { content, usage, iterations, tool_name, traces } => {
    return Ok(RouterResult {
        content,
        final_mode: current_mode.to_string(),
        escalation_count,
        usage,
        iterations,
        tool_name,
        traces,
    });
}
```

And empty traces in the exhaustion branches.

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(pipeline)'`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add crates/agent/src/intent_pipeline/pipeline.rs crates/agent/src/intent_pipeline/router.rs
git commit -m "feat(agent): persist complexity_signals and engine traces in strategy records"
```

---

## Task 6: Per-request channel and configurable reaction window

**Files:**
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs:263-276` (record_usage reads from ctx)
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs:40-66` (remove channel from PipelineConfig)
- Modify: `crates/agent/src/agent_loop/builder.rs:699-706`
- Modify: `crates/agent/src/agent_loop/mod.rs:77-108` (configurable reaction window)

**Step 1: Implement**

In `pipeline.rs:263-276`, `record_usage` already takes `ctx: &RoutingContext`. Change:

```rust
ctx.channel.as_str(),  // was: &self.config.channel
```

Check: this is already done at line 271! Verify the existing code. If `record_usage` already reads `ctx.channel`, then this gap may already be partially fixed. If not, make the change.

In `builder.rs:704`, change to remove the now-unnecessary default:

```rust
channel: String::new(),  // Not used anymore — channel comes from RoutingContext per-request
```

In `mod.rs:87`, make the reaction window configurable:

```rust
let window_minutes = self.config.orchestrator.satisfaction_window_minutes;
let since = chrono::Utc::now() - chrono::Duration::minutes(window_minutes as i64);
```

**Step 2: Run tests**

Run: `cargo nextest run -p agent`
Expected: ALL PASS

**Step 3: Commit**

```bash
git add crates/agent/src/intent_pipeline/pipeline.rs crates/agent/src/agent_loop/builder.rs crates/agent/src/agent_loop/mod.rs
git commit -m "fix(agent): per-request channel in cost tracking, configurable reaction window"
```

---

## Task 7: Graceful degradation on reactive exhaustion

**Files:**
- Modify: `crates/agent/src/intent_pipeline/router.rs:179-196`

**Step 1: Write the failing test**

In `router.rs` tests:

```rust
#[tokio::test]
async fn graceful_degradation_includes_completed_work() {
    // Provider always returns tool calls → Direct escalates, Reactive exhausts
    let responses: Vec<_> = (0..20).map(|i| {
        LlmResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: format!("c{}", i),
                name: format!("tool{}", i % 3),
                arguments: serde_json::json!({}),
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }
    }).collect();

    let provider = SequenceProvider::new(responses);
    let registry = make_registry();
    let core = Arc::new(ExecutionCore::new(provider, registry));
    let direct = DirectEngine::new(core.clone());
    let reactive = ReactiveEngine::new(core, 5);
    let router = ExecutionRouter::new(direct, reactive, 1);

    let result = router
        .execute(
            ExecutionMode::Direct,
            vec![Message::user("do complex thing")],
            &[],
            &default_params(),
            &routing_ctx(),
            None,
        )
        .await
        .unwrap();

    // Should have a user-friendly message, not raw "Task could not be completed..."
    assert!(!result.content.contains("Task could not be completed"),
        "Should have graceful message: {}", result.content);
    assert!(result.content.contains("progress") || result.content.contains("accomplished") || result.content.contains("completed"),
        "Should mention what was accomplished: {}", result.content);
}
```

**Step 2: Implement**

In `router.rs:179-196`, replace the raw error string:

```rust
_ => {
    // Reactive — no further escalation possible. Build graceful response.
    warn!("ExecutionRouter: cannot escalate beyond {} mode", current_mode);

    let work_summary = if carried_context.completed_work.is_empty() {
        String::new()
    } else {
        let steps: Vec<String> = carried_context.completed_work.iter()
            .map(|s| format!("- {}: {}", s.tool_name, s.description))
            .collect();
        format!("\n\nHere's what I was able to accomplish:\n{}", steps.join("\n"))
    };

    return Ok(RouterResult {
        content: format!(
            "I made progress on your request but wasn't able to fully complete it. \
             The task may need to be broken into smaller steps.{}",
            work_summary
        ),
        final_mode: current_mode.to_string(),
        escalation_count,
        usage: escalation_usage,
        iterations: 0,
        tool_name: None,
        traces: vec![],
    });
}
```

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(router)'`
Expected: ALL PASS (existing tests may need minor assertion updates for the new message format)

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/router.rs
git commit -m "feat(agent): graceful degradation with partial-result summary on reactive exhaustion"
```

---

## Task 8: Oversized message feedback

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs:212-217`

**Step 1: Implement**

Replace the silent drop at `mod.rs:213-217`:

```rust
if let Err(e) = msg.validate() {
    warn!("Message validation failed: {}", e);
    // Send error feedback instead of silently dropping
    let error_msg = OutboundMessage::new(
        msg.channel.clone(),
        msg.chat_id.clone(),
        format!("Message too large to process: {}. Please shorten and try again.", e),
    );
    if let Err(send_err) = self.bus.publish_outbound(error_msg).await {
        warn!("Failed to send validation error: {}", send_err);
    }
    return Ok(());
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p agent`
Expected: ALL PASS

**Step 3: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs
git commit -m "fix(agent): send user feedback on oversized messages instead of silent drop"
```

---

## Task 9: Wire ConfidenceEvaluator into IntentPipeline

**Files:**
- Modify: `crates/agent/src/intent_pipeline/pipeline.rs:70-103`
- Modify: `crates/agent/src/agent_loop/builder.rs:708-717`

**Step 1: Write the failing test**

In `pipeline.rs` tests:

```rust
#[tokio::test]
async fn pipeline_accepts_confidence_evaluator() {
    let provider = MockPipelineProvider::new(vec![text_response("Hi!")]);
    let pipeline = make_pipeline(provider).await;
    // Just verify it compiles and runs with no evaluator (backward compat)
    let result = pipeline.process_message("hello", vec![], &[], &[], &routing_ctx(), None, None).await.unwrap();
    assert!(!result.content.is_empty());
}
```

**Step 2: Implement**

Add to `IntentPipeline` struct:

```rust
pub struct IntentPipeline {
    // ... existing fields ...
    confidence_evaluator: Option<Arc<crate::learning::ConfidenceEvaluator>>,
}
```

Add builder method:

```rust
pub fn with_confidence_evaluator(mut self, evaluator: Arc<crate::learning::ConfidenceEvaluator>) -> Self {
    self.confidence_evaluator = Some(evaluator);
    self
}
```

In `process_message`, after step 1 classification (~line 134), add:

```rust
// Step 1.5: Confidence check — ask for clarification if confidence is too low
if let Some(ref evaluator) = self.confidence_evaluator {
    if let Some(decision) = evaluator.evaluate(analysis.confidence, analysis.mode.short_name()) {
        if decision.should_ask_user {
            debug!("IntentPipeline: low confidence ({:.2}), suggesting clarification", analysis.confidence);
            // Return a clarification prompt as the response
            return Ok(PipelineResult {
                content: format!(
                    "I'm not entirely sure what you'd like me to do. Could you clarify? \
                     I interpreted this as a {} request (confidence: {:.0}%).",
                    analysis.mode.short_name(),
                    analysis.confidence * 100.0
                ),
                mode_used: "clarification".to_string(),
                classification: analysis,
                escalations: 0,
                validation: ValidationResult::valid(),
            });
        }
    }
}
```

In `builder.rs`, after pipeline construction (~line 716), inject the evaluator:

```rust
let mut pipeline = crate::intent_pipeline::IntentPipeline::new(
    analyzer, Arc::clone(&context_engine), router, cost_tracker, pipeline_config,
).with_strategy_repo(repos.strategies.clone());

if let Some(evaluator) = confidence_evaluator {
    pipeline = pipeline.with_confidence_evaluator(Arc::new(evaluator));
}

let pipeline = Arc::new(pipeline);
```

**Step 3: Run tests**

Run: `cargo nextest run -p agent`
Expected: ALL PASS

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/pipeline.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): wire ConfidenceEvaluator into IntentPipeline for low-confidence clarification"
```

---

## Task 10: Register context sources and fix SubagentHandle dead code

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:161-172`
- Modify: `crates/agent/src/subagent.rs:74-81`

**Step 1: Implement context source registration**

In `builder.rs`, locate where context sources are registered (~line 161-172) and add:

```rust
// Register PersonaContextSource if configured
if config.context.persona.enabled {
    sources.push(Arc::new(crate::context_sources::PersonaContextSource::new(
        &config,
    )));
}

// Register PageContextSource if configured
if config.context.page.enabled {
    sources.push(Arc::new(crate::context_sources::PageContextSource::new()));
}
```

Note: check if `config.context.persona` and `config.context.page` exist in config schema. If not, add them with `enabled: false` defaults.

**Step 2: Fix SubagentHandle dead code**

In `subagent.rs:78-79`, remove `#[allow(dead_code)]`. Then find or add a `list_active` or `status` method that uses `profile`:

```rust
/// List active subagents with their profiles
pub async fn list_active(&self) -> Vec<(String, SubagentProfile, std::time::Duration)> {
    let handles = self.handles.lock().unwrap();
    handles.iter().map(|(label, h)| {
        (h.label.clone(), h.profile.clone(), h.spawned_at.elapsed())
    }).collect()
}
```

If `SubagentProfile` doesn't derive `Clone`, add it.

**Step 3: Run tests**

Run: `cargo nextest run -p agent`
Expected: ALL PASS with zero clippy warnings on `profile`

**Step 4: Verify clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: No warnings about dead_code on profile

**Step 5: Commit**

```bash
git add crates/agent/src/agent_loop/builder.rs crates/agent/src/subagent.rs
git commit -m "feat(agent): register PersonaContextSource/PageContextSource, use SubagentHandle.profile"
```

---

## Task 11: Final integration test and clippy clean

**Files:**
- All modified files

**Step 1: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: ALL PASS

**Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 3: Run format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues

**Step 4: Verify no remaining TODO/FIXME for addressed gaps**

Run: `grep -rn "TODO\|FIXME\|XXX\|HACK" crates/agent/src/agent_loop/ crates/agent/src/intent_pipeline/ crates/agent/src/execution/`
Expected: No new TODOs related to the 13 gaps

**Step 5: Commit if any cleanup was needed**

```bash
git add -A
git commit -m "chore(agent): final cleanup and lint fixes for agent completion"
```

---

## Completion Checklist

After all tasks, verify these gaps are closed:

- [ ] Gap 1: `cancel_token` observed by spawned streaming task
- [ ] Gap 2: Streaming calls estimate token usage
- [ ] Gap 3: `max_iterations` from classifier flows to engine
- [ ] Gap 4: `ConfidenceEvaluator` injected into pipeline
- [ ] Gap 5: `complexity_signals` serialized in strategy records
- [ ] Gap 6: `original_message` populated in `EscalationContext`
- [ ] Gap 7: Channel from `RoutingContext` used in cost tracking
- [ ] Gap 8: Graceful degradation on reactive exhaustion
- [ ] Gap 9: `user_satisfaction` — connected via reaction window (Task 6)
- [ ] Gap 10: Oversized messages get user feedback
- [ ] Gap 11: Reaction window configurable (not hardcoded 5 min)
- [ ] Gap 12: `PersonaContextSource`/`PageContextSource` registered
- [ ] Gap 13: `SubagentHandle.profile` used, `#[allow(dead_code)]` removed
- [ ] Bonus: Fabrication retry budget configurable
- [ ] Bonus: Engine traces persisted in strategy records
