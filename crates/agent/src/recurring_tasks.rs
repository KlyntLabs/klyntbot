//! RecurringTaskSpawner - Background job for spawning recurring task instances.
//!
//! Follows the ReminderEngine pattern (ADR-013): periodic check loop with
//! CancellationToken for graceful shutdown. Reads template tasks, checks if
//! instances are due, clones them as concrete todos, and advances the template's
//! next_instance_date.

use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use tools::rrule_utils;
use tools::todo_store::TodoStore;
use tools::todo_types::Todo;

/// Background spawner for recurring task instances.
pub struct RecurringTaskSpawner {
    todo_store: Arc<RwLock<TodoStore>>,
    timezone: String,
    check_interval: StdDuration,
    task_handle: Option<JoinHandle<()>>,
    cancel_token: CancellationToken,
}

impl RecurringTaskSpawner {
    /// Create a new RecurringTaskSpawner.
    pub fn new(
        todo_store: Arc<RwLock<TodoStore>>,
        timezone: String,
        check_interval: StdDuration,
    ) -> Self {
        Self {
            todo_store,
            timezone,
            check_interval,
            task_handle: None,
            cancel_token: CancellationToken::new(),
        }
    }

    /// Start the background spawner task.
    pub fn start(&mut self) {
        let todo_store = Arc::clone(&self.todo_store);
        let timezone = self.timezone.clone();
        let check_interval = self.check_interval;
        let cancel_token = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            debug!(
                "RecurringTaskSpawner started (interval: {:?})",
                check_interval
            );
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        debug!("RecurringTaskSpawner cancelled");
                        break;
                    }
                    _ = tokio::time::sleep(check_interval) => {
                        if let Err(e) = Self::check_and_spawn(&todo_store, &timezone).await {
                            error!("RecurringTaskSpawner check failed: {}", e);
                        }
                    }
                }
            }
        });

        self.task_handle = Some(handle);
    }

    /// Stop the background spawner.
    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            let _ = handle.await;
        }
    }

    /// Check all templates and spawn instances that are due.
    async fn check_and_spawn(
        todo_store: &Arc<RwLock<TodoStore>>,
        _timezone: &str,
    ) -> common::Result<()> {
        let mut store = todo_store.write().await;
        let templates = store.list_templates().await?;
        let now = chrono::Utc::now();

        for template in &templates {
            // Skip templates without a recurrence rule
            let rule = match &template.recurrence_rule {
                Some(r) => r.clone(),
                None => continue,
            };

            // Check if an instance is due
            if !rrule_utils::should_spawn_instance(template.next_instance_date, now) {
                continue;
            }

            // Create instance from template
            let mut instance = Todo::default_instance();
            instance.title = template.title.clone();
            instance.description = template.description.clone();
            instance.priority = template.priority;
            instance.tags = template.tags.clone();
            instance.project_id = template.project_id.clone();
            instance.recurrence_parent_id = Some(template.id.clone());
            instance.due_date = template.next_instance_date;

            let instance_id = instance.id.clone();
            store.add(instance).await?;

            // Advance next_instance_date
            let next = rrule_utils::next_occurrence(&rule, now)?;
            store.update_next_instance_date(&template.id, next).await?;

            info!(
                "Spawned recurring instance [{}] from template [{}] '{}'",
                instance_id, template.id, template.title
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use tempfile::TempDir;
    use tools::todo_types::{TodoFilter, TodoStatus};

    async fn create_test_store() -> (Arc<RwLock<TodoStore>>, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("todos.jsonl");
        let store = Arc::new(RwLock::new(TodoStore::new(file_path)));
        (store, temp_dir)
    }

    fn create_template(title: &str, rule: &str, next: chrono::DateTime<Utc>) -> Todo {
        let mut todo = Todo::default_instance();
        todo.title = title.to_string();
        todo.is_template = true;
        todo.recurrence_rule = Some(rule.to_string());
        todo.next_instance_date = Some(next);
        todo
    }

    #[tokio::test]
    async fn test_spawns_due_instance() {
        let (store, _dir) = create_test_store().await;
        let past = Utc::now() - Duration::hours(1);
        let template = create_template("Daily standup", "FREQ=DAILY", past);
        let template_id = template.id.clone();

        {
            let mut s = store.write().await;
            s.add(template).await.unwrap();
        }

        RecurringTaskSpawner::check_and_spawn(&store, "UTC")
            .await
            .unwrap();

        let mut s = store.write().await;
        // List with include_templates to see everything
        let filter = TodoFilter {
            include_templates: true,
            ..Default::default()
        };
        let all = s.list(&filter).await.unwrap();
        assert_eq!(all.len(), 2, "Should have template + instance");

        let instance = all.iter().find(|t| !t.is_template).unwrap();
        assert_eq!(instance.title, "Daily standup");
        assert_eq!(instance.recurrence_parent_id, Some(template_id));
        assert_eq!(instance.status, TodoStatus::Todo);
    }

    #[tokio::test]
    async fn test_does_not_spawn_future_instance() {
        let (store, _dir) = create_test_store().await;
        let future = Utc::now() + Duration::hours(24);
        let template = create_template("Weekly review", "FREQ=WEEKLY", future);

        {
            let mut s = store.write().await;
            s.add(template).await.unwrap();
        }

        RecurringTaskSpawner::check_and_spawn(&store, "UTC")
            .await
            .unwrap();

        let mut s = store.write().await;
        let filter = TodoFilter {
            include_templates: true,
            ..Default::default()
        };
        let all = s.list(&filter).await.unwrap();
        assert_eq!(all.len(), 1, "Should only have the template");
    }

    #[tokio::test]
    async fn test_advances_next_instance_date() {
        let (store, _dir) = create_test_store().await;
        let past = Utc::now() - Duration::hours(1);
        let template = create_template("Daily task", "FREQ=DAILY", past);
        let template_id = template.id.clone();

        {
            let mut s = store.write().await;
            s.add(template).await.unwrap();
        }

        RecurringTaskSpawner::check_and_spawn(&store, "UTC")
            .await
            .unwrap();

        let mut s = store.write().await;
        let updated_template = s.get(&template_id).await.unwrap().unwrap();
        assert!(
            updated_template.next_instance_date.is_some(),
            "Should have advanced next_instance_date"
        );
        assert!(
            updated_template.next_instance_date.unwrap() > Utc::now(),
            "Next instance date should be in the future"
        );
    }

    #[tokio::test]
    async fn test_skips_template_without_rule() {
        let (store, _dir) = create_test_store().await;

        // Template with no recurrence_rule
        let mut todo = Todo::default_instance();
        todo.title = "No rule".to_string();
        todo.is_template = true;
        todo.next_instance_date = Some(Utc::now() - Duration::hours(1));

        {
            let mut s = store.write().await;
            s.add(todo).await.unwrap();
        }

        RecurringTaskSpawner::check_and_spawn(&store, "UTC")
            .await
            .unwrap();

        let mut s = store.write().await;
        let filter = TodoFilter {
            include_templates: true,
            ..Default::default()
        };
        let all = s.list(&filter).await.unwrap();
        assert_eq!(all.len(), 1, "Should not spawn without a rule");
    }

    #[tokio::test]
    async fn test_instance_inherits_template_fields() {
        let (store, _dir) = create_test_store().await;
        let past = Utc::now() - Duration::hours(1);

        let mut template = create_template("Team sync", "FREQ=WEEKLY;BYDAY=MO", past);
        template.description = Some("Weekly team sync meeting".to_string());
        template.priority = Some(4);
        template.tags = vec!["meetings".to_string(), "team".to_string()];
        template.project_id = Some("proj-abc".to_string());

        {
            let mut s = store.write().await;
            s.add(template).await.unwrap();
        }

        RecurringTaskSpawner::check_and_spawn(&store, "UTC")
            .await
            .unwrap();

        let mut s = store.write().await;
        let filter = TodoFilter {
            include_templates: true,
            ..Default::default()
        };
        let all = s.list(&filter).await.unwrap();
        let instance = all.iter().find(|t| !t.is_template).unwrap();

        assert_eq!(instance.title, "Team sync");
        assert_eq!(
            instance.description,
            Some("Weekly team sync meeting".to_string())
        );
        assert_eq!(instance.priority, Some(4));
        assert_eq!(instance.tags, vec!["meetings", "team"]);
        assert_eq!(instance.project_id, Some("proj-abc".to_string()));
        assert_eq!(instance.due_date, Some(past));
    }

    #[tokio::test]
    async fn test_start_stop_lifecycle() {
        let (store, _dir) = create_test_store().await;
        let mut spawner =
            RecurringTaskSpawner::new(store, "UTC".to_string(), StdDuration::from_secs(60));

        spawner.start();
        assert!(spawner.task_handle.is_some());

        spawner.stop().await;
        assert!(spawner.task_handle.is_none());
    }
}
