//! Search utilities for Klyntbot — SearchResult enum and re-exported RRF merge.
//!
//! The generic `rrf_merge` algorithm lives in `tools-core`. This module provides
//! the `SearchResult` enum (Action | Conversation) and its `Searchable` impl.

use crate::conversation_recall::RecallSearchResult;
use crate::todo_types::Action;
use tools_core::Searchable;

// Re-export the generic rrf_merge from tools-core.
pub use tools_core::rrf_merge;

/// Search result types that can be merged via RRF
#[derive(Debug, Clone)]
pub enum SearchResult {
    /// Action/task search result (boxed to reduce enum size variance)
    Todo(Box<Action>),
    /// Conversation message search result
    Conversation(RecallSearchResult),
}

impl SearchResult {
    /// Get the unique ID for this search result
    pub fn id(&self) -> &str {
        match self {
            SearchResult::Todo(action) => &action.id,
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
    use crate::todo_types::ActionStatus;
    use std::collections::HashMap;

    fn mock_action(id: &str, title: &str) -> Action {
        Action {
            id: id.to_string(),
            title: title.to_string(),
            description: None,
            area_id: "test".to_string(),
            key_result_id: None,
            status: ActionStatus::Todo,
            priority: Some(3),
            due_date: None,
            tags: Vec::new(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
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
            status_label_id: None,
            position: 0,
            group_id: None,
        }
    }

    #[test]
    fn test_searchable_impl() {
        let result = SearchResult::Todo(Box::new(mock_action("t1", "Test")));
        assert_eq!(result.search_id(), "t1");
    }

    #[test]
    fn test_rrf_merge_with_search_result() {
        let k = 60;
        let keyword = vec![
            SearchResult::Todo(Box::new(mock_action("1", "First"))),
            SearchResult::Todo(Box::new(mock_action("2", "Second"))),
        ];
        let semantic = vec![("1".to_string(), 0.9), ("3".to_string(), 0.8)];
        let mut map = HashMap::new();
        map.insert(
            "3".to_string(),
            SearchResult::Todo(Box::new(mock_action("3", "Third"))),
        );

        let results = rrf_merge(&keyword, &semantic, k, &map);

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0.id(), "1"); // Highest score (appears in both)
        assert_eq!(results[0].2, "both");
    }
}
