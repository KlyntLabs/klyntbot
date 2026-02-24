//! ReAct+ execution engine — enhanced ReAct loop with reasoning scratchpad,
//! reflection checkpoints, and escalation to autonomous task execution.

use std::collections::HashSet;
use std::sync::Arc;

use chrono::Utc;
use tracing::debug;

use common::Result;
use providers::{Message, Usage};
use tools::RoutingContext;

use super::core::ExecutionCore;
use super::scratchpad::{ReasoningTrace, Scratchpad};
use super::types::{accumulate_usage, CycleOutcome, ExecutionParams};

/// When to inject a reflection prompt into the conversation.
#[derive(Debug, Clone)]
pub enum ReflectionMode {
    /// Reflect after any tool failure.
    OnFailure,
    /// Reflect every N iterations.
    EveryN(u32),
    /// Never reflect.
    Never,
}

/// Outcome of a ReAct+ execution run.
#[derive(Debug)]
pub enum ReactOutcome {
    /// Successfully produced a final response.
    Response {
        content: String,
        traces: Vec<ReasoningTrace>,
        iterations: u32,
        usage: Usage,
        /// Name of the last tool called (for learning analytics). None if no tools called.
        last_tool_name: Option<String>,
    },
    /// Complexity exceeds ReAct+ capacity; escalate to autonomous planner.
    EscalateToAutonomous { reason: String, usage: Usage },
    /// Hit the iteration limit without a final response.
    MaxIterationsReached {
        partial_content: Option<String>,
        usage: Usage,
    },
}

/// Enhanced ReAct loop with scratchpad, reflection, and escalation.
pub struct ReactPlusEngine {
    core: Arc<ExecutionCore>,
    max_iterations: u32,
    reflection_mode: ReflectionMode,
}

impl ReactPlusEngine {
    pub fn new(core: Arc<ExecutionCore>) -> Self {
        Self {
            core,
            max_iterations: 10,
            reflection_mode: ReflectionMode::OnFailure,
        }
    }

    pub fn with_max_iterations(mut self, n: u32) -> Self {
        self.max_iterations = n;
        self
    }

    pub fn with_reflection_mode(mut self, mode: ReflectionMode) -> Self {
        self.reflection_mode = mode;
        self
    }

    /// Run the ReAct+ loop.
    ///
    /// Accepts `Arc<Vec<Message>>` to avoid cloning when the caller can
    /// transfer sole ownership (refcount == 1).
    pub async fn execute(
        &self,
        messages: Arc<Vec<Message>>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<ReactOutcome> {
        let mut messages = Arc::try_unwrap(messages).unwrap_or_else(|arc| (*arc).clone());
        let mut scratchpad = Scratchpad::new();
        let escalation_threshold = (self.max_iterations as f32 * 0.8).ceil() as u32;
        let mut accumulated_usage = Usage::default();
        let mut force_retried = false;
        // Track tool call signatures across iterations to detect duplicates.
        // Each entry is a string key: "tool_name|canonical_args_json".
        // Passed into run_cycle so duplicates are blocked BEFORE execution.
        let mut seen_tool_calls: HashSet<String> = HashSet::new();
        // Track the last tool name for analytics.
        let mut last_tool_name: Option<String> = None;

        for iteration in 1..=self.max_iterations {
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(crate::events::AgentEvent::IterationStart {
                        iteration: iteration as usize,
                        max: self.max_iterations as usize,
                    })
                    .await;
            }

            let (outcome, cycle_usage) = self
                .core
                .run_cycle(
                    &mut messages,
                    tools,
                    params,
                    ctx,
                    event_tx.as_ref(),
                    Some(&mut seen_tool_calls),
                )
                .await?;
            accumulate_usage(&mut accumulated_usage, &cycle_usage);

            match outcome {
                CycleOutcome::FinalResponse { content } => {
                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: "Received final response".to_string(),
                        planned_actions: vec![],
                        actual_action: "final_response".to_string(),
                        reflection: None,
                        timestamp: Utc::now(),
                    });

                    return Ok(ReactOutcome::Response {
                        content,
                        traces: scratchpad.traces().to_vec(),
                        iterations: iteration,
                        usage: accumulated_usage,
                        last_tool_name: last_tool_name.clone(),
                    });
                }

                CycleOutcome::FabricatedResponse { content } => {
                    if force_retried {
                        // Already retried once — graceful degradation
                        debug!("ReactPlus: fabrication retry exhausted, returning text as-is");
                        scratchpad.add(ReasoningTrace {
                            cycle: iteration,
                            thought: "Fabricated response after retry — giving up".to_string(),
                            planned_actions: vec![],
                            actual_action: "fabrication_degraded".to_string(),
                            reflection: None,
                            timestamp: Utc::now(),
                        });
                        return Ok(ReactOutcome::Response {
                            content,
                            traces: scratchpad.traces().to_vec(),
                            iterations: iteration,
                            usage: accumulated_usage,
                            last_tool_name: last_tool_name.clone(),
                        });
                    }

                    // First fabrication — inject force-tool-use prompt and retry
                    debug!("ReactPlus: detected fabricated response, injecting force-tool prompt");
                    force_retried = true;

                    let tool_list: Vec<&str> = tools
                        .iter()
                        .filter_map(|t| {
                            t.get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                        })
                        .collect();

                    messages.push(Message::user(format!(
                        "You returned a text response instead of calling a tool. \
                         You have these tools available: [{}]. \
                         You MUST call the appropriate tool to complete the user's request. \
                         Do NOT respond with text describing what you would do — actually call the tool.",
                        tool_list.join(", ")
                    )));

                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: "Detected fabricated tool response — forcing retry".to_string(),
                        planned_actions: tool_list.iter().map(|s| s.to_string()).collect(),
                        actual_action: "fabrication_retry".to_string(),
                        reflection: Some(
                            "LLM returned text instead of tool call, injecting force prompt"
                                .to_string(),
                        ),
                        timestamp: Utc::now(),
                    });

