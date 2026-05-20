use serde::{Deserialize, Serialize};

use crate::types::EntityKind;

/// Typed segment within a structured assistant message.
///
/// Serializes to `{ "type": "text", "content": "..." }` or
/// `{ "type": "tool", "name": "...", "success": true, "durationMs": 123 }`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
        #[serde(skip_serializing_if = "Option::is_none")]
        agent: Option<String>,
    },
}

pub const AGENT_CONTENT_CHUNK: &str = "agent:content_chunk";
pub const AGENT_DONE: &str = "agent:done";
pub const AGENT_TOOL_START: &str = "agent:tool_start";
pub const AGENT_TOOL_END: &str = "agent:tool_end";
pub const AGENT_ERROR: &str = "agent:error";
pub const AGENT_CANCELLED: &str = "agent:cancelled";
pub const AGENT_ENTITY_CREATED: &str = "agent:entity_created";
pub const AGENT_INTERACTION_REQUEST: &str = "agent:interaction_request";
pub const AGENT_PIPELINE_STARTED: &str = "agent:pipeline_started";
pub const AGENT_CONTEXT_ASSEMBLED: &str = "agent:context_assembled";
pub const AGENT_RETRIEVAL_ENHANCED: &str = "agent:retrieval_enhanced";
pub const AGENT_CLASSIFICATION_COMPLETE: &str = "agent:classification_complete";
pub const AGENT_EXECUTION_STARTED: &str = "agent:execution_started";
pub const AGENT_ITERATION_START: &str = "agent:iteration_start";
pub const AGENT_CONFIDENCE_ASSESSED: &str = "agent:confidence_assessed";
pub const AGENT_USAGE_REPORT: &str = "agent:usage_report";
pub const AGENT_MEMORY_ACCESS: &str = "agent:memory_access";
pub const AGENT_SKILL_LOADED: &str = "agent:skill_loaded";
pub const AGENT_LEARNING_EVENT: &str = "agent:learning_event";
pub const AGENT_SUBAGENT_SPAWNED: &str = "agent:subagent_spawned";
pub const AGENT_SELECTED: &str = "agent:agent_selected";
pub const AGENT_DELEGATION_STARTED: &str = "agent:delegation_started";
pub const AGENT_DELEGATION_COMPLETED: &str = "agent:delegation_completed";
pub const AGENT_PLAN_GENERATED: &str = "agent:plan_generated";
pub const AGENT_PLAN_STEP_COMPLETED: &str = "agent:plan_step_completed";
pub const AGENT_BUDGET_WARNING: &str = "agent:budget_warning";
pub const AGENT_MEMORY_PROMOTED: &str = "agent:memory_promoted";
pub const AUTOTUNER_REPORT: &str = "autotuner:report";
pub const AUTOTUNER_PROMOTION: &str = "autotuner:promotion";
pub const AUTOTUNER_ROLLBACK: &str = "autotuner:rollback";
pub const ENTITY_UPDATED: &str = "entity:updated";
pub const CHAT_THREAD_CREATED: &str = "chat:thread_created";
pub const CHAT_THREAD_UPDATED: &str = "chat:thread_updated";
pub const CHAT_MESSAGE_ADDED: &str = "chat:message_added";
pub const MCP_OAUTH_COMPLETE: &str = "mcp:oauth_complete";
pub const MCP_OAUTH_ERROR: &str = "mcp:oauth_error";
pub const MCP_SERVER_STATUS: &str = "mcp:server_status";
pub const MCP_STARTUP_COMPLETE: &str = "mcp:startup_complete";
pub const PRODUCTIVITY_DISTRACTION: &str = "productivity:distraction";
pub const PRODUCTIVITY_NUDGE: &str = "productivity:nudge";
pub const ACTIVITY_TICK: &str = "activity:tick";
pub const ACTIVITY_SWITCH: &str = "activity:switch";
pub const FOCUS_STATE_CHANGED: &str = "focus:state_changed";
pub const FOCUS_SYNC: &str = "focus:sync";
pub const FOCUS_PHASE_CHANGED: &str = "focus:phase_changed";
pub const FOCUS_WARNING: &str = "focus:warning";
pub const FOCUS_DND_UNAVAILABLE: &str = "focus:dnd_unavailable";
pub const DISTRACTION_DETECTED: &str = "distraction:detected";
pub const SCORE_UPDATED: &str = "score:updated";
pub const BUCKET_COMPLETED: &str = "bucket:completed";
pub const INSIGHT_GENERATED: &str = "insight:generated";

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpOAuthCompletePayload {
    pub server_name: String,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct McpStartupCompletePayload {
    pub ready: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContentChunkPayload {
    pub session_key: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DonePayload {
    pub session_key: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ToolStartPayload {
    pub session_key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentErrorPayload {
    pub session_key: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CancelledPayload {
    pub session_key: String,
    pub partial_content: String,
    pub partial_reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityCreatedPayload {
    pub session_key: String,
    /// Raw entity type string from the agent (may not map to a known EntityKind).
    pub entity_type: String,
    pub entity_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityUpdatedPayload {
    pub entity_kind: EntityKind,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DataVersionBumpedPayload {
    pub previous: u32,
    pub current: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatThreadPayload {
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessagePayload {
    pub session_key: String,
    /// Source that produced the message (e.g., "chat", "voice", "mcp", "cron").
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PipelineStartedPayload {
    pub session_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ContextAssembledPayload {
    pub session_key: String,
    pub total_tokens: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EnhancementStagePayload {
    pub name: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_detail: Option<String>,
    pub latency_ms: u64,
    pub llm_calls: u32,
    pub output_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalEnhancedPayload {
    pub session_key: String,
    pub stages: Vec<EnhancementStagePayload>,
    pub total_latency_ms: u64,
    pub total_llm_calls: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationCompletePayload {
    pub session_key: String,
    pub strategy: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionStartedPayload {
    pub session_key: String,
    pub engine: String,
    pub max_iterations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InteractionRequestPayload {
    pub session_key: String,
    pub request_id: String,
    #[specta(type = crate::specta_helpers::JsonValue)]
    pub request: common::InteractionRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct IterationStartPayload {
    pub session_key: String,
    pub iteration: usize,
    pub max_iterations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConfidenceAssessedPayload {
    pub session_key: String,
    pub score: f32,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
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

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MemoryAccessPayload {
    pub session_key: String,
    pub action: String,
    pub query: Option<String>,
    pub results_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SkillLoadedPayload {
    pub session_key: String,
    pub name: String,
    pub trigger: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LearningEventPayload {
    pub session_key: String,
    pub event_type: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SubagentSpawnedPayload {
    pub session_key: String,
    pub label: String,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentSelectedPayload {
    pub session_key: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DelegationStartedPayload {
    pub session_key: String,
    pub from_agent: String,
    pub to_agent: String,
    pub query: String,
    pub depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DelegationCompletedPayload {
    pub session_key: String,
    pub from_agent: String,
    pub to_agent: String,
    pub success: bool,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DistractionPayload {
    pub app_name: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NudgePayload {
    pub nudge_type: String,
    pub message: String,
}

pub const COACHING_INTERVENTION: &str = "coaching:intervention";
pub const DISTRACTION_INTERVENTION: &str = "distraction:intervention";
pub const DISTRACTION_VERDICT: &str = "distraction:verdict";

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InterventionPayload {
    pub app_name: String,
    pub window_title: Option<String>,
    pub session_id: String,
    pub needs_llm: bool,
    pub heuristic_verdict: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct VerdictPayload {
    pub classification: String,
    pub display_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanGeneratedPayload {
    pub session_key: String,
    pub steps: Vec<String>,
    pub raw_plan: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanStepCompletedPayload {
    pub session_key: String,
    pub step_index: usize,
    pub description: String,
    pub tool_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BudgetWarningPayload {
    pub session_key: String,
    pub monthly_spend_usd: f64,
    pub monthly_budget_usd: f64,
    pub usage_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusReachedPayload {
    pub session_key: String,
    pub round: u32,
    pub consensus_score: f64,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct MemoryPromotedPayload {
    pub session_key: String,
    pub fact_id: String,
    pub from_scope: String,
    pub to_scope: String,
    pub subject: String,
    pub predicate: String,
}

/// Accumulated transparency data for an assistant message.
/// Serialized into `SessionMessage.metadata.transparency`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_selected: Option<TransparencyAgentSelected>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub subagents: Vec<TransparencySubagent>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub learning: Vec<TransparencyLearning>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub delegations: Vec<TransparencyDelegation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<TransparencyPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enhancement: Option<TransparencyEnhancement>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyEnhancement {
    pub stages: Vec<EnhancementStagePayload>,
    pub total_latency_ms: u64,
    pub total_llm_calls: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub cache_read_tokens: u32,
    pub cache_write_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyCost {
    pub estimated_usd: f64,
    pub model: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyTiming {
    pub total_ms: u64,
    pub classification_ms: Option<u64>,
    pub context_assembly_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    pub success: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyMemoryAccess {
    pub action: String,
    pub query: Option<String>,
    pub results_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencySkill {
    pub name: String,
    pub trigger: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyExecution {
    pub engine: String,
    pub iterations: u32,
    pub max_iterations: u32,
    pub escalations: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyClassification {
    pub strategy: String,
    pub confidence: f32,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyAgentSelected {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencySubagent {
    pub label: String,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyLearning {
    pub event_type: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyDelegation {
    pub from_agent: String,
    pub to_agent: String,
    pub query: String,
    pub depth: u32,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TransparencyPlan {
    pub steps: Vec<String>,
    pub completed_steps: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTickPayload {
    pub app_name: String,
    pub site_name: Option<String>,
    pub category_type: Option<String>,
    pub is_idle: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySwitchPayload {
    pub from_app: Option<String>,
    pub to_app: String,
    pub to_site: Option<String>,
    pub category_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusStatePayload {
    pub state: String,
    pub since: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ScorePayload {
    pub score: f64,
    pub productive_secs: i64,
    pub distracting_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BucketPayload {
    pub bucket_start: String,
    pub productive_secs: i64,
    pub distracting_secs: i64,
    pub dominant_app: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct InsightPayload {
    pub id: String,
    pub insight_type: String,
    pub title: String,
    pub sentiment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusSyncPayload {
    pub phase: String,
    pub remaining_secs: u64,
    pub total_secs: u64,
    pub cycle_position: u32,
    pub long_break_after: u32,
    pub paused: bool,
    pub action_title: Option<String>,
    pub dnd_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusWarningPayload {
    pub phase: String,
    pub remaining_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FocusDndUnavailablePayload {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DistractionDetectedPayload {
    pub app_name: String,
    pub session_id: String,
    pub previous_app: String,
    pub previous_context: String,
    pub reason: String,
}

// ── tauri_specta::Event implementations ─────────────────────────────

impl tauri_specta::Event for MessageSegment {
    const NAME: &'static str = "message-segment";
}

impl tauri_specta::Event for McpOAuthCompletePayload {
    const NAME: &'static str = "mcp:oauth_complete";
}

impl tauri_specta::Event for McpServerStatusPayload {
    const NAME: &'static str = "mcp:server_status";
}

impl tauri_specta::Event for McpStartupCompletePayload {
    const NAME: &'static str = "mcp:startup_complete";
}

impl tauri_specta::Event for ContentChunkPayload {
    const NAME: &'static str = "content-chunk-payload";
}

impl tauri_specta::Event for DonePayload {
    const NAME: &'static str = "done-payload";
}

impl tauri_specta::Event for ToolStartPayload {
    const NAME: &'static str = "tool-start-payload";
}

impl tauri_specta::Event for ToolEndPayload {
    const NAME: &'static str = "tool-end-payload";
}

impl tauri_specta::Event for AgentErrorPayload {
    const NAME: &'static str = "agent:error";
}

impl tauri_specta::Event for CancelledPayload {
    const NAME: &'static str = "agent:cancelled";
}

impl tauri_specta::Event for EntityCreatedPayload {
    const NAME: &'static str = "entity-created-payload";
}

impl tauri_specta::Event for EntityUpdatedPayload {
    const NAME: &'static str = "entity:updated";
}

impl tauri_specta::Event for DataVersionBumpedPayload {
    const NAME: &'static str = "data-version-bumped-payload";
}

impl tauri_specta::Event for ChatMessagePayload {
    const NAME: &'static str = "chat:message_added";
}

impl tauri_specta::Event for PipelineStartedPayload {
    const NAME: &'static str = "pipeline-started-payload";
}

impl tauri_specta::Event for ContextAssembledPayload {
    const NAME: &'static str = "context-assembled-payload";
}

impl tauri_specta::Event for EnhancementStagePayload {
    const NAME: &'static str = "enhancement-stage-payload";
}

impl tauri_specta::Event for RetrievalEnhancedPayload {
    const NAME: &'static str = "retrieval-enhanced-payload";
}

impl tauri_specta::Event for ClassificationCompletePayload {
    const NAME: &'static str = "classification-complete-payload";
}

impl tauri_specta::Event for ExecutionStartedPayload {
    const NAME: &'static str = "execution-started-payload";
}

impl tauri_specta::Event for InteractionRequestPayload {
    const NAME: &'static str = "interaction-request-payload";
}

impl tauri_specta::Event for IterationStartPayload {
    const NAME: &'static str = "iteration-start-payload";
}

impl tauri_specta::Event for ConfidenceAssessedPayload {
    const NAME: &'static str = "confidence-assessed-payload";
}

impl tauri_specta::Event for UsageReportPayload {
    const NAME: &'static str = "usage-report-payload";
}

impl tauri_specta::Event for MemoryAccessPayload {
    const NAME: &'static str = "memory-access-payload";
}

impl tauri_specta::Event for SkillLoadedPayload {
    const NAME: &'static str = "skill-loaded-payload";
}

impl tauri_specta::Event for LearningEventPayload {
    const NAME: &'static str = "learning-event-payload";
}

impl tauri_specta::Event for SubagentSpawnedPayload {
    const NAME: &'static str = "subagent-spawned-payload";
}

impl tauri_specta::Event for AgentSelectedPayload {
    const NAME: &'static str = "agent:agent_selected";
}

impl tauri_specta::Event for DelegationStartedPayload {
    const NAME: &'static str = "delegation-started-payload";
}

impl tauri_specta::Event for DelegationCompletedPayload {
    const NAME: &'static str = "delegation-completed-payload";
}

impl tauri_specta::Event for DistractionPayload {
    const NAME: &'static str = "productivity:distraction";
}

impl tauri_specta::Event for NudgePayload {
    const NAME: &'static str = "productivity:nudge";
}

impl tauri_specta::Event for VerdictPayload {
    const NAME: &'static str = "distraction:verdict";
}

impl tauri_specta::Event for PlanGeneratedPayload {
    const NAME: &'static str = "plan-generated-payload";
}

impl tauri_specta::Event for PlanStepCompletedPayload {
    const NAME: &'static str = "plan-step-completed-payload";
}

impl tauri_specta::Event for BudgetWarningPayload {
    const NAME: &'static str = "budget-warning-payload";
}

impl tauri_specta::Event for TransparencyData {
    const NAME: &'static str = "transparency-data";
}

impl tauri_specta::Event for TransparencyEnhancement {
    const NAME: &'static str = "transparency-enhancement";
}

impl tauri_specta::Event for TransparencyUsage {
    const NAME: &'static str = "transparency-usage";
}

impl tauri_specta::Event for TransparencyCost {
    const NAME: &'static str = "transparency-cost";
}

impl tauri_specta::Event for TransparencyTiming {
    const NAME: &'static str = "transparency-timing";
}

impl tauri_specta::Event for TransparencyTool {
    const NAME: &'static str = "transparency-tool";
}

impl tauri_specta::Event for TransparencyMemoryAccess {
    const NAME: &'static str = "transparency-memory-access";
}

impl tauri_specta::Event for TransparencySkill {
    const NAME: &'static str = "transparency-skill";
}

impl tauri_specta::Event for TransparencyExecution {
    const NAME: &'static str = "transparency-execution";
}

impl tauri_specta::Event for TransparencyClassification {
    const NAME: &'static str = "transparency-classification";
}

impl tauri_specta::Event for TransparencyAgentSelected {
    const NAME: &'static str = "transparency-agent-selected";
}

impl tauri_specta::Event for TransparencySubagent {
    const NAME: &'static str = "transparency-subagent";
}

impl tauri_specta::Event for TransparencyLearning {
    const NAME: &'static str = "transparency-learning";
}

impl tauri_specta::Event for TransparencyDelegation {
    const NAME: &'static str = "transparency-delegation";
}

impl tauri_specta::Event for TransparencyPlan {
    const NAME: &'static str = "transparency-plan";
}

impl tauri_specta::Event for ActivityTickPayload {
    const NAME: &'static str = "activity:tick";
}

impl tauri_specta::Event for ActivitySwitchPayload {
    const NAME: &'static str = "activity:switch";
}

impl tauri_specta::Event for FocusStatePayload {
    const NAME: &'static str = "focus:state_changed";
}

impl tauri_specta::Event for BucketPayload {
    const NAME: &'static str = "bucket:completed";
}

impl tauri_specta::Event for InsightPayload {
    const NAME: &'static str = "insight:generated";
}

impl tauri_specta::Event for FocusWarningPayload {
    const NAME: &'static str = "focus:warning";
}

impl tauri_specta::Event for FocusDndUnavailablePayload {
    const NAME: &'static str = "focus:dnd_unavailable";
}

impl tauri_specta::Event for DistractionDetectedPayload {
    const NAME: &'static str = "distraction:detected";
}

// Manual impls for structs with multiple constant mappings (primary name chosen)

impl tauri_specta::Event for InterventionPayload {
    const NAME: &'static str = "distraction:intervention";
}

impl tauri_specta::Event for ScorePayload {
    const NAME: &'static str = "score:updated";
}

impl tauri_specta::Event for MemoryPromotedPayload {
    const NAME: &'static str = "autotuner:promotion";
}

impl tauri_specta::Event for FocusSyncPayload {
    const NAME: &'static str = "focus:sync";
}
