//! Tool registry for dynamic tool management.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

use super::{DynTool, Tool, RoutingContext};
use klyntbot_core::{Result, ToolError};

/// Registry for agent tools
pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
    cached_definitions: Option<Vec<Value>>,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            cached_definitions: None,
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        debug!("Registering tool: {}", name);
        self.tools.insert(name, Arc::new(tool));
        // Invalidate cache when registry changes
        self.cached_definitions = None;
    }

    /// Unregister a tool by name
    pub fn unregister(&mut self, name: &str) {
        self.tools.remove(name);
        // Invalidate cache when registry changes
        self.cached_definitions = None;
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<DynTool> {
        self.tools.get(name).cloned()
    }

    /// Check if a tool is registered
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all tool definitions in OpenAI function-calling format
    pub fn get_definitions(&mut self) -> Vec<Value> {
        if let Some(defs) = &self.cached_definitions {
            return defs.clone();
        }

        // First time: build and cache all tool schemas
        let definitions: Vec<Value> = self.tools.values().map(|tool| tool.to_schema()).collect();
        debug!("Cached {} tool definitions", definitions.len());
        self.cached_definitions = Some(definitions.clone());
        definitions
    }

    /// Execute a tool by name with given parameters and routing context
    pub async fn execute(&self, name: &str, params: Value, ctx: &RoutingContext) -> Result<String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        // Validate parameters
        let errors = tool.validate_params(&params);
        if !errors.is_empty() {
            return Err(ToolError::InvalidParams(errors.join("; ")).into());
        }

        // Execute the tool
        match tool.execute(params, ctx).await {
            Ok(result) => Ok(result),
            Err(e) => {
                warn!("Tool {} execution failed: {}", name, e);
                Err(e)
            }
        }
    }

    /// Get list of registered tool names
    pub fn tool_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get the number of registered tools
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockTool;

    #[async_trait]
    impl Tool for MockTool {
        fn name(&self) -> &str {
            "mock_tool"
        }

        fn description(&self) -> &str {
            "A mock tool for testing"
        }

        fn parameters(&self) -> Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Input string"
                    }
                },
                "required": ["input"]
            })
        }

        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok("mock result".to_string())
        }
    }

    #[tokio::test]
    async fn test_registry() {
        let mut registry = ToolRegistry::new();
        assert!(registry.is_empty());

        registry.register(MockTool);
        assert_eq!(registry.len(), 1);
        assert!(registry.has("mock_tool"));

        let defs = registry.get_definitions();
        assert_eq!(defs.len(), 1);

        let params = serde_json::json!({"input": "test"});
        let ctx = RoutingContext::new("cli".into(), "test".into());
        let result = registry.execute("mock_tool", params, &ctx).await.unwrap();
        assert_eq!(result, "mock result");

        registry.unregister("mock_tool");
        assert!(registry.is_empty());
    }
}
