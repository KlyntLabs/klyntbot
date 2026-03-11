//! docs tool — search and fetch documentation from the content registry.

use common::Result;
use tools_core::{tool_actions, ActionParams, RoutingContext};

use async_trait::async_trait;
use std::sync::Arc;

/// Handler trait for content registry access.
///
/// Implemented by the agent layer which holds the ContentRegistry.
/// Follows the dependency inversion pattern used by other tools.
#[async_trait]
pub trait ContentRegistryHandler: Send + Sync {
    /// Search content by query. Returns JSON array of results.
    async fn search(&self, query: &str, limit: usize) -> Result<String>;

    /// Get a specific entry by ID. Returns JSON object or error.
    async fn get(&self, id: &str) -> Result<String>;

    /// List all available content entries. Returns JSON array.
    async fn list(&self) -> Result<String>;
}

#[derive(Debug, ActionParams)]
pub struct SearchParams {
    /// Search query for finding documentation or skills
    #[param(required)]
    pub query: String,
    /// Maximum number of results to return (default: 10)
    pub limit: Option<i64>,
}

#[derive(Debug, ActionParams)]
pub struct GetParams {
    /// Document or skill ID to retrieve (e.g. "stripe/api")
    #[param(required)]
    pub id: String,
}

#[derive(Debug, ActionParams)]
pub struct ListParams {}

pub struct DocsTool {
    handler: Option<Arc<dyn ContentRegistryHandler>>,
}

impl DocsTool {
    pub fn new(handler: Option<Arc<dyn ContentRegistryHandler>>) -> Self {
        Self { handler }
    }

    fn handler(&self) -> Result<&dyn ContentRegistryHandler> {
        self.handler.as_deref().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Content registry not available".into()).into()
        })
    }
}

#[tool_actions(
    name = "docs",
    description = "Search and fetch documentation for APIs, SDKs, and libraries from the content registry. Use before writing code against external services to get current, accurate API reference.",
    category = "Search",
    tags = "documentation,api,sdk,reference",
    cost = "Free"
)]
impl DocsTool {
    #[action(name = "search")]
    async fn search(&self, params: SearchParams, _ctx: &RoutingContext) -> Result<String> {
        let limit = params.limit.unwrap_or(10) as usize;
        self.handler()?.search(&params.query, limit).await
    }

    #[action(name = "get")]
    async fn get(&self, params: GetParams, _ctx: &RoutingContext) -> Result<String> {
        self.handler()?.get(&params.id).await
    }

    #[action(name = "list")]
    async fn list(&self, _params: ListParams, _ctx: &RoutingContext) -> Result<String> {
        self.handler()?.list().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct MockHandler;

    #[async_trait]
    impl ContentRegistryHandler for MockHandler {
        async fn search(&self, query: &str, limit: usize) -> Result<String> {
            Ok(json!({
                "results": [{"id": "test/doc", "name": "Test", "score": 1.0}],
                "query": query,
                "limit": limit
            })
            .to_string())
        }

        async fn get(&self, id: &str) -> Result<String> {
            Ok(json!({"id": id, "name": "Test Doc"}).to_string())
        }

        async fn list(&self) -> Result<String> {
            Ok(json!({"docs": [], "skills": []}).to_string())
        }
    }

    #[test]
    fn test_docs_tool_creates() {
        let tool = DocsTool::new(None);
        assert!(tool.handler.is_none());
    }

    #[test]
    fn test_docs_tool_with_handler() {
        let tool = DocsTool::new(Some(Arc::new(MockHandler)));
        assert!(tool.handler.is_some());
    }

    #[test]
    fn test_docs_tool_no_handler_errors() {
        let tool = DocsTool::new(None);
        let result = tool.handler();
        assert!(result.is_err());
    }
}
