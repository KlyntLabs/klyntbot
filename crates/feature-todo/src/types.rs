//! Domain types for the feature-todo crate.
//!
//! These mirror `tools::todo_types` but are self-contained within this crate,
//! avoiding a dependency on the `tools` crate.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::storage::rows::{TodoAttachmentRow, TodoTimeEntryRow};
use crate::storage::TodoRow;

/// A task/todo item with focus tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<u8>,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub status: TodoStatus,
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
}

impl Todo {
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
            priority: None,
            due_date: None,
            tags: Vec::new(),
            status: TodoStatus::Todo,
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
        }
    }
}

/// Task status lifecycle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    Todo,
    Doing,
    Done,
    Archived,
}

impl TodoStatus {
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

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Todo => "Todo",
            Self::Doing => "Doing",
            Self::Done => "Done",
            Self::Archived => "Archived",
        }
    }
}

impl std::fmt::Display for TodoStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Attachment to a todo (file, URL, or note).
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

/// Time tracking entry for a todo.
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

// ── Row ↔ Domain Conversions ──────────────────────────────────────────────────

impl From<TodoRow> for Todo {
    fn from(row: TodoRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            priority: row.priority.map(|p| p as u8),
            due_date: row.due_date,
            tags: row.tags,
            status: TodoStatus::from_str_loose(&row.status).unwrap_or(TodoStatus::Todo),
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
        }
    }
}

impl From<&Todo> for TodoRow {
    fn from(todo: &Todo) -> Self {
        Self {
            id: todo.id.clone(),
            title: todo.title.clone(),
            description: todo.description.clone(),
            priority: todo.priority.map(|p| p as i16),
            due_date: todo.due_date,
            tags: todo.tags.clone(),
            status: todo.status.as_str().to_string(),
            focused_at: todo.focused_at,
            focus_deadline: todo.focus_deadline,
            focus_expired_count: todo.focus_expired_count as i32,
            created_at: todo.created_at,
            updated_at: todo.updated_at,
            completed_at: todo.completed_at,
            parent_id: todo.parent_id.clone(),
            project_id: todo.project_id.clone(),
            total_tracked_secs: todo.total_tracked_secs as i64,
            estimated_minutes: todo.estimated_minutes.map(|m| m as i32),
            calendar_event_uid: todo.calendar_event_uid.clone(),
            last_reminded_at: todo.last_reminded_at,
            recurrence_rule: todo.recurrence_rule.clone(),
            recurrence_parent_id: todo.recurrence_parent_id.clone(),
            is_template: todo.is_template,
            next_instance_date: todo.next_instance_date,
        }
    }
}

// Bridge conversion from storage::TodoRow (identical layout to feature_todo::TodoRow).
// Needed because the agent crate reads through storage::TodoRepo but uses feature_todo::Todo.
impl From<storage::TodoRow> for Todo {
    fn from(row: storage::TodoRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            priority: row.priority.map(|p| p as u8),
            due_date: row.due_date,
            tags: row.tags,
            status: TodoStatus::from_str_loose(&row.status).unwrap_or(TodoStatus::Todo),
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
        }
    }
}

impl Todo {
    /// Convert to a `storage::TodoRow` for direct persistence via `storage::TodoRepo`.
    pub fn to_storage_row(&self) -> storage::TodoRow {
        storage::TodoRow {
            id: self.id.clone(),
            title: self.title.clone(),
            description: self.description.clone(),
            priority: self.priority.map(|p| p as i16),
            due_date: self.due_date,
            tags: self.tags.clone(),
            status: self.status.as_str().to_string(),
            focused_at: self.focused_at,
            focus_deadline: self.focus_deadline,
            focus_expired_count: self.focus_expired_count as i32,
            created_at: self.created_at,
            updated_at: self.updated_at,
            completed_at: self.completed_at,
            parent_id: self.parent_id.clone(),
            project_id: self.project_id.clone(),
            total_tracked_secs: self.total_tracked_secs as i64,
            estimated_minutes: self.estimated_minutes.map(|m| m as i32),
            calendar_event_uid: self.calendar_event_uid.clone(),
            last_reminded_at: self.last_reminded_at,
            recurrence_rule: self.recurrence_rule.clone(),
            recurrence_parent_id: self.recurrence_parent_id.clone(),
            is_template: self.is_template,
            next_instance_date: self.next_instance_date,
        }
    }
}

impl From<TodoAttachmentRow> for Attachment {
    fn from(row: TodoAttachmentRow) -> Self {
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

impl From<TodoTimeEntryRow> for TimeEntry {
    fn from(row: TodoTimeEntryRow) -> Self {
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
impl tools_core::Searchable for Todo {
    fn search_id(&self) -> &str {
        &self.id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id_length() {
        let id = Todo::generate_id();
        assert_eq!(id.len(), 8);
    }

    #[test]
    fn test_generate_id_uniqueness() {
        use std::collections::HashSet;
        let ids: HashSet<String> = (0..50).map(|_| Todo::generate_id()).collect();
        assert_eq!(ids.len(), 50);
    }

    #[test]
    fn test_todo_status_round_trip() {
        assert_eq!(TodoStatus::from_str_loose("todo"), Some(TodoStatus::Todo));
        assert_eq!(TodoStatus::from_str_loose("DONE"), Some(TodoStatus::Done));
        assert_eq!(TodoStatus::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_default_instance_not_template() {
        let t = Todo::default_instance();
        assert!(!t.is_template);
        assert!(t.blocked_by.is_empty());
    }

    #[test]
    fn test_serde_round_trip() {
        let t = Todo::default_instance();
        let json = serde_json::to_string(&t).unwrap();
        let parsed: Todo = serde_json::from_str(&json).unwrap();
        assert_eq!(t.id, parsed.id);
        assert!(!parsed.is_template);
    }
}
