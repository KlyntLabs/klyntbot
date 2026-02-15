//! TodoStore - Append-only JSONL persistence for todo system
//!
//! Uses an append-only journal pattern for O(1) writes instead of O(n) full rewrites.
//! Journal entries are either upserts (full Todo) or deletes (tombstones).
//! Periodic compaction rewrites the file when stale entries exceed a threshold.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus, TodoSummary};
use common::Result;

/// Compaction threshold: compact when journal has this many more entries than live todos
const COMPACTION_THRESHOLD: usize = 100;

/// A single entry in the append-only journal.
///
/// Tagged enum serialized as `{"_op":"upsert","todo":{...}}` or `{"_op":"delete","id":"..."}`.
/// Plain Todo JSON lines (legacy format) are handled during load for backwards compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "_op")]
enum JournalEntry {
    #[serde(rename = "upsert")]
    Upsert { todo: Box<Todo> },
    #[serde(rename = "delete")]
    Delete { id: String },
}

/// Append-only JSONL-backed todo storage with lazy loading and automatic compaction
pub struct TodoStore {
    file_path: PathBuf,
    /// In-memory index: id -> Todo (authoritative state after load)
    index: HashMap<String, Todo>,
    /// Ordered list of live todo IDs (preserves insertion order)
    order: Vec<String>,
    loaded: bool,
    /// Number of journal entries on disk (including stale/overwritten ones)
    journal_len: usize,
}

