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
