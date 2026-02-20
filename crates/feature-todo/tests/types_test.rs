//! Unit tests for feature-todo domain types.

use feature_todo::types::{AttachmentType, TimeEntrySource, Todo, TodoStatus};

#[test]
fn test_todo_generate_id_unique() {
    use std::collections::HashSet;
    let ids: HashSet<String> = (0..100).map(|_| Todo::generate_id()).collect();
    assert_eq!(ids.len(), 100, "All 100 generated IDs should be unique");
}

#[test]
fn test_todo_generate_id_length() {
    let id = Todo::generate_id();
    assert_eq!(id.len(), 8, "ID should be exactly 8 characters");
}

#[test]
fn test_default_instance_not_template() {
    let todo = Todo::default_instance();
    assert!(
        !todo.is_template,
        "default_instance should not be a template"
    );
    assert!(todo.blocked_by.is_empty());
    assert!(todo.blocks.is_empty());
    assert!(todo.attachments.is_empty());
}

#[test]
fn test_todo_status_from_str_loose() {
    assert_eq!(TodoStatus::from_str_loose("todo"), Some(TodoStatus::Todo));
    assert_eq!(TodoStatus::from_str_loose("TODO"), Some(TodoStatus::Todo));
    assert_eq!(TodoStatus::from_str_loose("doing"), Some(TodoStatus::Doing));
    assert_eq!(TodoStatus::from_str_loose("done"), Some(TodoStatus::Done));
    assert_eq!(
        TodoStatus::from_str_loose("archived"),
        Some(TodoStatus::Archived)
    );
    assert_eq!(TodoStatus::from_str_loose("unknown"), None);
}

#[test]
fn test_todo_status_display() {
    assert_eq!(TodoStatus::Todo.as_str(), "todo");
    assert_eq!(TodoStatus::Done.as_str(), "done");
    assert_eq!(format!("{}", TodoStatus::Doing), "doing");
}

#[test]
fn test_todo_serde_round_trip() {
    let mut todo = Todo::default_instance();
    todo.title = "Test task".to_string();
    todo.priority = Some(2);
    todo.tags = vec!["work".to_string(), "urgent".to_string()];

    let json = serde_json::to_string(&todo).unwrap();
    let parsed: Todo = serde_json::from_str(&json).unwrap();

    assert_eq!(parsed.title, "Test task");
    assert_eq!(parsed.priority, Some(2));
    assert_eq!(parsed.tags, vec!["work", "urgent"]);
    assert!(!parsed.is_template);
}

#[test]
fn test_todo_serde_backward_compat() {
    // Old JSON without extended fields should deserialize with defaults
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
    assert!(todo.blocked_by.is_empty());
    assert!(todo.blocks.is_empty());
}

#[test]
fn test_attachment_type_round_trip() {
    assert_eq!(AttachmentType::parse("file"), Some(AttachmentType::File));
    assert_eq!(AttachmentType::parse("url"), Some(AttachmentType::Url));
    assert_eq!(AttachmentType::parse("note"), Some(AttachmentType::Note));
    assert_eq!(AttachmentType::parse("unknown"), None);
}

#[test]
fn test_time_entry_source_default() {
    let source = TimeEntrySource::default();
    assert_eq!(source, TimeEntrySource::Focus);
}
