//! Bridges klyntbot's internal ToolRegistry to MCP tool calls.
//!
//! Translates MCP `CallToolRequestParams` -> internal `Tool::execute()` -> `CallToolResult`.

use std::collections::HashSet;
use std::sync::Arc;

use common::{ChannelName, ChatId, SessionMode, MCP_CHANNEL};
use rmcp::model::{CallToolResult, Content, Tool as McpTool};
use rmcp::ErrorData as McpError;
use tokio::sync::RwLock;
use tools_core::registry::ToolRegistry;
use tools_core::RoutingContext;

use super::schema;

/// Bridges klyntbot's ToolRegistry to MCP protocol.
pub struct ToolRegistryBridge {
    registry: Arc<RwLock<ToolRegistry>>,
    whitelist: Arc<std::sync::RwLock<HashSet<String>>>,
    domain_bus: Option<Arc<bus::DomainEventBus>>,
}

impl ToolRegistryBridge {
    pub fn new(registry: Arc<RwLock<ToolRegistry>>, whitelist: Vec<String>) -> Self {
        Self {
            registry,
            whitelist: Arc::new(std::sync::RwLock::new(whitelist.into_iter().collect())),
            domain_bus: None,
        }
    }

    pub fn new_with_bus(
        registry: Arc<RwLock<ToolRegistry>>,
        whitelist: Vec<String>,
        domain_bus: Arc<bus::DomainEventBus>,
    ) -> Self {
        Self {
            registry,
            whitelist: Arc::new(std::sync::RwLock::new(whitelist.into_iter().collect())),
            domain_bus: Some(domain_bus),
        }
    }

    /// Update the whitelist at runtime.
    pub fn update_whitelist(&self, tools: Vec<String>) {
        let mut wl = self.whitelist.write().expect("whitelist lock");
        *wl = tools.into_iter().collect();
    }

    /// Check whether a tool name is in the whitelist.
    pub fn is_whitelisted(&self, name: &str) -> bool {
        self.whitelist
            .read()
            .expect("whitelist lock")
            .contains(name)
    }

    /// Alias for [`is_whitelisted`] — backwards compatibility.
    pub fn is_exposed(&self, name: &str) -> bool {
        self.is_whitelisted(name)
    }

    /// List all whitelisted tools as MCP Tool definitions.
    pub async fn list_tools(&self) -> Vec<McpTool> {
        let names: Vec<String> = self
            .whitelist
            .read()
            .expect("whitelist lock")
            .iter()
            .cloned()
            .collect();
        let reg = self.registry.read().await;
        let mut tools = Vec::new();
        for name in &names {
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
        session_id: &str,
    ) -> Result<CallToolResult, McpError> {
        // Whitelist check
        if !self.is_whitelisted(tool_name) {
            return Err(McpError::invalid_request(
                format!("Tool '{tool_name}' is not exposed via MCP"),
                None,
            ));
        }

        // Build MCP routing context with per-connection session isolation
        let ctx = RoutingContext {
            channel: ChannelName::new(MCP_CHANNEL),
            session_mode: SessionMode::Assistant,
            chat_id: ChatId::new(format!("mcp:{}", session_id)),
            interaction_tx: None,
            is_direct_mode: true,
            delegation_depth: 0,
            entity_tx: None,
            interaction_channel: None,
            champion_params: None,
            cancel_token: None,
            event_tx: None,
            hook_engine: None,
            session_key: None,
            message_id: None,
            repo_id: String::new(),
            agent_id: "root".into(),
            agent_profile: "root".into(),
            plan_mode_active: false,
            plan_session_id: None,
            previous_anti_passivity_violation: false,
            same_turn_user_msg_emitted: false,
            workspace_cwd: None,
            agent_chain: vec!["root".into()],
            job_supervisor: None,
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

        let started_at = std::time::Instant::now();
        let outcome = tool.execute(arguments.clone(), &ctx).await;
        let duration_ms = started_at.elapsed().as_millis() as u64;

        if let Some(bus) = &self.domain_bus {
            bus.publish(bus::DomainEvent::ToolCallExecuted {
                tool_name: tool_name.to_string(),
                args_preview: Some(arguments.to_string().chars().take(512).collect::<String>()),
                session_key: Some(format!("mcp:{session_id}")),
                duration_ms: Some(duration_ms as i64),
            });
        }

        match outcome {
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

            let result = bridge
                .execute("read_file", serde_json::json!({}), "test-session")
                .await;
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
                .execute(
                    "task",
                    serde_json::json!({"action": "list"}),
                    "test-session",
                )
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

            let result = bridge
                .execute("task", serde_json::json!({}), "test-session")
                .await;
            assert!(result.is_err());
        });
    }

    #[test]
    fn whitelist_reflects_updates_at_call_time() {
        let registry = Arc::new(RwLock::new(ToolRegistry::new()));
        let bridge = ToolRegistryBridge::new(registry, vec!["tasks".to_string()]);

        // Initially "notes" is not whitelisted
        assert!(!bridge.is_whitelisted("notes"));

        // Update whitelist to include "notes"
        bridge.update_whitelist(vec!["tasks".to_string(), "notes".to_string()]);

        // Now it should be whitelisted
        assert!(bridge.is_whitelisted("notes"));
    }
}
