use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ReforgeStateRow {
    pub id: String,
    pub last_run_at: Option<String>,
    pub last_run_stats: Option<String>,
    pub run_count: i64,
}
