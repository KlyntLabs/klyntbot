use chrono::{DateTime, Utc};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow)]
pub struct TodoRow {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub status: String,
    pub focused_at: Option<DateTime<Utc>>,
    pub focus_deadline: Option<DateTime<Utc>>,
    pub focus_expired_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub parent_id: Option<String>,
    pub project_id: Option<String>,
    pub total_tracked_secs: i64,
    pub estimated_minutes: Option<i32>,
    pub calendar_event_uid: Option<String>,
    pub last_reminded_at: Option<DateTime<Utc>>,
    pub recurrence_rule: Option<String>,
    pub recurrence_parent_id: Option<String>,
    pub is_template: bool,
    pub next_instance_date: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TodoAttachmentRow {
    pub id: uuid::Uuid,
    pub todo_id: String,
    pub attachment_type: String,
    pub value: String,
    pub title: Option<String>,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TodoTimeEntryRow {
    pub id: uuid::Uuid,
    pub todo_id: String,
    pub source: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
pub struct TodoDependencyRow {
    pub task_id: String,
    pub blocker_id: String,
}
