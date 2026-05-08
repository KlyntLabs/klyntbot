//! Core types for the coding TodoWrite tool.

pub use bus::domain_events::{ConcurrencyClass, TodoStatus};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub title: String,
    pub status: TodoStatus,
    pub concurrency: ConcurrencyClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked_by: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delegated_to: Option<String>,
    pub created_at: jiff::Timestamp,
    pub updated_at: jiff::Timestamp,
}

/// LLM-supplied partial shape; `id` is auto-assigned if absent,
/// `created_at`/`updated_at` are stamped by the handler.
#[derive(Debug, Clone, Deserialize)]
pub struct TodoItemInput {
    #[serde(default)]
    pub id: Option<String>,
    pub title: String,
    pub status: TodoStatus,
    pub concurrency: ConcurrencyClass,
    #[serde(default)]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    #[serde(default)]
    pub delegated_to: Option<String>,
}

impl From<TodoItem> for TodoItemInput {
    fn from(item: TodoItem) -> Self {
        Self {
            id: Some(item.id),
            title: item.title,
            status: item.status,
            concurrency: item.concurrency,
            blocked_reason: item.blocked_reason,
            blocked_by: item.blocked_by,
            delegated_to: item.delegated_to,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo_status_serializes_snake_case() {
        assert_eq!(serde_json::to_string(&TodoStatus::InProgress).unwrap(), "\"in_progress\"");
        assert_eq!(serde_json::to_string(&TodoStatus::Pending).unwrap(), "\"pending\"");
        assert_eq!(serde_json::to_string(&TodoStatus::Done).unwrap(), "\"done\"");
        assert_eq!(serde_json::to_string(&TodoStatus::Blocked).unwrap(), "\"blocked\"");
    }

    #[test]
    fn todo_status_deserializes_snake_case() {
        let v: TodoStatus = serde_json::from_str("\"in_progress\"").unwrap();
        assert_eq!(v, TodoStatus::InProgress);
    }

    #[test]
    fn todo_status_rejects_unknown() {
        let r: Result<TodoStatus, _> = serde_json::from_str("\"frobnicating\"");
        assert!(r.is_err());
    }

    #[test]
    fn concurrency_class_serializes_snake_case() {
        assert_eq!(serde_json::to_string(&ConcurrencyClass::Safe).unwrap(), "\"safe\"");
        assert_eq!(serde_json::to_string(&ConcurrencyClass::Sequential).unwrap(), "\"sequential\"");
        assert_eq!(serde_json::to_string(&ConcurrencyClass::Exclusive).unwrap(), "\"exclusive\"");
    }

    #[test]
    fn todo_item_roundtrip_minimal() {
        let now = jiff::Timestamp::from_second(1_780_000_000).unwrap();
        let item = TodoItem {
            id: "01HX0000000000000000000000".into(),
            title: "Read schema".into(),
            status: TodoStatus::Pending,
            concurrency: ConcurrencyClass::Safe,
            blocked_reason: None,
            blocked_by: vec![],
            delegated_to: None,
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: TodoItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
        // optional fields elided when empty
        assert!(!json.contains("blocked_reason"));
        assert!(!json.contains("blocked_by"));
        assert!(!json.contains("delegated_to"));
    }

    #[test]
    fn todo_item_roundtrip_full() {
        let now = jiff::Timestamp::from_second(1_780_000_000).unwrap();
        let item = TodoItem {
            id: "01HX0000000000000000000000".into(),
            title: "Add migration".into(),
            status: TodoStatus::Blocked,
            concurrency: ConcurrencyClass::Exclusive,
            blocked_reason: Some("waiting on user clarification".into()),
            blocked_by: vec!["01HX0000000000000000000001".into()],
            delegated_to: Some("subagent_a3f2".into()),
            created_at: now,
            updated_at: now,
        };
        let json = serde_json::to_string(&item).unwrap();
        let back: TodoItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item, back);
    }

    #[test]
    fn todo_item_input_minimal_required_fields() {
        let json = r#"{"title":"Read schema","status":"pending","concurrency":"safe"}"#;
        let parsed: TodoItemInput = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.title, "Read schema");
        assert_eq!(parsed.status, TodoStatus::Pending);
        assert_eq!(parsed.concurrency, ConcurrencyClass::Safe);
        assert_eq!(parsed.id, None);
        assert!(parsed.blocked_by.is_empty());
    }

    #[test]
    fn todo_item_input_rejects_missing_required() {
        let json = r#"{"title":"x","status":"pending"}"#; // no concurrency
        let r: Result<TodoItemInput, _> = serde_json::from_str(json);
        assert!(r.is_err());
    }
}
