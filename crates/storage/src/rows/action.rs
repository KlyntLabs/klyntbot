//! Row structs for the `actions`, `action_attachments`, `action_time_entries`,
//! and `action_dependencies` tables.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `actions` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionRow {
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
}

/// Row struct for the `action_attachments` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionAttachmentRow {
    pub id: uuid::Uuid,
    pub action_id: String,
    pub attachment_type: String,
    pub value: String,
    pub title: Option<String>,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Row struct for the `action_time_entries` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionTimeEntryRow {
    pub id: uuid::Uuid,
    pub action_id: String,
    pub source: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub note: Option<String>,
}

/// Row struct for the `action_dependencies` edge table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionDependencyRow {
    pub action_id: String,
    pub blocker_id: String,
}
