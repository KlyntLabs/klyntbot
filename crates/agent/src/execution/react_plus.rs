//! ReAct+ execution engine — enhanced ReAct loop with reasoning scratchpad,
//! reflection checkpoints, and escalation to autonomous task execution.

use std::sync::Arc;

use chrono::Utc;

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
    pub async fn execute(
        &self,
        mut messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
    ) -> Result<ReactOutcome> {
        let mut scratchpad = Scratchpad::new();
        let escalation_threshold = (self.max_iterations as f32 * 0.8).ceil() as u32;
        let mut accumulated_usage = Usage::default();

        for iteration in 1..=self.max_iterations {
            let (outcome, cycle_usage) = self
                .core
                .run_cycle(&mut messages, tools, params, ctx)
                .await?;
            accumulate_usage(&mut accumulated_usage, &cycle_usage);

            match outcome {
                CycleOutcome::FinalResponse { content }
                | CycleOutcome::FabricatedResponse { content } => {
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
                    });
                }

                CycleOutcome::ToolsExecuted { results } => {
                    let tool_names: Vec<String> =
                        results.iter().map(|r| r.tool_name.clone()).collect();
                    let had_failure = results.iter().any(|r| !r.success);
                    let failure_details: Vec<String> = results
                        .iter()
                        .filter(|r| !r.success)
                        .map(|r| format!("{}: {}", r.tool_name, r.result))
                        .collect();

                    let mut reflection = None;

                    // Check if reflection should trigger
                    let should_reflect = match &self.reflection_mode {
                        ReflectionMode::OnFailure => had_failure,
                        ReflectionMode::EveryN(n) => iteration % n == 0,
                        ReflectionMode::Never => false,
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
        let messages = vec![Message::user("do something")];

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx())
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

        let messages = vec![Message::user("do something")];
        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx())
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
        let messages = vec![Message::user("complex task")];

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx())
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
        let messages = vec![Message::user("multi-step task")];

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx())
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
        let messages = vec![Message::user("try something")];

        let outcome = engine
            .execute(messages, &[], &default_params(), &routing_ctx())
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
}
