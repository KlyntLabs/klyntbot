//! RecurringTaskSpawner - Background job for spawning recurring task instances.
//!
//! Follows the ReminderEngine pattern (ADR-013): periodic check loop with
//! CancellationToken for graceful shutdown. Reads template tasks, checks if
//! instances are due, clones them as concrete todos, and advances the template's
//! next_instance_date.

use std::time::Duration as StdDuration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

use tools::rrule_utils;
use tools::todo_types::Todo;

/// Background spawner for recurring task instances.
pub struct RecurringTaskSpawner {
    todo_repo: storage::TodoRepo,
    timezone: String,
    check_interval: StdDuration,
    task_handle: Option<JoinHandle<()>>,
    cancel_token: CancellationToken,
}

impl RecurringTaskSpawner {
    /// Create a new RecurringTaskSpawner backed by a SQL TodoRepo.
    pub fn new(
        todo_repo: storage::TodoRepo,
        timezone: String,
        check_interval: StdDuration,
    ) -> Self {
        Self {
            todo_repo,
            timezone,
            check_interval,
            task_handle: None,
            cancel_token: CancellationToken::new(),
        }
    }

    /// Start the background spawner task.
    pub fn start(&mut self) {
        let todo_repo = self.todo_repo.clone();
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
                        if let Err(e) = Self::check_and_spawn(&todo_repo, &timezone).await {
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

    /// Check all templates and spawn instances that are due via SQL.
    async fn check_and_spawn(repo: &storage::TodoRepo, _timezone: &str) -> common::Result<()> {
        let template_rows = repo
            .list_templates()
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        let now = chrono::Utc::now();

        for tpl_row in &template_rows {
            // Skip templates without a recurrence rule
            let rule = match &tpl_row.recurrence_rule {
                Some(r) => r.clone(),
                None => continue,
            };

            // Check if an instance is due
            if !rrule_utils::should_spawn_instance(tpl_row.next_instance_date, now) {
                continue;
            }

            // Create instance from template (domain type, then convert to row)
            let mut instance = Todo::default_instance();
            instance.title = tpl_row.title.clone();
            instance.description = tpl_row.description.clone();
            instance.priority = tpl_row.priority.map(|p| p as u8);
            instance.tags = tpl_row.tags.clone();
            instance.project_id = tpl_row.project_id.clone();
            instance.recurrence_parent_id = Some(tpl_row.id.clone());
            instance.due_date = tpl_row.next_instance_date;

            let instance_id = instance.id.clone();
            let instance_row: storage::TodoRow = (&instance).into();
            repo.add(&instance_row)
                .await
                .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;

            // Advance next_instance_date via update patch
            let next = rrule_utils::next_occurrence(&rule, now)?;
            let patch = storage::TodoPatch {
                id: tpl_row.id.clone(),
                next_instance_date: Some(next),
                ..Default::default()
            };
            if let Err(e) = repo.update(&patch).await {
                tracing::warn!(
                    "Failed to advance next_instance_date for template [{}]: {}",
                    tpl_row.id,
                    e
                );
            }

            info!(
                "Spawned recurring instance [{}] from template [{}] '{}'",
                instance_id, tpl_row.id, tpl_row.title
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use tools::todo_types::TodoStatus;

    /// Try to connect to a test database. Returns None if no DB is available.
    async fn test_todo_repo() -> Option<storage::TodoRepo> {
        let url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/klyntbot_test".to_string());
        match storage::StoragePool::connect(&url).await {
            Ok(pool) => Some(storage::Repos::from_pool(&pool).todos),
            Err(_) => None,
        }
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
        let Some(repo) = test_todo_repo().await else {
            return;
        };
        let past = Utc::now() - Duration::hours(1);
        let template = create_template("Daily standup", "FREQ=DAILY", past);
        let template_id = template.id.clone();

        let tpl_row: storage::TodoRow = (&template).into();
        repo.add(&tpl_row).await.unwrap();

        RecurringTaskSpawner::check_and_spawn(&repo, "UTC")
            .await
            .unwrap();

        let filter = storage::TodoFilter {
            templates_only: false,
            ..Default::default()
        };
        let all: Vec<Todo> = repo
            .list(&filter)
            .await
            .unwrap()
            .into_iter()
            .map(Todo::from)
            .collect();
        let instance = all
            .iter()
            .find(|t| !t.is_template && t.title == "Daily standup");
        assert!(instance.is_some(), "Should have spawned an instance");
        let instance = instance.unwrap();
        assert_eq!(instance.recurrence_parent_id, Some(template_id));
        assert_eq!(instance.status, TodoStatus::Todo);

        // Cleanup
        let _ = repo.delete(&instance.id).await;
        let _ = repo.delete(&tpl_row.id).await;
    }

    #[tokio::test]
    async fn test_does_not_spawn_future_instance() {
        let Some(repo) = test_todo_repo().await else {
            return;
        };
        let future = Utc::now() + Duration::hours(24);
        let template = create_template("Weekly review", "FREQ=WEEKLY", future);
        let template_id = template.id.clone();

        let tpl_row: storage::TodoRow = (&template).into();
        repo.add(&tpl_row).await.unwrap();

        RecurringTaskSpawner::check_and_spawn(&repo, "UTC")
            .await
            .unwrap();

        let filter = storage::TodoFilter::default();
        let all: Vec<Todo> = repo
            .list(&filter)
            .await
            .unwrap()
            .into_iter()
            .map(Todo::from)
            .collect();
        let instance = all
            .iter()
            .find(|t| t.recurrence_parent_id.as_deref() == Some(&template_id));
        assert!(instance.is_none(), "Should not spawn future instance");

        // Cleanup
        let _ = repo.delete(&template_id).await;
    }

    #[tokio::test]
    async fn test_skips_template_without_rule() {
        let Some(repo) = test_todo_repo().await else {
            return;
        };

        let mut todo = Todo::default_instance();
        todo.title = "No rule".to_string();
        todo.is_template = true;
        todo.next_instance_date = Some(Utc::now() - Duration::hours(1));
        let todo_id = todo.id.clone();

        let row: storage::TodoRow = (&todo).into();
        repo.add(&row).await.unwrap();

        RecurringTaskSpawner::check_and_spawn(&repo, "UTC")
            .await
            .unwrap();

        let filter = storage::TodoFilter::default();
        let all: Vec<Todo> = repo
            .list(&filter)
            .await
            .unwrap()
            .into_iter()
            .map(Todo::from)
            .collect();
        let instance = all
            .iter()
            .find(|t| t.recurrence_parent_id.as_deref() == Some(&todo_id));
        assert!(instance.is_none(), "Should not spawn without a rule");

        // Cleanup
        let _ = repo.delete(&todo_id).await;
    }

    #[tokio::test]
    async fn test_start_stop_lifecycle() {
        let Some(repo) = test_todo_repo().await else {
            return;
        };
        let mut spawner =
            RecurringTaskSpawner::new(repo, "UTC".to_string(), StdDuration::from_secs(60));

        spawner.start();
        assert!(spawner.task_handle.is_some());

        spawner.stop().await;
        assert!(spawner.task_handle.is_none());
    }
}
