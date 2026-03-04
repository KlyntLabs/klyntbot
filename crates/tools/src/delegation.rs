//! Delegation tool for agent-to-agent composition.
//!
//! Enables one agent to delegate a query to a specialist agent, running
//! synchronously and returning the response. Uses depth tracking to prevent
//! infinite delegation loops.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use super::{PermissionLevel, RoutingContext, Tool};
use crate::params::ParamExtractor;
use common::ToolError;

/// Default maximum delegation depth to prevent infinite loops.
const DEFAULT_MAX_DEPTH: u32 = 2;

/// Trait for delegating queries to other agents (dependency inversion).
/// Implemented by `AgentRuntime` in the agent crate.
#[async_trait]
pub trait DelegationHandler: Send + Sync {
    /// Delegate a query to another agent, running synchronously.
    /// Returns the agent's response as a string.
    async fn delegate(
        &self,
        agent_name: &str,
        query: &str,
        ctx: &RoutingContext,
        depth: u32,
    ) -> common::Result<String>;
}

/// Tool that delegates queries to specialist agents.
///
/// The LLM sees `allowed_agents` as an enum in the schema, so it can only
/// delegate to agents that the current profile's `can_delegate_to` permits.
pub struct DelegationTool {
    handler: Option<Arc<dyn DelegationHandler>>,
    allowed_agents: Vec<String>,
    current_depth: u32,
    max_depth: u32,
}

impl DelegationTool {
    /// Create a new delegation tool without a handler (will error if used).
    pub fn new() -> Self {
        Self {
            handler: None,
            allowed_agents: vec![],
            current_depth: 0,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// Create with a DelegationHandler implementation.
    pub fn with_handler(handler: Arc<dyn DelegationHandler>) -> Self {
        Self {
            handler: Some(handler),
            allowed_agents: vec![],
            current_depth: 0,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// Set the list of agents this tool is allowed to delegate to.
    pub fn with_allowed_agents(mut self, agents: Vec<String>) -> Self {
        self.allowed_agents = agents;
        self
    }

    /// Set the current delegation depth and maximum depth.
    pub fn with_depth(mut self, current: u32, max: u32) -> Self {
        self.current_depth = current;
        self.max_depth = max;
        self
    }
}

impl Default for DelegationTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DelegationTool {
    fn name(&self) -> &str {
        "delegate"
    }

    fn description(&self) -> &str {
        "Delegate a query to a specialist agent. Use when you need another agent's expertise."
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Standard
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "description": "Name of the agent to delegate to",
                    "enum": self.allowed_agents,
                },
                "query": {
                    "type": "string",
                    "description": "The query or task to delegate"
                }
            },
            "required": ["agent", "query"]
        })
    }

    async fn execute(&self, args: Value, ctx: &RoutingContext) -> common::Result<String> {
        // Cheapest guard first: depth check before handler/param resolution.
        if self.current_depth >= self.max_depth {
            return Ok("Cannot delegate further — maximum delegation depth reached.".into());
        }

        let handler = self.handler.as_ref().ok_or_else(|| {
            ToolError::ExecutionFailed("DelegationHandler not available".to_string())
        })?;

        let p = ParamExtractor::new(&args);
        let agent = p.required_str("agent")?;
        let query = p.required_str("query")?;

        // Defense-in-depth: schema enum already restricts this, but direct Rust
        // callers may bypass schema validation.
        if !self.allowed_agents.is_empty() && !self.allowed_agents.iter().any(|a| a == agent) {
            return Err(ToolError::InvalidParams(format!(
                "Cannot delegate to '{agent}' — not in allowed agents list"
            ))
            .into());
        }

        debug!(
            "Delegating to agent '{}' (depth {}/{}): {}",
            agent,
            self.current_depth + 1,
            self.max_depth,
            query
        );

        handler
            .delegate(agent, query, ctx, self.current_depth + 1)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx() -> RoutingContext {
        RoutingContext::new("cli".into(), "test".into())
    }

    /// Shared mock that echoes agent/query/depth for assertions.
    struct MockDelegationHandler;

    #[async_trait]
    impl DelegationHandler for MockDelegationHandler {
        async fn delegate(
            &self,
            agent: &str,
            query: &str,
            _ctx: &RoutingContext,
            depth: u32,
        ) -> common::Result<String> {
            Ok(format!("Agent {agent} handled '{query}' at depth {depth}"))
        }
    }

    fn mock_handler() -> Arc<dyn DelegationHandler> {
        Arc::new(MockDelegationHandler)
    }

    #[tokio::test]
    async fn test_delegation_tool_schema() {
        let tool = DelegationTool::new();
        assert_eq!(tool.name(), "delegate");
        let schema = tool.parameters();
        let required = schema["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "agent"));
        assert!(required.iter().any(|v| v == "query"));
    }

    #[tokio::test]
    async fn test_delegation_tool_without_handler_errors() {
        let tool = DelegationTool::new();
        let args = serde_json::json!({"agent": "calendar", "query": "list events"});
        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delegation_tool_max_depth_reached() {
        let tool = DelegationTool::with_handler(mock_handler())
            .with_depth(2, 2)
            .with_allowed_agents(vec!["task".to_string()]);
        let args = serde_json::json!({"agent": "task", "query": "list tasks"});
        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert!(result.contains("maximum delegation depth"));
    }

    #[tokio::test]
    async fn test_delegation_tool_disallowed_agent() {
        let tool = DelegationTool::with_handler(mock_handler())
            .with_allowed_agents(vec!["task".to_string(), "calendar".to_string()]);
        let args = serde_json::json!({"agent": "finance", "query": "show budget"});
        let result = tool.execute(args, &test_ctx()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delegation_tool_successful_delegation() {
        let tool = DelegationTool::with_handler(mock_handler())
            .with_allowed_agents(vec!["task".to_string()])
            .with_depth(0, 2);
        let args = serde_json::json!({"agent": "task", "query": "list tasks"});
        let result = tool.execute(args, &test_ctx()).await.unwrap();
        assert_eq!(result, "Agent task handled 'list tasks' at depth 1");
    }

    #[tokio::test]
    async fn test_delegation_tool_allowed_agents_in_schema() {
        let tool = DelegationTool::new()
            .with_allowed_agents(vec!["task".to_string(), "finance".to_string()]);
        let schema = tool.parameters();
        let agent_enum = schema["properties"]["agent"]["enum"].as_array().unwrap();
        assert_eq!(agent_enum.len(), 2);
        assert!(agent_enum.iter().any(|v| v == "task"));
        assert!(agent_enum.iter().any(|v| v == "finance"));
    }
}
