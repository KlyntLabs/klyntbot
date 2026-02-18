//! ReAct+ Engine — ReAct loop with reasoning scratchpad and reflection.
//!
//! Drives tool-assisted execution by running `ExecutionCore::run_cycle()` in a loop,
//! recording reasoning traces in a `Scratchpad`, and returning the final response
//! or a scratchpad summary when max iterations are reached.

use chrono::Utc;

use common::Result;
use context_engine::ExecutionStrategy;
use providers::Message;
use tools::RoutingContext;

use crate::execution::{CycleOutcome, ExecutionCore, ExecutionParams, ReasoningTrace, Scratchpad};

/// ReAct+ engine for tool-assisted tasks.
///
/// Wraps an `ExecutionCore` and drives it through multiple LLM-tool cycles,
/// recording each cycle's reasoning in a `Scratchpad`.
pub struct ReactEngine {
    core: ExecutionCore,
}

impl ReactEngine {
    pub fn new(core: ExecutionCore) -> Self {
        Self { core }
    }

    /// Execute a ReAct loop: cycle through LLM→tool rounds until a final
    /// response is produced or `max_iterations` is reached.
    pub async fn execute(
        &self,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        strategy: &ExecutionStrategy,
        ctx: &RoutingContext,
    ) -> Result<ReactResult> {
        let mut scratchpad = Scratchpad::new();
        let mut messages = messages;

        let max_iterations = match strategy {
            ExecutionStrategy::ToolAssisted { max_iterations } => *max_iterations,
            ExecutionStrategy::AutonomousTask { max_iterations } => *max_iterations,
            _ => 5,
        };

        for cycle in 0..max_iterations {
            let (outcome, _cycle_usage) = self
                .core
                .run_cycle(&mut messages, tools, params, ctx)
                .await?;

            match outcome {
                CycleOutcome::FinalResponse { content } => {
                    scratchpad.add(ReasoningTrace {
                        cycle,
                        thought: "Final response generated".to_string(),
                        planned_actions: vec![],
                        actual_action: "respond".to_string(),
                        reflection: None,
                        timestamp: Utc::now(),
                    });
                    return Ok(ReactResult {
                        content,
                        scratchpad,
                        cycles_used: cycle + 1,
                        hit_max_iterations: false,
                    });
                }
                CycleOutcome::ToolsExecuted { results } => {
                    let tool_names: Vec<String> =
                        results.iter().map(|r| r.tool_name.clone()).collect();
                    let summary: Vec<String> = results
                        .iter()
                        .map(|r| {
                            if r.success {
                                format!("{}:ok", r.tool_name)
                            } else {
                                format!("{}:err", r.tool_name)
                            }
                        })
                        .collect();

                    scratchpad.add(ReasoningTrace {
                        cycle,
                        thought: format!("Executing tools: {}", tool_names.join(", ")),
                        planned_actions: tool_names,
                        actual_action: summary.join(", "),
                        reflection: None,
                        timestamp: Utc::now(),
                    });
                }
                CycleOutcome::EmptyResponse => {
                    scratchpad.add(ReasoningTrace {
                        cycle,
                        thought: "Empty response received".to_string(),
                        planned_actions: vec![],
                        actual_action: "empty".to_string(),
                        reflection: Some("No content or tool calls — stopping".to_string()),
                        timestamp: Utc::now(),
                    });
                    return Ok(ReactResult {
                        content: scratchpad.summarize(),
                        scratchpad,
                        cycles_used: cycle + 1,
                        hit_max_iterations: false,
                    });
                }
            }
        }

        // Max iterations reached
        Ok(ReactResult {
            content: scratchpad.summarize(),
            scratchpad,
            cycles_used: max_iterations,
            hit_max_iterations: true,
        })
    }
}

