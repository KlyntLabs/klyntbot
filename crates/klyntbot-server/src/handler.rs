//! MCP server handler — bridges rmcp protocol to klyntbot's AppCore.
//!
//! Combines built-in tools (get_status) with dynamically bridged tools
//! from klyntbot's internal ToolRegistry.

use std::sync::Arc;

use app_core::events::AppEventEmitter;
use app_core::AppCore;
use desktop_shared::types::EntityKind;
use rmcp::handler::server::ServerHandler;
use rmcp::model::*;
use rmcp::service::{RequestContext, RoleServer};
use rmcp::ErrorData as McpError;

use crate::bridge::agent::AgentBridge;
use crate::bridge::registry::ToolRegistryBridge;

pub struct KlyntbotServerHandler {
    app: Arc<AppCore>,
    bridge: ToolRegistryBridge,
    agent_bridge: AgentBridge,
    /// Pre-built static tool definitions (computed once at construction).
    status_tool: Tool,
    agent_tool: Option<Tool>,
}

impl KlyntbotServerHandler {
    pub fn new(app: Arc<AppCore>, whitelist: Vec<String>) -> Self {
        let registry = app.agent.tool_registry();
        let has_agent = whitelist.iter().any(|w| w == "agent");
        let bridge = ToolRegistryBridge::new(registry, whitelist);
        let agent_bridge = AgentBridge::new(Arc::clone(&app));

        let status_tool = Self::build_status_tool();
        let agent_tool = if has_agent {
            Some(Self::build_agent_tool())
        } else {
            None
        };

        Self {
            app,
            bridge,
            agent_bridge,
            status_tool,
            agent_tool,
        }
    }

    fn build_status_tool() -> Tool {
        serde_json::from_value(serde_json::json!({
            "name": "get_status",
            "description": "Get klyntbot's current status, version, and capabilities",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        }))
        .expect("static tool definition is always valid")
    }

    fn build_agent_tool() -> Tool {
        serde_json::from_value(serde_json::json!({
            "name": "agent",
            "description": "Send a natural language request to klyntbot's agent pipeline. The agent analyzes intent, assembles context from memory/projects/tasks, selects tools, and executes a multi-step plan.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "action": { "type": "string", "enum": ["chat", "status"] },
                    "message": { "type": "string", "description": "Natural language request for the agent" },
                    "session_key": { "type": "string", "description": "Optional session key for conversation continuity. Omit for one-shot requests." }
                },
                "required": ["action", "message"]
            }
        }))
        .expect("static tool definition is always valid")
    }

    async fn handle_get_status(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "status": "running",
                "version": env!("CARGO_PKG_VERSION"),
                "mode": format!("{:?}", self.app.mode),
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
                "Klyntbot MCP server — personal AI agent with task management, memory, and productivity tools.".to_string(),
            ),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let mut tools = vec![self.status_tool.clone()];
        if let Some(ref agent) = self.agent_tool {
            tools.push(agent.clone());
        }
        tools.extend(self.bridge.list_tools().await);

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name = request.name.as_ref();
        let params = serde_json::Value::Object(request.arguments.unwrap_or_default());

        match name {
            "get_status" => self.handle_get_status().await,
            "agent" if self.agent_tool.is_some() => {
                let result = self.agent_bridge.execute(params).await?;
                // Agent calls may mutate any entity — broadcast a broad invalidation.
                // The FE listeners filter by entityKind so this is safe.
                for kind in [EntityKind::Task, EntityKind::Project, EntityKind::Note] {
                    self.app.event_emitter.emit_entity_updated(kind, "*");
                }
                Ok(result)
            }
            _ => {
                let result = self.bridge.execute(name, params.clone()).await?;
                emit_entity_update_for_tool(&self.app.event_emitter, name, &params);
                Ok(result)
            }
        }
    }

    fn get_tool(&self, name: &str) -> Option<Tool> {
        match name {
            "get_status" => Some(self.status_tool.clone()),
            "agent" => self.agent_tool.clone(),
            _ => None,
        }
    }
}

/// Read-only actions that should not trigger entity update events.
const READ_ONLY_ACTIONS: &[&str] = &["list", "show", "get", "search", "status", "stats", "query"];

/// Emit an `entity:updated` event after a successful MCP tool call,
/// but only for mutating actions (create, update, delete, etc.).
fn emit_entity_update_for_tool(
    emitter: &Arc<dyn AppEventEmitter>,
    tool_name: &str,
    params: &serde_json::Value,
) {
    let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");

    if READ_ONLY_ACTIONS.contains(&action) {
        return;
    }

    let entity_kind = match tool_name {
        "tasks" => EntityKind::Task,
        "project" => EntityKind::Project,
        "area" => EntityKind::Area,
        "notes" => EntityKind::Note,
        "okr" => EntityKind::Objective,
        "finance" => EntityKind::Finance,
        "productivity" => EntityKind::Productivity,
        "work_context" => EntityKind::Productivity,
        _ => return,
    };

    let id = params.get("id").and_then(|v| v.as_str()).unwrap_or("*");

    emitter.emit_entity_updated(entity_kind.clone(), id);

    // OKR tool can mutate both objectives and key results.
    if tool_name == "okr" {
        emitter.emit_entity_updated(EntityKind::KeyResult, id);
    }
}
