//! Engine dispatch — maps `ExecutionStrategy` to the appropriate engine
//! and handles escalation between engines.
//!
//! Escalation chain: DirectResponse → ToolAssisted (ReAct+) → AutonomousTask (PlanExecute)

use std::sync::Arc;

use common::Result;
use context_engine::ExecutionStrategy;
use providers::Message;
use tools::RoutingContext;
use tracing::{debug, warn};

use super::core::ExecutionCore;
use super::direct::{DirectEngine, DirectOutcome};
use super::react_plus::{ReactOutcome, ReactPlusEngine, ReflectionMode};
use super::types::ExecutionParams;

/// Unified result from any engine execution.
#[derive(Debug)]
pub struct DispatchResult {
    /// The final text content.
    pub content: String,
    /// Which strategy actually produced the result.
    pub final_strategy: ExecutionStrategy,
    /// Number of escalations that occurred.
    pub escalation_count: u32,
}

/// Dispatches execution to the appropriate engine based on strategy,
/// with automatic escalation on engine limits.
pub struct EngineDispatch {
    core: Arc<ExecutionCore>,
    max_escalations: u32,
}

impl EngineDispatch {
    pub fn new(core: Arc<ExecutionCore>) -> Self {
        Self {
            core,
            max_escalations: 2,
        }
    }

    pub fn with_max_escalations(mut self, max: u32) -> Self {
        self.max_escalations = max;
        self
    }

