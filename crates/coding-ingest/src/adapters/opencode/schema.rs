//! SQLite schema shapes for opencode messages table.

use serde::Deserialize;

/// One row from opencode's `messages` table.
#[derive(Debug, Deserialize, sqlx::FromRow)]
pub struct MessageRow {
    /// Row id (monotonically increasing).
    pub id: i64,
    /// Session identifier.
    pub session_id: String,
    /// Message role: user, assistant, tool, system.
    pub role: String,
    /// Message body text.
    pub content: String,
    /// Structured tool_calls JSON (assistant messages).
    #[serde(default)]
    pub tool_calls: Option<String>,
    /// Tool call id (tool role messages).
    #[serde(default)]
    pub tool_call_id: Option<String>,
    /// Arbitrary metadata JSON (includes cwd, etc.).
    #[serde(default)]
    pub metadata: Option<String>,
    /// Creation timestamp (SQLite text or integer).
    pub created_at: String,
}