                    // Continue to next iteration (the retry)
                }

                CycleOutcome::ToolsExecuted { results } => {
                    let tool_names: Vec<String> =
                        results.iter().map(|r| r.tool_name.clone()).collect();
                    // Capture the last tool name for analytics.
                    if let Some(name) = tool_names.last() {
                        last_tool_name = Some(name.clone());
                    }
                    let had_failure = results.iter().any(|r| !r.success);
                    let failure_details: Vec<String> = results
                        .iter()
                        .filter(|r| !r.success)
                        .map(|r| format!("{}: {}", r.tool_name, r.result))
                        .collect();

                    // ── Duplicate tool call handling ──────────────────
                    // Duplicates are now blocked BEFORE execution inside
                    // run_cycle (via the seen_tool_calls set). If run_cycle
                    // detected a duplicate, it returns results with
                    // success=false and "Skipped:" messages — no side effects.
                    // We inject a clear directive for the LLM to move on.
                    let all_skipped_duplicates = !results.is_empty()
                        && results
                            .iter()
                            .all(|r| !r.success && r.result.starts_with("Skipped:"));

                    let mut reflection = None;

                    if all_skipped_duplicates {
                        debug!(
                            "ReactPlus: run_cycle blocked duplicate tool calls on iteration {}: {:?}",
                            iteration, tool_names
                        );
                        let dup_prompt = format!(
                            "You just attempted to call the same tool(s) with the same arguments as a previous iteration: [{}]. \
                             The calls were blocked — the results are already in the conversation above. \
                             Do NOT repeat these calls. Either proceed with a final response using the results you already have, \
                             or take a DIFFERENT action.",
                            tool_names.join(", ")
                        );
                        messages.push(Message::user(&dup_prompt));
                        reflection = Some(dup_prompt);
                    }

                    // Check if reflection should trigger (skip if we already injected a duplicate warning)
                    let should_reflect = if reflection.is_none() {
                        match &self.reflection_mode {
                            ReflectionMode::OnFailure => had_failure,
                            ReflectionMode::EveryN(n) => iteration % n == 0,
                            ReflectionMode::Never => false,
                        }
                    } else {
                        false
                    };

                    if should_reflect {
                        let reflection_prompt = if had_failure {
                            format!(
                                "Reflection: Tool failures occurred: {}. What went wrong and how should I adjust my approach?",
                                failure_details.join("; ")
                            )
                        } else {
                            format!(
                                "Reflection (cycle {}): What progress have I made and what should I do next?",
                                iteration
                            )
                        };
                        messages.push(Message::user(&reflection_prompt));
                        reflection = Some(reflection_prompt);
                    }

                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: format!("Executed {} tool(s)", tool_names.len()),
                        planned_actions: tool_names,
                        actual_action: "tools_executed".to_string(),
                        reflection,
                        timestamp: Utc::now(),
                    });

                    // Check escalation threshold
                    if iteration >= escalation_threshold {
                        return Ok(ReactOutcome::EscalateToAutonomous {
                            reason: format!(
                                "Used {}% of max iterations ({}/{}), task may need planning",
                                (iteration * 100) / self.max_iterations,
                                iteration,
                                self.max_iterations
                            ),
                            usage: accumulated_usage,
                        });
                    }
                }

                CycleOutcome::EmptyResponse => {
                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: "Received empty response from LLM".to_string(),
                        planned_actions: vec![],
                        actual_action: "empty_response".to_string(),
                        reflection: None,
                        timestamp: Utc::now(),
                    });
                    // Continue — the LLM may recover on the next cycle
                }
            }
        }

        Ok(ReactOutcome::MaxIterationsReached {
            partial_content: None,
            usage: accumulated_usage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, LlmProvider, LlmResponse, ToolCall, Usage};
    use serde_json::Value;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::RwLock;
    use tools::{registry::ToolRegistry, Tool};

    // ── Mock provider with sequence of responses ──

    struct SequenceProvider {
        responses: Mutex<Vec<LlmResponse>>,
    }

    impl SequenceProvider {
        fn new(responses: Vec<LlmResponse>) -> Arc<Self> {
            Arc::new(Self {
                responses: Mutex::new(responses),
            })
        }
    }

    #[async_trait]
    impl LlmProvider for SequenceProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            let mut responses = self.responses.lock().unwrap();
            if responses.is_empty() {
                Ok(LlmResponse {
                    content: Some("fallback response".to_string()),
                    tool_calls: vec![],
                    finish_reason: "stop".to_string(),
                    usage: Usage::default(),
                    reasoning_content: None,
                })
            } else {
                Ok(responses.remove(0))
            }
        }

        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    // ── Mock tool ──

    struct OkTool;

    #[async_trait]
    impl Tool for OkTool {
        fn name(&self) -> &str {
            "ok_tool"
        }
        fn description(&self) -> &str {
            "Always succeeds"
        }
        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> common::Result<String> {
            Ok("success".to_string())
        }
    }

    struct FailTool;

    #[async_trait]
    impl Tool for FailTool {
        fn name(&self) -> &str {
            "fail_tool"
        }
        fn description(&self) -> &str {
            "Always fails"
        }
        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> common::Result<String> {
            Err(common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed("deliberate error".to_string()),
            ))
        }
    }

    // ── Helpers ──

    fn make_tool_call_response(tool_name: &str) -> LlmResponse {
        LlmResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: "call_1".to_string(),
                name: tool_name.to_string(),
                arguments: serde_json::json!({}),
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }
    }

    fn make_text_response(text: &str) -> LlmResponse {
        LlmResponse {
            content: Some(text.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }
    }

    fn make_registry_with_ok() -> Arc<RwLock<ToolRegistry>> {
        let mut reg = ToolRegistry::new();
        reg.register(OkTool);
        Arc::new(RwLock::new(reg))
    }

    fn make_registry_with_fail() -> Arc<RwLock<ToolRegistry>> {
        let mut reg = ToolRegistry::new();
        reg.register(FailTool);
        Arc::new(RwLock::new(reg))
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    fn default_params() -> ExecutionParams {
        ExecutionParams::new("mock").with_timeout(Duration::from_secs(5))
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_single_iteration_final_response() {
        // Provider returns one tool call, then a final response
        let provider = SequenceProvider::new(vec![
            make_tool_call_response("ok_tool"),
            make_text_response("Done! Here's your answer."),
        ]);
        let registry = make_registry_with_ok();
        let core = Arc::new(ExecutionCore::new(provider, registry));

        let engine = ReactPlusEngine::new(core).with_max_iterations(10);
        let messages = Arc::new(vec![Message::user("do something")]);

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx(), None)
            .await
            .unwrap();

        match outcome {
            ReactOutcome::Response {
                content,
                traces,
                iterations,
                ..
            } => {
                assert!(content.contains("Done"));
                assert_eq!(iterations, 2); // tool call + final response
                assert_eq!(traces.len(), 2);
                assert_eq!(traces[0].actual_action, "tools_executed");
                assert_eq!(traces[1].actual_action, "final_response");
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_max_iterations_returns_reached() {
        // Provider always returns tool calls — never a final response
        let responses: Vec<LlmResponse> =
            (0..5).map(|_| make_tool_call_response("ok_tool")).collect();
        let provider = SequenceProvider::new(responses);
        let registry = make_registry_with_ok();
        let _core = Arc::new(ExecutionCore::new(provider, registry));

        // max_iterations=3, escalation at 80% = ceil(2.4) = 3
        // So iteration 3 hits escalation first. Use max_iterations=2 to hit MaxIterationsReached.
        // Actually with max_iterations=2, escalation_threshold = ceil(1.6) = 2, so iteration 2 hits escalation.
        // For MaxIterationsReached we need escalation_threshold > max_iterations, which won't happen.
        // The only way to get MaxIterationsReached is if we never hit escalation threshold —
        // which means the tool call must produce a final response or empty before escalation.
        //
        // Let's use ReflectionMode::Never and ensure escalation doesn't trigger by
        // making the provider always return empty responses instead of tool calls.
        let empty_responses: Vec<LlmResponse> = (0..5)
            .map(|_| LlmResponse {
                content: Some("".to_string()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
            .collect();
        let provider2 = SequenceProvider::new(empty_responses);
        let registry2 = make_registry_with_ok();
        let core2 = Arc::new(ExecutionCore::new(provider2, registry2));

        let engine = ReactPlusEngine::new(core2)
            .with_max_iterations(3)
            .with_reflection_mode(ReflectionMode::Never);

        let messages = Arc::new(vec![Message::user("do something")]);
        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx(), None)
            .await
            .unwrap();

        assert!(matches!(outcome, ReactOutcome::MaxIterationsReached { .. }));
    }

    #[tokio::test]
    async fn test_escalate_at_80_percent() {
        // With max_iterations=5, escalation at ceil(4.0) = 4
        // So after tool call on iteration 4, should escalate
        let responses: Vec<LlmResponse> = (0..10)
            .map(|_| make_tool_call_response("ok_tool"))
            .collect();
        let provider = SequenceProvider::new(responses);
        let registry = make_registry_with_ok();
        let core = Arc::new(ExecutionCore::new(provider, registry));

        let engine = ReactPlusEngine::new(core).with_max_iterations(5);
        let messages = Arc::new(vec![Message::user("complex task")]);

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx(), None)
            .await
            .unwrap();

        match outcome {
            ReactOutcome::EscalateToAutonomous { reason, .. } => {
                assert!(reason.contains("80%"));
            }
            other => panic!("Expected EscalateToAutonomous, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_scratchpad_accumulates_traces() {
        // 3 tool calls then final response = 4 traces
        let responses = vec![
            make_tool_call_response("ok_tool"),
            make_tool_call_response("ok_tool"),
            make_tool_call_response("ok_tool"),
            make_text_response("All done"),
        ];
        let provider = SequenceProvider::new(responses);
        let registry = make_registry_with_ok();
        let core = Arc::new(ExecutionCore::new(provider, registry));

        let engine = ReactPlusEngine::new(core).with_max_iterations(10);
        let messages = Arc::new(vec![Message::user("multi-step task")]);

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx(), None)
            .await
            .unwrap();

        match outcome {
            ReactOutcome::Response {
                traces, iterations, ..
            } => {
                assert_eq!(traces.len(), 4);
                assert_eq!(iterations, 4);
                // Verify cycle numbers are sequential
                for (i, trace) in traces.iter().enumerate() {
                    assert_eq!(trace.cycle, (i + 1) as u32);
                }
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_reflection_on_failure() {
        // Tool fails, then final response
        let responses = vec![
            make_tool_call_response("fail_tool"),
            make_text_response("I adjusted my approach"),
        ];
        let provider = SequenceProvider::new(responses);
        let registry = make_registry_with_fail();
        let core = Arc::new(ExecutionCore::new(provider, registry));

        let engine = ReactPlusEngine::new(core)
            .with_max_iterations(10)
            .with_reflection_mode(ReflectionMode::OnFailure);
        let messages = Arc::new(vec![Message::user("try something")]);

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx(), None)
            .await
            .unwrap();

        match outcome {
            ReactOutcome::Response { traces, .. } => {
                // First trace should have a reflection since the tool failed
                assert!(traces[0].reflection.is_some());
                assert!(traces[0]
                    .reflection
                    .as_ref()
                    .unwrap()
                    .contains("Reflection"));
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    // ── Fabrication retry tests ──

    #[tokio::test]
    async fn test_fabricated_response_triggers_retry() {
        // First call: LLM returns fabricated text (no tool calls)
        // After force-retry prompt: LLM calls the tool correctly
        // Third call: LLM returns final response
        let responses = vec![
            // Iteration 1: fabricated response
            LlmResponse {
                content: Some(
                    "I've created the task:\n**Task Created:** Buy groceries (ID: 9c4e5f3b)\n- **Priority:** P3\n- **Due Date:** Tomorrow".to_string()
                ),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            // Iteration 2 (after force prompt): proper tool call
            make_tool_call_response("ok_tool"),
            // Iteration 3: final response
            make_text_response("Done! Task created successfully."),
        ];
        let provider = SequenceProvider::new(responses);
        let registry = make_registry_with_ok();
        let core = Arc::new(ExecutionCore::new(provider, registry));

        let engine = ReactPlusEngine::new(core).with_max_iterations(10);
        let messages = Arc::new(vec![Message::user("create task: buy")]);

        // Need tool definitions so fabrication detection triggers
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "todo",
                "description": "Manage tasks",
                "parameters": {"type": "object", "properties": {}}
            }
        })];

        let outcome = engine
            .execute(messages, &tools, &default_params(), &routing_ctx(), None)
            .await
            .unwrap();

        match outcome {
            ReactOutcome::Response {
                content,
                iterations,
                ..
            } => {
                assert!(content.contains("Done"));
                // Should have taken 3 iterations: fabricated → retry with tool → final
                assert_eq!(iterations, 3);
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_fabricated_response_retry_only_once() {
        // Both attempts return fabricated text — should give up after one retry
        let responses = vec![
            // Iteration 1: fabricated
            LlmResponse {
                content: Some(
                    "Task Created: Buy groceries (ID: abcdef12)\n- Priority: P3\n- Due Date: Tomorrow".to_string()
                ),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            // Iteration 2 (after force prompt): still fabricated
            LlmResponse {
                content: Some(
                    "Task Created: Buy groceries (ID: abcdef12)\n- Priority: P3\n- Due Date: Tomorrow".to_string()
                ),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
        ];
        let provider = SequenceProvider::new(responses);
        let registry = make_registry_with_ok();
        let core = Arc::new(ExecutionCore::new(provider, registry));

        let engine = ReactPlusEngine::new(core).with_max_iterations(10);
        let messages = Arc::new(vec![Message::user("create task: buy")]);
        let tools = vec![serde_json::json!({
            "type": "function",
            "function": {
                "name": "todo",
                "description": "Manage tasks",
                "parameters": {"type": "object", "properties": {}}
            }
        })];

        let outcome = engine
            .execute(messages, &tools, &default_params(), &routing_ctx(), None)
            .await
            .unwrap();

        // Should gracefully degrade — return the text as FinalResponse after one retry
        match outcome {
            ReactOutcome::Response { content, .. } => {
                assert!(content.contains("Task Created"));
            }
            other => panic!("Expected graceful degradation Response, got {:?}", other),
        }
    }

    // ── Duplicate tool call detection tests ──

    #[tokio::test]
    async fn test_duplicate_tool_calls_inject_reflection() {
        // Iteration 1: LLM calls ok_tool with {} → tools executed
        // Iteration 2: LLM calls ok_tool with {} again (duplicate) → reflection injected
        // Iteration 3: LLM returns final response after seeing the warning
        let responses = vec![
            make_tool_call_response("ok_tool"),
            make_tool_call_response("ok_tool"), // duplicate!
            make_text_response("Done, using existing results."),
        ];
        let provider = SequenceProvider::new(responses);
        let registry = make_registry_with_ok();
        let core = Arc::new(ExecutionCore::new(provider, registry));

        let engine = ReactPlusEngine::new(core)
            .with_max_iterations(10)
            .with_reflection_mode(ReflectionMode::Never);
        let messages = Arc::new(vec![Message::user("do something")]);

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx(), None)
            .await
            .unwrap();

        match outcome {
            ReactOutcome::Response {
                traces, iterations, ..
            } => {
                assert_eq!(iterations, 3);
                // Second trace should have a reflection about duplicate detection
                assert!(traces[1].reflection.is_some());
                let refl = traces[1].reflection.as_ref().unwrap();
                assert!(refl.contains("same tool"));
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_different_args_not_flagged_as_duplicate() {
        // Two calls to ok_tool with different arguments should NOT trigger duplicate detection.
        // Since our mock tool ignores args, we need a provider that returns different arg values.
        let responses = vec![
            LlmResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "ok_tool".to_string(),
                    arguments: serde_json::json!({"action": "search"}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            LlmResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "call_2".to_string(),
                    name: "ok_tool".to_string(),
                    arguments: serde_json::json!({"action": "add"}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            make_text_response("All done."),
        ];
        let provider = SequenceProvider::new(responses);
        let registry = make_registry_with_ok();
        let core = Arc::new(ExecutionCore::new(provider, registry));

        let engine = ReactPlusEngine::new(core)
            .with_max_iterations(10)
            .with_reflection_mode(ReflectionMode::Never);
        let messages = Arc::new(vec![Message::user("do two things")]);

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx(), None)
            .await
            .unwrap();

        match outcome {
            ReactOutcome::Response { traces, .. } => {
                // Neither trace should have a duplicate reflection
                assert!(traces[0].reflection.is_none());
                assert!(traces[1].reflection.is_none());
            }
            other => panic!("Expected Response, got {:?}", other),
        }
    }
}