/// Result of a ReAct engine execution.
pub struct ReactResult {
    /// The final text content (LLM response or scratchpad summary).
    pub content: String,
    /// The scratchpad with all reasoning traces.
    pub scratchpad: Scratchpad,
    /// Number of cycles actually used.
    pub cycles_used: u32,
    /// Whether max iterations were hit without a final response.
    pub hit_max_iterations: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, LlmProvider, LlmResponse, ToolCall, Usage};
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::sync::RwLock;
    use tools::{registry::ToolRegistry, Tool};

    // ── Mock Provider ──────────────────────────────────────────

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
                    content: Some(String::new()),
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

    fn text_response(text: &str) -> LlmResponse {
        LlmResponse {
            content: Some(text.to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }
    }

    fn tool_call_response(name: &str) -> LlmResponse {
        LlmResponse {
            content: None,
            tool_calls: vec![ToolCall {
                id: format!("call_{}", name),
                name: name.to_string(),
                arguments: serde_json::json!({}),
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }
    }

    fn empty_response() -> LlmResponse {
        LlmResponse {
            content: Some(String::new()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }
    }

    // ── Mock Tool ──────────────────────────────────────────────

    struct CountingTool {
        call_count: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl Tool for CountingTool {
        fn name(&self) -> &str {
            "counter"
        }
        fn description(&self) -> &str {
            "Counts calls"
        }
        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }
        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> common::Result<String> {
            let n = self.call_count.fetch_add(1, Ordering::SeqCst) + 1;
            Ok(format!("call #{}", n))
        }
    }

    // ── Helpers ────────────────────────────────────────────────

    fn make_registry(tool: impl Tool + 'static) -> Arc<RwLock<ToolRegistry>> {
        let mut reg = ToolRegistry::new();
        reg.register(tool);
        Arc::new(RwLock::new(reg))
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    fn tool_assisted(max: u32) -> ExecutionStrategy {
        ExecutionStrategy::ToolAssisted {
            max_iterations: max,
        }
    }

    // ── Tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_single_cycle_no_tools_returns_final() {
        let provider = SequenceProvider::new(vec![text_response("Hello!")]);
        let registry = make_registry(CountingTool {
            call_count: Arc::new(AtomicUsize::new(0)),
        });
        let core = ExecutionCore::new(provider, registry);
        let engine = ReactEngine::new(core);

        let result = engine
            .execute(
                vec![Message::user("hi")],
                &[],
                &ExecutionParams::new("mock"),
                &tool_assisted(5),
                &routing_ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.content, "Hello!");
        assert_eq!(result.cycles_used, 1);
        assert!(!result.hit_max_iterations);
        assert_eq!(result.scratchpad.len(), 1);
    }

    #[tokio::test]
    async fn test_tool_cycle_then_final() {
        // Cycle 1: tool call → Cycle 2: final response
        let provider = SequenceProvider::new(vec![
            tool_call_response("counter"),
            text_response("Done using tools"),
        ]);
        let tool_count = Arc::new(AtomicUsize::new(0));
        let registry = make_registry(CountingTool {
            call_count: tool_count.clone(),
        });
        let core = ExecutionCore::new(provider, registry);
        let engine = ReactEngine::new(core);

        let result = engine
            .execute(
                vec![Message::user("use a tool")],
                &[],
                &ExecutionParams::new("mock"),
                &tool_assisted(5),
                &routing_ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.content, "Done using tools");
        assert_eq!(result.cycles_used, 2);
        assert!(!result.hit_max_iterations);
        assert_eq!(tool_count.load(Ordering::SeqCst), 1);
        // Scratchpad: 1 tool trace + 1 final trace
        assert_eq!(result.scratchpad.len(), 2);
    }

    #[tokio::test]
    async fn test_max_iterations_respected() {
        // Provider always returns tool calls — should stop at max_iterations
        let provider = SequenceProvider::new(vec![
            tool_call_response("counter"),
            tool_call_response("counter"),
            tool_call_response("counter"),
            tool_call_response("counter"),
            tool_call_response("counter"),
        ]);
        let tool_count = Arc::new(AtomicUsize::new(0));
        let registry = make_registry(CountingTool {
            call_count: tool_count.clone(),
        });
        let core = ExecutionCore::new(provider, registry);
        let engine = ReactEngine::new(core);

        let result = engine
            .execute(
                vec![Message::user("loop")],
                &[],
                &ExecutionParams::new("mock"),
                &tool_assisted(3),
                &routing_ctx(),
            )
            .await
            .unwrap();

        assert!(result.hit_max_iterations);
        assert_eq!(result.cycles_used, 3);
        assert_eq!(tool_count.load(Ordering::SeqCst), 3);
        // Content should be the scratchpad summary
        assert!(result.content.contains("3 cycles"));
    }

    #[tokio::test]
    async fn test_scratchpad_records_cycles() {
        let provider = SequenceProvider::new(vec![
            tool_call_response("counter"),
            tool_call_response("counter"),
            text_response("final"),
        ]);
        let registry = make_registry(CountingTool {
            call_count: Arc::new(AtomicUsize::new(0)),
        });
        let core = ExecutionCore::new(provider, registry);
        let engine = ReactEngine::new(core);

        let result = engine
            .execute(
                vec![Message::user("go")],
                &[],
                &ExecutionParams::new("mock"),
                &tool_assisted(10),
                &routing_ctx(),
            )
            .await
            .unwrap();

        // 2 tool cycles + 1 final = 3 traces
        assert_eq!(result.scratchpad.len(), 3);

        let traces = result.scratchpad.traces();
        assert_eq!(traces[0].cycle, 0);
        assert!(traces[0].thought.contains("counter"));
        assert_eq!(traces[1].cycle, 1);
        assert_eq!(traces[2].cycle, 2);
        assert_eq!(traces[2].actual_action, "respond");
    }

    #[tokio::test]
    async fn test_empty_cycle_stops_loop() {
        let provider = SequenceProvider::new(vec![empty_response()]);
        let registry = make_registry(CountingTool {
            call_count: Arc::new(AtomicUsize::new(0)),
        });
        let core = ExecutionCore::new(provider, registry);
        let engine = ReactEngine::new(core);

        let result = engine
            .execute(
                vec![Message::user("empty")],
                &[],
                &ExecutionParams::new("mock"),
                &tool_assisted(10),
                &routing_ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.cycles_used, 1);
        assert!(!result.hit_max_iterations);
        assert_eq!(result.scratchpad.len(), 1);
        assert!(result.scratchpad.traces()[0]
            .reflection
            .as_ref()
            .unwrap()
            .contains("stopping"));
    }
}
