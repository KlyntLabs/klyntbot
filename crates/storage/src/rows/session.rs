//! Row structs for `sessions` and `session_messages` tables.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `sessions` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRow {
    pub key: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    pub pinned: bool,
}

/// Row struct for the `session_messages` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMessageRow {
    pub id: uuid::Uuid,
    pub session_key: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: i64,
    pub project_id: Option<String>,
    pub conversation_type: Option<String>,
    pub pinned: bool,
}
