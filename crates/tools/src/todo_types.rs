//! Action system types (formerly Todo types)
//!
//! Re-exports core domain types from `feature-todo` and provides
//! tool-layer abstractions (`ActionToolFilter`, `ActionToolPatch`)
//! with typed conversions to storage-layer equivalents.

use chrono::{DateTime, Utc};
use std::collections::HashMap;

// ── Re-exports from feature-todo (canonical definitions) ────────────────────
pub use feature_todo::types::{
    Action, ActionStatus, Attachment, AttachmentType, TimeEntry, TimeEntrySource, Todo, TodoStatus,
};

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

/// Convert a domain `TodoPatch` into a storage `ActionPatch`.
impl TodoPatch {
    pub fn to_storage_patch(&self, id: &str) -> storage::ActionPatch {
        storage::ActionPatch {
            id: id.to_string(),
            title: self.title.clone(),
            description: self.description.clone(),
            priority: self.priority.map(|p| Some(p as i16)),
            due_date: self.due_date,
            tags: self.tags.clone(),
            status: self.status.map(|s| s.as_str().to_string()),
            calendar_event_uid: self.calendar_event_uid.clone(),
            next_instance_date: None,
            estimated_minutes: self.estimated_minutes.map(|opt| opt.map(|m| m as i32)),
            last_reminded_at: self.last_reminded_at,
            recurrence_rule: None,
            area_id: self.area_id.clone(),
            project_id: None,
            key_result_id: self.key_result_id.clone(),
            status_label_id: None,
            position: None,
        }
    }
}

/// Convert a domain `TodoFilter` into a storage `ActionFilter`.
impl TodoFilter {
    pub fn to_storage_filter(&self) -> storage::ActionFilter {
        storage::ActionFilter {
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
