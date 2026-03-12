//! context_request tool — allows the agent to request additional context mid-execution.
//!
//! Defined in Layer 4 (tools). The handler is implemented by the agent runtime (Layer 5)
//! which has access to `ContextEngine` and the current `AssembledContext`.
//! Follows the dependency inversion pattern used by LearningTool, DelegationTool, etc.

use async_trait::async_trait;
use common::Result;
use serde_json::Value;
use std::sync::Arc;

use crate::{RoutingContext, Tool};
use common::ToolError;

/// Handler trait for context expansion.
/// Implemented by the agent runtime which holds `ContextEngine` + current `AssembledContext`.
#[async_trait]
pub trait ContextExpansionHandler: Send + Sync {
    /// Expand context by loading a deferred source.
    /// Returns a human-readable summary of what was loaded.
    async fn expand_context(&self, source_name: &str, query: Option<&str>) -> Result<String>;

    /// List available context sources and their status.
    async fn list_available(&self) -> Result<String>;
}

/// Tool for requesting additional context mid-execution.
pub struct ContextRequestTool {
    handler: Option<Arc<dyn ContextExpansionHandler>>,
}

impl ContextRequestTool {
    pub fn new(handler: Option<Arc<dyn ContextExpansionHandler>>) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl Tool for ContextRequestTool {
    fn name(&self) -> &str {
        "context_request"
    }

    fn description(&self) -> &str {
        "Request additional context mid-execution. Use when you need more information \
         from a specific context source (e.g., project details, additional memories, \
         user history) to complete the current task. Use action 'list' to see available \
         sources, or 'load' to load a specific source."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["load", "list"],
                    "description": "Action to perform: 'load' a specific source, or 'list' available sources."
                },
                "source": {
                    "type": "string",
                    "description": "Name of the context source to load (from the inventory list). Required for 'load' action."
                },
                "query": {
                    "type": "string",
                    "description": "Optional query to filter the context (e.g., for memory retrieval)."
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let handler = self
            .handler
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionFailed("Context expansion not available".into()))?;

        let action = args.get("action").and_then(|v| v.as_str()).ok_or_else(|| {
            ToolError::InvalidParams("Missing required 'action' parameter".into())
        })?;

        match action {
            "load" => {
                let source = args.get("source").and_then(|v| v.as_str()).ok_or_else(|| {
                    ToolError::InvalidParams(
                        "Missing required 'source' parameter for load action".into(),
                    )
                })?;
                let query = args.get("query").and_then(|v| v.as_str());
                handler.expand_context(source, query).await
            }
            "list" => handler.list_available().await,
            other => Err(ToolError::InvalidParams(format!(
                "Unknown action '{}'. Use 'load' or 'list'.",
                other
            ))
            .into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHandler;

    #[async_trait]
    impl ContextExpansionHandler for MockHandler {
        async fn expand_context(&self, source_name: &str, _query: Option<&str>) -> Result<String> {
            Ok(format!("Loaded context from '{}'", source_name))
        }

        async fn list_available(&self) -> Result<String> {
            Ok("Available: memories (loaded), project (deferred)".into())
        }
    }

    #[tokio::test]
    async fn test_context_request_list() {
        let tool = ContextRequestTool::new(Some(Arc::new(MockHandler)));
        let ctx = RoutingContext::new("cli".into(), "test".into());
        let result = tool
            .execute(serde_json::json!({"action": "list"}), &ctx)
            .await
            .unwrap();
        assert!(result.contains("Available"));
    }

    #[tokio::test]
    async fn test_context_request_load() {
        let tool = ContextRequestTool::new(Some(Arc::new(MockHandler)));
        let ctx = RoutingContext::new("cli".into(), "test".into());
        let result = tool
            .execute(
                serde_json::json!({"action": "load", "source": "project"}),
                &ctx,
            )
            .await
            .unwrap();
        assert!(result.contains("Loaded context from 'project'"));
    }

    #[tokio::test]
    async fn test_context_request_load_missing_source() {
        let tool = ContextRequestTool::new(Some(Arc::new(MockHandler)));
        let ctx = RoutingContext::new("cli".into(), "test".into());
        let result = tool
            .execute(serde_json::json!({"action": "load"}), &ctx)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_context_request_missing_action() {
        let tool = ContextRequestTool::new(Some(Arc::new(MockHandler)));
        let ctx = RoutingContext::new("cli".into(), "test".into());
        let result = tool.execute(serde_json::json!({}), &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_context_request_no_handler() {
        let tool = ContextRequestTool::new(None);
        let ctx = RoutingContext::new("cli".into(), "test".into());
        let result = tool
            .execute(serde_json::json!({"action": "list"}), &ctx)
            .await;
        assert!(result.is_err());
    }
}
