//! ClientHandler implementation for handling server-initiated MCP requests.
//!
//! When an MCP server sends requests to the client (sampling, roots, elicitation),
//! this handler routes them to the appropriate klyntbot subsystem.

use std::sync::Arc;

use rmcp::ErrorData as McpError;
use rmcp::handler::client::ClientHandler;
use rmcp::model::*;
use rmcp::service::{NotificationContext, RequestContext, RoleClient};
use tracing::{debug, info, warn};

/// Trait for delegating sampling (LLM completion) requests.
/// Implemented outside this crate where the LLM provider is available.
#[async_trait::async_trait]
pub trait SamplingDelegate: Send + Sync {
    async fn sample(
        &self,
        params: CreateMessageRequestParams,
    ) -> std::result::Result<CreateMessageResult, McpError>;
}

/// Klyntbot's MCP client handler.
///
/// Handles server-initiated requests like sampling (LLM completions),
/// roots listing, and notifications.
pub struct KlyntbotClientHandler {
    /// Server name for logging context
    server_name: String,
    /// Optional channel to notify when the server signals a tool list change.
    tool_list_changed_tx: Option<tokio::sync::mpsc::Sender<String>>,
    /// Workspace root path for roots listing.
    data_dir: Option<String>,
    /// Optional sampling delegate for LLM completion requests.
    sampling_delegate: Option<Arc<dyn SamplingDelegate>>,
}

impl KlyntbotClientHandler {
    pub fn new(server_name: &str) -> Self {
        Self {
            server_name: server_name.to_string(),
            tool_list_changed_tx: None,
            data_dir: None,
            sampling_delegate: None,
        }
    }

    /// Create a handler with a tool-list-changed notification channel.
    pub fn with_tool_list_changed_tx(mut self, tx: tokio::sync::mpsc::Sender<String>) -> Self {
        self.tool_list_changed_tx = Some(tx);
        self
    }

    /// Set the data directory for roots listing.
    pub fn with_data_dir(mut self, dir: String) -> Self {
        self.data_dir = Some(dir);
        self
    }

    /// Set the sampling delegate for LLM completion requests.
    pub fn with_sampling_delegate(mut self, delegate: Arc<dyn SamplingDelegate>) -> Self {
        self.sampling_delegate = Some(delegate);
        self
    }
}

impl ClientHandler for KlyntbotClientHandler {
    fn get_info(&self) -> ClientInfo {
        ClientInfo {
            client_info: Implementation {
                name: "klyntbot".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            capabilities: ClientCapabilities {
                roots: Some(RootsCapabilities {
                    list_changed: Some(false),
                }),
                sampling: if self.sampling_delegate.is_some() {
                    Some(SamplingCapability::default())
                } else {
                    None
                },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    async fn create_message(
        &self,
        params: CreateMessageRequestParams,
        _context: RequestContext<RoleClient>,
    ) -> Result<CreateMessageResult, McpError> {
        if let Some(ref delegate) = self.sampling_delegate {
            debug!(
                server = %self.server_name,
                "MCP sampling request received"
            );
            delegate.sample(params).await
        } else {
            Err(McpError::method_not_found::<CreateMessageRequestMethod>())
        }
    }

    async fn list_roots(
        &self,
        _context: RequestContext<RoleClient>,
    ) -> Result<ListRootsResult, McpError> {
        let mut roots = Vec::new();
        if let Some(ref dir) = self.data_dir {
            roots.push(Root {
                uri: format!("file://{}", dir),
                name: Some("klyntbot-data".to_string()),
            });
        }
        Ok(ListRootsResult { roots })
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        match params.level {
            LoggingLevel::Error
            | LoggingLevel::Critical
            | LoggingLevel::Alert
            | LoggingLevel::Emergency => {
                warn!(
                    server = %self.server_name,
                    "MCP server log: {:?}",
                    params.data
                );
            }
            _ => {
                debug!(
                    server = %self.server_name,
                    "MCP server log: {:?}",
                    params.data
                );
            }
        }
    }

    async fn on_tool_list_changed(&self, _context: NotificationContext<RoleClient>) {
        info!(
            server = %self.server_name,
            "MCP server tool list changed, triggering re-discovery"
        );
        if let Some(ref tx) = self.tool_list_changed_tx {
            if let Err(tokio::sync::mpsc::error::TrySendError::Full(_)) =
                tx.try_send(self.server_name.clone())
            {
                tracing::warn!(server = %self.server_name, "tool_list_changed channel full, notification dropped");
            }
        }
    }

    async fn on_resource_list_changed(&self, _context: NotificationContext<RoleClient>) {
        debug!(
            server = %self.server_name,
            "MCP server resource list changed"
        );
    }

    async fn on_prompt_list_changed(&self, _context: NotificationContext<RoleClient>) {
        debug!(
            server = %self.server_name,
            "MCP server prompt list changed"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_handler_info() {
        let handler = KlyntbotClientHandler::new("test-server");
        let info = handler.get_info();
        assert_eq!(info.client_info.name, "klyntbot");
        // Without sampling delegate, sampling capability is not advertised
        assert!(info.capabilities.sampling.is_none());
        // Roots are always advertised
        assert!(info.capabilities.roots.is_some());
    }
}
