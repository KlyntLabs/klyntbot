//! Todo system types and confidence scoring
//!
//! This module defines the core data structures for the todo system,
//! including tasks, statuses, filters, and confidence calculation.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A task/todo item with confidence scoring and focus tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<u8>,
    pub due_date: Option<DateTime<Utc>>,
    pub tags: Vec<String>,
    pub status: TodoStatus,
    pub confidence: f32,
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
    /// Calculate confidence score (0.0-1.0) from field completeness
    ///
    /// Scoring breakdown:
    /// - Title quality (>3 words): 0.25
    /// - Description (>10 chars): 0.25
    /// - Priority set: 0.15
    /// - Due date set: 0.20
    /// - Tags present: 0.15
    pub fn calculate_confidence(&self) -> f32 {
        let mut score = 0.0;

        // Title quality (0.25)
        if self.title.split_whitespace().count() > 3 {
            score += 0.25;
        }

        // Description (0.25)
        if let Some(desc) = &self.description {
            if desc.len() > 10 {
                score += 0.25;
            }
        }

        // Priority (0.15)
        if self.priority.is_some() {
            score += 0.15;
        }

        // Due date (0.20)
        if self.due_date.is_some() {
            score += 0.20;
        }

        // Tags (0.15)
        if !self.tags.is_empty() {
            score += 0.15;
        }

        score
    }

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
    pub low_confidence: Vec<String>,
    pub upcoming_week: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confidence_calculation_empty() {
        let todo = Todo {
            id: "test1234".to_string(),
            title: "Fix".to_string(),
            description: None,
            priority: None,
            due_date: None,
            tags: vec![],
            status: TodoStatus::Todo,
            confidence: 0.0,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
        };

        let score = todo.calculate_confidence();
        assert_eq!(score, 0.0, "Empty todo should have 0.0 confidence");
    }

    #[test]
    fn test_confidence_calculation_full() {
        let todo = Todo {
            id: "test1234".to_string(),
            title: "Fix the authentication timeout bug".to_string(),
            description: Some("Auth tokens expire during long sessions".to_string()),
            priority: Some(4),
            due_date: Some(Utc::now()),
            tags: vec!["backend".to_string(), "auth".to_string()],
            status: TodoStatus::Todo,
            confidence: 0.0,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
        };

        let score = todo.calculate_confidence();
        assert_eq!(score, 1.0, "Complete todo should have 1.0 confidence");
    }

    #[test]
    fn test_confidence_calculation_partial() {
        let todo = Todo {
            id: "test1234".to_string(),
            title: "Fix the bug".to_string(), // 3 words, not >3
            description: Some("Short".to_string()), // <10 chars
            priority: Some(3),
            due_date: None,
            tags: vec!["backend".to_string()],
            status: TodoStatus::Todo,
            confidence: 0.0,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
        };

        let score = todo.calculate_confidence();
        // Priority (0.15) + Tags (0.15) = 0.30
        assert_eq!(score, 0.30, "Partial todo should have 0.30 confidence");
    }

    #[test]
    fn test_confidence_calculation_title_boundary() {
        let mut todo = Todo {
            id: "test1234".to_string(),
            title: "One two three".to_string(), // Exactly 3 words
            description: None,
            priority: None,
            due_date: None,
            tags: vec![],
            status: TodoStatus::Todo,
            confidence: 0.0,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
        };

        let score = todo.calculate_confidence();
        assert_eq!(score, 0.0, "3-word title should not score (needs >3)");

        todo.title = "One two three four".to_string(); // 4 words
        let score = todo.calculate_confidence();
        assert_eq!(score, 0.25, "4-word title should score 0.25");
    }

    #[test]
    fn test_confidence_calculation_description_boundary() {
        let mut todo = Todo {
            id: "test1234".to_string(),
            title: "Test".to_string(),
            description: Some("1234567890".to_string()), // Exactly 10 chars
            priority: None,
            due_date: None,
            tags: vec![],
            status: TodoStatus::Todo,
            confidence: 0.0,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
        };

        let score = todo.calculate_confidence();
        assert_eq!(
            score, 0.0,
            "10-char description should not score (needs >10)"
        );

        todo.description = Some("12345678901".to_string()); // 11 chars
        let score = todo.calculate_confidence();
        assert_eq!(score, 0.25, "11-char description should score 0.25");
    }

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
