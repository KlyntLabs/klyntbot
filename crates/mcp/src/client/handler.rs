//! ClientHandler implementation for handling server-initiated MCP requests.
//!
//! When an MCP server sends requests to the client (sampling, roots, elicitation),
//! this handler routes them to the appropriate klyntbot subsystem.

use rmcp::handler::client::ClientHandler;
use rmcp::model::*;
use rmcp::service::{NotificationContext, RoleClient};
use tracing::{debug, info, warn};

/// Klyntbot's MCP client handler.
///
/// Handles server-initiated requests like sampling (LLM completions),
/// roots listing, and notifications.
pub struct KlyntbotClientHandler {
    /// Server name for logging context
    server_name: String,
}

impl KlyntbotClientHandler {
    pub fn new(server_name: &str) -> Self {
        Self {
            server_name: server_name.to_string(),
        }
    }
}

impl ClientHandler for KlyntbotClientHandler {
    // Use default implementations for now — they return appropriate errors.
    // Sampling, elicitation, and roots will be wired up in a follow-up task
    // once the basic client flow is working.

    fn get_info(&self) -> ClientInfo {
        ClientInfo {
            client_info: Implementation {
                name: "klyntbot".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                ..Default::default()
            },
            ..Default::default()
        }
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
            "MCP server tool list changed (dynamic refresh not yet implemented)"
        );
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
    }
}
