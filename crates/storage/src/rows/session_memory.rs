use serde::{Deserialize, Serialize};

use crate::sqlite_types::SqlTs;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct SessionMemoryRow {
    pub session_key: String,
    pub content: String,
    pub turn_count: i64,
    pub updated_at: SqlTs,
}
