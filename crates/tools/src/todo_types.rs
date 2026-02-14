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

    // ── New fields (Phase 1 additions) ──
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

impl Todo {
    /// Generate short ID (8 chars) from UUID
    pub fn generate_id() -> String {
        uuid::Uuid::new_v4().to_string()[..8].to_string()
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
}

/// Filter criteria for listing todos
#[derive(Debug, Clone, Default)]
pub struct TodoFilter {
    pub status: Option<TodoStatus>,
    pub priority_min: Option<u8>,
    pub tag: Option<String>,
    pub limit: Option<usize>,
    // Phase 2 additions
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
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
}
