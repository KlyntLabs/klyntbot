//! McpTool: adapts MCP server tools to the `tools_core::Tool` trait.
//!
//! Each MCP tool discovered via `tools/list` becomes an `McpTool` instance
//! registered in klyntbot's `ToolRegistry`. The naming convention is
//! `mcp_{server_name}_{tool_name}` to avoid collisions with built-in tools.

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

use common::{KlyntbotError, Result};
use tools_core::{RoutingContext, Tool};

use super::sanitize;
use crate::allowlist::{AllowDecision, McpChannelAllowlist};
use crate::client::circuit_breaker::McpCircuitBreaker;

/// Classify whether an MCP error is transient (worth retrying) or permanent.
fn is_transient_mcp_error(err: &KlyntbotError) -> bool {
    matches!(
        err,
        KlyntbotError::Timeout(_)
            | KlyntbotError::Io(_)
            | KlyntbotError::Bus(_)
            | KlyntbotError::BusDisconnected
            | KlyntbotError::Tool(common::ToolError::ExecutionFailed(_))
    )
}

/// An MCP tool adapted to the klyntbot `Tool` trait.
///
/// Holds a reference to the rmcp `Peer` handle for making JSON-RPC calls
/// to the remote MCP server.
pub struct McpTool {
    /// Namespaced tool name: "mcp_{server}_{tool}" (sanitized)
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
    /// Per-server tool call timeout
    tool_timeout: Duration,
    /// Circuit breaker shared across all tools on the same server
    circuit_breaker: Arc<McpCircuitBreaker>,
    /// Per-channel allowlist — gates which servers are available per channel.
    channel_allowlist: Arc<McpChannelAllowlist>,
}

impl McpTool {
    pub fn new(
        server_name: &str,
        tool_def: &rmcp::model::Tool,
        peer: Arc<rmcp::service::Peer<rmcp::service::RoleClient>>,
        tool_timeout: Duration,
        circuit_breaker: Arc<McpCircuitBreaker>,
        channel_allowlist: Arc<McpChannelAllowlist>,
    ) -> Self {
        let original_name = tool_def.name.to_string();
        let namespaced_name = sanitize::build_tool_name(server_name, &original_name);
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
            tool_timeout,
            circuit_breaker,
            channel_allowlist,
        }
    }

    /// Get the server name this tool belongs to.
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Execute a single MCP tool call (no retry logic).
    async fn execute_once(&self, args: &Value) -> Result<String> {
        debug!(
            server = %self.server_name,
            tool = %self.original_name,
            args = %args,
            "Calling MCP tool"
        );

        let start = std::time::Instant::now();
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
                let elapsed = start.elapsed();
                warn!(
                    server = %self.server_name,
                    tool = %self.original_name,
                    elapsed_ms = elapsed.as_millis() as u64,
                    error = %e,
                    "MCP tool call failed"
                );
                KlyntbotError::Tool(common::ToolError::ExecutionFailed(format!(
                    "MCP tool call failed (server={}, tool={}): {e}",
                    self.server_name, self.original_name
                )))
            })?;

        debug!(
            server = %self.server_name,
            tool = %self.original_name,
            elapsed_ms = start.elapsed().as_millis() as u64,
            content_parts = result.content.len(),
            is_error = result.is_error.unwrap_or(false),
            "MCP tool call completed"
        );

        // Extract text content in a single pass
        let text_parts: Vec<&str> = result
            .content
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.as_ref()))
            .collect();

        // Check if the server reported an error
        if result.is_error.unwrap_or(false) {
            return Err(KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                format!(
                    "MCP server error ({}): {}",
                    self.original_name,
                    text_parts.join("\n")
                ),
            )));
        }

        if text_parts.is_empty() {
            // If no text content, serialize the entire result as JSON
            Ok(serde_json::to_string(&result.content).unwrap_or_else(|_| "OK".to_string()))
        } else {
            Ok(text_parts.join("\n"))
        }
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

    fn custom_timeout(&self) -> Option<Duration> {
        Some(self.tool_timeout)
    }

    async fn execute(&self, arguments: Value, ctx: &RoutingContext) -> Result<String> {
        // Per-channel allowlist check
        let channel_str = ctx.channel.to_string();
        if self
            .channel_allowlist
            .decide(&channel_str, &self.server_name)
            == AllowDecision::Denied
        {
            return Err(KlyntbotError::Tool(common::ToolError::PermissionDenied(
                format!(
                    "MCP server '{}' is not allowed in channel '{}'",
                    self.server_name, channel_str
                ),
            )));
        }

        if self.circuit_breaker.is_open(&self.server_name) {
            return Err(KlyntbotError::Tool(common::ToolError::ExecutionFailed(
                format!(
                    "MCP server '{}' circuit breaker is open — server appears unavailable",
                    self.server_name
                ),
            )));
        }

        let delays = [
            Duration::from_millis(500),
            Duration::from_secs(1),
            Duration::from_secs(2),
        ];
        let mut last_err = None;

        for (attempt, delay) in delays.iter().enumerate() {
            match self.execute_once(&arguments).await {
                Ok(result) => {
                    self.circuit_breaker.record_success(&self.server_name);
                    return Ok(result);
                }
                Err(e) if is_transient_mcp_error(&e) => {
                    warn!(
                        "MCP tool '{}' on '{}' attempt {}/3 failed: {}",
                        self.namespaced_name,
                        self.server_name,
                        attempt + 1,
                        e
                    );
                    let just_opened = self.circuit_breaker.record_failure(&self.server_name);
                    if just_opened {
                        warn!(
                            "Circuit breaker opened for MCP server '{}'",
                            self.server_name
                        );
                        return Err(e);
                    }
                    last_err = Some(e);
                    if attempt < delays.len() - 1 {
                        tokio::time::sleep(*delay).await;
                    }
                }
                Err(e) => return Err(e),
            }
        }

        // Last failure was already recorded inside the loop — don't double-count
        Err(last_err.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_tool_naming_convention() {
        // Verify the naming format produces "mcp_{server}_{tool}"
        let name = super::sanitize::build_tool_name("linear", "list_issues");
        assert!(name.starts_with("mcp_"));
        assert_eq!(name, "mcp_linear_list_issues");
        // Verify underscores separate the three parts
        let parts: Vec<&str> = name.splitn(3, '_').collect();
        assert_eq!(parts, vec!["mcp", "linear", "list_issues"]);
    }

    #[test]
    fn test_is_transient_mcp_error_timeout() {
        let err = KlyntbotError::Timeout("test".into());
        assert!(is_transient_mcp_error(&err));
    }

    #[test]
    fn test_is_transient_mcp_error_io() {
        let err = KlyntbotError::Io(std::io::Error::new(
            std::io::ErrorKind::ConnectionReset,
            "reset",
        ));
        assert!(is_transient_mcp_error(&err));
    }

    #[test]
    fn test_is_transient_mcp_error_tool_not_found() {
        let err = KlyntbotError::Tool(common::ToolError::NotFound("test".into()));
        assert!(!is_transient_mcp_error(&err));
    }

    #[test]
    fn test_is_transient_mcp_error_invalid_params() {
        let err = KlyntbotError::Tool(common::ToolError::InvalidParams("test".into()));
        assert!(!is_transient_mcp_error(&err));
    }
}
