//! Row struct for the `agent_tasks` table.

use crate::sqlite_types::SqlTs;
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `agent_tasks` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTaskRow {
    pub id: String,
    pub session_key: String,
    pub description: String,
    pub status: String,
    pub owner_agent_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub blocked_by: String, // JSON array of task IDs
    pub created_at: SqlTs,
    pub updated_at: SqlTs,
}
