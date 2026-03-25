//! Row structs for the `tasks`, `task_activity`, `task_executions`, `task_suggestions`,
//! `task_decompositions`, `task_estimation_history`, `task_attachments`, `task_time_entries`,
//! and `task_dependencies` tables.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `tasks` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRow {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub area_id: String,
    pub project_id: Option<String>,
    pub key_result_id: Option<String>,
    pub parent_id: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<DateTime<Utc>>,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub status: String,
    pub focused_at: Option<DateTime<Utc>>,
    pub focus_deadline: Option<DateTime<Utc>>,
    pub focus_expired_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_tracked_secs: i64,
    pub estimated_minutes: Option<i32>,
    pub calendar_event_uid: Option<String>,
    pub last_reminded_at: Option<DateTime<Utc>>,
    pub recurrence_rule: Option<String>,
    pub recurrence_parent_id: Option<String>,
    pub is_template: bool,
    pub next_instance_date: Option<DateTime<Utc>>,
    pub status_label_id: Option<String>,
    pub position: i32,
    pub group_id: Option<String>,
    pub task_type: String,
    pub acceptance_criteria: Option<String>,
    pub agent_config: Option<String>,
    pub execution_state: String,
    pub spawned_execution_id: Option<String>,
    pub context_snapshot: Option<String>,
    pub energy_level: Option<String>,
    pub estimated_focus_blocks: Option<i32>,
    pub actual_minutes: Option<i32>,
    pub complexity_score: Option<i32>,
    pub completed: bool,
    pub objective_id: Option<String>,
    pub scheduled_start: Option<DateTime<Utc>>,
    pub scheduled_end: Option<DateTime<Utc>>,
}

/// Row struct for the `task_activity` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskActivityRow {
    pub id: String,
    pub task_id: String,
    pub activity_type: String,
    pub field_changed: Option<String>,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
    pub actor_type: String,
    pub actor_id: Option<String>,
    pub summary: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Row struct for the `task_executions` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskExecutionRow {
    pub id: String,
    pub task_id: String,
    pub status: String,
    pub agent_profile: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub tokens_used: Option<i64>,
    pub cost_usd: Option<f64>,
    pub input_context: Option<String>,
    pub output_summary: Option<String>,
    pub error_message: Option<String>,
    pub artifacts: Option<String>,
    pub metrics: Option<String>,
    pub retry_count: i32,
    pub created_at: DateTime<Utc>,
}

/// Row struct for the `task_suggestions` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSuggestionRow {
    pub id: String,
    pub task_id: Option<String>,
    pub suggestion_type: String,
    pub title: String,
    pub description: Option<String>,
    pub confidence: f64,
    pub action_payload: Option<String>,
    pub status: String,
    pub trigger: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Row struct for the `task_decompositions` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDecompositionRow {
    pub id: String,
    pub task_id: String,
    pub plan: String,
    pub confidence: f64,
    pub status: String,
    pub reasoning: Option<String>,
    pub created_at: DateTime<Utc>,
    pub applied_at: Option<DateTime<Utc>>,
}

/// Row struct for the `task_estimation_history` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskEstimationRow {
    pub id: String,
    pub task_id: String,
    pub estimated_minutes: i32,
    pub actual_minutes: i32,
    pub deviation_pct: f64,
    pub complexity_score: Option<i32>,
    pub energy_level: Option<String>,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub project_id: Option<String>,
    pub completed_at: DateTime<Utc>,
}

/// Row struct for the `task_attachments` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAttachmentRow {
    pub id: uuid::Uuid,
    pub task_id: String,
    pub attachment_type: String,
    pub value: String,
    pub title: Option<String>,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub source: String,
}

/// Row struct for the `task_time_entries` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskTimeEntryRow {
    pub id: uuid::Uuid,
    pub task_id: String,
    pub source: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub note: Option<String>,
    pub energy_level: Option<String>,
}

/// Row struct for the `task_dependencies` edge table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDependencyRow {
    pub task_id: String,
    pub blocker_id: String,
    pub dep_type: String,
}
