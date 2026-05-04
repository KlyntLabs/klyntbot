//! Subagent lifecycle events emitted by SubagentManager.
//! Mirrors desktop_shared::coding::SubagentEvent but lives in agent crate
//! to avoid a circular dependency.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubagentLifecycleEvent {
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
        reason: String,
        cancelled_at: i64,
    },
}
