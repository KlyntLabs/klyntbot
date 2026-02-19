//! Tool registry for dynamic tool management.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

use super::{DynTool, RoutingContext, Tool, ToolPermissions};
use common::{Result, ToolError};

/// Registry for agent tools
pub struct ToolRegistry {
    tools: HashMap<String, DynTool>,
    cached_definitions: Mutex<Option<Vec<Value>>>,
    permissions: Option<ToolPermissions>,
}

impl ToolRegistry {
    /// Create a new tool registry
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
            cached_definitions: Mutex::new(None),
            permissions: None,
        }
    }

    /// Set the permission configuration for this registry.
    pub fn set_permissions(&mut self, permissions: ToolPermissions) {
        self.permissions = Some(permissions);
    }

    /// Register a tool
    pub fn register(&mut self, tool: impl Tool + 'static) {
        let name = tool.name().to_string();
        debug!("Registering tool: {}", name);
        self.tools.insert(name, Arc::new(tool));
        // Invalidate cache when registry changes
        *self.cached_definitions.lock().expect("cache lock poisoned") = None;
    }

    /// Unregister a tool by name
    pub fn unregister(&mut self, name: &str) {
        self.tools.remove(name);
        // Invalidate cache when registry changes
        *self.cached_definitions.lock().expect("cache lock poisoned") = None;
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<DynTool> {
        self.tools.get(name).cloned()
    }

    /// Check if a tool is registered
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get all tool definitions in OpenAI function-calling format.
    /// Uses interior mutability so only a shared reference is needed.
    pub fn get_definitions(&self) -> Vec<Value> {
        let mut cache = self.cached_definitions.lock().expect("cache lock poisoned");
        if let Some(defs) = cache.as_ref() {
            return defs.clone();
        }

        // First time: build and cache all tool schemas
        let definitions: Vec<Value> = self.tools.values().map(|tool| tool.to_schema()).collect();
        debug!("Cached {} tool definitions", definitions.len());
        *cache = Some(definitions.clone());
        definitions
    }

    /// Execute a tool by name with given parameters and routing context
    pub async fn execute(&self, name: &str, params: Value, ctx: &RoutingContext) -> Result<String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        // Check permissions if configured
        if let Some(ref perms) = self.permissions {
            let required = tool.permission_level();
            let channel = ctx.channel.as_str();
            if !perms.is_allowed(channel, required) {
                return Err(ToolError::PermissionDenied(format!(
                    "Tool '{}' requires {} permission, channel '{}' has insufficient access",
                    name, required, channel
                ))
                .into());
            }
        }

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
    use crate::PermissionLevel;
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

    struct ElevatedMockTool;

    #[async_trait]
    impl Tool for ElevatedMockTool {
        fn name(&self) -> &str {
            "elevated_tool"
        }

        fn description(&self) -> &str {
            "A tool requiring elevated permissions"
        }

        fn parameters(&self) -> Value {
            serde_json::json!({"type": "object", "properties": {}})
        }

        fn permission_level(&self) -> PermissionLevel {
            PermissionLevel::Elevated
        }

        async fn execute(&self, _args: Value, _ctx: &RoutingContext) -> Result<String> {
            Ok("elevated result".to_string())
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

    #[tokio::test]
    async fn test_no_permissions_allows_all() {
        let mut registry = ToolRegistry::new();
        registry.register(ElevatedMockTool);

        let ctx = RoutingContext::new("telegram".into(), "test".into());
        let result = registry
            .execute("elevated_tool", serde_json::json!({}), &ctx)
            .await;
        assert!(result.is_ok(), "No permissions configured = allow all");
    }

    #[tokio::test]
    async fn test_permission_granted() {
        let mut registry = ToolRegistry::new();
        registry.register(ElevatedMockTool);

        let mut perms = ToolPermissions::new(PermissionLevel::ReadOnly);
        perms.set_channel_level("cli", PermissionLevel::Admin);
        registry.set_permissions(perms);

        let ctx = RoutingContext::new("cli".into(), "test".into());
        let result = registry
            .execute("elevated_tool", serde_json::json!({}), &ctx)
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_permission_denied() {
        let mut registry = ToolRegistry::new();
        registry.register(ElevatedMockTool);

        let perms = ToolPermissions::new(PermissionLevel::ReadOnly);
        registry.set_permissions(perms);

        let ctx = RoutingContext::new("telegram".into(), "test".into());
        let result = registry
            .execute("elevated_tool", serde_json::json!({}), &ctx)
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Permission denied"),
            "Error should mention permission denied: {}",
            err
        );
        assert!(err.contains("elevated_tool"));
        assert!(err.contains("telegram"));
    }
}
