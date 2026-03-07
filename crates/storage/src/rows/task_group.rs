//! Row struct for the `task_groups` table.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `task_groups` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupRow {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub color: Option<String>,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}
