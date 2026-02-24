//! Row struct for the `usage_records` table.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `usage_records` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageRecordRow {
    pub id: uuid::Uuid,
    pub timestamp: DateTime<Utc>,
    pub request_id: String,
    pub model: String,
    pub provider: String,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub cache_read_tokens: i32,
    pub cache_write_tokens: i32,
    pub estimated_cost_usd: f64,
    pub channel: String,
    pub strategy: String,
}
