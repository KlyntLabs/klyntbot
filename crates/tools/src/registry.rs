//! Tool registry for dynamic tool management.
//!
//! Re-exported from tools-core. Tests here exercise the registry through the
//! `tools` crate's re-exported types (RoutingContext, etc.).

pub use tools_core::ToolRegistry;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RoutingContext, Tool};
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
