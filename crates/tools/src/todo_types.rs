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
}

/// Filter criteria for listing todos
#[derive(Debug, Clone, Default)]
pub struct TodoFilter {
    pub status: Option<TodoStatus>,
    pub priority_min: Option<u8>,
    pub tag: Option<String>,
    pub limit: Option<usize>,
}

/// Summary statistics for todo collection
#[derive(Debug, Clone)]
pub struct TodoSummary {
    pub total: usize,
    pub by_status: HashMap<TodoStatus, usize>,
    pub overdue: Vec<String>,
    pub upcoming_week: Vec<String>,
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
