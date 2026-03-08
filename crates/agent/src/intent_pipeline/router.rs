//! ExecutionRouter — dispatches execution to the appropriate engine based on
//! `ExecutionMode`.

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
    /// The final text content.
    pub content: String,
    /// Which execution mode actually produced the result.
    pub final_mode: String,
    /// Accumulated token usage across all engine cycles.
    pub usage: Usage,
    /// Number of iterations the execution engine used.
    pub iterations: u32,
    /// Name of the last tool called. None if no tools called.
    pub tool_name: Option<String>,
    /// Reasoning traces from engine execution.
    pub traces: Vec<crate::execution::ReasoningTrace>,
}

/// Dispatches execution to the appropriate engine based on `ExecutionMode`.
pub struct ExecutionRouter {
    direct: DirectEngine,
    reactive: ReactiveEngine,
}

impl ExecutionRouter {
    pub fn new(direct: DirectEngine, reactive: ReactiveEngine) -> Self {
        Self { direct, reactive }
    }

    /// Execute with the given mode.
    ///
    /// If Direct mode detects tool calls (misclassification), automatically
    /// escalates to Reactive mode with the original messages and tools.
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

        let result = match mode {
            ExecutionMode::Direct => {
                debug!("ExecutionRouter: executing with Direct mode");
                // Clone messages so we can retry with Reactive on escalation.
                let messages_backup = messages.clone();
                let direct_result = self
                    .direct
                    .execute(messages, tools, params, ctx, event_tx.clone())
                    .await?;

                match direct_result {
                    EngineResult::Escalate {
                        usage: direct_usage,
                    } => {
                        debug!("ExecutionRouter: Direct mode escalated to Reactive (tool calls detected)");
                        let reactive_result = self
                            .reactive
                            .execute(messages_backup, tools, params, ctx, event_tx)
                            .await?;
                        match reactive_result {
                            EngineResult::Complete {
                                content,
                                mut usage,
                                iterations,
                                tool_name,
                                traces,
                            } => {
                                // Combine usage from the failed Direct attempt.
                                usage.prompt_tokens += direct_usage.prompt_tokens;
                                usage.completion_tokens += direct_usage.completion_tokens;
                                EngineResult::Complete {
                                    content,
                                    usage,
                                    iterations: iterations + 1,
                                    tool_name,
                                    traces,
                                }
                            }
                            // Escalate from Reactive shouldn't happen, but handle it.
                            other => other,
                        }
                    }
                    complete => complete,
                }
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

        let EngineResult::Complete {
            content,
            usage,
            iterations,
            tool_name,
            traces,
        } = result
        else {
            unreachable!("only Direct can produce Escalate and it is handled above");
        };

        // If escalation happened, the final mode is "reactive" (via escalation).
        let final_mode = if matches!(mode, ExecutionMode::Direct) && iterations > 1 {
            "reactive(escalated)"
        } else {
            mode.short_name()
        };

        Ok(RouterResult {
            content,
            final_mode: final_mode.to_string(),
            usage,
            iterations,
            tool_name,
            traces,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use providers::{ChatParams, LlmProvider, LlmResponse, Usage};
    use serde_json::Value;
    use std::sync::Arc;
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
            _messages: &[providers::Message],
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
        ExecutionRouter::new(direct, reactive)
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
                vec![providers::Message::user("hi")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.final_mode, "direct");
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
                vec![providers::Message::user("do something")],
                &[],
                &default_params(),
                &routing_ctx(),
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.final_mode, "reactive");
        assert!(result.content.contains("Done"));
    }
}
