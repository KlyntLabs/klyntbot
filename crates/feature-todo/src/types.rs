//! Domain types for the action system (formerly "todo").
//!
//! Canonical definitions of `Action`, `ActionStatus`, `Attachment`, `TimeEntry`,
//! and their conversions from/to storage row types. Re-exported by
//! `tools::todo_types` for use across the workspace.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use storage::{ActionAttachmentRow, ActionRow, ActionTimeEntryRow};

// ── Type aliases for backward compatibility ──────────────────────────────────
pub type Todo = Action;
pub type TodoStatus = ActionStatus;

/// An action item (task) with focus tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub area_id: String,
    pub key_result_id: Option<String>,
    pub priority: Option<u8>,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub status: ActionStatus,
    pub focused_at: Option<DateTime<Utc>>,
    pub focus_deadline: Option<DateTime<Utc>>,
    pub focus_expired_count: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub time_entries: Vec<TimeEntry>,
    #[serde(default)]
    pub total_tracked_secs: u64,
    #[serde(default)]
    pub estimated_minutes: Option<u32>,
    #[serde(default)]
    pub calendar_event_uid: Option<String>,
    #[serde(default)]
    pub last_reminded_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub recurrence_rule: Option<String>,
    #[serde(default)]
    pub recurrence_parent_id: Option<String>,
    #[serde(default)]
    pub is_template: bool,
    #[serde(default)]
    pub next_instance_date: Option<DateTime<Utc>>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub blocks: Vec<String>,
    #[serde(default)]
    pub status_label_id: Option<String>,
    #[serde(default)]
    pub position: i32,
    #[serde(default)]
    pub group_id: Option<String>,
}

impl Action {
    /// Generate an 8-char short ID from a UUID.
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
    }

    /// Create a default blank instance (useful as a starting point).
    pub fn default_instance() -> Self {
        let now = Utc::now();
        Self {
            id: Self::generate_id(),
            title: String::new(),
            description: None,
            area_id: String::new(),
            key_result_id: None,
            priority: None,
            due_date: None,
            tags: Vec::new(),
            status: ActionStatus::Todo,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            parent_id: None,
            project_id: None,
            attachments: Vec::new(),
            time_entries: Vec::new(),
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
            status_label_id: None,
            position: 0,
            group_id: None,
        }
    }
}

/// Task status lifecycle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ActionStatus {
    Todo,
    Doing,
    Done,
    Archived,
}

impl ActionStatus {
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "todo" => Some(Self::Todo),
            "doing" => Some(Self::Doing),
            "done" => Some(Self::Done),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Todo => "todo",
            Self::Doing => "doing",
            Self::Done => "done",
            Self::Archived => "archived",
        }
    }

    /// Map legacy status to status_group for AI queries.
    pub fn to_group(&self) -> &'static str {
        match self {
            Self::Todo => "not_started",
            Self::Doing => "active",
            Self::Done => "done",
            Self::Archived => "done",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Todo => "Todo",
            Self::Doing => "Doing",
            Self::Done => "Done",
            Self::Archived => "Archived",
        }
    }
}

impl std::fmt::Display for ActionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Attachment to an action (file, URL, or note).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    #[serde(rename = "type")]
    pub attachment_type: AttachmentType,
    pub title: Option<String>,
    pub value: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Type of attachment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    File,
    Url,
    Note,
}

impl AttachmentType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "file" => Some(Self::File),
            "url" => Some(Self::Url),
            "note" => Some(Self::Note),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Url => "url",
            Self::Note => "note",
        }
    }
}

/// Time tracking entry for an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<u64>,
    pub note: Option<String>,
    #[serde(default)]
    pub source: TimeEntrySource,
}

/// Source of a time entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimeEntrySource {
    #[default]
    Focus,
    Manual,
}

// ── Row <-> Domain Conversions ──────────────────────────────────────────────

