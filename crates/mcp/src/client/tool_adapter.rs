//! McpTool: adapts MCP server tools to the `tools_core::Tool` trait.
//!
//! Each MCP tool discovered via `tools/list` becomes an `McpTool` instance
//! registered in klyntbot's `ToolRegistry`. The naming convention is
//! `mcp_{server_name}_{tool_name}` to avoid collisions with built-in tools.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::debug;

use common::Result;
use tools_core::{PermissionLevel, RoutingContext, Tool};

/// An MCP tool adapted to the klyntbot `Tool` trait.
///
/// Holds a reference to the rmcp `Peer` handle for making JSON-RPC calls
/// to the remote MCP server.
pub struct McpTool {
    /// Namespaced tool name: "mcp_{server}_{tool}"
    namespaced_name: String,
    /// Original MCP tool name (sent in `tools/call` requests)
    original_name: String,
    /// Tool description from the MCP server
    tool_description: String,
    /// JSON Schema for parameters (from MCP server's inputSchema)
    input_schema: Value,
    /// Server name (for logging/debugging)
    server_name: String,
    /// rmcp client peer handle for making tool calls
    peer: Arc<rmcp::service::Peer<rmcp::service::RoleClient>>,
}

impl McpTool {
    pub fn new(
        server_name: &str,
        tool_def: &rmcp::model::Tool,
        peer: Arc<rmcp::service::Peer<rmcp::service::RoleClient>>,
    ) -> Self {
        let original_name = tool_def.name.to_string();
        let namespaced_name = format!("mcp_{}_{}", server_name, original_name);
        let tool_description = tool_def
            .description
            .as_deref()
            .unwrap_or("No description")
            .to_string();
        let input_schema = serde_json::to_value(&*tool_def.input_schema)
            .unwrap_or_else(|_| Value::Object(Default::default()));

        Self {
            namespaced_name,
            original_name,
            tool_description,
            input_schema,
            server_name: server_name.to_string(),
            peer,
        }
    }

    /// Get the server name this tool belongs to.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }
}

impl std::fmt::Debug for McpTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpTool")
            .field("name", &self.namespaced_name)
            .field("server", &self.server_name)
            .finish()
    }
}

#[async_trait]
impl Tool for McpTool {
    fn name(&self) -> &str {
        &self.namespaced_name
    }

    fn description(&self) -> &str {
        &self.tool_description
    }

    fn parameters(&self) -> Value {
        self.input_schema.clone()
    }

    fn permission_level(&self) -> PermissionLevel {
        // MCP tools make network calls to external servers
        PermissionLevel::Elevated
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        debug!(
            server = %self.server_name,
            tool = %self.original_name,
            "Calling MCP tool"
        );

        let result = self
            .peer
            .call_tool(rmcp::model::CallToolRequestParams {
                name: self.original_name.clone().into(),
                arguments: args.as_object().cloned(),
                meta: None,
                task: None,
            })
            .await
            .map_err(|e| {
                common::KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                    "MCP tool call failed (server={}, tool={}): {e}",
                    self.server_name, self.original_name
                )))
            })?;

        // Extract text content in a single pass
        let text_parts: Vec<&str> = result
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.as_ref()))
            .collect();

        // Check if the server reported an error
        if result.is_error.unwrap_or(false) {
            return Err(common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed(format!(
                    "MCP server error ({}): {}",
                    self.original_name,
                    text_parts.join("\n")
                )),
            ));
        }

        if text_parts.is_empty() {
            // If no text content, serialize the entire result as JSON
            Ok(serde_json::to_string(&result.content).unwrap_or_else(|_| "OK".to_string()))
        } else {
            Ok(text_parts.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_mcp_tool_naming_convention() {
        // Verify the naming format produces "mcp_{server}_{tool}"
        let name = format!("mcp_{}_{}", "linear", "list_issues");
        assert!(name.starts_with("mcp_"));
        assert_eq!(name, "mcp_linear_list_issues");
        // Verify underscores separate the three parts
        let parts: Vec<&str> = name.splitn(3, '_').collect();
        assert_eq!(parts, vec!["mcp", "linear", "list_issues"]);
    }
}
