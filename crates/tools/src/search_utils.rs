//! Search utilities for Klyntbot — SearchResult enum and re-exported RRF merge.
//!
//! The generic `rrf_merge` algorithm lives in `tools-core`. This module provides
//! the `SearchResult` enum (Todo | Conversation) and its `Searchable` impl.

use crate::conversation_embedding::ConversationEmbeddingRecord;
use crate::todo_types::Todo;
use tools_core::Searchable;

// Re-export the generic rrf_merge from tools-core.
pub use tools_core::rrf_merge;

/// Search result types that can be merged via RRF
#[derive(Debug, Clone)]
pub enum SearchResult {
    /// Todo task search result (boxed to reduce enum size variance)
    Todo(Box<Todo>),
    /// Conversation message search result
    Conversation(ConversationEmbeddingRecord),
}

impl SearchResult {
    /// Get the unique ID for this search result
    pub fn id(&self) -> &str {
        match self {
            SearchResult::Todo(todo) => &todo.id,
            SearchResult::Conversation(record) => &record.id,
        }
    }
}

impl Searchable for SearchResult {
    fn search_id(&self) -> &str {
        self.id()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::todo_types::TodoStatus;
    use std::collections::HashMap;

    fn mock_todo(id: &str, title: &str) -> Todo {
        use chrono::Utc;
        Todo {
            id: id.to_string(),
            title: title.to_string(),
            description: None,
            status: TodoStatus::Todo,
            priority: Some(3),
            due_date: None,
            tags: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
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

    #[test]
    fn test_searchable_impl() {
        let result = SearchResult::Todo(Box::new(mock_todo("t1", "Test")));
        assert_eq!(result.search_id(), "t1");
    }

    #[test]
    fn test_rrf_merge_with_search_result() {
        let k = 60;
        let keyword = vec![
            SearchResult::Todo(Box::new(mock_todo("1", "First"))),
            SearchResult::Todo(Box::new(mock_todo("2", "Second"))),
        ];
        let semantic = vec![("1".to_string(), 0.9), ("3".to_string(), 0.8)];
        let mut map = HashMap::new();
        map.insert(
            "3".to_string(),
            SearchResult::Todo(Box::new(mock_todo("3", "Third"))),
        );

        let results = rrf_merge(&keyword, &semantic, k, &map);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0.id(), "1"); // Highest score (appears in both)
        assert_eq!(results[0].2, "both");
    }
}
