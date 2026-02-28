//! ExecutionRouter — maps `ExecutionMode` to the appropriate engine and
//! handles escalation between engines.
//!
//! Escalation chain: Direct → Reactive → Planned
//! Each escalation carries full context via `EscalationContext` so no work
//! is repeated.

use common::Result;
use providers::{Message, Usage};
use tools::RoutingContext;
use tracing::{debug, warn};

use super::engines::direct::DirectEngine;
use super::engines::planned::PlannedEngine;
use super::engines::reactive::ReactiveEngine;
use super::engines::EngineResult;
use super::types::ExecutionMode;
use crate::execution::ExecutionParams;

/// Result from the execution router.
#[derive(Debug)]
pub struct RouterResult {
    /// The final text content.
    pub content: String,
    /// Which execution mode actually produced the result.
    pub final_mode: String,
    /// Number of escalations that occurred.
    pub escalation_count: u32,
    /// Accumulated token usage across all engine cycles.
    pub usage: Usage,
    /// Number of iterations the execution engine used.
    pub iterations: u32,
    /// Name of the last tool called. None if no tools called.
    pub tool_name: Option<String>,
}

/// Dispatches execution to the appropriate engine based on `ExecutionMode`,
/// with automatic escalation on engine limits.
pub struct ExecutionRouter {
    direct: DirectEngine,
    reactive: ReactiveEngine,
    planned: Option<PlannedEngine>,
    max_escalations: u32,
}

impl ExecutionRouter {
    pub fn new(
        direct: DirectEngine,
        reactive: ReactiveEngine,
        planned: Option<PlannedEngine>,
        max_escalations: u32,
    ) -> Self {
        Self {
            direct,
            reactive,
            planned,
            max_escalations,
        }
    }

