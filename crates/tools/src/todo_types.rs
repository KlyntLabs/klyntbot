//! Action system types (formerly Todo types)
//!
//! Defines core domain types for the legacy action system and provides
//! tool-layer abstractions (`TodoPatch`, `TodoFilter`, `TodoSummary`)
//! with typed conversions to storage-layer equivalents.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use storage::TaskRow;

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

impl From<TaskRow> for Action {
    fn from(row: TaskRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            description: row.description,
            area_id: row.area_id,
            key_result_id: row.key_result_id,
            priority: row.priority.map(|p| p as u8),
            due_date: row.due_date.map(|ts| common::time::bridge::jiff_to_chrono(*ts)),
            tags: row.tags,
            status: ActionStatus::from_str_loose(&row.status).unwrap_or(ActionStatus::Todo),
            focused_at: row.focused_at.map(|ts| common::time::bridge::jiff_to_chrono(*ts)),
            focus_deadline: row.focus_deadline.map(|ts| common::time::bridge::jiff_to_chrono(*ts)),
            focus_expired_count: row.focus_expired_count as u32,
            created_at: common::time::bridge::jiff_to_chrono(*row.created_at),
            updated_at: common::time::bridge::jiff_to_chrono(*row.updated_at),
            completed_at: row.completed_at.map(|ts| common::time::bridge::jiff_to_chrono(*ts)),
            parent_id: row.parent_id,
            project_id: row.project_id,
            attachments: Vec::new(),
            time_entries: Vec::new(),
            total_tracked_secs: row.total_tracked_secs as u64,
            estimated_minutes: row.estimated_minutes.map(|m| m as u32),
            calendar_event_uid: row.calendar_event_uid,
            last_reminded_at: row.last_reminded_at.map(|ts| common::time::bridge::jiff_to_chrono(*ts)),
            recurrence_rule: row.recurrence_rule,
            recurrence_parent_id: row.recurrence_parent_id,
            is_template: row.is_template,
            next_instance_date: row.next_instance_date.map(|ts| common::time::bridge::jiff_to_chrono(*ts)),
            blocked_by: Vec::new(),
            blocks: Vec::new(),
            status_label_id: row.status_label_id,
            position: row.position,
            group_id: row.group_id,
        }
    }
}

impl From<&Action> for TaskRow {
    fn from(action: &Action) -> Self {
        Self {
            id: action.id.clone(),
            title: action.title.clone(),
            description: action.description.clone(),
            area_id: action.area_id.clone(),
            key_result_id: action.key_result_id.clone(),
            priority: action.priority.map(|p| p as i16),
            due_date: action.due_date.map(|dt| storage::sqlite_types::SqlTs(common::time::bridge::chrono_to_jiff(dt))),
            tags: action.tags.clone(),
            status: action.status.as_str().to_string(),
            focused_at: action.focused_at.map(|dt| storage::sqlite_types::SqlTs(common::time::bridge::chrono_to_jiff(dt))),
            focus_deadline: action.focus_deadline.map(|dt| storage::sqlite_types::SqlTs(common::time::bridge::chrono_to_jiff(dt))),
            focus_expired_count: action.focus_expired_count as i32,
            created_at: storage::sqlite_types::SqlTs(common::time::bridge::chrono_to_jiff(action.created_at)),
            updated_at: storage::sqlite_types::SqlTs(common::time::bridge::chrono_to_jiff(action.updated_at)),
            completed_at: action.completed_at.map(|dt| storage::sqlite_types::SqlTs(common::time::bridge::chrono_to_jiff(dt))),
            parent_id: action.parent_id.clone(),
            project_id: action.project_id.clone(),
            total_tracked_secs: action.total_tracked_secs as i64,
            estimated_minutes: action.estimated_minutes.map(|m| m as i32),
            calendar_event_uid: action.calendar_event_uid.clone(),
            last_reminded_at: action.last_reminded_at.map(|dt| storage::sqlite_types::SqlTs(common::time::bridge::chrono_to_jiff(dt))),
            recurrence_rule: action.recurrence_rule.clone(),
            recurrence_parent_id: action.recurrence_parent_id.clone(),
            is_template: action.is_template,
            next_instance_date: action.next_instance_date.map(|dt| storage::sqlite_types::SqlTs(common::time::bridge::chrono_to_jiff(dt))),
            status_label_id: action.status_label_id.clone(),
            position: action.position,
            group_id: action.group_id.clone(),
            task_type: "standard".to_string(),
            acceptance_criteria: None,
            agent_config: None,
            execution_state: "idle".to_string(),
            spawned_execution_id: None,
            context_snapshot: None,
            energy_level: None,
            estimated_focus_blocks: None,
            actual_minutes: None,
            complexity_score: None,
            completed: action.status == ActionStatus::Done,
            objective_id: None,
            scheduled_start: None,
            scheduled_end: None,
        }
    }
}

/// Searchable implementation for integration with tools-core RRF search.
impl tools_core::Searchable for Action {
    fn search_id(&self) -> &str {
        &self.id
    }
}

/// Generate a short 8-character ID from a random UUID.
pub fn generate_short_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

/// Partial update for existing actions (tool-layer abstraction).
#[derive(Debug, Clone, Default)]
pub struct TodoPatch {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<u8>,
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<TodoStatus>,
    pub last_reminded_at: Option<Option<DateTime<Utc>>>,
    pub calendar_event_uid: Option<Option<String>>,
    pub estimated_minutes: Option<Option<u32>>,
    pub area_id: Option<String>,
    pub key_result_id: Option<Option<String>>,
}

