//! Repository for the `tasks` table and its satellite tables
//! (`task_attachments`, `task_time_entries`, `task_dependencies`,
//!  `task_activity`, `task_executions`, `task_suggestions`, `task_estimation_history`).

mod activity;
mod attachments;
mod core;
mod decompositions;
mod dependencies;
mod estimations;
mod executions;
mod focus;
mod hierarchy;
mod reporting;
mod suggestions;
mod time_entries;

#[cfg(test)]
mod tests;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

/// Filter criteria for listing tasks.
#[derive(Debug, Default, Clone)]
pub struct TaskFilter {
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub area_id: Option<String>,
    pub project_id: Option<String>,
    pub key_result_id: Option<String>,
    pub unassigned: bool,
    pub root_only: bool,
    pub priority_min: Option<i16>,
    pub due_after: Option<DateTime<Utc>>,
    pub due_before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub templates_only: bool,
    pub status_group: Option<String>,
    pub group_id: Option<String>,
    pub task_type: Option<String>,
    pub execution_state: Option<String>,
    pub energy_level: Option<String>,
    pub completed: Option<bool>,
}

/// Patch struct for partial task updates. Only non-None fields are overwritten.
#[derive(Debug, Default, Clone)]
pub struct TaskPatch {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Option<i16>>,
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub calendar_event_uid: Option<Option<String>>,
    pub next_instance_date: Option<Option<DateTime<Utc>>>,
    pub last_reminded_at: Option<Option<DateTime<Utc>>>,
    pub estimated_minutes: Option<Option<i32>>,
    pub recurrence_rule: Option<Option<String>>,
    pub area_id: Option<String>,
    pub project_id: Option<Option<String>>,
    pub key_result_id: Option<Option<String>>,
    pub status_label_id: Option<Option<String>>,
    pub position: Option<i32>,
    pub group_id: Option<Option<String>>,
    pub task_type: Option<String>,
    pub acceptance_criteria: Option<Option<String>>,
    pub agent_config: Option<Option<String>>,
    pub execution_state: Option<String>,
    pub spawned_execution_id: Option<Option<String>>,
    pub energy_level: Option<Option<String>>,
    pub complexity_score: Option<Option<i32>>,
    pub completed: Option<bool>,
    pub actual_minutes: Option<Option<i32>>,
    pub objective_id: Option<Option<String>>,
}

/// Aggregate counts by status.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub todo: i64,
    pub doing: i64,
    pub done: i64,
    pub total: i64,
}

/// Repository for task CRUD, hierarchy, focus, dependencies, attachments,
/// time tracking, activity log, executions, suggestions, and estimation.
#[derive(Debug, Clone)]
pub struct TaskRepo {
    pool: SqlitePool,
}

impl TaskRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
