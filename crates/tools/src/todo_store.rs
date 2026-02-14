//! TodoStore - JSONL persistence for todo system
//!
//! Provides lazy-loading JSONL storage with full CRUD operations,
//! focus management, and summary generation.

use chrono::Utc;
use std::path::PathBuf;
use tokio::fs;

use crate::todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus, TodoSummary};
use common::Result;

/// JSONL-backed todo storage with lazy loading
pub struct TodoStore {
    file_path: PathBuf,
    todos: Vec<Todo>,
    loaded: bool,
    dirty: bool,
}

impl TodoStore {
    /// Create a new TodoStore (does not load from disk yet)
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            todos: Vec::new(),
            loaded: false,
            dirty: false,
        }
    }

    /// Ensure the store is loaded before any read operation
    async fn ensure_loaded(&mut self) -> Result<()> {
        if !self.loaded {
            self.load().await?;
        }
        Ok(())
    }

    /// Load todos from JSONL file
    async fn load(&mut self) -> Result<()> {
        if !self.file_path.exists() {
            self.todos = Vec::new();
            self.loaded = true;
            return Ok(());
        }

        let content = fs::read_to_string(&self.file_path).await?;
        self.todos = content
            .lines()
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<Todo>(line).ok())
            .collect();

        self.loaded = true;
        Ok(())
    }

    /// Save todos to JSONL file (full rewrite)
    async fn save(&self) -> Result<()> {
        let mut content = String::new();
        for todo in &self.todos {
            content.push_str(&serde_json::to_string(todo)?);
            content.push('\n');
        }

        // Ensure parent dir exists
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&self.file_path, content).await?;
        Ok(())
    }

    /// Add a new todo
    pub async fn add(&mut self, mut todo: Todo) -> Result<Todo> {
        self.ensure_loaded().await?;

        todo.updated_at = Utc::now();

        self.todos.push(todo.clone());
        self.dirty = true;
        self.save().await?;

        Ok(todo)
    }

    /// Get a todo by ID
    pub async fn get(&mut self, id: &str) -> Result<Option<Todo>> {
        self.ensure_loaded().await?;
        Ok(self.todos.iter().find(|t| t.id == id).cloned())
    }

    /// Update a todo with a patch
    pub async fn update(&mut self, id: &str, patch: TodoPatch) -> Result<Option<Todo>> {
        self.ensure_loaded().await?;

        let updated = if let Some(todo) = self.todos.iter_mut().find(|t| t.id == id) {
            if let Some(title) = patch.title {
                todo.title = title;
            }
            if let Some(desc) = patch.description {
                todo.description = desc;
            }
            if let Some(priority) = patch.priority {
                todo.priority = Some(priority);
            }
            if let Some(due) = patch.due_date {
                todo.due_date = due;
            }
            if let Some(tags) = patch.tags {
                todo.tags = tags;
            }
            if let Some(status) = patch.status {
                todo.status = status;

                // SG-7: Auto-clear focus when marking Done/Archived
                if status == TodoStatus::Done || status == TodoStatus::Archived {
                    todo.focused_at = None;
                    todo.focus_deadline = None;
                }

                if status == TodoStatus::Done {
                    todo.completed_at = Some(Utc::now());
                }
            }

            todo.updated_at = Utc::now();

            Some(todo.clone())
        } else {
            None
        };

        if updated.is_some() {
            self.dirty = true;
            self.save().await?;
        }

        Ok(updated)
    }

    /// Delete a todo by ID
    pub async fn delete(&mut self, id: &str) -> Result<bool> {
        self.ensure_loaded().await?;

        let before_len = self.todos.len();
        self.todos.retain(|t| t.id != id);

        if self.todos.len() < before_len {
            self.dirty = true;
            self.save().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List todos with optional filters
    pub async fn list(&mut self, filter: &TodoFilter) -> Result<Vec<Todo>> {
        self.ensure_loaded().await?;

        let mut filtered: Vec<Todo> = self
            .todos
            .iter()
            .filter(|t| {
                if let Some(status) = filter.status {
                    if t.status != status {
                        return false;
                    }
                }
                if let Some(min_pri) = filter.priority_min {
                    if t.priority.unwrap_or(0) < min_pri {
                        return false;
                    }
                }
                if let Some(tag) = &filter.tag {
                    if !t.tags.contains(tag) {
                        return false;
                    }
                }
                true
            })
            .cloned()
            .collect();

        if let Some(limit) = filter.limit {
            filtered.truncate(limit);
        }

        Ok(filtered)
    }

    /// Focus a task (with slot limit enforcement)
    pub async fn focus(&mut self, id: &str, max_slots: usize, deadline_hours: u64) -> Result<bool> {
        self.ensure_loaded().await?;

        // Check slot limit
        let focused_count = self.todos.iter().filter(|t| t.focused_at.is_some()).count();
        if focused_count >= max_slots {
            return Err(common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed(format!(
                    "Focus slots full ({}/{})",
                    focused_count, max_slots
                )),
            ));
        }

        if let Some(todo) = self.todos.iter_mut().find(|t| t.id == id) {
            let now = Utc::now();
            todo.focused_at = Some(now);
            todo.focus_deadline = Some(now + chrono::Duration::hours(deadline_hours as i64));
            todo.updated_at = now;

            self.dirty = true;
            self.save().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Unfocus a task (manual user action)
    pub async fn unfocus(&mut self, id: &str) -> Result<bool> {
        self.ensure_loaded().await?;

        if let Some(todo) = self.todos.iter_mut().find(|t| t.id == id) {
            todo.focused_at = None;
            todo.focus_deadline = None;
            todo.updated_at = Utc::now();

            self.dirty = true;
            self.save().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get all focused tasks
    pub async fn focused(&mut self) -> Result<Vec<Todo>> {
        self.ensure_loaded().await?;
        Ok(self
            .todos
            .iter()
            .filter(|t| t.focused_at.is_some())
            .cloned()
            .collect())
    }

    /// Get tasks with expired focus deadlines
    pub async fn overdue_focus(&mut self) -> Result<Vec<Todo>> {
        self.ensure_loaded().await?;
        let now = Utc::now();
        Ok(self
            .todos
            .iter()
            .filter(|t| {
                if let Some(deadline) = t.focus_deadline {
                    deadline < now
                } else {
                    false
                }
            })
            .cloned()
            .collect())
    }

    /// Auto-unfocus expired tasks and increment their expired count (SG-2)
    ///
    /// This method is called by the cron job for automatic focus expiration.
    /// Returns the list of tasks that were auto-unfocused for notification.
    pub async fn auto_unfocus_expired(&mut self) -> Result<Vec<Todo>> {
        self.ensure_loaded().await?;

        let now = Utc::now();
        let mut affected = Vec::new();

        for todo in self.todos.iter_mut() {
            if let Some(deadline) = todo.focus_deadline {
                if deadline < now {
                    // Increment expired count
                    todo.focus_expired_count += 1;

                    // Clear focus fields
                    todo.focused_at = None;
                    todo.focus_deadline = None;
                    todo.updated_at = now;

                    affected.push(todo.clone());
                }
            }
        }

        if !affected.is_empty() {
            self.dirty = true;
            self.save().await?;
        }

        Ok(affected)
    }

    /// Generate summary statistics
    pub async fn summary(&mut self) -> Result<TodoSummary> {
        self.ensure_loaded().await?;

        use std::collections::HashMap;
        let now = Utc::now();

        let mut by_status = HashMap::new();
        let mut overdue = Vec::new();
        let mut upcoming_week = Vec::new();

        for todo in &self.todos {
            *by_status.entry(todo.status).or_insert(0) += 1;

            if let Some(due) = todo.due_date {
                if due < now {
                    overdue.push(todo.id.clone());
                } else if due < now + chrono::Duration::days(7) {
                    upcoming_week.push(todo.id.clone());
                }
            }
        }

        Ok(TodoSummary {
            total: self.todos.len(),
            by_status,
            overdue,
            upcoming_week,
        })
    }

    /// Generate markdown context for AI system prompt
    pub async fn to_context_string(&mut self) -> Result<String> {
        self.ensure_loaded().await?;

        let focused = self.focused().await?;
        let summary = self.summary().await?;

        let mut ctx = String::from("# Active Tasks\n\n");

        // Focus section
        ctx.push_str(&format!("## 🎯 Focus ({}/3)\n", focused.len()));
        if focused.is_empty() {
            ctx.push_str("_(no focused tasks)_\n");
        } else {
            for todo in focused {
                let time_left = if let Some(deadline) = todo.focus_deadline {
                    let remaining = deadline.signed_duration_since(Utc::now());
                    format!("{}h left", remaining.num_hours().max(0))
                } else {
                    "no deadline".to_string()
                };
                ctx.push_str(&format!(
                    "- [{}] {} (P{}, {}) — {}\n",
                    todo.id,
                    todo.title,
                    todo.priority.unwrap_or(3),
                    todo.tags.join("/"),
                    time_left
                ));
            }
        }

        ctx.push('\n');

        // Backlog
        let active: Vec<_> = self
            .todos
            .iter()
            .filter(|t| {
                t.status != TodoStatus::Done
                    && t.status != TodoStatus::Archived
                    && t.focused_at.is_none()
            })
            .take(10)
            .collect();

        ctx.push_str(&format!("## 📋 Backlog ({} active)\n", active.len()));
        for todo in active {
            let overdue_marker = if let Some(due) = todo.due_date {
                if due < Utc::now() {
                    " — OVERDUE"
                } else {
                    ""
                }
            } else {
                ""
            };
            ctx.push_str(&format!(
                "- [{}] {} (P{}, {}){}\n",
                todo.id,
                todo.title,
                todo.priority.unwrap_or(3),
                todo.tags.join("/"),
                overdue_marker
            ));
        }

        ctx.push_str("\n## Stats\n");
        ctx.push_str(&format!("- Overdue: {}\n", summary.overdue.len()));

        Ok(ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn create_test_store() -> (TodoStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("todos.jsonl");
        let store = TodoStore::new(file_path);
        (store, temp_dir)
    }

    fn create_test_todo(title: &str) -> Todo {
        Todo {
            id: Todo::generate_id(),
            title: title.to_string(),
            description: None,
            priority: None,
            due_date: None,
            tags: vec![],
            status: TodoStatus::Todo,

            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
        }
    }

    #[tokio::test]
    async fn test_new_store_not_loaded() {
        let (store, _dir) = create_test_store().await;
        assert!(!store.loaded);
        assert_eq!(store.todos.len(), 0);
    }

    #[tokio::test]
    async fn test_add_todo() {
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("Test task");

        let added = store.add(todo.clone()).await.unwrap();
        assert_eq!(added.title, "Test task");
    }

    #[tokio::test]
    async fn test_get_todo() {
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("Test task");
        let id = todo.id.clone();

        store.add(todo).await.unwrap();
        let retrieved = store.get(&id).await.unwrap();

        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, id);
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let (mut store, _dir) = create_test_store().await;
        let result = store.get("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_update_todo() {
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("Original");
        let id = todo.id.clone();

        store.add(todo).await.unwrap();

        let patch = TodoPatch {
            title: Some("Updated".to_string()),
            ..Default::default()
        };

        let updated = store.update(&id, patch).await.unwrap();
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().title, "Updated");
    }

    #[tokio::test]
    async fn test_update_to_done_sets_completed_at() {
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("Task");
        let id = todo.id.clone();

        store.add(todo).await.unwrap();

        let patch = TodoPatch {
            status: Some(TodoStatus::Done),
            ..Default::default()
        };

        let updated = store.update(&id, patch).await.unwrap().unwrap();
        assert_eq!(updated.status, TodoStatus::Done);
        assert!(updated.completed_at.is_some());
    }

    #[tokio::test]
    async fn test_sg7_done_clears_focus() {
        // SG-7: Marking Done should clear focus fields
        let (mut store, _dir) = create_test_store().await;
        let mut todo = create_test_todo("Focused task");
        let id = todo.id.clone();

        // Set focus fields manually
        todo.focused_at = Some(Utc::now());
        todo.focus_deadline = Some(Utc::now() + chrono::Duration::hours(18));

        store.add(todo).await.unwrap();

        // Mark as Done
        let patch = TodoPatch {
            status: Some(TodoStatus::Done),
            ..Default::default()
        };

        let updated = store.update(&id, patch).await.unwrap().unwrap();
        assert_eq!(updated.status, TodoStatus::Done);
        assert!(updated.focused_at.is_none(), "focused_at should be cleared");
        assert!(
            updated.focus_deadline.is_none(),
            "focus_deadline should be cleared"
        );
    }

    #[tokio::test]
    async fn test_sg7_archived_clears_focus() {
        // SG-7: Marking Archived should clear focus fields
        let (mut store, _dir) = create_test_store().await;
        let mut todo = create_test_todo("Focused task");
        let id = todo.id.clone();

        todo.focused_at = Some(Utc::now());
        todo.focus_deadline = Some(Utc::now() + chrono::Duration::hours(18));

        store.add(todo).await.unwrap();

        let patch = TodoPatch {
            status: Some(TodoStatus::Archived),
            ..Default::default()
        };

        let updated = store.update(&id, patch).await.unwrap().unwrap();
        assert_eq!(updated.status, TodoStatus::Archived);
        assert!(updated.focused_at.is_none());
        assert!(updated.focus_deadline.is_none());
    }

    #[tokio::test]
    async fn test_delete_todo() {
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("To delete");
        let id = todo.id.clone();

        store.add(todo).await.unwrap();
        let deleted = store.delete(&id).await.unwrap();

        assert!(deleted);
        assert!(store.get(&id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let (mut store, _dir) = create_test_store().await;
        let deleted = store.delete("nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_list_all() {
        let (mut store, _dir) = create_test_store().await;

        store.add(create_test_todo("Task 1")).await.unwrap();
        store.add(create_test_todo("Task 2")).await.unwrap();

        let todos = store.list(&TodoFilter::default()).await.unwrap();
        assert_eq!(todos.len(), 2);
    }

    #[tokio::test]
    async fn test_list_by_status() {
        let (mut store, _dir) = create_test_store().await;

        let todo1 = create_test_todo("Todo");
        let mut todo2 = create_test_todo("Done");
        todo2.status = TodoStatus::Done;

        store.add(todo1).await.unwrap();
        store.add(todo2).await.unwrap();

        let filter = TodoFilter {
            status: Some(TodoStatus::Done),
            ..Default::default()
        };

        let todos = store.list(&filter).await.unwrap();
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].status, TodoStatus::Done);
    }

    #[tokio::test]
    async fn test_list_by_priority() {
        let (mut store, _dir) = create_test_store().await;

        let mut todo1 = create_test_todo("Low");
        todo1.priority = Some(1);
        let mut todo2 = create_test_todo("High");
        todo2.priority = Some(5);

        store.add(todo1).await.unwrap();
        store.add(todo2).await.unwrap();

        let filter = TodoFilter {
            priority_min: Some(4),
            ..Default::default()
        };

        let todos = store.list(&filter).await.unwrap();
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].priority, Some(5));
    }

    #[tokio::test]
    async fn test_list_by_tag() {
        let (mut store, _dir) = create_test_store().await;

        let mut todo1 = create_test_todo("Backend");
        todo1.tags = vec!["backend".to_string()];
        let mut todo2 = create_test_todo("Frontend");
        todo2.tags = vec!["frontend".to_string()];

        store.add(todo1).await.unwrap();
        store.add(todo2).await.unwrap();

        let filter = TodoFilter {
            tag: Some("backend".to_string()),
            ..Default::default()
        };

        let todos = store.list(&filter).await.unwrap();
        assert_eq!(todos.len(), 1);
        assert!(todos[0].tags.contains(&"backend".to_string()));
    }

    #[tokio::test]
    async fn test_list_with_limit() {
        let (mut store, _dir) = create_test_store().await;

        for i in 0..10 {
            store
                .add(create_test_todo(&format!("Task {}", i)))
                .await
                .unwrap();
        }

        let filter = TodoFilter {
            limit: Some(5),
            ..Default::default()
        };

        let todos = store.list(&filter).await.unwrap();
        assert_eq!(todos.len(), 5);
    }

    #[tokio::test]
    async fn test_focus_task() {
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("Focus me");
        let id = todo.id.clone();

        store.add(todo).await.unwrap();
        let focused = store.focus(&id, 3, 18).await.unwrap();

        assert!(focused);

        let retrieved = store.get(&id).await.unwrap().unwrap();
        assert!(retrieved.focused_at.is_some());
        assert!(retrieved.focus_deadline.is_some());
    }

    #[tokio::test]
    async fn test_focus_slot_limit() {
        let (mut store, _dir) = create_test_store().await;

        // Fill all 3 slots
        for i in 0..3 {
            let todo = create_test_todo(&format!("Task {}", i));
            let id = todo.id.clone();
            store.add(todo).await.unwrap();
            store.focus(&id, 3, 18).await.unwrap();
        }

        // Try to focus a 4th task
        let todo = create_test_todo("Fourth task");
        let id = todo.id.clone();
        store.add(todo).await.unwrap();

        let result = store.focus(&id, 3, 18).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unfocus_task() {
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("Task");
        let id = todo.id.clone();

        store.add(todo).await.unwrap();
        store.focus(&id, 3, 18).await.unwrap();

        let unfocused = store.unfocus(&id).await.unwrap();
        assert!(unfocused);

        let retrieved = store.get(&id).await.unwrap().unwrap();
        assert!(retrieved.focused_at.is_none());
        assert!(retrieved.focus_deadline.is_none());
    }

    #[tokio::test]
    async fn test_focused_list() {
        let (mut store, _dir) = create_test_store().await;

        let todo1 = create_test_todo("Focused 1");
        let id1 = todo1.id.clone();
        let todo2 = create_test_todo("Focused 2");
        let id2 = todo2.id.clone();
        let todo3 = create_test_todo("Not focused");

        store.add(todo1).await.unwrap();
        store.add(todo2).await.unwrap();
        store.add(todo3).await.unwrap();

        store.focus(&id1, 3, 18).await.unwrap();
        store.focus(&id2, 3, 18).await.unwrap();

        let focused = store.focused().await.unwrap();
        assert_eq!(focused.len(), 2);
    }

    #[tokio::test]
    async fn test_overdue_focus() {
        let (mut store, _dir) = create_test_store().await;

        let mut todo = create_test_todo("Expired");
        todo.focused_at = Some(Utc::now() - chrono::Duration::hours(20));
        todo.focus_deadline = Some(Utc::now() - chrono::Duration::hours(2));

        store.add(todo).await.unwrap();

        let overdue = store.overdue_focus().await.unwrap();
        assert_eq!(overdue.len(), 1);
    }

    #[tokio::test]
    async fn test_sg2_auto_unfocus_expired() {
        // SG-2: auto_unfocus_expired should unfocus and increment counter
        let (mut store, _dir) = create_test_store().await;

        let mut todo = create_test_todo("Expired task");
        let id = todo.id.clone();
        todo.focused_at = Some(Utc::now() - chrono::Duration::hours(20));
        todo.focus_deadline = Some(Utc::now() - chrono::Duration::hours(2));
        todo.focus_expired_count = 0;

        store.add(todo).await.unwrap();

        let affected = store.auto_unfocus_expired().await.unwrap();
        assert_eq!(affected.len(), 1);
        assert_eq!(affected[0].id, id);

        let retrieved = store.get(&id).await.unwrap().unwrap();
        assert!(retrieved.focused_at.is_none(), "should be unfocused");
        assert!(
            retrieved.focus_deadline.is_none(),
            "deadline should be cleared"
        );
        assert_eq!(
            retrieved.focus_expired_count, 1,
            "counter should be incremented"
        );
    }

    #[tokio::test]
    async fn test_sg2_multiple_expirations_accumulate() {
        // SG-2: Multiple expirations should accumulate the counter
        let (mut store, _dir) = create_test_store().await;

        let mut todo = create_test_todo("Task");
        let id = todo.id.clone();
        todo.focused_at = Some(Utc::now() - chrono::Duration::hours(20));
        todo.focus_deadline = Some(Utc::now() - chrono::Duration::hours(2));
        todo.focus_expired_count = 2; // Already expired twice before

        store.add(todo).await.unwrap();

        store.auto_unfocus_expired().await.unwrap();

        let retrieved = store.get(&id).await.unwrap().unwrap();
        assert_eq!(
            retrieved.focus_expired_count, 3,
            "should accumulate from 2 to 3"
        );
    }

    #[tokio::test]
    async fn test_summary() {
        let (mut store, _dir) = create_test_store().await;

        let todo1 = create_test_todo("Todo");
        let mut todo2 = create_test_todo("Done");
        todo2.status = TodoStatus::Done;

        store.add(todo1).await.unwrap();
        store.add(todo2).await.unwrap();

        let summary = store.summary().await.unwrap();
        assert_eq!(summary.total, 2);
        assert_eq!(summary.by_status.get(&TodoStatus::Todo), Some(&1));
        assert_eq!(summary.by_status.get(&TodoStatus::Done), Some(&1));
    }

    #[tokio::test]
    async fn test_jsonl_round_trip() {
        let (mut store, dir) = create_test_store().await;

        let mut todo = create_test_todo("Test with all fields");
        todo.description = Some("A description".to_string());
        todo.priority = Some(4);
        todo.tags = vec!["test".to_string(), "backend".to_string()];
        todo.due_date = Some(Utc::now() + chrono::Duration::days(7));

        let id = todo.id.clone();
        store.add(todo).await.unwrap();

        // Create a new store pointing to the same file
        let file_path = dir.path().join("todos.jsonl");
        let mut store2 = TodoStore::new(file_path);

        let loaded = store2.get(&id).await.unwrap().unwrap();
        assert_eq!(loaded.title, "Test with all fields");
        assert_eq!(loaded.description, Some("A description".to_string()));
        assert_eq!(loaded.priority, Some(4));
        assert_eq!(loaded.tags.len(), 2);
    }

    #[tokio::test]
    async fn test_malformed_jsonl_skipped() {
        let (mut store, _dir) = create_test_store().await;

        // Write malformed JSONL manually with proper serialization
        let todo1 = create_test_todo("Valid");
        let todo2 = create_test_todo("Also valid");

        let line1 = serde_json::to_string(&todo1).unwrap();
        let line2 = serde_json::to_string(&todo2).unwrap();

        let content = format!("{}\ninvalid json line here\n{}\n", line1, line2);

        fs::write(&store.file_path, content).await.unwrap();

        store.load().await.unwrap();

        // Should only load the 2 valid lines, skipping the malformed one
        let todos = store.list(&TodoFilter::default()).await.unwrap();
        assert_eq!(todos.len(), 2);
    }

    #[tokio::test]
    async fn test_context_string_generation() {
        let (mut store, _dir) = create_test_store().await;

        let todo = create_test_todo("Test task for context");
        let id = todo.id.clone();

        store.add(todo).await.unwrap();
        store.focus(&id, 3, 18).await.unwrap();

        let context = store.to_context_string().await.unwrap();

        assert!(context.contains("# Active Tasks"));
        assert!(context.contains("## 🎯 Focus"));
        assert!(context.contains("Test task for context"));
        assert!(context.contains("## 📋 Backlog"));
        assert!(context.contains("## Stats"));
    }
}
