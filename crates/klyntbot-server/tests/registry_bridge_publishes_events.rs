//! Verifies that the MCP `ToolRegistryBridge` publishes `DomainEvent::ToolCallExecuted`
//! after a successful tool execution, so the cognitive activity log and coding-memory
//! distiller observe MCP-driven tool calls.

use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus, ToolExecutionEvent};
use klyntbot_server::bridge::registry::ToolRegistryBridge;
use std::sync::Arc;
use tools_core::{RoutingContext, Tool};

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }
    fn description(&self) -> &str {
        "Echo back the input"
    }
    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }
    async fn execute(
        &self,
        _args: serde_json::Value,
        _ctx: &RoutingContext,
    ) -> common::Result<String> {
        Ok("hi".into())
    }
}

#[tokio::test]
async fn mcp_tool_call_publishes_tool_call_executed() {
    let bus = Arc::new(DomainEventBus::new(64));
    let mut rx = bus.subscribe();

    let registry = Arc::new(tokio::sync::RwLock::new(
        tools_core::registry::ToolRegistry::new(),
    ));
    registry.write().await.register(EchoTool);
    let bridge = ToolRegistryBridge::new_with_bus(registry, vec!["echo".into()], bus.clone());

    let result: Result<rmcp::model::CallToolResult, rmcp::ErrorData> = bridge
        .execute("echo", serde_json::json!({"text": "hi"}), "session-1")
        .await;
    assert!(result.is_ok(), "execute failed: {:?}", result.err());

    // Drain bus — expect at least one ToolCallExecuted event for "echo".
    let mut saw = false;
    while let Ok(ev) = rx.try_recv() {
        if let DomainEvent::ToolExecution(ToolExecutionEvent::ToolCallExecuted {
            tool_name, ..
        }) = ev
        {
            if tool_name == "echo" {
                saw = true;
                break;
            }
        }
    }
    assert!(saw, "ToolCallExecuted not published for MCP echo call");
}
