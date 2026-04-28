//! SQLite schema shapes for opencode messages table.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct MessageRow {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    #[serde(default)]
    pub tool_calls: Option<String>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub metadata: Option<String>,
    pub created_at: String,
}
