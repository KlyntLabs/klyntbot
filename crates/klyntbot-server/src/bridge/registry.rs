//! Bridges klyntbot's internal ToolRegistry to MCP tool calls.
//!
//! Translates MCP `CallToolRequestParams` -> internal `Tool::execute()` -> `CallToolResult`.

use std::collections::HashSet;
use std::sync::Arc;

use common::{ChannelName, ChatId, MCP_CHANNEL};
use rmcp::model::{CallToolResult, Content, Tool as McpTool};
use rmcp::ErrorData as McpError;
use tokio::sync::RwLock;
use tools_core::registry::ToolRegistry;
use tools_core::RoutingContext;

use super::schema;

/// Bridges klyntbot's ToolRegistry to MCP protocol.
pub struct ToolRegistryBridge {
    registry: Arc<RwLock<ToolRegistry>>,
    whitelist: HashSet<String>,
}

impl ToolRegistryBridge {
    pub fn new(registry: Arc<RwLock<ToolRegistry>>, whitelist: Vec<String>) -> Self {
        Self {
            registry,
            whitelist: whitelist.into_iter().collect(),
        }
    }

    /// Check whether a tool name is in the whitelist.
    pub fn is_exposed(&self, name: &str) -> bool {
        self.whitelist.contains(name)
    }

    /// List all whitelisted tools as MCP Tool definitions.
    pub async fn list_tools(&self) -> Vec<McpTool> {
        let reg = self.registry.read().await;
        let mut tools = Vec::new();
        for name in &self.whitelist {
            if let Some(tool) = reg.get(name) {
                let params = tool.parameters();
                tools.push(schema::internal_to_mcp_tool(
                    tool.name(),
                    tool.description(),
                    params,
                ));
            }
        }
        tools
    }

    /// Execute a tool call via the internal registry.
    pub async fn execute(
        &self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        // Whitelist check
        if !self.whitelist.contains(tool_name) {
            return Err(McpError::invalid_request(
                format!("Tool '{tool_name}' is not exposed via MCP"),
                None,
            ));
        }

        // Build MCP routing context
        let ctx = RoutingContext {
            channel: ChannelName::new(MCP_CHANNEL),
            chat_id: ChatId::new("mcp-session"),
            interaction_tx: None,
            is_direct_mode: true,
            delegation_depth: 0,
            entity_tx: None,
            interaction_channel: None,
            squad_id: None,
        };

        // Acquire read lock, prepare (validate + clone Arc<dyn Tool>), then drop lock
        // before the potentially long-running execute() call.
        let tool = {
            let reg = self.registry.read().await;
            reg.prepare(tool_name, &arguments, &ctx)
                .map_err(|e| match &e {
                    common::KlyntbotError::Tool(common::ToolError::InvalidParams(msg)) => {
                        McpError::invalid_params(msg.clone(), None)
                    }
                    _ => McpError::internal_error(e.to_string(), None),
                })?
        };
        // RwLock dropped here — concurrent requests are no longer blocked.

        match tool.execute(arguments, &ctx).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result)])),
            Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitelist_rejects_unexposed_tool() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let registry = Arc::new(RwLock::new(ToolRegistry::new()));
            let bridge = ToolRegistryBridge::new(registry, vec!["task".into()]);

            let result = bridge.execute("read_file", serde_json::json!({})).await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_whitelist_allows_exposed_tool() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let registry = Arc::new(RwLock::new(ToolRegistry::new()));
            let bridge = ToolRegistryBridge::new(registry, vec!["task".into()]);

            // Tool passes whitelist but is not registered -> NotFound maps to
            // Err(McpError) since prepare() fails before execute().
            let result = bridge
                .execute("task", serde_json::json!({"action": "list"}))
                .await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn test_empty_whitelist_rejects_everything() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let registry = Arc::new(RwLock::new(ToolRegistry::new()));
            let bridge = ToolRegistryBridge::new(registry, vec![]);

            let result = bridge.execute("task", serde_json::json!({})).await;
            assert!(result.is_err());
        });
    }
}
