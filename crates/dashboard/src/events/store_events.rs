//! Event-emitting store wrappers for the dashboard.
//!
//! Instead of modifying `TodoStore` or `ProjectStore` (which would add
//! dashboard dependencies to the `tools` crate), these thin wrappers
//! intercept writes and emit `DashboardEvent`s on every mutation.
//!
//! Reads delegate directly to the inner store — no event overhead for queries.

use std::sync::Arc;
use tokio::sync::RwLock;

use common::Result;
use tools::project_store::ProjectStore;
use tools::project_types::{Project, ProjectFilter, ProjectPatch};
use tools::todo_store::TodoStore;
use tools::todo_types::{Todo, TodoFilter, TodoPatch, TodoStatus, TodoSummary};

use super::{ChangeAction, DashboardEvent, DashboardEventBus};

/// Event-emitting wrapper around `TodoStore`.
///
/// GraphQL mutations route through this wrapper so that every write
/// automatically broadcasts a `DashboardEvent` to all WebSocket subscribers.
pub struct ObservableTodoStore {
    inner: Arc<RwLock<TodoStore>>,
    event_bus: Arc<DashboardEventBus>,
}

impl ObservableTodoStore {
    /// Create a new wrapper around an existing shared `TodoStore`.
    pub fn new(inner: Arc<RwLock<TodoStore>>, event_bus: Arc<DashboardEventBus>) -> Self {
        Self { inner, event_bus }
    }

    /// Get the inner store reference for read-only delegation.
    pub fn inner(&self) -> &Arc<RwLock<TodoStore>> {
        &self.inner
    }

    fn emit(&self, event: DashboardEvent) {
        self.event_bus.publish(event);
    }

    /// Add a new todo and emit a `Created` event.
    pub async fn add(&self, todo: Todo) -> Result<Todo> {
        let result = self.inner.write().await.add(todo).await?;
        self.emit(DashboardEvent::TodoChanged {
            action: ChangeAction::Created,
            id: result.id.clone(),
        });
        Ok(result)
    }

    /// Get a todo by ID (read-only, no event).
    pub async fn get(&self, id: &str) -> Result<Option<Todo>> {
        self.inner.write().await.get(id).await
    }

    /// Update a todo and emit an `Updated` event.
    pub async fn update(&self, id: &str, patch: TodoPatch) -> Result<Option<Todo>> {
        let result = self.inner.write().await.update(id, patch).await?;
        if result.is_some() {
            self.emit(DashboardEvent::TodoChanged {
                action: ChangeAction::Updated,
                id: id.to_string(),
            });
        }
        Ok(result)
    }

    /// Delete a todo and emit a `Deleted` event.
    pub async fn delete(&self, id: &str) -> Result<bool> {
        let deleted = self.inner.write().await.delete(id).await?;
        if deleted {
            self.emit(DashboardEvent::TodoChanged {
                action: ChangeAction::Deleted,
                id: id.to_string(),
            });
        }
        Ok(deleted)
    }

    /// List todos (read-only, no event).
    pub async fn list(&self, filter: &TodoFilter) -> Result<Vec<Todo>> {
        self.inner.write().await.list(filter).await
    }

    /// Get summary (read-only, no event).
    pub async fn summary(&self) -> Result<TodoSummary> {
        self.inner.write().await.summary().await
    }

    /// Focus a todo and emit an `Updated` event.
    pub async fn focus(&self, id: &str, max_slots: usize, deadline_hours: u64) -> Result<bool> {
        let focused = self
            .inner
            .write()
            .await
            .focus(id, max_slots, deadline_hours)
            .await?;
        if focused {
            self.emit(DashboardEvent::TodoChanged {
                action: ChangeAction::Updated,
                id: id.to_string(),
            });
        }
        Ok(focused)
    }

    /// Unfocus a todo and emit an `Updated` event.
    pub async fn unfocus(&self, id: &str) -> Result<bool> {
        let unfocused = self.inner.write().await.unfocus(id).await?;
        if unfocused {
            self.emit(DashboardEvent::TodoChanged {
                action: ChangeAction::Updated,
                id: id.to_string(),
            });
        }
        Ok(unfocused)
    }

    /// Complete a todo (transition to Done) and emit an `Updated` event.
    pub async fn complete(&self, id: &str) -> Result<Option<Todo>> {
        let patch = TodoPatch {
            status: Some(TodoStatus::Done),
            ..Default::default()
        };
        let result = self.inner.write().await.update(id, patch).await?;
        if result.is_some() {
            self.emit(DashboardEvent::TodoChanged {
                action: ChangeAction::Updated,
                id: id.to_string(),
            });
        }
        Ok(result)
    }
}

/// Event-emitting wrapper around `ProjectStore`.
pub struct ObservableProjectStore {
    inner: Arc<RwLock<ProjectStore>>,
    event_bus: Arc<DashboardEventBus>,
}

impl ObservableProjectStore {
    pub fn new(inner: Arc<RwLock<ProjectStore>>, event_bus: Arc<DashboardEventBus>) -> Self {
        Self { inner, event_bus }
    }

    pub fn inner(&self) -> &Arc<RwLock<ProjectStore>> {
        &self.inner
    }

    fn emit(&self, event: DashboardEvent) {
        self.event_bus.publish(event);
    }

    pub async fn add(&self, project: Project) -> Result<Project> {
        let result = self.inner.write().await.add(project).await?;
        self.emit(DashboardEvent::ProjectChanged {
            action: ChangeAction::Created,
            id: result.id.clone(),
        });
        Ok(result)
    }

    pub async fn get(&self, id: &str) -> Result<Option<Project>> {
        self.inner.write().await.get(id).await
    }

    pub async fn update(&self, id: &str, patch: ProjectPatch) -> Result<Option<Project>> {
        let result = self.inner.write().await.update(id, patch).await?;
        if result.is_some() {
            self.emit(DashboardEvent::ProjectChanged {
                action: ChangeAction::Updated,
                id: id.to_string(),
            });
        }
        Ok(result)
    }

    pub async fn delete(&self, id: &str) -> Result<bool> {
        let deleted = self.inner.write().await.delete(id).await?;
        if deleted {
            self.emit(DashboardEvent::ProjectChanged {
                action: ChangeAction::Deleted,
                id: id.to_string(),
            });
        }
        Ok(deleted)
    }

    pub async fn list(&self, filter: &ProjectFilter) -> Result<Vec<Project>> {
        self.inner.write().await.list(filter).await
    }
}
