//! PlanExecute engine — thin adapter over existing PlanExecutor for the strategy dispatch path.

use std::sync::Arc;

use tokio::sync::RwLock;

use common::Result;
use context_engine::AssembledContext;
use providers::DynProvider;
use tools::{registry::ToolRegistry, RoutingContext};

/// Wraps the existing plan execution infrastructure and exposes it
/// through the new engine interface. Full wiring happens in Phase 9.
pub struct PlanExecuteEngine {
    provider: DynProvider,
    tool_registry: Arc<RwLock<ToolRegistry>>,
}

impl PlanExecuteEngine {
    pub fn new(provider: DynProvider, tool_registry: Arc<RwLock<ToolRegistry>>) -> Self {
        Self {
            provider,
            tool_registry,
        }
    }

    /// Execute a plan from the assembled context.
    ///
    /// Currently a stub that returns a delegation message.
    /// Phase 9 will wire this to extract plan_id from context
    /// and delegate to `PlanExecutor::execute_step` / `AgentLoop::run_plan_execution`.
    pub async fn execute(
        &self,
        _context: AssembledContext,
        _ctx: &RoutingContext,
    ) -> Result<String> {
        // Stub: full wiring in Phase 9
        Ok("Plan execution delegated to plan executor.".to_string())
    }

    /// Access the provider (for Phase 9 wiring).
    pub fn provider(&self) -> &DynProvider {
        &self.provider
    }

    /// Access the tool registry (for Phase 9 wiring).
    pub fn tool_registry(&self) -> &Arc<RwLock<ToolRegistry>> {
        &self.tool_registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use context_engine::BudgetReport;
    use providers::{ChatParams, LlmProvider, LlmResponse, Message, Usage};
    use serde_json::Value;
    use std::sync::Mutex;

    // ── Mock provider ────────────────────────────────────────────

    struct MockProvider {
        call_count: Mutex<usize>,
    }

    impl MockProvider {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                call_count: Mutex::new(0),
            })
        }
    }

    #[async_trait]
    impl LlmProvider for MockProvider {
        async fn chat(
            &self,
            _messages: &[Message],
            _tools: Option<&[Value]>,
            _params: &ChatParams,
        ) -> common::Result<LlmResponse> {
            *self.call_count.lock().unwrap() += 1;
            Ok(LlmResponse {
                content: Some("done".to_string()),
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

    fn make_context() -> AssembledContext {
        AssembledContext {
            messages: vec![Message::user("execute plan")],
            token_count: 10,
            budget_report: BudgetReport {
                total_window: 1000,
                total_allocated: 10,
                remaining: 990,
                per_priority: vec![],
            },
        }
    }

    fn routing_ctx() -> RoutingContext {
        RoutingContext::new("test".into(), "test".into())
    }

    // ── Tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_plan_execute_delegates_to_plan_executor() {
        let provider = MockProvider::new();
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let engine = PlanExecuteEngine::new(provider, registry.clone());

        let result = engine.execute(make_context(), &routing_ctx()).await;
        assert!(result.is_ok());

        // Verify engine holds correct tool registry reference
        assert!(Arc::ptr_eq(engine.tool_registry(), &registry));
    }

    #[tokio::test]
    async fn test_plan_execute_returns_completion_message() {
        let provider = MockProvider::new();
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let engine = PlanExecuteEngine::new(provider, registry);

        let result = engine
            .execute(make_context(), &routing_ctx())
            .await
            .unwrap();
        assert!(result.contains("Plan execution delegated"));
        assert!(result.contains("plan executor"));
    }

    #[tokio::test]
    async fn test_plan_execute_returns_ok_not_error() {
        let provider = MockProvider::new();
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let engine = PlanExecuteEngine::new(provider, registry);

        // Even with minimal context, should not error (stub always succeeds)
        let result = engine.execute(make_context(), &routing_ctx()).await;
        assert!(
            result.is_ok(),
            "Stub should not return error: {:?}",
            result.err()
        );
    }
}
