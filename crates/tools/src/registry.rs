//! Tool registry for dynamic tool management.
//!
//! Re-exported from tools-core. Tests here exercise the registry through the
//! `tools` crate's re-exported types (RoutingContext, PermissionLevel, etc.).

pub use tools_core::ToolRegistry;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PermissionLevel, RoutingContext, Tool, ToolPermissions};
    use async_trait::async_trait;
    use common::Result;
    use serde_json::Value;

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
