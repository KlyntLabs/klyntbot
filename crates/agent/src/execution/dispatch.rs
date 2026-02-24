//! Engine dispatch — maps `ExecutionStrategy` to the appropriate engine
//! and handles escalation between engines.
//!
//! Escalation chain: DirectResponse → ToolAssisted (ReAct+) → AutonomousTask (PlanExecute)

use std::sync::Arc;

use common::Result;
use context_engine::ExecutionStrategy;
use providers::{Message, Usage};
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
    /// Accumulated token usage across all engine cycles.
    pub usage: Usage,
    /// Number of iterations the execution engine used.
    pub iterations_used: u32,
}

/// Dispatches execution to the appropriate engine based on strategy,
/// with automatic escalation on engine limits.
pub struct EngineDispatch {
    core: Arc<ExecutionCore>,
    max_escalations: u32,
    /// When present, `AutonomousTask` is handled by the plan-generate engine.
    /// When absent, falls back to `ReactPlusEngine` (preserves test compatibility).
    plan_generate_engine: Option<Arc<super::plan_generate::PlanGenerateEngine>>,
}

impl EngineDispatch {
    pub fn new(core: Arc<ExecutionCore>) -> Self {
        Self {
            core,
            max_escalations: 2,
            plan_generate_engine: None,
        }
    }

    pub fn with_max_escalations(mut self, max: u32) -> Self {
        self.max_escalations = max;
        self
    }

    /// Attach a `PlanGenerateEngine` so that `AutonomousTask` uses plan-based execution.
    pub fn with_plan_generate_engine(
        mut self,
        engine: Arc<super::plan_generate::PlanGenerateEngine>,
    ) -> Self {
        self.plan_generate_engine = Some(engine);
        self
    }

    /// Execute with the given strategy, escalating if an engine signals it
    /// cannot handle the request.
    ///
    /// Accepts `Arc<Vec<Message>>` to avoid deep-cloning the message history
    /// at the dispatch boundary. Engines convert to a mutable `Vec` internally,
    /// using `Arc::try_unwrap` to avoid a clone when the refcount is 1.
    pub async fn execute(
        &self,
        strategy: ExecutionStrategy,
        messages: Arc<Vec<Message>>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
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
                    // Move the Arc — DirectEngine returns messages on escalation,
                    // so we never need the original after this point.
                    let outcome = engine
                        .execute(current_messages, params, ctx, event_tx.as_ref())
                        .await?;

                    match outcome {
                        DirectOutcome::Response(content) => {
                            return Ok(DispatchResult {
                                content,
                                final_strategy: current_strategy,
                                escalation_count,
                                usage: Usage::default(),
                                iterations_used: 1,
                            });
                        }
                        DirectOutcome::EscalateToToolAssisted { messages } => {
                            if escalation_count >= self.max_escalations {
                                return Ok(DispatchResult {
                                    content: String::new(),
                                    final_strategy: current_strategy,
                                    escalation_count,
                                    usage: Usage::default(),
                                    iterations_used: 0,
                                });
                            }
                            debug!("DirectEngine escalated to ToolAssisted");
                            escalation_count += 1;
                            current_messages = Arc::new(messages);
                            current_strategy =
                                ExecutionStrategy::ToolAssisted { max_iterations: 5 };
                            continue;
                        }
                    }
                }

                ExecutionStrategy::ToolAssisted { max_iterations } => {
                    let max_iter = *max_iterations;
                    let engine = ReactPlusEngine::new(self.core.clone())
                        .with_max_iterations(max_iter)
                        .with_reflection_mode(ReflectionMode::OnFailure);

                    // Clone the Arc (O(1)) — we may need current_messages again
                    // if the engine escalates to AutonomousTask.
                    let outcome = engine
                        .execute(
                            current_messages.clone(),
                            tools,
                            params,
                            ctx,
                            event_tx.clone(),
                        )
                        .await?;

                    match outcome {
                        ReactOutcome::Response {
                            content,
                            usage,
                            iterations,
                            ..
                        } => {
                            return Ok(DispatchResult {
                                content,
                                final_strategy: current_strategy,
                                escalation_count,
                                usage,
                                iterations_used: iterations,
                            });
                        }
                        ReactOutcome::EscalateToAutonomous { reason, usage } => {
                            if escalation_count >= self.max_escalations {
                                warn!(
                                    "Escalation limit reached ({}), returning partial",
                                    self.max_escalations
                                );
                                return Ok(DispatchResult {
                                    content: format!("Task exceeded complexity limits: {}", reason),
                                    final_strategy: current_strategy,
                                    escalation_count,
                                    usage,
                                    iterations_used: 0,
                                });
                            }
                            debug!("ReactPlusEngine escalated: {}", reason);
                            escalation_count += 1;
                            current_strategy =
                                ExecutionStrategy::AutonomousTask { max_iterations: 50 };
                            continue;
                        }
                        ReactOutcome::MaxIterationsReached {
                            partial_content,
                            usage,
                        } => {
                            return Ok(DispatchResult {
                                content: partial_content
                                    .unwrap_or_else(|| "Max iterations reached".to_string()),
                                final_strategy: current_strategy,
                                escalation_count,
                                usage,
                                iterations_used: max_iter,
                            });
                        }
                    }
                }