    /// Execute with the given mode, escalating if an engine signals it
    /// cannot handle the request.
    pub async fn execute(
        &self,
        mode: ExecutionMode,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> Result<RouterResult> {
        use super::engines::ExecutionEngine;

        let mut escalation_count = 0u32;
        let initial_mode_name = mode.short_name();

        // Phase 1: Execute with initial mode (move messages — not used after this)
        let initial_result = match mode {
            ExecutionMode::Direct => {
                debug!("ExecutionRouter: starting with Direct mode");
                self.direct
                    .execute(messages, tools, params, ctx, event_tx.clone())
                    .await?
            }
            ExecutionMode::Reactive { max_iterations } => {
                debug!(
                    "ExecutionRouter: starting with Reactive mode (max_iterations={})",
                    max_iterations
                );
                self.reactive
                    .execute(messages, tools, params, ctx, event_tx.clone())
                    .await?
            }
            ExecutionMode::Planned { ref visibility, .. } => {
                debug!("ExecutionRouter: starting with Planned mode");
                if let Some(ref planned) = self.planned {
                    planned
                        .execute_with_visibility(
                            messages,
                            tools,
                            params,
                            ctx,
                            event_tx.clone(),
                            visibility.clone(),
                        )
                        .await?
                } else {
                    warn!("ExecutionRouter: Planned mode requested but no PlannedEngine configured, falling back to Reactive");
                    self.reactive
                        .execute(messages, tools, params, ctx, event_tx.clone())
                        .await?
                }
            }
        };

        // Phase 2: Handle escalation chain
        let mut current_result = initial_result;
        let mut current_mode = initial_mode_name;

        loop {
            match current_result {
                EngineResult::Complete {
                    content,
                    usage,
                    iterations,
                    tool_name,
                    ..
                } => {
                    return Ok(RouterResult {
                        content,
                        final_mode: current_mode.to_string(),
                        escalation_count,
                        usage,
                        iterations,
                        tool_name,
                    });
                }
                EngineResult::Escalate {
                    reason,
                    carried_context,
                    usage: escalation_usage,
                } => {
                    escalation_count += 1;

                    if escalation_count > self.max_escalations {
                        warn!(
                            "ExecutionRouter: max escalations ({}) reached, returning last context",
                            self.max_escalations
                        );
                        return Ok(RouterResult {
                            content: format!(
                                "Task exceeded maximum escalation limit ({}). Last reason: {}",
                                self.max_escalations, reason
                            ),
                            final_mode: current_mode.to_string(),
                            escalation_count,
                            usage: escalation_usage,
                            iterations: 0,
                            tool_name: None,
                        });
                    }

                    debug!(
                        "ExecutionRouter: escalation {} from {} — {}",
                        escalation_count, current_mode, reason
                    );

                    // Determine next mode in escalation chain
                    match current_mode {
                        "direct" => {
                            // Direct → Reactive
                            current_mode = "reactive";
                            current_result = self
                                .reactive
                                .execute(
                                    carried_context.messages,
                                    tools,
                                    params,
                                    ctx,
                                    event_tx.clone(),
                                )
                                .await?;
                        }
                        "reactive" => {
                            // Reactive → Planned (with prior work)
                            if let Some(ref planned) = self.planned {
                                current_mode = "planned";
                                current_result = planned
                                    .execute_with_prior_work(
                                        carried_context,
                                        tools,
                                        params,
                                        ctx,
                                        event_tx.clone(),
                                    )
                                    .await?;
                            } else {
                                // No planned engine — return what we have
                                warn!("ExecutionRouter: Reactive escalated but no PlannedEngine configured");
                                return Ok(RouterResult {
                                    content: format!("Task needs planning but planning is not configured. Reason: {}", reason),
                                    final_mode: "reactive".to_string(),
                                    escalation_count,
                                    usage: escalation_usage,
                                    iterations: 0,
                                    tool_name: None,
                                });
                            }
                        }
                        "planned" => {
                            // Planned — no further escalation possible
                            warn!("ExecutionRouter: cannot escalate beyond planned mode");
                            return Ok(RouterResult {
                                content: format!(
                                    "Task could not be completed in planned mode. Reason: {}",
                                    reason
                                ),
                                final_mode: current_mode.to_string(),
                                escalation_count,
                                usage: escalation_usage,
                                iterations: 0,
                                tool_name: None,
                            });
                        }
                        unknown => {
                            warn!(
                                "ExecutionRouter: unknown mode '{}', cannot escalate",
                                unknown
                            );
                            return Ok(RouterResult {
                                content: format!(
                                    "Task could not be completed in {} mode. Reason: {}",
                                    unknown, reason
                                ),
                                final_mode: current_mode.to_string(),
                                escalation_count,
                                usage: escalation_usage,
                                iterations: 0,
                                tool_name: None,
                            });
                        }
                    }
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
    use std::sync::{Arc, Mutex};
    use tokio::sync::RwLock;
    use tools::registry::ToolRegistry;

    use crate::execution::ExecutionCore;

    // ── Mock providers ──────────────────────────────────────────

    /// Provider that returns a text response (Direct mode completes).
    struct TextProvider {
        text: String,
    }

    #[async_trait]
    impl LlmProvider for TextProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> Result<LlmResponse> {
            Ok(LlmResponse {
                content: Some(self.text.clone()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            })
        }
        fn default_model(&self) -> &str {
            "mock"
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    /// Provider with a sequence of responses for testing escalation chains.
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
        ) -> Result<LlmResponse> {
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

    // ── Helpers ──────────────────────────────────────────────────

    fn make_registry() -> Arc<RwLock<ToolRegistry>> {
        Arc::new(RwLock::new(ToolRegistry::new()))
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    fn default_params() -> ExecutionParams {
        ExecutionParams::new("mock")
    }

    fn make_router_with_provider(provider: providers::DynProvider) -> ExecutionRouter {
        let registry = make_registry();
        let core = Arc::new(ExecutionCore::new(provider, registry));
        let direct = DirectEngine::new(core.clone());
        let reactive = ReactiveEngine::new(core, 10);
        ExecutionRouter::new(direct, reactive, None, 2)
    }

    // ── Tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn routes_direct_to_direct_engine() {
        let provider: providers::DynProvider = Arc::new(TextProvider {
            text: "Hello!".to_string(),
        });
        let router = make_router_with_provider(provider);

        let result = router
            .execute(
                ExecutionMode::Direct,
                vec![Message::user("hi")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.final_mode, "direct");
        assert_eq!(result.escalation_count, 0);
        assert_eq!(result.content, "Hello!");
    }

    #[tokio::test]
    async fn routes_reactive_to_reactive_engine() {
        let provider: providers::DynProvider = Arc::new(TextProvider {
            text: "Done!".to_string(),
        });
        let router = make_router_with_provider(provider);

        let result = router
            .execute(
                ExecutionMode::Reactive { max_iterations: 10 },
                vec![Message::user("do something")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.final_mode, "reactive");
        assert_eq!(result.escalation_count, 0);
        assert!(result.content.contains("Done"));
    }

    #[tokio::test]
    async fn handles_escalation_from_direct_to_reactive() {
        // ToolCallProvider makes Direct escalate (tool calls in direct mode).
        // After escalation to Reactive, the tool isn't registered so the
        // "web_search" call fails. Reactive then gets a text fallback.
        let provider = SequenceProvider::new(vec![
            // Direct mode: returns tool call → escalates
            LlmResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "call_1".to_string(),
                    name: "web_search".to_string(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            // Reactive mode: returns text response → completes
            LlmResponse {
                content: Some("Handled reactively".to_string()),
                tool_calls: vec![],
                finish_reason: "stop".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
        ]);

        let router = make_router_with_provider(provider);

        let result = router
            .execute(
                ExecutionMode::Direct,
                vec![Message::user("search for something")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.final_mode, "reactive");
        assert_eq!(result.escalation_count, 1);
        assert!(result.content.contains("Handled reactively"));
    }

    #[tokio::test]
    async fn respects_max_escalation_limit() {
        // Provider always returns tool calls → Direct escalates to Reactive,
        // Reactive also escalates (no planned engine) → hits max.
        // With max_escalations=1, only one escalation is allowed.
        let provider = SequenceProvider::new(vec![
            // Direct: tool call → escalate to reactive
            LlmResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "c1".to_string(),
                    name: "tool".to_string(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            // Reactive iteration 1: tool call (will be "executed" but tool not found)
            LlmResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "c2".to_string(),
                    name: "tool".to_string(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            // Reactive keeps going with tool calls until escalation threshold
            LlmResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "c3".to_string(),
                    name: "tool2".to_string(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            LlmResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "c4".to_string(),
                    name: "tool3".to_string(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
            LlmResponse {
                content: None,
                tool_calls: vec![ToolCall {
                    id: "c5".to_string(),
                    name: "tool4".to_string(),
                    arguments: serde_json::json!({}),
                }],
                finish_reason: "tool_calls".to_string(),
                usage: Usage::default(),
                reasoning_content: None,
            },
        ]);

        let registry = make_registry();
        let core = Arc::new(ExecutionCore::new(provider, registry));
        let direct = DirectEngine::new(core.clone());
        // Use max_iterations=5 so escalation happens at iteration 4
        let reactive = ReactiveEngine::new(core, 5);
        let router = ExecutionRouter::new(direct, reactive, None, 1);

        let result = router
            .execute(
                ExecutionMode::Direct,
                vec![Message::user("complex task")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        // Should have escalated once (Direct → Reactive) then Reactive escalated
        // but hit the max_escalations=1 limit
        assert!(
            result.escalation_count <= 2,
            "escalation count should be bounded"
        );
        assert!(
            result.content.contains("exceeded")
                || result.content.contains("not configured")
                || result.content.contains("needs planning"),
            "should indicate limit or no planned engine: {}",
            result.content
        );
    }
}
