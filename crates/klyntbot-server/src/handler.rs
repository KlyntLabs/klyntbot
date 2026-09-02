//! MCP server handler — bridges rmcp protocol to klyntbot's AppCore.
//!
//! Combines MCP-server builtins (`get_status`, optional `agent`) with
//! registry tools from a validated [`ExposureValidation`] effective set.

use std::sync::Arc;

use app_core::events::AppEventEmitter;
use app_core::AppCore;
use desktop_shared::types::EntityKind;
use mcp::server::exposure::{BuiltinId, ExposureValidation};
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
    /// Per-connection session identifier for routing context isolation.
    session_id: String,
}

impl KlyntbotServerHandler {
    /// Build a handler from AppCore + shared exposure validation.
    ///
    /// Registry tools come from `effective_registry_tools`. `get_status` is
    /// always advertised; `agent` follows `effective_builtins` (`BuiltinId`),
    /// not a registry-tool name (auto-fill no longer inserts `"agent"`).
    pub fn new(app: Arc<AppCore>, exposure: &ExposureValidation) -> Self {
        let registry = app.agent.tool_registry();
        let effective_registry_tools = exposure.effective_registry_tools.clone();
        let include_agent = exposure.effective_builtins.contains(&BuiltinId::Agent);
        let bridge = if let Ok(bus) = app.domain_event_bus() {
            ToolRegistryBridge::new_with_bus(registry, effective_registry_tools, bus)
        } else {
            ToolRegistryBridge::new(registry, effective_registry_tools)
        };
        let agent_bridge = AgentBridge::new(Arc::clone(&app));

        let status_tool = Self::build_status_tool();
        let agent_tool = if include_agent {
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
            session_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Build the list of MCP resources exposed by this server.
    fn build_resources() -> Vec<Resource> {
        vec![
            Resource {
                raw: RawResource {
                    uri: "klyntbot://status".into(),
                    name: "status".into(),
                    title: Some("Agent Status".into()),
                    description: Some("Agent status, version, and current mode".into()),
                    mime_type: Some("application/json".into()),
                    size: None,
                    icons: None,
                    meta: None,
                },
                annotations: None,
            },
            Resource {
                raw: RawResource {
                    uri: "klyntbot://memory/recent".into(),
                    name: "memory_recent".into(),
                    title: Some("Recent Memories".into()),
                    description: Some("Recent semantic facts from the agent's memory".into()),
                    mime_type: Some("application/json".into()),
                    size: None,
                    icons: None,
                    meta: None,
                },
                annotations: None,
            },
            Resource {
                raw: RawResource {
                    uri: "klyntbot://tasks/today".into(),
                    name: "tasks_today".into(),
                    title: Some("Today's Tasks".into()),
                    description: Some("Tasks and deadlines due today".into()),
                    mime_type: Some("application/json".into()),
                    size: None,
                    icons: None,
                    meta: None,
                },
                annotations: None,
            },
            Resource {
                raw: RawResource {
                    uri: "klyntbot://config/skills".into(),
                    name: "config_skills".into(),
                    title: Some("Active Skills".into()),
                    description: Some("List of currently active orchestrator skills".into()),
                    mime_type: Some("application/json".into()),
                    size: None,
                    icons: None,
                    meta: None,
                },
                annotations: None,
            },
        ]
    }

    /// Execute a tool via the bridge and collect its text output into a single string.
    async fn call_tool_for_resource(&self, tool_name: &str, args: serde_json::Value) -> String {
        match self.bridge.execute(tool_name, args, &self.session_id).await {
            Ok(result) => result
                .content
                .iter()
                .filter_map(|c| c.as_text().map(|t| t.text.clone()))
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => format!("Error fetching {}: {}", tool_name, e.message),
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
                resources: Some(ResourcesCapability {
                    subscribe: Some(false),
                    list_changed: Some(false),
                }),
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

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        Ok(ListResourcesResult {
            meta: None,
            next_cursor: None,
            resources: Self::build_resources(),
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = request.uri.as_str();

        let text = match uri {
            "klyntbot://status" => serde_json::json!({
                "status": "running",
                "version": env!("CARGO_PKG_VERSION"),
                "mode": format!("{:?}", self.app.mode),
            })
            .to_string(),
            "klyntbot://memory/recent" => {
                self.call_tool_for_resource("memory", serde_json::json!({"action": "status"}))
                    .await
            }
            "klyntbot://tasks/today" => {
                self.call_tool_for_resource(
                    "tasks",
                    serde_json::json!({"action": "list", "filter": "today"}),
                )
                .await
            }
            "klyntbot://config/skills" => {
                let store = self.app.agent.skill_store();
                let store = store.read().await;
                let skills: Vec<serde_json::Value> = store
                    .names()
                    .into_iter()
                    .filter_map(|name| {
                        store.get(name).map(|entry| {
                            serde_json::json!({
                                "name": entry.frontmatter.name,
                                "description": entry.frontmatter.description,
                            })
                        })
                    })
                    .collect();
                serde_json::json!({ "skills": skills }).to_string()
            }
            _ => {
                return Err(McpError::new(
                    ErrorCode::INVALID_PARAMS,
                    format!("Unknown resource URI: {}", uri),
                    None,
                ));
            }
        };

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::text(text, uri)],
        })
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
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let name = request.name.as_ref();
        let params = serde_json::Value::Object(request.arguments.unwrap_or_default());

        match name {
            "get_status" => self.handle_get_status().await,
            "agent" if self.agent_tool.is_some() => {
                // Build a progress emitter if the client provided a progress token.
                let progress = context.meta.get_progress_token().map(|token| {
                    crate::bridge::agent::ProgressEmitter {
                        peer: context.peer.clone(),
                        token,
                    }
                });

                let result = self.agent_bridge.execute(params, progress).await?;
                // Agent calls may mutate any entity — broadcast a broad invalidation.
                // The FE listeners filter by entityKind so this is safe.
                for kind in [EntityKind::Task, EntityKind::Project, EntityKind::Note] {
                    self.app.event_emitter.emit_entity_updated(kind, "*");
                }
                Ok(result)
            }
            _ => {
                let result = self
                    .bridge
                    .execute(name, params.clone(), &self.session_id)
                    .await?;
                emit_entity_update_for_tool(&self.app.event_emitter, name, &params, &result);
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

/// Emit `entity:updated` after a successful bridge-executed tool call via the
/// shared `app-core` entity-update intent projection.
fn emit_entity_update_for_tool(
    emitter: &Arc<dyn AppEventEmitter>,
    tool_name: &str,
    params: &serde_json::Value,
    result: &CallToolResult,
) {
    if result.is_error.unwrap_or(false) {
        return;
    }

    let action = params.get("action").and_then(|v| v.as_str());
    for intent in app_core::entity_update_intent::project_entity_update(tool_name, action) {
        emitter.emit_entity_updated(intent.kind, intent.id);
    }
}

#[cfg(test)]
mod entity_update_tests {
    use super::*;
    use desktop_shared::types::EntityKind;
    use rmcp::model::{CallToolResult, Content};
    use std::sync::Mutex;

    struct RecordingEmitter {
        events: Mutex<Vec<(EntityKind, String)>>,
    }

    impl AppEventEmitter for RecordingEmitter {
        fn emit_event(&self, _event_name: &str, _payload: serde_json::Value) {}

        fn emit_entity_updated(&self, kind: EntityKind, id: &str) {
            self.events.lock().unwrap().push((kind, id.to_string()));
        }
    }

    #[test]
    fn emit_entity_update_for_tool_success_mutating() {
        let emitter = Arc::new(RecordingEmitter {
            events: Mutex::new(Vec::new()),
        });
        let result = CallToolResult::success(vec![Content::text("ok")]);
        emit_entity_update_for_tool(
            &(emitter.clone() as Arc<dyn AppEventEmitter>),
            "tasks",
            &serde_json::json!({"action": "create"}),
            &result,
        );
        let events = emitter.events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].0, EntityKind::Task);
        assert_eq!(events[0].1, "*");
    }

    #[test]
    fn emit_entity_update_for_tool_read_only_skips() {
        let emitter = Arc::new(RecordingEmitter {
            events: Mutex::new(Vec::new()),
        });
        let result = CallToolResult::success(vec![Content::text("ok")]);
        emit_entity_update_for_tool(
            &(emitter.clone() as Arc<dyn AppEventEmitter>),
            "tasks",
            &serde_json::json!({"action": "list"}),
            &result,
        );
        assert!(emitter.events.lock().unwrap().is_empty());
    }

    #[test]
    fn emit_entity_update_for_tool_error_skips() {
        let emitter = Arc::new(RecordingEmitter {
            events: Mutex::new(Vec::new()),
        });
        let result = CallToolResult::error(vec![Content::text("boom")]);
        emit_entity_update_for_tool(
            &(emitter.clone() as Arc<dyn AppEventEmitter>),
            "tasks",
            &serde_json::json!({"action": "create"}),
            &result,
        );
        assert!(emitter.events.lock().unwrap().is_empty());
    }

    #[test]
    fn emit_entity_update_for_tool_okr_multi_kind() {
        let emitter = Arc::new(RecordingEmitter {
            events: Mutex::new(Vec::new()),
        });
        let result = CallToolResult::success(vec![Content::text("ok")]);
        emit_entity_update_for_tool(
            &(emitter.clone() as Arc<dyn AppEventEmitter>),
            "okr",
            &serde_json::json!({"action": "objective.create"}),
            &result,
        );
        let events = emitter.events.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].0, EntityKind::Objective);
        assert_eq!(events[1].0, EntityKind::KeyResult);
        assert!(events.iter().all(|(_, id)| id == "*"));
    }

    #[test]
    fn emit_entity_update_parity_cases_match_projection() {
        use app_core::entity_update_intent::PARITY_CASES;

        let result = CallToolResult::success(vec![Content::text("ok")]);
        for (tool, action, expected) in PARITY_CASES {
            let emitter = Arc::new(RecordingEmitter {
                events: Mutex::new(Vec::new()),
            });
            let params = match action {
                Some(a) => serde_json::json!({ "action": a }),
                None => serde_json::json!({}),
            };
            emit_entity_update_for_tool(
                &(emitter.clone() as Arc<dyn AppEventEmitter>),
                tool,
                &params,
                &result,
            );
            let kinds: Vec<EntityKind> = emitter
                .events
                .lock()
                .unwrap()
                .iter()
                .map(|(k, _)| *k)
                .collect();
            assert_eq!(
                kinds.as_slice(),
                *expected,
                "mcp parity {tool:?} {action:?}"
            );
            assert!(emitter
                .events
                .lock()
                .unwrap()
                .iter()
                .all(|(_, id)| id == "*"));
        }
    }
}
