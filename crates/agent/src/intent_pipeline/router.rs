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

        let EngineResult::Complete {
            content,
            usage,
            iterations,
            tool_name,
            traces,
        } = result;

        Ok(RouterResult {
            content,
            final_mode: mode_name.to_string(),
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