                ExecutionStrategy::AutonomousTask { max_iterations } => {
                    let max_iter = *max_iterations;

                    // Prefer plan-based execution when a PlanGenerateEngine is wired in.
                    if let Some(ref plan_engine) = self.plan_generate_engine {
                        let mut result = plan_engine
                            .execute(current_messages, params, ctx, event_tx.as_ref())
                            .await?;
                        // Override escalation count with the outer dispatch count.
                        result.escalation_count = escalation_count;
                        return Ok(result);
                    }

                    // Fallback: ReactPlus with higher iteration limits (no PlanRepo wired).
                    let engine = ReactPlusEngine::new(self.core.clone())
                        .with_max_iterations(max_iter)
                        .with_reflection_mode(ReflectionMode::EveryN(5));

                    // Move the Arc — AutonomousTask always returns.
                    let outcome = engine
                        .execute(current_messages, tools, params, ctx, event_tx.clone())
                        .await?;

                    let (content, usage, iterations_used) = match outcome {
                        ReactOutcome::Response {
                            content,
                            usage,
                            iterations,
                            ..
                        } => (content, usage, iterations),
                        ReactOutcome::EscalateToAutonomous { reason, usage } => {
                            warn!("Autonomous task hit escalation: {}", reason);
                            (
                                format!("Task complexity exceeded autonomous capacity: {}", reason),
                                usage,
                                0,
                            )
                        }
                        ReactOutcome::MaxIterationsReached {
                            partial_content,
                            usage,
                        } => (
                            partial_content.unwrap_or_else(|| {
                                "Autonomous task hit iteration limit".to_string()
                            }),
                            usage,
                            max_iter,
                        ),
                    };

                    return Ok(DispatchResult {
                        content,
                        final_strategy: current_strategy,
                        escalation_count,
                        usage,
                        iterations_used,
                    });
                }

                ExecutionStrategy::Clarification { reason } => {
                    return Ok(DispatchResult {
                        content: reason.clone(),
                        final_strategy: current_strategy,
                        escalation_count: 0,
                        usage: Usage::default(),
                        iterations_used: 0,
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
                Arc::new(vec![Message::user("what is Rust?")]),
                &[],
                &default_params(),
                &routing_ctx(),
                None,
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
                Arc::new(vec![Message::user("search for Rust tutorials")]),
                &[],
                &default_params(),
                &routing_ctx(),
                None,
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
                Arc::new(vec![Message::user("search")]),
                &[],
                &default_params(),
                &routing_ctx(),
                None,
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
                Arc::new(vec![Message::user("build the project")]),
                &[],
                &default_params(),
                &routing_ctx(),
                None,
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
                Arc::new(vec![Message::user("help me")]),
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.content, "What programming language?");
        assert!(matches!(
            result.final_strategy,
            ExecutionStrategy::Clarification { .. }
        ));
    }

    #[test]
    fn test_dispatch_result_has_iterations_used() {
        let result = DispatchResult {
            content: "hello".to_string(),
            final_strategy: ExecutionStrategy::DirectResponse,
            escalation_count: 0,
            usage: Usage::default(),
            iterations_used: 1,
        };
        assert_eq!(result.iterations_used, 1);
    }

    #[tokio::test]
    async fn test_direct_response_iterations_used_is_one() {
        let provider = SequenceProvider::new(vec![text_response("answer")]);
        let dispatch = EngineDispatch::new(make_core(provider));

        let result = dispatch
            .execute(
                ExecutionStrategy::DirectResponse,
                Arc::new(vec![Message::user("hi")]),
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.iterations_used, 1);
    }

    #[tokio::test]
    async fn test_clarification_iterations_used_is_zero() {
        let provider = SequenceProvider::new(vec![]);
        let dispatch = EngineDispatch::new(make_core(provider));

        let result = dispatch
            .execute(
                ExecutionStrategy::Clarification {
                    reason: "What?".to_string(),
                },
                Arc::new(vec![Message::user("help")]),
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.iterations_used, 0);
    }

    #[tokio::test]
    async fn test_escalated_to_react_carries_iterations() {
        // Direct escalates to ToolAssisted, which returns a response on iteration 1
        let provider = SequenceProvider::new(vec![
            tool_call_response("web_search"),
            text_response("Found it"),
        ]);
        let dispatch = EngineDispatch::new(make_core(provider));

        let result = dispatch
            .execute(
                ExecutionStrategy::DirectResponse,
                Arc::new(vec![Message::user("search")]),
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.escalation_count, 1);
        // ReactPlus returned on iteration 1 (final response)
        assert_eq!(result.iterations_used, 1);
    }
}
