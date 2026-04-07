use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionMemoryRow {
    pub session_key: String,
    pub content: String,
    pub turn_count: i64,
    pub updated_at: String,
}