/// Filter criteria for listing actions (tool-layer abstraction).
#[derive(Debug, Clone, Default)]
pub struct TodoFilter {
    pub status: Option<TodoStatus>,
    pub priority_min: Option<u8>,
    pub tag: Option<String>,
    pub limit: Option<usize>,
    pub area_id: Option<String>,
    pub project_id: Option<String>,
    pub key_result_id: Option<String>,
    pub parent_id: Option<String>,
    pub unassigned: bool,
    pub include_templates: bool,
}

/// Summary statistics for action collection.
#[derive(Debug, Clone)]
pub struct TodoSummary {
    pub total: usize,
    pub by_status: HashMap<TodoStatus, usize>,
    pub overdue: Vec<String>,
    pub upcoming_week: Vec<String>,
}

/// Convert a domain `TodoPatch` into a storage `TaskPatch`.
impl TodoPatch {
    pub fn to_storage_patch(&self, id: &str) -> storage::TaskPatch {
        storage::TaskPatch {
            id: id.to_string(),
            title: self.title.clone(),
            description: self.description.clone(),
            priority: self.priority.map(|p| Some(p as i16)),
            due_date: self.due_date.map(|opt| opt.map(common::time::bridge::chrono_to_jiff)),
            tags: self.tags.clone(),
            status: self.status.map(|s| s.as_str().to_string()),
            calendar_event_uid: self.calendar_event_uid.clone(),
            next_instance_date: None,
            estimated_minutes: self.estimated_minutes.map(|opt| opt.map(|m| m as i32)),
            last_reminded_at: self.last_reminded_at.map(|opt| opt.map(common::time::bridge::chrono_to_jiff)),
            recurrence_rule: None,
            area_id: self.area_id.clone(),
            project_id: None,
            key_result_id: self.key_result_id.clone(),
            status_label_id: None,
            position: None,
            group_id: None,
            ..Default::default()
        }
    }
}

/// Convert a domain `TodoFilter` into a storage `TaskFilter`.
impl TodoFilter {
    pub fn to_storage_filter(&self) -> storage::TaskFilter {
        storage::TaskFilter {
            status: self.status.map(|s| s.as_str().to_string()),
            tags: self.tag.as_ref().map(|t| vec![t.clone()]),
            area_id: self.area_id.clone(),
            project_id: self.project_id.clone(),
            key_result_id: self.key_result_id.clone(),
            unassigned: self.unassigned,
            priority_min: self.priority_min.map(|p| p as i16),
            limit: self.limit.map(|l| l as i64),
            due_after: None,
            due_before: None,
            templates_only: self.include_templates,
            root_only: false,
            status_group: None,
            group_id: None,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id1 = Todo::generate_id();
        let id2 = Todo::generate_id();

        assert_eq!(id1.len(), 8, "ID should be 8 characters");
        assert_eq!(id2.len(), 8, "ID should be 8 characters");
        assert_ne!(id1, id2, "IDs should be unique");
    }

    #[test]
    fn test_generate_id_uniqueness_100() {
        use std::collections::HashSet;
        let mut ids = HashSet::new();

        for _ in 0..100 {
            let id = Todo::generate_id();
            assert_eq!(id.len(), 8, "ID should be 8 characters");
            assert!(
                ids.insert(id.clone()),
                "ID {} already exists - not unique!",
                id
            );
        }

        assert_eq!(ids.len(), 100, "Should have 100 unique IDs");
    }

    #[test]
    fn test_serde_with_area_id() {
        let json = r#"{
            "id": "abc12345",
            "title": "Old task",
            "area_id": "work",
            "description": null,
            "priority": null,
            "due_date": null,
            "tags": [],
            "status": "todo",
            "focused_at": null,
            "focus_deadline": null,
            "focus_expired_count": 0,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z",
            "completed_at": null
        }"#;
        let todo: Todo = serde_json::from_str(json).unwrap();
        assert_eq!(todo.id, "abc12345");
        assert_eq!(todo.area_id, "work");
        assert!(!todo.is_template);
        assert!(todo.recurrence_rule.is_none());
        assert!(todo.recurrence_parent_id.is_none());
        assert!(todo.next_instance_date.is_none());
        assert!(todo.blocked_by.is_empty());
        assert!(todo.blocks.is_empty());
        assert!(todo.parent_id.is_none());
        assert!(todo.project_id.is_none());
    }

    #[test]
    fn test_new_fields_serde_round_trip() {
        let mut todo = Todo::default_instance();
        todo.area_id = "work".to_string();
        todo.is_template = true;
        todo.recurrence_rule = Some("FREQ=DAILY;BYHOUR=9".to_string());
        todo.blocked_by = vec!["task1".to_string(), "task2".to_string()];
        todo.blocks = vec!["task3".to_string()];

        let json = serde_json::to_string(&todo).unwrap();
        let parsed: Todo = serde_json::from_str(&json).unwrap();

        assert!(parsed.is_template);
        assert_eq!(
            parsed.recurrence_rule,
            Some("FREQ=DAILY;BYHOUR=9".to_string())
        );
        assert_eq!(parsed.blocked_by, vec!["task1", "task2"]);
        assert_eq!(parsed.blocks, vec!["task3"]);
    }
}
