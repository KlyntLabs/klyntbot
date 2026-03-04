use serde::{Deserialize, Serialize};

use crate::types::EntityKind;

/// Typed segment within a structured assistant message.
///
/// Serializes to `{ "type": "text", "content": "..." }` or
/// `{ "type": "tool", "name": "...", "success": true, "durationMs": 123 }`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageSegment {
    #[serde(rename = "text")]
    Text { content: String },
    #[serde(rename = "tool", rename_all = "camelCase")]
    Tool {
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        success: bool,
        duration_ms: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        estimated_tokens: Option<u32>,
    },
}

pub const AGENT_CONTENT_CHUNK: &str = "agent:content_chunk";
pub const AGENT_DONE: &str = "agent:done";
pub const AGENT_TOOL_START: &str = "agent:tool_start";
pub const AGENT_TOOL_END: &str = "agent:tool_end";
pub const AGENT_ERROR: &str = "agent:error";
pub const AGENT_ENTITY_CREATED: &str = "agent:entity_created";
pub const AGENT_INTERACTION_REQUEST: &str = "agent:interaction_request";
pub const AGENT_CLASSIFICATION_COMPLETE: &str = "agent:classification_complete";
pub const AGENT_EXECUTION_STARTED: &str = "agent:execution_started";
pub const AGENT_ITERATION_START: &str = "agent:iteration_start";
pub const AGENT_CONFIDENCE_ASSESSED: &str = "agent:confidence_assessed";
pub const AGENT_USAGE_REPORT: &str = "agent:usage_report";
pub const AGENT_MEMORY_ACCESS: &str = "agent:memory_access";
pub const AGENT_SKILL_LOADED: &str = "agent:skill_loaded";
pub const AGENT_LEARNING_EVENT: &str = "agent:learning_event";
pub const AGENT_SUBAGENT_SPAWNED: &str = "agent:subagent_spawned";
pub const ENTITY_UPDATED: &str = "entity:updated";
pub const MCP_OAUTH_COMPLETE: &str = "mcp:oauth_complete";
pub const MCP_OAUTH_ERROR: &str = "mcp:oauth_error";
pub const MCP_SERVER_STATUS: &str = "mcp:server_status";
pub const MCP_STARTUP_COMPLETE: &str = "mcp:startup_complete";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthCompletePayload {
    pub server_name: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerStatusPayload {
    pub server_name: String,
    /// One of: "starting", "ready", "failed", "skipped"
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpStartupCompletePayload {
    pub ready: usize,
    pub failed: usize,
    pub skipped: usize,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolEndPayload {
    pub session_key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub success: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<u32>,
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
pub struct ClassificationCompletePayload {
    pub session_key: String,
    pub strategy: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionStartedPayload {
    pub session_key: String,
    pub engine: String,
    pub max_iterations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionRequestPayload {
    pub session_key: String,
    pub request_id: String,
    pub request: common::InteractionRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IterationStartPayload {
    pub session_key: String,
    pub iteration: usize,
    pub max_iterations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceAssessedPayload {
    pub session_key: String,
    pub score: f32,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageReportPayload {
    pub session_key: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
    pub estimated_cost_usd: f64,
    pub model: String,
    pub response_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAccessPayload {
    pub session_key: String,
    pub action: String,
    pub query: Option<String>,
    pub results_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillLoadedPayload {
    pub session_key: String,
    pub name: String,
    pub trigger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningEventPayload {
    pub session_key: String,
    pub event_type: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSpawnedPayload {
    pub session_key: String,
    pub label: String,
    pub profile: String,
}

/// Accumulated transparency data for an assistant message.
/// Serialized into `SessionMessage.metadata.transparency`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TransparencyUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<TransparencyCost>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<TransparencyTiming>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tools: Vec<TransparencyTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_tokens_total: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub memory_accesses: Vec<TransparencyMemoryAccess>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub skills: Vec<TransparencySkill>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<TransparencyExecution>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<TransparencyClassification>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub subagents: Vec<TransparencySubagent>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub learning: Vec<TransparencyLearning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyCost {
    pub estimated_usd: f64,
    pub model: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyTiming {
    pub total_ms: u64,
    pub classification_ms: Option<u64>,
    pub context_assembly_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub success: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyMemoryAccess {
    pub action: String,
    pub query: Option<String>,
    pub results_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencySkill {
    pub name: String,
    pub trigger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyExecution {
    pub engine: String,
    pub iterations: u32,
    pub max_iterations: u32,
    pub escalations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyClassification {
    pub strategy: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencySubagent {
    pub label: String,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyLearning {
    pub event_type: String,
    pub detail: String,
}
