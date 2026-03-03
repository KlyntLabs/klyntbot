//! KlyntbotServerHandler: implements rmcp's ServerHandler.
//!
//! Exposes dedicated tools for external AI agents to interact with klyntbot.
//! These are separate from klyntbot's internal tools — external agents get
//! a curated API, not raw access to all internal tools.

use rmcp::handler::server::tool::{ToolCallContext, ToolRouter};
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::{tool, tool_router, ErrorData as McpError};

/// The MCP server handler exposing klyntbot capabilities.
#[derive(Clone)]
pub struct KlyntbotServerHandler {
    tool_router: ToolRouter<Self>,
}

impl Default for KlyntbotServerHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl KlyntbotServerHandler {
    pub fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Get klyntbot's current status, version, and capabilities")]
    async fn get_status(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "status": "running",
                "version": env!("CARGO_PKG_VERSION"),
            })
            .to_string(),
        )]))
    }
}

impl ServerHandler for KlyntbotServerHandler {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability::default()),
                ..Default::default()
            },
            server_info: Implementation {
                name: "klyntbot".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            instructions: Some(
                "Klyntbot MCP server — exposes task management and agent capabilities.".to_string(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: self.tool_router.list_all(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let ctx = ToolCallContext::new(self, request, context);
        self.tool_router.call(ctx).await
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        self.tool_router.get(name).cloned()
    }
}

/// Runs the MCP server on stdio transport.
///
/// Streamable HTTP transport will be added in a follow-up when axum
/// integration is wired up.
pub struct McpServerRunner;

impl McpServerRunner {
    /// Start the MCP server on stdio. Blocks until the server is stopped.
    pub async fn run() -> anyhow::Result<()> {
        use rmcp::service::ServiceExt;

        let handler = KlyntbotServerHandler::new();
        tracing::info!("Starting MCP server (stdio)");

        let transport = rmcp::transport::io::stdio();
        let service = handler.serve(transport).await?;
        service.waiting().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_handler_creates() {
        let handler = KlyntbotServerHandler::new();
        let info = handler.get_info();
        assert_eq!(info.server_info.name.as_str(), "klyntbot");
    }

    #[test]
    fn test_server_handler_lists_tools() {
        let handler = KlyntbotServerHandler::new();
        let tools = handler.tool_router.list_all();
        assert!(!tools.is_empty(), "Should have at least one tool");
        assert!(
            tools.iter().any(|t| t.name.as_ref() == "get_status"),
            "Should have get_status tool"
        );
    }

    #[test]
    fn test_server_runner_is_unit_struct() {
        // McpServerRunner is a unit struct with a static run() method
        let _ = McpServerRunner;
    }
}
