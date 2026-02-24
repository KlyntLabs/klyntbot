//! Row structs for `plans` and `plan_steps` tables.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `plans` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRow {
    pub id: uuid::Uuid,
    pub session_key: String,
    pub goal_id: Option<uuid::Uuid>,
    pub title: String,
    pub description: String,
    pub status: String,
    pub current_step_index: i32,
    pub iteration_limit: i32,
    pub backtrack_history: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Row struct for the `plan_steps` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanStepRow {
    pub id: uuid::Uuid,
    pub plan_id: uuid::Uuid,
    pub step_index: i32,
    pub description: String,
    pub reasoning: String,
    #[sqlx(json)]
    pub expected_tools: Vec<String>,
    pub status: String,
    pub attempt_count: i16,
    pub max_attempts: i16,
    pub result: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}
