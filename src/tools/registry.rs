//! Tool registry for dynamic tool management.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, warn};

use super::{DynTool, Tool};
use crate::error::{Result, ToolError};

/// Registry for agent tools
pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        debug!("Registering tool: {}", name);
        self.tools.insert(name, Arc::new(tool));
    }

    /// Unregister a tool by name
    pub fn unregister(&mut self, name: &str) {
        self.tools.remove(name);
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
    pub fn get_definitions(&self) -> Vec<Value> {
        self.tools.values().map(|tool| tool.to_schema()).collect()
    }

    /// Execute a tool by name with given parameters
    pub async fn execute(&self, name: &str, params: Value) -> Result<String> {
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
        match tool.execute(params).await {
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

        async fn execute(&self, _args: Value) -> Result<String> {
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
        let result = registry.execute("mock_tool", params).await.unwrap();
        assert_eq!(result, "mock result");

        registry.unregister("mock_tool");
        assert!(registry.is_empty());
    }
}
