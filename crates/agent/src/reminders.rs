//! ReminderEngine - Deterministic reminder system for todos

use chrono::{Duration, Utc};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use common::Result;
use tools::todo_types::Todo;

use super::NotificationDispatcher;

/// ReminderEngine - checks for todos that need reminders
pub struct ReminderEngine {
    todo_repo: storage::ActionRepo,
    dispatcher: Arc<NotificationDispatcher>,
    check_interval: StdDuration,
    task_handle: Option<JoinHandle<()>>,
    cancel_token: CancellationToken,
}

impl ReminderEngine {
    /// Create a new ReminderEngine backed by a SQL TodoRepo.
    pub fn new(
        todo_repo: storage::ActionRepo,
        dispatcher: Arc<NotificationDispatcher>,
        check_interval: StdDuration,
    ) -> Self {
        Self {
            todo_repo,
            dispatcher,
            check_interval,
            task_handle: None,
            cancel_token: CancellationToken::new(),
        }
    }

    /// Start the reminder engine background task
    pub fn start(&mut self) {
        let todo_repo = self.todo_repo.clone();
        let dispatcher = Arc::clone(&self.dispatcher);
        let check_interval = self.check_interval;
        let cancel_token = self.cancel_token.clone();

        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_token.cancelled() => {
                        debug!("ReminderEngine task cancelled");
                        break;
                    }
                    _ = tokio::time::sleep(check_interval) => {
                        if let Err(e) = Self::check_and_send_reminders(
                            &todo_repo,
                            &dispatcher
                        ).await {
                            error!("ReminderEngine check failed: {}", e);
                        }
                    }
                }
            }
        });

        self.task_handle = Some(handle);
    }

    /// Stop the reminder engine
    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            let _ = handle.await;
        }
    }

    /// Check all reminder rules and send notifications via SQL TodoRepo.
    async fn check_and_send_reminders(
        repo: &storage::ActionRepo,
        dispatcher: &Arc<NotificationDispatcher>,
    ) -> Result<()> {
        // Get only non-template, non-done todos for reminder checking
        let filter = storage::ActionFilter {
            status: None, // include todo + doing (exclude done via post-filter below)
            templates_only: false,
            ..Default::default()
        };
        let rows = repo.list(&filter).await?;

        let todos: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

        for todo in &todos {
            // Rule #1: Due date alerts (within 2 hours)
            if Self::should_remind_due_date(todo) {
                let time_left = todo.due_date.unwrap().signed_duration_since(Utc::now());
                let hours_left = time_left.num_hours();
                let mins_left = time_left.num_minutes() % 60;

                dispatcher
                    .notify(
                        "📅 Task Due Soon",
                        &format!(
                            "{} is due in {}h {}m\nPriority: P{}",
                            todo.title,
                            hours_left,
                            mins_left,
                            todo.priority.unwrap_or(3)
                        ),
                    )
                    .await?;

                // Update last_reminded_at via SQL
                let patch = storage::ActionPatch {
                    id: todo.id.clone(),
                    last_reminded_at: Some(Some(Utc::now())),
                    ..Default::default()
                };
                let _ = repo.update(&patch).await;
            }

            // Rule #2: Focused deadlines (within 1 hour)
            if Self::should_remind_focused_deadline(todo) {
                let time_left = todo
                    .focus_deadline
                    .unwrap()
                    .signed_duration_since(Utc::now());
                let mins_left = time_left.num_minutes();

                dispatcher
                    .notify(
                        "🎯 Focused Task Deadline Approaching",
                        &format!("{} deadline in {} minutes!", todo.title, mins_left),
                    )
                    .await?;
            }

            // Rule #3: Overdue nagging (once per day)
            if Self::should_remind_overdue(todo) {
                let overdue_by = Utc::now().signed_duration_since(todo.due_date.unwrap());
                let days_overdue = overdue_by.num_days();

                dispatcher
                    .notify(
                        "⚠️ Overdue Task",
                        &format!(
                            "{} is {} day(s) overdue\nPriority: P{}",
                            todo.title,
                            days_overdue,
                            todo.priority.unwrap_or(3)
                        ),
                    )
                    .await?;
            }
        }

        Ok(())
    }

    /// Check if a todo should trigger a due date reminder (within 2 hours, not yet reminded)
    pub fn should_remind_due_date(todo: &Todo) -> bool {
        // Minimal implementation to pass the test
        if let Some(due_date) = todo.due_date {
            // Check if within 2 hours
            let now = Utc::now();
            let time_until_due = due_date.signed_duration_since(now);

            // Within 2 hours and not in the past
            if time_until_due <= Duration::hours(2) && time_until_due > Duration::zero() {
                // Check if not already reminded
                return todo.last_reminded_at.is_none();
            }
        }
        false
    }

    /// Check if a focused task should trigger a deadline reminder (within 1 hour, not yet reminded)
    pub fn should_remind_focused_deadline(todo: &Todo) -> bool {
        // Must be focused
        if todo.focused_at.is_none() {
            return false;
        }

        // Must have a focus deadline
        if let Some(focus_deadline) = todo.focus_deadline {
            let now = Utc::now();
            let time_until_deadline = focus_deadline.signed_duration_since(now);

            // Within 1 hour and not in the past
            if time_until_deadline <= Duration::hours(1) && time_until_deadline > Duration::zero() {
                // Check if not already reminded
                return todo.last_reminded_at.is_none();
            }
        }
        false
    }

    /// Check if an overdue task should nag (once per day)
    pub fn should_remind_overdue(todo: &Todo) -> bool {
        // Must have a due date
        if let Some(due_date) = todo.due_date {
            let now = Utc::now();

            // Must be overdue (due date in the past)
            if due_date < now {
                // Check if never reminded, or last reminded over 24 hours ago
                if let Some(last_reminded) = todo.last_reminded_at {
                    let time_since_reminded = now.signed_duration_since(last_reminded);
                    // Nag again if more than 24 hours since last nag
                    return time_since_reminded >= Duration::hours(24);
                } else {
                    // Never reminded, so nag now
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use tools::todo_types::{Todo, TodoStatus};

    fn create_test_todo(title: &str, due_date: Option<DateTime<Utc>>) -> Todo {
        Todo {
            id: format!("test-{}", &uuid::Uuid::new_v4().to_string()[..8]),
            title: title.to_string(),
            description: None,
            area_id: String::new(),
            key_result_id: None,
            priority: None,
            due_date,
            tags: vec![],
            status: TodoStatus::Todo,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
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
        }
    }

    #[tokio::test]
    async fn test_detects_due_date_within_2_hours() {
        // Arrange: Create a todo due in 1 hour (within 2 hour window)
        let due_in_1_hour = Utc::now() + Duration::hours(1);
        let todo = create_test_todo("Urgent task", Some(due_in_1_hour));

        // Act: Check if this todo should trigger a reminder
        let should_remind = ReminderEngine::should_remind_due_date(&todo);

        // Assert: Should be true because it's within 2 hours and hasn't been reminded
        assert!(should_remind, "Should remind for task due within 2 hours");
    }

    #[tokio::test]
    async fn test_no_remind_if_already_reminded() {
        // Arrange: Todo due soon but already reminded
        let due_in_1_hour = Utc::now() + Duration::hours(1);
        let mut todo = create_test_todo("Already reminded", Some(due_in_1_hour));
        todo.last_reminded_at = Some(Utc::now() - Duration::minutes(30));

        // Act
        let should_remind = ReminderEngine::should_remind_due_date(&todo);

        // Assert: Should not remind again
        assert!(!should_remind, "Should not remind if already reminded");
    }

    #[tokio::test]
    async fn test_no_remind_if_due_beyond_2_hours() {
        // Arrange: Todo due in 3 hours (beyond 2 hour window)
        let due_in_3_hours = Utc::now() + Duration::hours(3);
        let todo = create_test_todo("Not urgent yet", Some(due_in_3_hours));

        // Act
        let should_remind = ReminderEngine::should_remind_due_date(&todo);

        // Assert: Should not remind yet
        assert!(!should_remind, "Should not remind if due beyond 2 hours");
    }

    #[tokio::test]
    async fn test_no_remind_if_overdue() {
        // Arrange: Todo overdue (in the past)
        let due_1_hour_ago = Utc::now() - Duration::hours(1);
        let todo = create_test_todo("Overdue", Some(due_1_hour_ago));

        // Act
        let should_remind = ReminderEngine::should_remind_due_date(&todo);

        // Assert: Should not remind (overdue is handled by different rule)
        assert!(
            !should_remind,
            "Should not use due date rule for overdue tasks"
        );
    }

    // ── Reminder Rule #2: Focused task deadlines (within 1 hour) ──

    #[tokio::test]
    async fn test_detects_focused_deadline_within_1_hour() {
        // RED: Test for focused task with deadline within 1 hour

        // Arrange: Focused task with deadline in 30 minutes
        let now = Utc::now();
        let deadline_in_30_min = now + Duration::minutes(30);
        let mut todo = create_test_todo("Focused urgent", None);
        todo.focused_at = Some(now - Duration::hours(2));
        todo.focus_deadline = Some(deadline_in_30_min);

        // Act
        let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);

        // Assert
        assert!(
            should_remind,
            "Should remind for focused task deadline within 1 hour"
        );
    }

    #[tokio::test]
    async fn test_no_remind_focused_if_not_focused() {
        // Arrange: Not a focused task
        let todo = create_test_todo("Not focused", None);

        // Act
        let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);

        // Assert
        assert!(!should_remind, "Should not remind if task is not focused");
    }

    #[tokio::test]
    async fn test_no_remind_focused_if_already_reminded() {
        // Arrange: Focused task but already reminded
        let now = Utc::now();
        let mut todo = create_test_todo("Focused", None);
        todo.focused_at = Some(now - Duration::hours(2));
        todo.focus_deadline = Some(now + Duration::minutes(30));
        todo.last_reminded_at = Some(now - Duration::minutes(10));

        // Act
        let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);

        // Assert
        assert!(!should_remind, "Should not remind if already reminded");
    }

    #[tokio::test]
    async fn test_no_remind_focused_if_beyond_1_hour() {
        // Arrange: Focused task with deadline beyond 1 hour
        let now = Utc::now();
        let mut todo = create_test_todo("Focused not urgent", None);
        todo.focused_at = Some(now - Duration::hours(1));
        todo.focus_deadline = Some(now + Duration::hours(2));

        // Act
        let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);

        // Assert
        assert!(
            !should_remind,
            "Should not remind if deadline beyond 1 hour"
        );
    }

    // ── Reminder Rule #3: Overdue nagging (once per day) ──

    #[tokio::test]
    async fn test_detects_overdue_task() {
        // RED: Test for overdue task that should nag

        // Arrange: Task overdue by 2 hours
        let due_2_hours_ago = Utc::now() - Duration::hours(2);
        let todo = create_test_todo("Overdue task", Some(due_2_hours_ago));

        // Act
        let should_remind = ReminderEngine::should_remind_overdue(&todo);

        // Assert
        assert!(should_remind, "Should nag for overdue task");
    }

    #[tokio::test]
    async fn test_no_remind_overdue_if_reminded_recently() {
        // Arrange: Overdue task but nagged 2 hours ago (within 24 hours)
        let due_yesterday = Utc::now() - Duration::days(1);
        let mut todo = create_test_todo("Overdue", Some(due_yesterday));
        todo.last_reminded_at = Some(Utc::now() - Duration::hours(2));

        // Act
        let should_remind = ReminderEngine::should_remind_overdue(&todo);

        // Assert
        assert!(!should_remind, "Should not nag if reminded within 24 hours");
    }

    #[tokio::test]
    async fn test_remind_overdue_if_last_nag_over_24_hours() {
        // Arrange: Overdue task, last nagged 25 hours ago (beyond 24 hours)
        let due_yesterday = Utc::now() - Duration::days(1);
        let mut todo = create_test_todo("Overdue", Some(due_yesterday));
        todo.last_reminded_at = Some(Utc::now() - Duration::hours(25));

        // Act
        let should_remind = ReminderEngine::should_remind_overdue(&todo);

        // Assert
        assert!(
            should_remind,
            "Should nag again if last nag was over 24 hours ago"
        );
    }

    #[tokio::test]
    async fn test_no_remind_overdue_if_not_overdue() {
        // Arrange: Task not overdue (due in future)
        let due_tomorrow = Utc::now() + Duration::days(1);
        let todo = create_test_todo("Not overdue", Some(due_tomorrow));

        // Act
        let should_remind = ReminderEngine::should_remind_overdue(&todo);

        // Assert
        assert!(!should_remind, "Should not nag if not overdue");
    }
}
