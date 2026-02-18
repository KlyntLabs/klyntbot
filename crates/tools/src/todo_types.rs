//! Todo system types
//!
//! This module defines the core data structures for the todo system,
//! including tasks, statuses, filters, and summaries.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A task/todo item with focus tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    // ── Existing fields (unchanged) ──
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

    // ── Extended fields ──
    #[serde(default)]
    pub parent_id: Option<String>, // Hierarchical: links to parent todo

    #[serde(default)]
    pub project_id: Option<String>, // Project grouping

    #[serde(default)]
    pub attachments: Vec<Attachment>, // Inline attachments

    #[serde(default)]
    pub time_entries: Vec<TimeEntry>, // Time tracking log

    #[serde(default)]
    pub total_tracked_secs: u64, // Denormalized total (avoids summing entries)

    #[serde(default)]
    pub estimated_minutes: Option<u32>, // For calendar event duration

    #[serde(default)]
    pub calendar_event_uid: Option<String>, // CalDAV UID link

    #[serde(default)]
    pub last_reminded_at: Option<DateTime<Utc>>, // Notification dedup

    // ── Recurring tasks ──
    #[serde(default)]
    pub recurrence_rule: Option<String>, // RRULE string, e.g. "FREQ=DAILY;BYHOUR=9"

    #[serde(default)]
    pub recurrence_parent_id: Option<String>, // Links spawned instance → template

    #[serde(default)]
    pub is_template: bool, // true = recurring template, filtered from normal lists

    #[serde(default)]
    pub next_instance_date: Option<DateTime<Utc>>, // Cached next occurrence

    // ── Task dependencies ──
    #[serde(default)]
    pub blocked_by: Vec<String>, // IDs of tasks this task depends on

    #[serde(default)]
    pub blocks: Vec<String>, // IDs of tasks that depend on this task
}

/// Task status lifecycle
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    Todo,
    Doing,
    Done,
    Archived,
}

impl TodoStatus {
    /// Parse a status string loosely (case-insensitive, common aliases).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "todo" => Some(Self::Todo),
            "doing" => Some(Self::Doing),
            "done" => Some(Self::Done),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }

    /// Human-readable display name.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Todo => "Todo",
            Self::Doing => "Doing",
            Self::Done => "Done",
            Self::Archived => "Archived",
        }
    }
}

/// Generate a short 8-character ID from a random UUID.
/// Used by both `Todo` and `Project` for ID generation.
pub fn generate_short_id() -> String {
    uuid::Uuid::new_v4().to_string()[..8].to_string()
}

impl Todo {
    /// Generate short ID (8 chars) from UUID
    pub fn generate_id() -> String {
        generate_short_id()
    }

    /// Create a default instance suitable for spawning from a template.
    /// All fields are set to their zero/empty/None values. Callers should
    /// override id, title, description, priority, tags, due_date, project_id,
    /// and recurrence_parent_id as needed.
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

/// Partial update for existing todos
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
}

/// Filter criteria for listing todos
#[derive(Debug, Clone, Default)]
pub struct TodoFilter {
    pub status: Option<TodoStatus>,
    pub priority_min: Option<u8>,
    pub tag: Option<String>,
    pub limit: Option<usize>,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    // Template filtering (default false — templates hidden from normal lists)
    pub include_templates: bool,
}

/// Summary statistics for todo collection
#[derive(Debug, Clone)]
pub struct TodoSummary {
    pub total: usize,
    pub by_status: HashMap<TodoStatus, usize>,
    pub overdue: Vec<String>,
    pub upcoming_week: Vec<String>,
}

/// Attachment to a todo (file, URL, or note)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String, // 8-char UUID
    #[serde(rename = "type")]
    pub attachment_type: AttachmentType,
    pub title: Option<String>,
    pub value: String, // Path, URL, or note content
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Type of attachment
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AttachmentType {
    File,
    Url,
    Note,
}

/// Time tracking entry for a todo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: String, // 8-char UUID
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>, // None = still running
    pub duration_secs: Option<u64>,      // Computed on close
    pub note: Option<String>,
    #[serde(default)]
    pub source: TimeEntrySource,
}

/// Source of a time entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimeEntrySource {
    #[default]
    Focus, // Auto-created by focus/unfocus
    Manual, // Created by log_time action
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
        // AC-9.12: Test 100 generations, all unique
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
    fn test_todo_status_serde() {
        use serde_json;

        // Serialize
        let status = TodoStatus::Doing;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"doing\"");

        // Deserialize
        let parsed: TodoStatus = serde_json::from_str("\"todo\"").unwrap();
        assert_eq!(parsed, TodoStatus::Todo);
    }

    #[test]
    fn test_todo_filter_default() {
        let filter = TodoFilter::default();
        assert!(filter.status.is_none());
        assert!(filter.priority_min.is_none());
        assert!(filter.tag.is_none());
        assert!(filter.limit.is_none());
        assert!(!filter.include_templates);
    }

    #[test]
    fn test_todo_patch_default() {
        let patch = TodoPatch::default();
        assert!(patch.title.is_none());
        assert!(patch.description.is_none());
        assert!(patch.priority.is_none());
        assert!(patch.due_date.is_none());
        assert!(patch.tags.is_none());
        assert!(patch.status.is_none());
    }

    #[test]
    fn test_default_instance() {
        let todo = Todo::default_instance();
        assert_eq!(todo.id.len(), 8);
        assert!(todo.title.is_empty());
        assert_eq!(todo.status, TodoStatus::Todo);
        assert!(!todo.is_template);
        assert!(todo.recurrence_rule.is_none());
        assert!(todo.recurrence_parent_id.is_none());
        assert!(todo.next_instance_date.is_none());
        assert!(todo.blocked_by.is_empty());
        assert!(todo.blocks.is_empty());
    }

    #[test]
    fn test_backward_compat_serde_without_new_fields() {
        // Verify existing JSONL entries (without new fields) deserialize correctly
        let json = r#"{
            "id": "abc12345",
            "title": "Old task",
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
