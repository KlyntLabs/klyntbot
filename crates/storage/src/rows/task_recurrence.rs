//! Row struct for the `task_recurrence_templates` table.
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecurrenceTemplateRow {
    pub id: String,
    pub source_task_id: String,
    pub rrule: String,
    pub iana_tz: String,
    pub materialize_ahead: i64,
    pub next_instance_at_ms: Option<i64>,
    pub last_instance_at_ms: Option<i64>,
    pub until_at_ms: Option<i64>,
    pub count_remaining: Option<i64>,
    pub enabled: bool,
    pub created_at_ms: i64,
}