impl From<ActionRow> for Action {
    fn from(row: ActionRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            area_id: row.area_id,
            key_result_id: row.key_result_id,
            priority: row.priority.map(|p| p as u8),
            due_date: row.due_date,
            tags: row.tags,
            status: ActionStatus::from_str_loose(&row.status).unwrap_or(ActionStatus::Todo),
            focused_at: row.focused_at,
            focus_deadline: row.focus_deadline,
            focus_expired_count: row.focus_expired_count as u32,
            created_at: row.created_at,
            updated_at: row.updated_at,
            completed_at: row.completed_at,
            parent_id: row.parent_id,
            project_id: row.project_id,
            attachments: Vec::new(),
            time_entries: Vec::new(),
            total_tracked_secs: row.total_tracked_secs as u64,
            estimated_minutes: row.estimated_minutes.map(|m| m as u32),
            calendar_event_uid: row.calendar_event_uid,
            last_reminded_at: row.last_reminded_at,
            recurrence_rule: row.recurrence_rule,
            recurrence_parent_id: row.recurrence_parent_id,
            is_template: row.is_template,
            next_instance_date: row.next_instance_date,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
            status_label_id: row.status_label_id,
            position: row.position,
            group_id: row.group_id,
        }
    }
}

impl From<&Action> for ActionRow {
    fn from(action: &Action) -> Self {
        Self {
            id: action.id.clone(),
            title: action.title.clone(),
            description: action.description.clone(),
            area_id: action.area_id.clone(),
            key_result_id: action.key_result_id.clone(),
            priority: action.priority.map(|p| p as i16),
            due_date: action.due_date,
            tags: action.tags.clone(),
            status: action.status.as_str().to_string(),
            focused_at: action.focused_at,
            focus_deadline: action.focus_deadline,
            focus_expired_count: action.focus_expired_count as i32,
            created_at: action.created_at,
            updated_at: action.updated_at,
            completed_at: action.completed_at,
            parent_id: action.parent_id.clone(),
            project_id: action.project_id.clone(),
            total_tracked_secs: action.total_tracked_secs as i64,
            estimated_minutes: action.estimated_minutes.map(|m| m as i32),
            calendar_event_uid: action.calendar_event_uid.clone(),
            last_reminded_at: action.last_reminded_at,
            recurrence_rule: action.recurrence_rule.clone(),
            recurrence_parent_id: action.recurrence_parent_id.clone(),
            is_template: action.is_template,
            next_instance_date: action.next_instance_date,
            status_label_id: action.status_label_id.clone(),
            position: action.position,
            group_id: action.group_id.clone(),
        }
    }
}

impl From<ActionAttachmentRow> for Attachment {
    fn from(row: ActionAttachmentRow) -> Self {
        Self {
            id: row.id.to_string(),
            attachment_type: match row.attachment_type.as_str() {
                "file" => AttachmentType::File,
                "url" => AttachmentType::Url,
                _ => AttachmentType::Note,
            },
            title: row.title,
            value: row.value,
            tags: row.tags,
            created_at: row.created_at,
        }
    }
}

impl From<ActionTimeEntryRow> for TimeEntry {
    fn from(row: ActionTimeEntryRow) -> Self {
        Self {
            id: row.id.to_string(),
            started_at: row.started_at,
            ended_at: row.ended_at,
            duration_secs: row.duration_secs.map(|s| s as u64),
            note: row.note,
            source: match row.source.as_str() {
                "manual" => TimeEntrySource::Manual,
                _ => TimeEntrySource::Focus,
            },
        }
    }
}

/// Searchable implementation for integration with tools-core RRF search.
impl tools_core::Searchable for Action {
    fn search_id(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id_length() {
        let id = Action::generate_id();
        assert_eq!(id.len(), 8);
    }

    #[test]
    fn test_generate_id_uniqueness() {
        use std::collections::HashSet;
        let ids: HashSet<String> = (0..50).map(|_| Action::generate_id()).collect();
        assert_eq!(ids.len(), 50);
    }

    #[test]
    fn test_action_status_round_trip() {
        assert_eq!(
            ActionStatus::from_str_loose("todo"),
            Some(ActionStatus::Todo)
        );
        assert_eq!(
            ActionStatus::from_str_loose("DONE"),
            Some(ActionStatus::Done)
        );
        assert_eq!(ActionStatus::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_serde_round_trip() {
        let mut t = Action::default_instance();
        t.area_id = "area-1".to_string();
        let json = serde_json::to_string(&t).unwrap();
        let parsed: Action = serde_json::from_str(&json).unwrap();
        assert_eq!(t.id, parsed.id);
        assert!(!parsed.is_template);
    }

    // Backward compat aliases
    #[test]
    fn test_type_aliases() {
        let _: Todo = Action::default_instance();
        let _: TodoStatus = ActionStatus::Todo;
    }
}