    /// Execute with the given strategy, escalating if an engine signals it
    /// cannot handle the request.
    pub async fn execute(
        &self,
        strategy: ExecutionStrategy,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
    ) -> Result<DispatchResult> {
        let mut current_strategy = strategy;
        let mut current_messages = messages;
        let mut escalation_count = 0u32;

        loop {
            debug!(
                "EngineDispatch: executing with strategy {:?} (escalation {})",
                current_strategy, escalation_count
            );

            match &current_strategy {
                ExecutionStrategy::DirectResponse => {
                    let engine = DirectEngine::new(self.core.clone());
                    let outcome = engine
                        .execute(current_messages.clone(), params, ctx)
                        .await?;

                    match outcome {
                        DirectOutcome::Response(content) => {
                            return Ok(DispatchResult {
                                content,
                                final_strategy: current_strategy,
                                escalation_count,
                            });
                        }
                        DirectOutcome::EscalateToToolAssisted { messages } => {
                            if escalation_count >= self.max_escalations {
                                return Ok(DispatchResult {
                                    content: String::new(),
                                    final_strategy: current_strategy,
                                    escalation_count,
                                });
                            }
                            debug!("DirectEngine escalated to ToolAssisted");
                            escalation_count += 1;
                            current_messages = messages;
                            current_strategy = ExecutionStrategy::ToolAssisted {
                                max_iterations: 5,
                            };
                            continue;
                        }
                    }
                }

                ExecutionStrategy::ToolAssisted { max_iterations } => {
                    let engine = ReactPlusEngine::new(self.core.clone())
                        .with_max_iterations(*max_iterations)
                        .with_reflection_mode(ReflectionMode::OnFailure);

                    let outcome = engine
                        .execute(current_messages.clone(), tools, params, ctx)
                        .await?;

                    match outcome {
                        ReactOutcome::Response { content, .. } => {
                            return Ok(DispatchResult {
                                content,
                                final_strategy: current_strategy,
                                escalation_count,
                            });
                        }
                        ReactOutcome::EscalateToAutonomous { reason } => {
                            if escalation_count >= self.max_escalations {
                                warn!(
                                    "Escalation limit reached ({}), returning partial",
                                    self.max_escalations
                                );
                                return Ok(DispatchResult {
                                    content: format!(
                                        "Task exceeded complexity limits: {}",
                                        reason
                                    ),
                                    final_strategy: current_strategy,
                                    escalation_count,
                                });
                            }
                            debug!("ReactPlusEngine escalated: {}", reason);
                            escalation_count += 1;
                            current_strategy = ExecutionStrategy::AutonomousTask {
                                max_iterations: 50,
                            };
                            continue;
                        }
                        ReactOutcome::MaxIterationsReached { partial_content } => {
                            return Ok(DispatchResult {
                                content: partial_content
                                    .unwrap_or_else(|| "Max iterations reached".to_string()),
                                final_strategy: current_strategy,
                                escalation_count,
                            });
                        }
                    }
                }

                ExecutionStrategy::AutonomousTask { max_iterations } => {
                    // For autonomous tasks, we use ReactPlus with higher iteration limits.
                    // Full PlanExecute integration requires plan steps from a planner —
                    // that's wired up by the orchestrator layer above dispatch.
                    let engine = ReactPlusEngine::new(self.core.clone())
                        .with_max_iterations(*max_iterations)
                        .with_reflection_mode(ReflectionMode::EveryN(5));

                    let outcome = engine
                        .execute(current_messages.clone(), tools, params, ctx)
                        .await?;

                    let content = match outcome {
                        ReactOutcome::Response { content, .. } => content,
                        ReactOutcome::EscalateToAutonomous { reason } => {
                            warn!("Autonomous task hit escalation: {}", reason);
                            format!("Task complexity exceeded autonomous capacity: {}", reason)
                        }
                        ReactOutcome::MaxIterationsReached { partial_content } => {
                            partial_content
                                .unwrap_or_else(|| "Autonomous task hit iteration limit".to_string())
                        }
                    };

                    return Ok(DispatchResult {
                        content,
                        final_strategy: current_strategy,
                        escalation_count,
                    });
                }

                ExecutionStrategy::Clarification { reason } => {
                    return Ok(DispatchResult {
                        content: reason.clone(),
                        final_strategy: current_strategy,
                        escalation_count: 0,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, LlmProvider, LlmResponse, ToolCall, Usage};
    use serde_json::Value;
    use std::sync::Mutex;
    use tokio::sync::RwLock;
    use tools::registry::ToolRegistry;

    // ── Mock Provider ──

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
                    content: Some("fallback".to_string()),
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

    // ── Helpers ──

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

    fn make_core(provider: Arc<dyn LlmProvider>) -> Arc<ExecutionCore> {
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        Arc::new(ExecutionCore::new(provider, registry))
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    fn default_params() -> ExecutionParams {
        ExecutionParams::new("mock")
    }

    // ── Tests ──

    #[tokio::test]
    async fn test_direct_strategy_calls_direct_engine() {
        let provider = SequenceProvider::new(vec![text_response("Direct answer")]);
        let dispatch = EngineDispatch::new(make_core(provider));

        let result = dispatch
            .execute(
                ExecutionStrategy::DirectResponse,
                vec![Message::user("what is Rust?")],
                &[],
                &default_params(),
                &routing_ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.content, "Direct answer");
        assert!(matches!(
            result.final_strategy,
            ExecutionStrategy::DirectResponse
        ));
        assert_eq!(result.escalation_count, 0);
    }

    #[tokio::test]
    async fn test_escalation_from_direct_to_react() {
        // First call: LLM wants tools (DirectEngine escalates)
        // Second call: LLM returns text (ReactPlus produces final response)
        let provider = SequenceProvider::new(vec![
            tool_call_response("web_search"),
            text_response("Found the answer after searching"),
        ]);
        let dispatch = EngineDispatch::new(make_core(provider));

        let result = dispatch
            .execute(
                ExecutionStrategy::DirectResponse,
                vec![Message::user("search for Rust tutorials")],
                &[],
                &default_params(),
                &routing_ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.escalation_count, 1);
        assert!(matches!(
            result.final_strategy,
            ExecutionStrategy::ToolAssisted { .. }
        ));
        assert!(result.content.contains("Found the answer"));
    }

    #[tokio::test]
    async fn test_escalation_limit_prevents_infinite_loop() {
        // With max_escalations=0, no escalation from Direct
        let provider = SequenceProvider::new(vec![tool_call_response("web_search")]);
        let dispatch = EngineDispatch::new(make_core(provider)).with_max_escalations(0);

        let result = dispatch
            .execute(
                ExecutionStrategy::DirectResponse,
                vec![Message::user("search")],
                &[],
                &default_params(),
                &routing_ctx(),
            )
            .await
            .unwrap();

        // Should NOT have escalated
        assert_eq!(result.escalation_count, 0);
        assert!(matches!(
            result.final_strategy,
            ExecutionStrategy::DirectResponse
        ));
    }

    #[tokio::test]
    async fn test_autonomous_strategy_uses_react_with_high_iterations() {
        let provider = SequenceProvider::new(vec![text_response("Autonomous result")]);
        let dispatch = EngineDispatch::new(make_core(provider));

        let result = dispatch
            .execute(
                ExecutionStrategy::AutonomousTask { max_iterations: 50 },
                vec![Message::user("build the project")],
                &[],
                &default_params(),
                &routing_ctx(),
            )
            .await
            .unwrap();

        assert!(matches!(
            result.final_strategy,
            ExecutionStrategy::AutonomousTask { .. }
        ));
        assert_eq!(result.content, "Autonomous result");
        assert_eq!(result.escalation_count, 0);
    }

    #[tokio::test]
    async fn test_clarification_returns_reason_directly() {
        let provider = SequenceProvider::new(vec![]);
        let dispatch = EngineDispatch::new(make_core(provider));

        let result = dispatch
            .execute(
                ExecutionStrategy::Clarification {
                    reason: "What programming language?".to_string(),
                },
                vec![Message::user("help me")],
                &[],
                &default_params(),
                &routing_ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.content, "What programming language?");
        assert!(matches!(
            result.final_strategy,
            ExecutionStrategy::Clarification { .. }
        ));
    }
}
