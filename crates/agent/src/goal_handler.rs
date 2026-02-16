//! GoalHandlerImpl — Implements GoalHandler trait for agent crate (Layer 5).
//!
//! Bridges the GoalHandler trait (defined in tools at Layer 3) with the
//! GoalStore (defined in goal at Layer 2), following the dependency
//! inversion pattern used by CalendarSyncAdapter.

use async_trait::async_trait;
use common::{GoalError, Result};
use goal::{Goal, GoalProgress, GoalStatus, GoalStore};
use std::sync::Arc;
use tokio::sync::RwLock;
use tools::goal_tool::GoalHandler;
use uuid::Uuid;

/// Implements GoalHandler by delegating to GoalStore.
pub struct GoalHandlerImpl {
    store: Arc<RwLock<GoalStore>>,
}

impl GoalHandlerImpl {
    pub fn new(store: Arc<RwLock<GoalStore>>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl GoalHandler for GoalHandlerImpl {
    async fn create_goal(&self, goal: Goal) -> Result<Uuid> {
        let mut store = self.store.write().await;
        let created = store.add(goal).await?;
        Ok(created.id)
    }

    async fn get_goal(&self, id: &Uuid) -> Result<Option<Goal>> {
        let mut store = self.store.write().await;
        store.get(id).await
    }

    async fn list_goals(&self, status: Option<GoalStatus>) -> Result<Vec<Goal>> {
        let mut store = self.store.write().await;
        store.list(status).await
    }

    async fn update_goal(&self, goal: Goal) -> Result<()> {
        let mut store = self.store.write().await;
        store.update(goal).await?;
        Ok(())
    }

    async fn delete_goal(&self, id: &Uuid) -> Result<()> {
        let mut store = self.store.write().await;
        let deleted = store.delete(id).await?;
        if !deleted {
            return Err(GoalError::NotFound(id.to_string()).into());
        }
        Ok(())
    }

    async fn calculate_progress(&self, id: &Uuid) -> Result<GoalProgress> {
        let mut store = self.store.write().await;
        store
            .calculate_progress(id)
            .await?
            .ok_or_else(|| GoalError::NotFound(id.to_string()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_test_goal(title: &str) -> Goal {
        let now = Utc::now();
        Goal {
            id: Uuid::new_v4(),
            title: title.to_string(),
            description: String::new(),
            status: GoalStatus::Active,
            priority: 3,
            target_date: None,
            created_at: now,
            updated_at: now,
            metrics: vec![],
            linked_project_ids: vec![],
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_goal_handler_create_and_get() {
        let tmp = TempDir::new().unwrap();
        let store = Arc::new(RwLock::new(GoalStore::new(
            tmp.path().join("goals.jsonl"),
        )));

        let handler = GoalHandlerImpl::new(store);

        let goal = create_test_goal("Test Goal");
        let goal_id = handler.create_goal(goal).await.unwrap();

        let retrieved = handler.get_goal(&goal_id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().title, "Test Goal");
    }

    #[tokio::test]
    async fn test_goal_handler_list() {
        let tmp = TempDir::new().unwrap();
        let store = Arc::new(RwLock::new(GoalStore::new(
            tmp.path().join("goals.jsonl"),
        )));

        let handler = GoalHandlerImpl::new(store);

        handler.create_goal(create_test_goal("Goal A")).await.unwrap();
        handler.create_goal(create_test_goal("Goal B")).await.unwrap();

        let all = handler.list_goals(None).await.unwrap();
        assert_eq!(all.len(), 2);

        let active = handler.list_goals(Some(GoalStatus::Active)).await.unwrap();
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn test_goal_handler_update() {
        let tmp = TempDir::new().unwrap();
        let store = Arc::new(RwLock::new(GoalStore::new(
            tmp.path().join("goals.jsonl"),
        )));

        let handler = GoalHandlerImpl::new(store);

        let goal = create_test_goal("Original");
        let id = handler.create_goal(goal).await.unwrap();

        let mut to_update = handler.get_goal(&id).await.unwrap().unwrap();
        to_update.title = "Updated".to_string();
        to_update.status = GoalStatus::Paused;
        handler.update_goal(to_update).await.unwrap();

        let updated = handler.get_goal(&id).await.unwrap().unwrap();
        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.status, GoalStatus::Paused);
    }

    #[tokio::test]
    async fn test_goal_handler_delete() {
        let tmp = TempDir::new().unwrap();
        let store = Arc::new(RwLock::new(GoalStore::new(
            tmp.path().join("goals.jsonl"),
        )));

        let handler = GoalHandlerImpl::new(store);

        let goal = create_test_goal("To Delete");
        let id = handler.create_goal(goal).await.unwrap();
        handler.delete_goal(&id).await.unwrap();

        let retrieved = handler.get_goal(&id).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_goal_handler_calculate_progress() {
        let tmp = TempDir::new().unwrap();
        let store = Arc::new(RwLock::new(GoalStore::new(
            tmp.path().join("goals.jsonl"),
        )));

        let handler = GoalHandlerImpl::new(store);

        let goal = create_test_goal("With Progress");
        let id = handler.create_goal(goal).await.unwrap();

        let progress = handler.calculate_progress(&id).await.unwrap();
        assert_eq!(progress.goal_id, id);
        assert_eq!(progress.completion_percentage, 0.0);
    }
}
