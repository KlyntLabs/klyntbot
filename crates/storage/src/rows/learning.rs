//! Row structs for `learning_outcomes`, `strategy_records`,
//! and `enrichment_feedback` tables.

use crate::sqlite_types::SqlTs;
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `learning_outcomes` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeRow {
    pub id: String,
    pub session_key: String,
    pub tool_name: String,
    pub success: bool,
    pub error_category: Option<String>,
    pub duration_ms: i64,
    pub confidence_score: Option<f32>,
    pub confidence_dimensions: Option<serde_json::Value>,
    pub created_at: SqlTs,
}

/// Row struct for the `strategy_records` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategyRecordRow {
    pub id: uuid::Uuid,
    pub timestamp: SqlTs,
    pub request_id: String,
    pub predicted_strategy: String,
    pub actual_strategy: String,
    pub escalation_count: i32,
    pub iterations_used: i32,
    pub max_iterations: i32,
    pub success: bool,
    pub user_satisfaction: Option<f32>,
    pub response_time_ms: i64,
    pub chat_id: Option<String>,
    pub tool_name: Option<String>,
    pub tool_success: Option<bool>,
    pub tool_duration_ms: Option<i64>,
    pub complexity_signals: serde_json::Value,
    pub execution_mode: Option<String>,
    /// Number of memory entries retrieved from the context engine for this message.
    pub retrieved_memory_count: Option<i32>,
    pub safety_cap_hit: bool,
    pub turns_used: i32,
    pub loop_detected: bool,
    pub loop_tools: Option<String>,
    pub context_fill_pct: Option<f64>,
}

/// Row struct for the `learning_state` key-value table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningStateRow {
    pub key: String,
    pub value: serde_json::Value,
    pub updated_at: SqlTs,
}

/// Aggregated strategy performance summary (from GROUP BY query).
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StrategySummaryRow {
    pub predicted_strategy: String,
    pub sample_count: i64,
    pub correct_count: i64,
    pub avg_escalations: f32,
}

/// Row struct for the `interaction_log` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InteractionLogRow {
    pub id: i32,
    pub timestamp: String,
    pub agent_name: String,
    pub tool_names: String,
    pub channel: String,
    pub duration_ms: Option<i64>,
}

/// Row struct for the `decision_log` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecisionLogRow {
    pub id: String,
    pub session_key: String,
    pub iteration: i32,
    pub tool_names: serde_json::Value,
    pub user_message_preview: String,
    pub assessment: serde_json::Value,
    pub outcome: Option<String>,
    pub created_at: SqlTs,
}
