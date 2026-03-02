use serde::{Deserialize, Serialize};

use crate::types::EntityKind;

pub const AGENT_CONTENT_CHUNK: &str = "agent:content_chunk";
pub const AGENT_DONE: &str = "agent:done";
pub const AGENT_TOOL_START: &str = "agent:tool_start";
pub const AGENT_TOOL_END: &str = "agent:tool_end";
pub const AGENT_ERROR: &str = "agent:error";
pub const AGENT_ENTITY_CREATED: &str = "agent:entity_created";
pub const AGENT_INTERACTION_REQUEST: &str = "agent:interaction_request";
pub const ENTITY_UPDATED: &str = "entity:updated";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentChunkPayload {
    pub session_key: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DonePayload {
    pub session_key: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStartPayload {
    pub session_key: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolEndPayload {
    pub session_key: String,
    pub name: String,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentErrorPayload {
    pub session_key: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityCreatedPayload {
    pub session_key: String,
    /// Raw entity type string from the agent (may not map to a known EntityKind).
    pub entity_type: String,
    pub entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityUpdatedPayload {
    pub entity_kind: EntityKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionRequestPayload {
    pub session_key: String,
    pub request_id: String,
    pub request: common::InteractionRequest,
}
