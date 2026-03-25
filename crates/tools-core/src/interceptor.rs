//! Pre-execution tool call interceptors.
//!
//! `ToolCallInterceptor` lets skills inject validation logic that runs
//! after `ToolRegistry::prepare()` but before `tool.execute()`. This turns
//! documented gotchas (e.g. "amounts must be in cents") into runtime guards.

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use std::sync::Arc;

/// Intercepts a tool call before execution.
///
/// Return `Ok(())` to allow the call, or `Err(...)` to block it with an
/// error message that gets returned to the LLM as the tool result.
#[async_trait]
pub trait ToolCallInterceptor: Send + Sync {
    /// Called before `tool.execute()`. Receives tool name, arguments, and
    /// the active skill name (if any).
    async fn before_call(
        &self,
        tool_name: &str,
        args: &Value,
        skill_name: Option<&str>,
    ) -> Result<()>;
}

/// Chains multiple interceptors. All must return `Ok(())` for the call to proceed.
/// First error short-circuits.
pub struct InterceptorChain {
    interceptors: Vec<Arc<dyn ToolCallInterceptor>>,
}

impl InterceptorChain {
    pub fn new() -> Self {
        Self {
            interceptors: Vec::new(),
        }
    }

    pub fn add(&mut self, interceptor: Arc<dyn ToolCallInterceptor>) {
        self.interceptors.push(interceptor);
    }

    pub fn is_empty(&self) -> bool {
        self.interceptors.is_empty()
    }

    /// Run all interceptors in order. First error short-circuits.
    pub async fn check(
        &self,
        tool_name: &str,
        args: &Value,
        skill_name: Option<&str>,
    ) -> Result<()> {
        for interceptor in &self.interceptors {
            interceptor.before_call(tool_name, args, skill_name).await?;
        }
        Ok(())
    }
}

impl Default for InterceptorChain {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::ToolError;
    use serde_json::json;

    struct AllowAll;

    #[async_trait]
    impl ToolCallInterceptor for AllowAll {
        async fn before_call(&self, _: &str, _: &Value, _: Option<&str>) -> Result<()> {
            Ok(())
        }
    }

    struct BlockFinance;

    #[async_trait]
    impl ToolCallInterceptor for BlockFinance {
        async fn before_call(
            &self,
            tool_name: &str,
            _args: &Value,
            _skill: Option<&str>,
        ) -> Result<()> {
            if tool_name == "finance" {
                return Err(ToolError::InvalidParams("blocked by interceptor".into()).into());
            }
            Ok(())
        }
    }

    #[tokio::test]
    async fn empty_chain_allows_everything() {
        let chain = InterceptorChain::new();
        assert!(chain.check("finance", &json!({}), None).await.is_ok());
    }

    #[tokio::test]
    async fn chain_runs_all_interceptors() {
        let mut chain = InterceptorChain::new();
        chain.add(Arc::new(AllowAll));
        chain.add(Arc::new(AllowAll));
        assert!(chain.check("finance", &json!({}), None).await.is_ok());
    }

    #[tokio::test]
    async fn chain_short_circuits_on_error() {
        let mut chain = InterceptorChain::new();
        chain.add(Arc::new(AllowAll));
        chain.add(Arc::new(BlockFinance));
        chain.add(Arc::new(AllowAll)); // should not be reached

        let result = chain.check("finance", &json!({}), None).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("blocked by interceptor"));
    }

    #[tokio::test]
    async fn chain_allows_non_matching_tools() {
        let mut chain = InterceptorChain::new();
        chain.add(Arc::new(BlockFinance));
        assert!(chain.check("tasks", &json!({}), None).await.is_ok());
    }
}
