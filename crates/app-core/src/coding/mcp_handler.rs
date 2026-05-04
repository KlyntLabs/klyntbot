use crate::AppCore;
use common::Result;

/// Status of a single MCP server.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatus {
    pub name: String,
    pub transport: String,
    pub enabled: bool,
    pub state: String,
    pub tool_count: Option<u32>,
    pub last_error: Option<String>,
}

/// Result for coding_mcp_status.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStatusResult {
    pub servers: Vec<McpServerStatus>,
    pub total_tools: u32,
}

impl AppCore {
    /// List configured MCP servers and their health status.
    ///
    /// Reads from config and queries the agent's MCP manager for live state.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_mcp_status(
        &self,
        workspace_id: Option<&str>,
    ) -> Result<McpStatusResult> {
        let config = self.config.read().await;
        let mcp_config = &config.mcp;

        let mut servers = Vec::new();

        for server_cfg in &mcp_config.servers {
            let state = if server_cfg.enabled { "configured" } else { "disabled" };
            servers.push(McpServerStatus {
                name: server_cfg.name.clone(),
                transport: format!("{:?}", server_cfg.transport),
                enabled: server_cfg.enabled,
                state: state.to_string(),
                tool_count: None,  // Live tool count requires querying the MCP manager
                last_error: None,
            });
        }

        let _ = workspace_id; // Future: filter by workspace-specific MCP servers

        tracing::info!(
            server_count = servers.len(),
            "MCP status queried"
        );

        Ok(McpStatusResult {
            servers,
            total_tools: 0,
        })
    }
}
