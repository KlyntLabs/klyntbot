//! Row struct for the `tool_usage` table.

use crate::sqlite_types::SqlTs;
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `tool_usage` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsageRow {
    pub id: String,
    pub tool_name: String,
    pub action: Option<String>,
    pub session_key: Option<String>,
    pub channel: Option<String>,
    pub intent_category: Option<String>,
    pub success: bool,
    pub duration_ms: Option<i64>,
    pub error_message: Option<String>,
    pub created_at: SqlTs,
}

/// Aggregated tool usage stats.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUsageStatsRow {
    pub tool_name: String,
    pub call_count: i64,
    pub success_count: i64,
    pub avg_duration_ms: Option<f64>,
}