impl TodoStore {
    /// Create a new TodoStore (does not load from disk yet)
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            index: HashMap::new(),
            order: Vec::new(),
            loaded: false,
            journal_len: 0,
        }
    }

    /// Ensure the store is loaded before any read operation
    async fn ensure_loaded(&mut self) -> Result<()> {
        if !self.loaded {
            self.load().await?;
        }
        Ok(())
    }

    /// Load todos from JSONL file, replaying the journal to build the index.
    ///
    /// Supports both legacy format (plain Todo JSON) and new journal format (JournalEntry).
    async fn load(&mut self) -> Result<()> {
        if !self.file_path.exists() {
            self.index = HashMap::new();
            self.order = Vec::new();
            self.loaded = true;
            self.journal_len = 0;
            return Ok(());
        }

        let content = fs::read_to_string(&self.file_path).await?;
        let mut index = HashMap::new();
        let mut order = Vec::new();
        let mut journal_len = 0;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            journal_len += 1;

            // Try new journal format first, then fall back to legacy plain Todo
            if let Ok(entry) = serde_json::from_str::<JournalEntry>(line) {
                match entry {
                    JournalEntry::Upsert { todo } => {
                        if !index.contains_key(&todo.id) {
                            order.push(todo.id.clone());
                        }
                        index.insert(todo.id.clone(), *todo);
                    }
                    JournalEntry::Delete { id } => {
                        if index.remove(&id).is_some() {
                            order.retain(|oid| oid != &id);
                        }
                    }
                }
            } else if let Ok(todo) = serde_json::from_str::<Todo>(line) {
                // Legacy format: plain Todo JSON line treated as upsert
                if !index.contains_key(&todo.id) {
                    order.push(todo.id.clone());
                }
                index.insert(todo.id.clone(), todo);
            }
            // Malformed lines are silently skipped (backwards compatible)
        }

        self.index = index;
        self.order = order;
        self.journal_len = journal_len;
        self.loaded = true;
        Ok(())
    }

    /// Append a single journal entry to the file (O(1) write)
    async fn append_entry(&mut self, entry: &JournalEntry) -> Result<()> {
        // Ensure parent dir exists
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?;

        let mut line = serde_json::to_string(entry)?;
        line.push('\n');
        file.write_all(line.as_bytes()).await?;
        file.flush().await?;

        self.journal_len += 1;
        Ok(())
    }

    /// Append multiple journal entries in a single buffered write
    async fn append_entries(&mut self, entries: &[JournalEntry]) -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }

        // Ensure parent dir exists
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.file_path)
            .await?;

        let mut buf = String::new();
        for entry in entries {
            buf.push_str(&serde_json::to_string(entry)?);
            buf.push('\n');
        }
        file.write_all(buf.as_bytes()).await?;
        file.flush().await?;

        self.journal_len += entries.len();
        Ok(())
    }

    /// Compact the journal file: rewrite with only live entries.
    ///
    /// Called automatically when stale entries exceed the threshold.
    async fn compact(&mut self) -> Result<()> {
        let mut content = String::with_capacity(self.index.len() * 256);
        for id in &self.order {
            if let Some(todo) = self.index.get(id) {
                let entry = JournalEntry::Upsert {
                    todo: Box::new(todo.clone()),
                };
                content.push_str(&serde_json::to_string(&entry)?);
                content.push('\n');
            }
        }

        // Ensure parent dir exists
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        fs::write(&self.file_path, content).await?;
        self.journal_len = self.index.len();
        Ok(())
    }

    /// Check if compaction is needed and run it
    async fn maybe_compact(&mut self) -> Result<()> {
        let stale = self.journal_len.saturating_sub(self.index.len());
        if stale >= COMPACTION_THRESHOLD {
            self.compact().await?;
        }
        Ok(())
    }

    /// Helper: get ordered Vec of live todos (from index, in insertion order)
    fn todos_ordered(&self) -> Vec<&Todo> {
        self.order
            .iter()
            .filter_map(|id| self.index.get(id))
            .collect()
    }

    /// Add a new todo
    pub async fn add(&mut self, mut todo: Todo) -> Result<Todo> {
        self.ensure_loaded().await?;

        todo.updated_at = Utc::now();

        let entry = JournalEntry::Upsert {
            todo: Box::new(todo.clone()),
        };
        self.append_entry(&entry).await?;

        self.order.push(todo.id.clone());
        self.index.insert(todo.id.clone(), todo.clone());

        self.maybe_compact().await?;
        Ok(todo)
    }

    /// Get a todo by ID (O(1) lookup)
    pub async fn get(&mut self, id: &str) -> Result<Option<Todo>> {
        self.ensure_loaded().await?;
        Ok(self.index.get(id).cloned())
    }

    /// Update a todo with a patch
    pub async fn update(&mut self, id: &str, patch: TodoPatch) -> Result<Option<Todo>> {
        self.ensure_loaded().await?;

        let updated = if let Some(todo) = self.index.get_mut(id) {
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
            if let Some(last_reminded) = patch.last_reminded_at {
                todo.last_reminded_at = last_reminded;
            }
            if let Some(calendar_uid) = patch.calendar_event_uid {
                todo.calendar_event_uid = calendar_uid;
            }

            todo.updated_at = Utc::now();

            Some(todo.clone())
        } else {
            None
        };

        if let Some(ref todo) = updated {
            let entry = JournalEntry::Upsert {
                todo: Box::new(todo.clone()),
            };
            self.append_entry(&entry).await?;
            self.maybe_compact().await?;
        }

        Ok(updated)
    }

    /// Delete a todo by ID.
    /// Also removes this ID from blocked_by/blocks in all other tasks.
    pub async fn delete(&mut self, id: &str) -> Result<bool> {
        self.ensure_loaded().await?;

        if self.index.remove(id).is_some() {
            self.order.retain(|oid| oid != id);

            // Defensive full scan: remove deleted ID from ALL tasks' blocked_by/blocks.
            // Guards against inconsistency from crashes, manual edits, or bugs.
            let mut affected_entries = Vec::new();
            for todo in self.index.values_mut() {
                let had_ref = todo.blocked_by.contains(&id.to_string())
                    || todo.blocks.contains(&id.to_string());
                if had_ref {
                    todo.blocked_by.retain(|bid| bid != id);
                    todo.blocks.retain(|bid| bid != id);
                    todo.updated_at = Utc::now();
                    affected_entries.push(JournalEntry::Upsert {
                        todo: Box::new(todo.clone()),
                    });
                }
            }

            // Append delete entry + any affected dependency cleanups
            affected_entries.push(JournalEntry::Delete { id: id.to_string() });
            self.append_entries(&affected_entries).await?;
            self.maybe_compact().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List todos with optional filters
    pub async fn list(&mut self, filter: &TodoFilter) -> Result<Vec<Todo>> {
        self.ensure_loaded().await?;

        let mut filtered: Vec<Todo> = self
            .todos_ordered()
            .into_iter()
            .filter(|t| {
                // Sprint 2: exclude templates from normal lists unless explicitly included
                if !filter.include_templates && t.is_template {
                    return false;
                }
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
                // Phase 2: project_id filter
                if let Some(ref project_id) = filter.project_id {
                    if t.project_id.as_ref() != Some(project_id) {
                        return false;
                    }
                }
                // Phase 2: parent_id filter
                if let Some(ref parent_id) = filter.parent_id {
                    if t.parent_id.as_ref() != Some(parent_id) {
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

    /// List only template todos (is_template == true).
    pub async fn list_templates(&mut self) -> Result<Vec<Todo>> {
        self.ensure_loaded().await?;
        Ok(self
            .todos_ordered()
            .into_iter()
            .filter(|t| t.is_template)
            .cloned()
            .collect())
    }

    /// Update the next_instance_date for a recurring template.
    /// Used by RecurringTaskSpawner after spawning an instance.
    pub async fn update_next_instance_date(
        &mut self,
        id: &str,
        next: Option<DateTime<Utc>>,
    ) -> Result<()> {
        self.ensure_loaded().await?;

        if let Some(todo) = self.index.get_mut(id) {
            todo.next_instance_date = next;
            todo.updated_at = Utc::now();
            let entry = JournalEntry::Upsert {
                todo: Box::new(todo.clone()),
            };
            self.append_entry(&entry).await?;
            self.maybe_compact().await?;
            Ok(())
        } else {
            Err(common::ToolError::ExecutionFailed(format!("Task not found: {}", id)).into())
        }
    }

    /// Focus a task (with slot limit enforcement)
    pub async fn focus(&mut self, id: &str, max_slots: usize, deadline_hours: u64) -> Result<bool> {
        self.ensure_loaded().await?;

        // Check slot limit
        let focused_count = self
            .index
            .values()
            .filter(|t| t.focused_at.is_some())
            .count();
        if focused_count >= max_slots {
            return Err(common::KlyntbotError::Tool(
                common::ToolError::ExecutionFailed(format!(
                    "Focus slots full ({}/{})",
                    focused_count, max_slots
                )),
            ));
        }

        if let Some(todo) = self.index.get_mut(id) {
            let now = Utc::now();
            todo.focused_at = Some(now);
            todo.focus_deadline = Some(now + chrono::Duration::hours(deadline_hours as i64));
            todo.updated_at = now;

            let entry = JournalEntry::Upsert {
                todo: Box::new(todo.clone()),
            };
            self.append_entry(&entry).await?;
            self.maybe_compact().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Unfocus a task (manual user action)
    pub async fn unfocus(&mut self, id: &str) -> Result<bool> {
        self.ensure_loaded().await?;

        if let Some(todo) = self.index.get_mut(id) {
            todo.focused_at = None;
            todo.focus_deadline = None;
            todo.updated_at = Utc::now();

            let entry = JournalEntry::Upsert {
                todo: Box::new(todo.clone()),
            };
            self.append_entry(&entry).await?;
            self.maybe_compact().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get all focused tasks
    pub async fn focused(&mut self) -> Result<Vec<Todo>> {
        self.ensure_loaded().await?;
        Ok(self
            .todos_ordered()
            .into_iter()
            .filter(|t| t.focused_at.is_some())
            .cloned()
            .collect())
    }

    /// Get tasks with expired focus deadlines
    pub async fn overdue_focus(&mut self) -> Result<Vec<Todo>> {
        self.ensure_loaded().await?;
        let now = Utc::now();
        Ok(self
            .todos_ordered()
            .into_iter()
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
        let mut entries = Vec::new();

        // Collect IDs to modify (can't mutate index while iterating)
        let expired_ids: Vec<String> = self
            .index
            .values()
            .filter(|t| {
                t.focus_deadline
                    .map(|deadline| deadline < now)
                    .unwrap_or(false)
            })
            .map(|t| t.id.clone())
            .collect();

        for id in expired_ids {
            if let Some(todo) = self.index.get_mut(&id) {
                // Increment expired count
                todo.focus_expired_count += 1;

                // Clear focus fields
                todo.focused_at = None;
                todo.focus_deadline = None;
                todo.updated_at = now;

                affected.push(todo.clone());
                entries.push(JournalEntry::Upsert {
                    todo: Box::new(todo.clone()),
                });
            }
        }

        if !entries.is_empty() {
            self.append_entries(&entries).await?;
            self.maybe_compact().await?;
        }

        Ok(affected)
    }

    /// Generate summary statistics
    pub async fn summary(&mut self) -> Result<TodoSummary> {
        self.ensure_loaded().await?;

        let now = Utc::now();

        let mut by_status = HashMap::new();
        let mut overdue = Vec::new();
        let mut upcoming_week = Vec::new();

        for todo in self.index.values() {
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
            total: self.index.len(),
            by_status,
            overdue,
            upcoming_week,
        })
    }

    /// Add a time entry to a todo (Phase 2)
    pub async fn add_time_entry(
        &mut self,
        id: &str,
        entry: crate::todo_types::TimeEntry,
    ) -> Result<bool> {
        self.ensure_loaded().await?;

        if let Some(todo) = self.index.get_mut(id) {
            todo.time_entries.push(entry);
            todo.updated_at = Utc::now();

            let journal_entry = JournalEntry::Upsert {
                todo: Box::new(todo.clone()),
            };
            self.append_entry(&journal_entry).await?;
            self.maybe_compact().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Close a running time entry (Phase 2)
    pub async fn close_time_entry(&mut self, id: &str, entry_id: &str) -> Result<bool> {
        self.ensure_loaded().await?;

        if let Some(todo) = self.index.get_mut(id) {
            if let Some(entry) = todo.time_entries.iter_mut().find(|e| e.id == entry_id) {
                if entry.ended_at.is_none() {
                    let now = Utc::now();
                    entry.ended_at = Some(now);
                    let duration = now.signed_duration_since(entry.started_at);
                    entry.duration_secs = Some(duration.num_seconds().max(0) as u64);

                    // Update denormalized total
                    todo.total_tracked_secs = todo
                        .time_entries
                        .iter()
                        .filter_map(|e| e.duration_secs)
                        .sum();

                    todo.updated_at = now;

                    let journal_entry = JournalEntry::Upsert {
                        todo: Box::new(todo.clone()),
                    };
                    self.append_entry(&journal_entry).await?;
                    self.maybe_compact().await?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Add an attachment to a todo (Phase 2)
    pub async fn add_attachment(
        &mut self,
        id: &str,
        attachment: crate::todo_types::Attachment,
    ) -> Result<bool> {
        self.ensure_loaded().await?;

        if let Some(todo) = self.index.get_mut(id) {
            todo.attachments.push(attachment);
            todo.updated_at = Utc::now();

            let journal_entry = JournalEntry::Upsert {
                todo: Box::new(todo.clone()),
            };
            self.append_entry(&journal_entry).await?;
            self.maybe_compact().await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Remove an attachment from a todo (Phase 2)
    pub async fn remove_attachment(&mut self, id: &str, attachment_id: &str) -> Result<bool> {
        self.ensure_loaded().await?;

        if let Some(todo) = self.index.get_mut(id) {
            let before_len = todo.attachments.len();
            todo.attachments.retain(|a| a.id != attachment_id);

            if todo.attachments.len() < before_len {
                todo.updated_at = Utc::now();

                let journal_entry = JournalEntry::Upsert {
                    todo: Box::new(todo.clone()),
                };
                self.append_entry(&journal_entry).await?;
                self.maybe_compact().await?;
                Ok(true)
            } else {
                Ok(false)
            }
        } else {
            Ok(false)
        }
    }

    /// Move a todo (reparent or change project) (Phase 2)
    pub async fn move_todo(
        &mut self,
        id: &str,
        new_parent_id: Option<String>,
        new_project_id: Option<String>,
    ) -> Result<Option<crate::todo_types::Todo>> {
        self.ensure_loaded().await?;

        if let Some(todo) = self.index.get_mut(id) {
            // Validate parent_id doesn't create a cycle
            if let Some(ref parent) = new_parent_id {
                if parent == id {
                    return Err(common::ToolError::InvalidParams(
                        "Cannot set task as its own parent".to_string(),
                    )
                    .into());
                }
                // TODO: Check for circular references by traversing parent chain
            }

            todo.parent_id = new_parent_id;
            todo.project_id = new_project_id;
            todo.updated_at = Utc::now();

            let updated = todo.clone();

            let journal_entry = JournalEntry::Upsert {
                todo: Box::new(updated.clone()),
            };
            self.append_entry(&journal_entry).await?;
            self.maybe_compact().await?;

            Ok(Some(updated))
        } else {
            Ok(None)
        }
    }

    /// Cascade completion to subtasks (Phase 2)
    pub async fn cascade_complete(&mut self, parent_id: &str) -> Result<Vec<String>> {
        self.ensure_loaded().await?;

        let mut completed_ids = Vec::new();
        let mut entries = Vec::new();

        // Find all direct children
        let child_ids: Vec<String> = self
            .index
            .values()
            .filter(|t| t.parent_id.as_ref() == Some(&parent_id.to_string()))
            .map(|t| t.id.clone())
            .collect();

        for child_id in child_ids {
            if let Some(todo) = self.index.get_mut(&child_id) {
                if todo.status != TodoStatus::Done {
                    todo.status = TodoStatus::Done;
                    todo.completed_at = Some(Utc::now());
                    todo.updated_at = Utc::now();

                    // Close any running time entries
                    for entry in &mut todo.time_entries {
                        if entry.ended_at.is_none() {
                            let now = Utc::now();
                            entry.ended_at = Some(now);
                            let duration = now.signed_duration_since(entry.started_at);
                            entry.duration_secs = Some(duration.num_seconds().max(0) as u64);
                        }
                    }

                    // Update denormalized total
                    todo.total_tracked_secs = todo
                        .time_entries
                        .iter()
                        .filter_map(|e| e.duration_secs)
                        .sum();

                    completed_ids.push(todo.id.clone());
                    entries.push(JournalEntry::Upsert {
                        todo: Box::new(todo.clone()),
                    });
                }
            }
        }

        if !entries.is_empty() {
            self.append_entries(&entries).await?;
            self.maybe_compact().await?;
        }

        Ok(completed_ids)
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
            .todos_ordered()
            .into_iter()
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

    // ── Dependency management ──────────────────────────────────────────

    /// Check if adding edge (from_id → to_id) would create a cycle.
    /// Uses iterative DFS from to_id following blocked_by edges.
    /// Returns true if cycle detected (i.e., to_id can reach from_id).
    pub async fn would_create_cycle(&mut self, from_id: &str, to_id: &str) -> Result<bool> {
        self.ensure_loaded().await?;

        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![to_id.to_string()];

        while let Some(current) = stack.pop() {
            if current == from_id {
                return Ok(true);
            }
            if !visited.insert(current.clone()) {
                continue;
            }
            if let Some(todo) = self.index.get(&current) {
                for dep in &todo.blocked_by {
                    if !visited.contains(dep) {
                        stack.push(dep.clone());
                    }
                }
            }
        }
        Ok(false)
    }

    /// Add a dependency: task_id is blocked by blocker_id.
    /// Maintains bidirectional consistency and checks for cycles.
    pub async fn add_dependency(&mut self, task_id: &str, blocker_id: &str) -> Result<()> {
        self.ensure_loaded().await?;

        // Validate both tasks exist
        if !self.index.contains_key(task_id) {
            return Err(
                common::ToolError::ExecutionFailed(format!("Task not found: {}", task_id)).into(),
            );
        }
        if !self.index.contains_key(blocker_id) {
            return Err(common::ToolError::ExecutionFailed(format!(
                "Task not found: {}",
                blocker_id
            ))
            .into());
        }

        // Self-dependency check
        if task_id == blocker_id {
            return Err(common::ToolError::InvalidParams("Cannot depend on self".into()).into());
        }

        // Cycle detection
        if self.would_create_cycle(task_id, blocker_id).await? {
            return Err(common::ToolError::InvalidParams(format!(
                "Adding dependency would create a cycle: {} → {} → ... → {}",
                task_id, blocker_id, task_id
            ))
            .into());
        }

        // Add blocked_by to task_id
        if let Some(task) = self.index.get_mut(task_id) {
            if !task.blocked_by.contains(&blocker_id.to_string()) {
                task.blocked_by.push(blocker_id.to_string());
                task.updated_at = Utc::now();
            }
        }

        // Add blocks to blocker_id
        if let Some(blocker) = self.index.get_mut(blocker_id) {
            if !blocker.blocks.contains(&task_id.to_string()) {
                blocker.blocks.push(task_id.to_string());
                blocker.updated_at = Utc::now();
            }
        }

        // Persist both changes atomically
        let entries = vec![
            JournalEntry::Upsert {
                todo: Box::new(self.index[task_id].clone()),
            },
            JournalEntry::Upsert {
                todo: Box::new(self.index[blocker_id].clone()),
            },
        ];
        self.append_entries(&entries).await?;
        self.maybe_compact().await?;

        Ok(())
    }

    /// Remove a dependency: task_id is no longer blocked by blocker_id.
    pub async fn remove_dependency(&mut self, task_id: &str, blocker_id: &str) -> Result<()> {
        self.ensure_loaded().await?;

        if let Some(task) = self.index.get_mut(task_id) {
            task.blocked_by.retain(|id| id != blocker_id);
            task.updated_at = Utc::now();
        }

        if let Some(blocker) = self.index.get_mut(blocker_id) {
            blocker.blocks.retain(|id| id != task_id);
            blocker.updated_at = Utc::now();
        }

        // Persist both changes atomically
        let mut entries = Vec::new();
        if let Some(task) = self.index.get(task_id) {
            entries.push(JournalEntry::Upsert {
                todo: Box::new(task.clone()),
            });
        }
        if let Some(blocker) = self.index.get(blocker_id) {
            entries.push(JournalEntry::Upsert {
                todo: Box::new(blocker.clone()),
            });
        }
        if !entries.is_empty() {
            self.append_entries(&entries).await?;
            self.maybe_compact().await?;
        }

        Ok(())
    }

    /// Get all incomplete blockers for a task.
    /// Returns tasks in blocked_by that are not Done or Archived.
    pub async fn incomplete_blockers(&mut self, task_id: &str) -> Result<Vec<Todo>> {
        self.ensure_loaded().await?;

        let blocked_by = match self.index.get(task_id) {
            Some(task) => task.blocked_by.clone(),
            None => return Ok(Vec::new()),
        };

        Ok(blocked_by
            .iter()
            .filter_map(|id| self.index.get(id))
            .filter(|t| t.status != TodoStatus::Done && t.status != TodoStatus::Archived)
            .cloned()
            .collect())
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

            // Phase 1 new fields (test defaults)
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

    #[tokio::test]
    async fn test_new_store_not_loaded() {
        let (store, _dir) = create_test_store().await;
        assert!(!store.loaded);
        assert_eq!(store.index.len(), 0);
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

    // ── Append-only specific tests ──────────────────────────────────────

    #[tokio::test]
    async fn test_append_only_journal_format() {
        // Verify the file uses journal entries, not plain Todo lines
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("Journal test");
        store.add(todo).await.unwrap();

        let content = fs::read_to_string(&store.file_path).await.unwrap();
        let line = content.lines().next().unwrap();

        // Should contain the _op tag
        assert!(
            line.contains("\"_op\":\"upsert\""),
            "Should use journal format"
        );
        assert!(line.contains("\"todo\":"), "Should wrap todo in entry");
    }

    #[tokio::test]
    async fn test_delete_writes_tombstone() {
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("To tombstone");
        let id = todo.id.clone();

        store.add(todo).await.unwrap();
        store.delete(&id).await.unwrap();

        let content = fs::read_to_string(&store.file_path).await.unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(lines.len(), 2, "Should have upsert + delete entries");
        assert!(
            lines[1].contains("\"_op\":\"delete\""),
            "Second line should be tombstone"
        );
        assert!(
            lines[1].contains(&id),
            "Tombstone should reference the deleted ID"
        );
    }

    #[tokio::test]
    async fn test_update_appends_not_rewrites() {
        let (mut store, _dir) = create_test_store().await;
        let todo = create_test_todo("Original");
        let id = todo.id.clone();

        store.add(todo).await.unwrap();

        let patch = TodoPatch {
            title: Some("Updated".to_string()),
            ..Default::default()
        };
        store.update(&id, patch).await.unwrap();

        let content = fs::read_to_string(&store.file_path).await.unwrap();
        let lines: Vec<&str> = content.lines().filter(|l| !l.is_empty()).collect();

        // Should have 2 entries: original add + update (both upserts)
        assert_eq!(lines.len(), 2, "Should append, not rewrite");
    }

    #[tokio::test]
    async fn test_journal_replay_deduplicates() {
        // Write a journal with duplicate entries for the same ID
        let (mut store, dir) = create_test_store().await;

        let todo = create_test_todo("V1");
        let id = todo.id.clone();
        store.add(todo).await.unwrap();

        let patch = TodoPatch {
            title: Some("V2".to_string()),
            ..Default::default()
        };
        store.update(&id, patch).await.unwrap();

        // Reload from disk
        let file_path = dir.path().join("todos.jsonl");
        let mut store2 = TodoStore::new(file_path);

        let todos = store2.list(&TodoFilter::default()).await.unwrap();
        assert_eq!(todos.len(), 1, "Should deduplicate to 1 todo");
        assert_eq!(todos[0].title, "V2", "Should use the latest version");
    }

    #[tokio::test]
    async fn test_journal_replay_respects_deletes() {
        let (mut store, dir) = create_test_store().await;

        let todo = create_test_todo("Will be deleted");
        let id = todo.id.clone();
        store.add(todo).await.unwrap();
        store.delete(&id).await.unwrap();

        // Reload from disk
        let file_path = dir.path().join("todos.jsonl");
        let mut store2 = TodoStore::new(file_path);

        let todos = store2.list(&TodoFilter::default()).await.unwrap();
        assert_eq!(
            todos.len(),
            0,
            "Deleted todo should not appear after replay"
        );
    }

    #[tokio::test]
    async fn test_backwards_compat_legacy_format() {
        // Write legacy format (plain Todo JSON lines) and verify we can read them
        let (mut store, _dir) = create_test_store().await;

        let todo1 = create_test_todo("Legacy 1");
        let todo2 = create_test_todo("Legacy 2");
        let id1 = todo1.id.clone();

        let content = format!(
            "{}\n{}\n",
            serde_json::to_string(&todo1).unwrap(),
            serde_json::to_string(&todo2).unwrap()
        );

        // Ensure parent dir exists
        if let Some(parent) = store.file_path.parent() {
            fs::create_dir_all(parent).await.unwrap();
        }
        fs::write(&store.file_path, content).await.unwrap();

        store.load().await.unwrap();

        let todos = store.list(&TodoFilter::default()).await.unwrap();
        assert_eq!(todos.len(), 2, "Should load legacy format");
        assert!(store.get(&id1).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_compaction_reduces_file_size() {
        let (mut store, _dir) = create_test_store().await;

        // Add a todo and update it many times to grow the journal
        let todo = create_test_todo("Compaction test");
        let id = todo.id.clone();
        store.add(todo).await.unwrap();

        for i in 0..50 {
            let patch = TodoPatch {
                title: Some(format!("Version {}", i)),
                ..Default::default()
            };
            store.update(&id, patch).await.unwrap();
        }

        // Journal should have 51 entries (1 add + 50 updates), 1 live todo
        let content_before = fs::read_to_string(&store.file_path).await.unwrap();
        let lines_before = content_before.lines().filter(|l| !l.is_empty()).count();
        assert_eq!(lines_before, 51);

        // Force compaction
        store.compact().await.unwrap();

        let content_after = fs::read_to_string(&store.file_path).await.unwrap();
        let lines_after = content_after.lines().filter(|l| !l.is_empty()).count();
        assert_eq!(lines_after, 1, "Compaction should reduce to 1 live entry");

        // Verify data integrity after compaction
        let retrieved = store.get(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.title, "Version 49");
    }

    #[tokio::test]
    async fn test_auto_compaction_triggers() {
        let (mut store, _dir) = create_test_store().await;

        // Add a todo and update it COMPACTION_THRESHOLD times
        let todo = create_test_todo("Auto compact");
        let id = todo.id.clone();
        store.add(todo).await.unwrap();

        for i in 0..COMPACTION_THRESHOLD {
            let patch = TodoPatch {
                title: Some(format!("V{}", i)),
                ..Default::default()
            };
            store.update(&id, patch).await.unwrap();
        }

        // After COMPACTION_THRESHOLD stale entries, compaction should have run
        let content = fs::read_to_string(&store.file_path).await.unwrap();
        let lines = content.lines().filter(|l| !l.is_empty()).count();

        // After compaction, should be just the live entries (1 todo)
        assert_eq!(lines, 1, "Auto-compaction should have cleaned up");
        assert_eq!(store.journal_len, 1);

        // Data should still be correct
        let retrieved = store.get(&id).await.unwrap().unwrap();
        assert_eq!(retrieved.title, format!("V{}", COMPACTION_THRESHOLD - 1));
    }

    #[tokio::test]
    async fn test_mixed_legacy_and_journal_format() {
        // Simulate a migration scenario: file starts with legacy lines, then journal entries
        let (mut store, _dir) = create_test_store().await;

        let legacy_todo = create_test_todo("Legacy");
        let legacy_id = legacy_todo.id.clone();

        let journal_todo = create_test_todo("Journal");
        let journal_id = journal_todo.id.clone();
        let journal_entry = JournalEntry::Upsert {
            todo: Box::new(journal_todo),
        };

        let content = format!(
            "{}\n{}\n",
            serde_json::to_string(&legacy_todo).unwrap(),
            serde_json::to_string(&journal_entry).unwrap()
        );

        if let Some(parent) = store.file_path.parent() {
            fs::create_dir_all(parent).await.unwrap();
        }
        fs::write(&store.file_path, content).await.unwrap();

        store.load().await.unwrap();

        assert!(store.get(&legacy_id).await.unwrap().is_some());
        assert!(store.get(&journal_id).await.unwrap().is_some());
        let todos = store.list(&TodoFilter::default()).await.unwrap();
        assert_eq!(todos.len(), 2);
    }

    #[tokio::test]
    async fn test_insertion_order_preserved() {
        let (mut store, _dir) = create_test_store().await;

        let todo_a = create_test_todo("Alpha");
        let todo_b = create_test_todo("Beta");
        let todo_c = create_test_todo("Charlie");

        store.add(todo_a.clone()).await.unwrap();
        store.add(todo_b.clone()).await.unwrap();
        store.add(todo_c.clone()).await.unwrap();

        let todos = store.list(&TodoFilter::default()).await.unwrap();
        assert_eq!(todos[0].title, "Alpha");
        assert_eq!(todos[1].title, "Beta");
        assert_eq!(todos[2].title, "Charlie");
    }

    // ── Dependency tests ──────────────────────────────────────────────

    #[tokio::test]
    async fn test_add_dependency() {
        let (mut store, _dir) = create_test_store().await;
        let task = create_test_todo("Blocked task");
        let blocker = create_test_todo("Blocker");
        let task_id = task.id.clone();
        let blocker_id = blocker.id.clone();

        store.add(task).await.unwrap();
        store.add(blocker).await.unwrap();

        store.add_dependency(&task_id, &blocker_id).await.unwrap();

        let t = store.get(&task_id).await.unwrap().unwrap();
        assert!(t.blocked_by.contains(&blocker_id));

        let b = store.get(&blocker_id).await.unwrap().unwrap();
        assert!(b.blocks.contains(&task_id));
    }

    #[tokio::test]
    async fn test_add_dependency_self_ref() {
        let (mut store, _dir) = create_test_store().await;
        let task = create_test_todo("Self ref");
        let id = task.id.clone();
        store.add(task).await.unwrap();

        let result = store.add_dependency(&id, &id).await;
        assert!(result.is_err(), "Self-dependency should fail");
    }

    #[tokio::test]
    async fn test_add_dependency_nonexistent_task() {
        let (mut store, _dir) = create_test_store().await;
        let task = create_test_todo("Exists");
        let id = task.id.clone();
        store.add(task).await.unwrap();

        let result = store.add_dependency(&id, "nonexistent").await;
        assert!(result.is_err(), "Nonexistent blocker should fail");

        let result = store.add_dependency("nonexistent", &id).await;
        assert!(result.is_err(), "Nonexistent task should fail");
    }

    #[tokio::test]
    async fn test_add_dependency_idempotent() {
        let (mut store, _dir) = create_test_store().await;
        let task = create_test_todo("Task");
        let blocker = create_test_todo("Blocker");
        let task_id = task.id.clone();
        let blocker_id = blocker.id.clone();

        store.add(task).await.unwrap();
        store.add(blocker).await.unwrap();

        store.add_dependency(&task_id, &blocker_id).await.unwrap();
        store.add_dependency(&task_id, &blocker_id).await.unwrap();

        let t = store.get(&task_id).await.unwrap().unwrap();
        assert_eq!(
            t.blocked_by.iter().filter(|x| **x == blocker_id).count(),
            1,
            "Should not duplicate"
        );
    }

    #[tokio::test]
    async fn test_would_create_cycle_simple() {
        let (mut store, _dir) = create_test_store().await;
        let a = create_test_todo("A");
        let b = create_test_todo("B");
        let a_id = a.id.clone();
        let b_id = b.id.clone();

        store.add(a).await.unwrap();
        store.add(b).await.unwrap();

        // A blocked by B
        store.add_dependency(&a_id, &b_id).await.unwrap();

        // B blocked by A would create a cycle
        assert!(store.would_create_cycle(&b_id, &a_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_would_create_cycle_transitive() {
        let (mut store, _dir) = create_test_store().await;
        let a = create_test_todo("A");
        let b = create_test_todo("B");
        let c = create_test_todo("C");
        let a_id = a.id.clone();
        let b_id = b.id.clone();
        let c_id = c.id.clone();

        store.add(a).await.unwrap();
        store.add(b).await.unwrap();
        store.add(c).await.unwrap();

        // A blocked by B, B blocked by C
        store.add_dependency(&a_id, &b_id).await.unwrap();
        store.add_dependency(&b_id, &c_id).await.unwrap();

        // C blocked by A would create A→B→C→A cycle
        assert!(store.would_create_cycle(&c_id, &a_id).await.unwrap());

        // A blocked by C would NOT create a cycle (C→B is not in the graph)
        assert!(!store.would_create_cycle(&a_id, &c_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_add_dependency_cycle_rejected() {
        let (mut store, _dir) = create_test_store().await;
        let a = create_test_todo("A");
        let b = create_test_todo("B");
        let a_id = a.id.clone();
        let b_id = b.id.clone();

        store.add(a).await.unwrap();
        store.add(b).await.unwrap();

        store.add_dependency(&a_id, &b_id).await.unwrap();

        let result = store.add_dependency(&b_id, &a_id).await;
        assert!(result.is_err(), "Cycle should be rejected");
    }

    #[tokio::test]
    async fn test_remove_dependency() {
        let (mut store, _dir) = create_test_store().await;
        let task = create_test_todo("Task");
        let blocker = create_test_todo("Blocker");
        let task_id = task.id.clone();
        let blocker_id = blocker.id.clone();

        store.add(task).await.unwrap();
        store.add(blocker).await.unwrap();

        store.add_dependency(&task_id, &blocker_id).await.unwrap();
        store
            .remove_dependency(&task_id, &blocker_id)
            .await
            .unwrap();

        let t = store.get(&task_id).await.unwrap().unwrap();
        assert!(t.blocked_by.is_empty());

        let b = store.get(&blocker_id).await.unwrap().unwrap();
        assert!(b.blocks.is_empty());
    }

    #[tokio::test]
    async fn test_remove_dependency_nonexistent_noop() {
        let (mut store, _dir) = create_test_store().await;
        let task = create_test_todo("Task");
        let task_id = task.id.clone();
        store.add(task).await.unwrap();

        // Should not error when removing a dependency that doesn't exist
        store
            .remove_dependency(&task_id, "nonexistent")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_incomplete_blockers() {
        let (mut store, _dir) = create_test_store().await;
        let task = create_test_todo("Task");
        let blocker1 = create_test_todo("Incomplete blocker");
        let mut blocker2 = create_test_todo("Done blocker");
        blocker2.status = TodoStatus::Done;
        let mut blocker3 = create_test_todo("Archived blocker");
        blocker3.status = TodoStatus::Archived;

        let task_id = task.id.clone();
        let b1_id = blocker1.id.clone();
        let b2_id = blocker2.id.clone();
        let b3_id = blocker3.id.clone();

        store.add(task).await.unwrap();
        store.add(blocker1).await.unwrap();
        store.add(blocker2).await.unwrap();
        store.add(blocker3).await.unwrap();

        store.add_dependency(&task_id, &b1_id).await.unwrap();
        store.add_dependency(&task_id, &b2_id).await.unwrap();
        store.add_dependency(&task_id, &b3_id).await.unwrap();

        let blockers = store.incomplete_blockers(&task_id).await.unwrap();
        assert_eq!(blockers.len(), 1, "Only incomplete blocker should appear");
        assert_eq!(blockers[0].id, b1_id);
    }

    #[tokio::test]
    async fn test_incomplete_blockers_nonexistent() {
        let (mut store, _dir) = create_test_store().await;
        let blockers = store.incomplete_blockers("nonexistent").await.unwrap();
        assert!(blockers.is_empty());
    }

    #[tokio::test]
    async fn test_delete_cascades_dependencies() {
        let (mut store, _dir) = create_test_store().await;
        let a = create_test_todo("A");
        let b = create_test_todo("B");
        let c = create_test_todo("C");
        let a_id = a.id.clone();
        let b_id = b.id.clone();
        let c_id = c.id.clone();

        store.add(a).await.unwrap();
        store.add(b).await.unwrap();
        store.add(c).await.unwrap();

        // A blocked by B, C blocked by B
        store.add_dependency(&a_id, &b_id).await.unwrap();
        store.add_dependency(&c_id, &b_id).await.unwrap();

        // Delete B — should clean up A.blocked_by and C.blocked_by
        store.delete(&b_id).await.unwrap();

        let a = store.get(&a_id).await.unwrap().unwrap();
        assert!(
            a.blocked_by.is_empty(),
            "A should no longer be blocked by deleted B"
        );

        let c = store.get(&c_id).await.unwrap().unwrap();
        assert!(
            c.blocked_by.is_empty(),
            "C should no longer be blocked by deleted B"
        );
    }

    #[tokio::test]
    async fn test_delete_cascades_blocks() {
        let (mut store, _dir) = create_test_store().await;
        let a = create_test_todo("A");
        let b = create_test_todo("B");
        let a_id = a.id.clone();
        let b_id = b.id.clone();

        store.add(a).await.unwrap();
        store.add(b).await.unwrap();

        // A blocked by B (so B.blocks contains A)
        store.add_dependency(&a_id, &b_id).await.unwrap();

        // Delete A — should clean up B.blocks
        store.delete(&a_id).await.unwrap();

        let b = store.get(&b_id).await.unwrap().unwrap();
        assert!(
            b.blocks.is_empty(),
            "B should no longer list deleted A in blocks"
        );
    }

    #[tokio::test]
    async fn test_dependency_persists_across_reload() {
        let (mut store, dir) = create_test_store().await;
        let task = create_test_todo("Task");
        let blocker = create_test_todo("Blocker");
        let task_id = task.id.clone();
        let blocker_id = blocker.id.clone();

        store.add(task).await.unwrap();
        store.add(blocker).await.unwrap();
        store.add_dependency(&task_id, &blocker_id).await.unwrap();

        // Reload from disk
        let file_path = dir.path().join("todos.jsonl");
        let mut store2 = TodoStore::new(file_path);

        let t = store2.get(&task_id).await.unwrap().unwrap();
        assert!(
            t.blocked_by.contains(&blocker_id),
            "Dependency should survive reload"
        );

        let b = store2.get(&blocker_id).await.unwrap().unwrap();
        assert!(
            b.blocks.contains(&task_id),
            "Reverse dependency should survive reload"
        );
    }

    // ── Template and recurring task tests ──────────────────────────────

    #[tokio::test]
    async fn test_list_excludes_templates_by_default() {
        let (mut store, _dir) = create_test_store().await;

        // Add a normal task
        let normal = create_test_todo("Normal task");
        store.add(normal.clone()).await.unwrap();

        // Add a template task
        let mut template = create_test_todo("Daily standup");
        template.is_template = true;
        template.recurrence_rule = Some("FREQ=DAILY;BYHOUR=9".to_string());
        store.add(template.clone()).await.unwrap();

        // Default filter (include_templates = false) should exclude templates
        let filter = TodoFilter::default();
        let results = store.list(&filter).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, normal.id);
    }

    #[tokio::test]
    async fn test_list_includes_templates_when_requested() {
        let (mut store, _dir) = create_test_store().await;

        let normal = create_test_todo("Normal task");
        store.add(normal.clone()).await.unwrap();

        let mut template = create_test_todo("Weekly review");
        template.is_template = true;
        store.add(template.clone()).await.unwrap();

        let filter = TodoFilter {
            include_templates: true,
            ..Default::default()
        };
        let results = store.list(&filter).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_list_templates_only() {
        let (mut store, _dir) = create_test_store().await;

        // Add 2 normal tasks and 1 template
        store.add(create_test_todo("Task A")).await.unwrap();
        store.add(create_test_todo("Task B")).await.unwrap();

        let mut template = create_test_todo("Monthly report");
        template.is_template = true;
        template.recurrence_rule = Some("FREQ=MONTHLY;BYMONTHDAY=1".to_string());
        store.add(template.clone()).await.unwrap();

        let templates = store.list_templates().await.unwrap();
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].id, template.id);
        assert!(templates[0].is_template);
    }

    #[tokio::test]
    async fn test_list_templates_empty() {
        let (mut store, _dir) = create_test_store().await;

        store.add(create_test_todo("Normal task")).await.unwrap();

        let templates = store.list_templates().await.unwrap();
        assert!(templates.is_empty());
    }

    #[tokio::test]
    async fn test_update_next_instance_date() {
        let (mut store, _dir) = create_test_store().await;

        let mut template = create_test_todo("Daily standup");
        template.is_template = true;
        template.recurrence_rule = Some("FREQ=DAILY".to_string());
        let id = template.id.clone();
        store.add(template).await.unwrap();

        // Initially None
        let t = store.get(&id).await.unwrap().unwrap();
        assert!(t.next_instance_date.is_none());

        // Set a next instance date
        let next = Utc::now() + chrono::Duration::days(1);
        store
            .update_next_instance_date(&id, Some(next))
            .await
            .unwrap();

        let t = store.get(&id).await.unwrap().unwrap();
        assert_eq!(t.next_instance_date.unwrap(), next);

        // Clear it
        store.update_next_instance_date(&id, None).await.unwrap();
        let t = store.get(&id).await.unwrap().unwrap();
        assert!(t.next_instance_date.is_none());
    }

    #[tokio::test]
    async fn test_update_next_instance_date_not_found() {
        let (mut store, _dir) = create_test_store().await;

        let result = store
            .update_next_instance_date("nonexistent", Some(Utc::now()))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_next_instance_date_persists() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("todos.jsonl");

        let next = Utc::now() + chrono::Duration::hours(12);
        let id;

        {
            let mut store = TodoStore::new(file_path.clone());
            let mut template = create_test_todo("Recurring task");
            template.is_template = true;
            id = template.id.clone();
            store.add(template).await.unwrap();
            store
                .update_next_instance_date(&id, Some(next))
                .await
                .unwrap();
        }

        // Reload from disk
        let mut store2 = TodoStore::new(file_path);
        let t = store2.get(&id).await.unwrap().unwrap();
        assert_eq!(
            t.next_instance_date.unwrap(),
            next,
            "next_instance_date should survive reload"
        );
    }
}
