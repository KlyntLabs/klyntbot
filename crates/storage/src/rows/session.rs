//! Row structs for `sessions` and `session_messages` tables.

use crate::sqlite_types::SqlTs;
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `sessions` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub key: String,
    pub metadata: serde_json::Value,
    pub created_at: SqlTs,
    pub updated_at: SqlTs,
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    pub pinned: bool,
    pub cwd: Option<String>,
    pub repo_id: Option<String>,
    pub repo_branch: Option<String>,
    pub tool_profile: Option<String>,
    pub approval_mode: String,
    pub total_cost_usd: f64,
    pub total_tokens: i64,
    pub parent_session_id: Option<String>,
}

/// Row struct for the `session_messages` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessageRow {
    pub id: uuid::Uuid,
    pub session_key: String,
    pub role: String,
    pub content: String,
    pub timestamp: SqlTs,
    pub request_id: Option<String>,
    pub tool_calls: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
}

/// Row struct for session listing with message count (aggregated query).
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionListRow {
    pub key: String,
    pub metadata: serde_json::Value,
    pub created_at: SqlTs,
    pub updated_at: SqlTs,
    pub message_count: i64,
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    pub pinned: bool,
}
