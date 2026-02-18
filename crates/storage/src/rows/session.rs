//! Row structs for `sessions` and `session_messages` tables.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

/// Row struct for the `sessions` table.
#[derive(Debug, Clone, FromRow)]
pub struct SessionRow {
    pub key: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Row struct for the `session_messages` table.
#[derive(Debug, Clone, FromRow)]
pub struct SessionMessageRow {
    pub id: uuid::Uuid,
    pub session_key: String,
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub request_id: Option<String>,
}
