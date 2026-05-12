//! Persisted row for `subagent_instances`.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// Lifecycle states for a subagent instance. Mirrors the CHECK constraint on
/// `subagent_instances.status`. `idle` and `stopped_turn` are resumable;
/// `failed`, `killed`, and `completed` are terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubagentStatus {
    Running,
    Idle,
    StoppedTurn,
    Failed,
    Killed,
    Completed,
}

impl SubagentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Idle => "idle",
            Self::StoppedTurn => "stopped_turn",
            Self::Failed => "failed",
            Self::Killed => "killed",
            Self::Completed => "completed",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "idle" => Some(Self::Idle),
            "stopped_turn" => Some(Self::StoppedTurn),
            "failed" => Some(Self::Failed),
            "killed" => Some(Self::Killed),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }

    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Failed | Self::Killed | Self::Completed)
    }

    pub fn is_resumable(&self) -> bool {
        matches!(self, Self::Idle | Self::StoppedTurn)
    }
}

/// Raw row from `subagent_instances`. Use `SubagentInstanceRepo` to map to/from this.
#[derive(Debug, Clone, FromRow)]
pub struct SubagentInstanceRow {
    pub agent_id: String,
    pub session_id: String,
    pub parent_agent_id: Option<String>,
    pub description: String,
    pub status: String,
    pub model: Option<String>,
    pub workspace_path: String,
    pub turn_cap: i64,
    pub turns_used: i64,
    pub turns_used_total: i64,
    pub partial_summary: Option<String>,
    pub last_cap_hit_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl SubagentInstanceRow {
    pub fn status_enum(&self) -> SubagentStatus {
        SubagentStatus::parse(&self.status).unwrap_or(SubagentStatus::Failed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips() {
        for s in [
            SubagentStatus::Running,
            SubagentStatus::Idle,
            SubagentStatus::StoppedTurn,
            SubagentStatus::Failed,
            SubagentStatus::Killed,
            SubagentStatus::Completed,
        ] {
            assert_eq!(SubagentStatus::parse(s.as_str()), Some(s));
        }
    }

    #[test]
    fn terminal_vs_resumable() {
        assert!(SubagentStatus::Idle.is_resumable());
        assert!(SubagentStatus::StoppedTurn.is_resumable());
        assert!(!SubagentStatus::Running.is_resumable());
        assert!(SubagentStatus::Failed.is_terminal());
        assert!(SubagentStatus::Killed.is_terminal());
        assert!(SubagentStatus::Completed.is_terminal());
        assert!(!SubagentStatus::Running.is_terminal());
        assert!(!SubagentStatus::Idle.is_terminal());
    }
}
