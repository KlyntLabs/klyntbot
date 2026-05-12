use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubagentEvent {
    Spawned {
        agent_id: String,
        label: String,
        profile: String,
        parent_session_id: String,
        spawned_at: i64,
    },
    Progress {
        agent_id: String,
        iteration: u32,
        last_tool: Option<String>,
    },
    Completed {
        agent_id: String,
        success: bool,
        summary: String,
        tokens_used: u64,
        duration_ms: u64,
    },
    Cancelled {
        agent_id: String,
        reason: SubagentCancelReason,
        cancelled_at: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SubagentCancelReason {
    UserRequested,
    Timeout,
    ParentCancelled,
    PolicyViolation,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubagentActiveSummary {
    pub agent_id: String,
    pub label: String,
    pub profile: String,
    pub iteration: u32,
    pub status: String,
    pub started_at: i64,
    pub last_tool: Option<String>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubagentDetail {
    pub agent_id: String,
    pub messages: Vec<serde_json::Value>,
    pub tokens_used: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SubagentInstanceSummary {
    pub agent_id: String,
    pub session_id: String,
    pub parent_agent_id: Option<String>,
    pub description: String,
    pub status: String,
    pub turns_used_total: i64,
    pub last_cap_hit_at: Option<i64>,
    pub updated_at: i64,
}

// Per-thread channels are emitted as `agent:subagent_event#<thread_id>` via raw
// `app.emit` (see crates/desktop/src/app_core.rs::fan_subagent_events_to_tauri).
// Frontend uses `listen<SubagentEvent>(...)` raw; this `NAME` is a registration
// placeholder so the type appears in the generated bindings.
impl tauri_specta::Event for SubagentEvent {
    const NAME: &'static str = "agent:subagent_event";
}
