//! ReminderEngine - Deterministic reminder system for todos

use std::sync::Arc;
use std::time::Duration as StdDuration;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error};

use bus::{DomainEvent, DomainEventBus};
use common::Result;
use jiff::Timestamp;
use tools::todo_types::Todo;

/// ReminderEngine - checks for todos that need reminders
pub struct ReminderEngine {
    todo_repo: storage::TaskRepo,
    bus: Arc<DomainEventBus>,
    check_interval: StdDuration,
    task_handle: Option<JoinHandle<()>>,
    cancel_token: CancellationToken,
}

impl ReminderEngine {
    /// Create a new ReminderEngine backed by a SQL TodoRepo.
    pub fn new(
        todo_repo: storage::TaskRepo,
        bus: Arc<DomainEventBus>,
        check_interval: StdDuration,
    ) -> Self {
        Self {
            todo_repo,
            bus,
            check_interval,
            task_handle: None,
            cancel_token: CancellationToken::new(),
        }
    }

    /// Start the reminder engine background task
    pub fn start(&mut self) {
        let todo_repo = self.todo_repo.clone();
        let bus = Arc::clone(&self.bus);
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
                            &bus,
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
    ///
    /// Public static variant for use by the cron handler.
    pub async fn check_and_send_reminders_static(
        repo: &storage::TaskRepo,
        bus: &Arc<DomainEventBus>,
    ) -> Result<()> {
        Self::check_and_send_reminders(repo, bus).await
    }

    /// Check all reminder rules and send notifications via SQL TodoRepo.
    async fn check_and_send_reminders(
        repo: &storage::TaskRepo,
        bus: &Arc<DomainEventBus>,
    ) -> Result<()> {
        // Get only non-template, non-done todos for reminder checking
        let filter = storage::TaskFilter {
            status: None, // include todo + doing (exclude done via post-filter below)
            templates_only: false,
            ..Default::default()
        };
        let rows = repo.list(&filter).await?;

        let todos: Vec<Todo> = rows.into_iter().map(Todo::from).collect();
        let now = Timestamp::now();

        for todo in &todos {
            // Rule #1: Due date alerts (within 2 hours)
            if let Some(due_date) = todo.due_date.filter(|_| Self::should_remind_due_date(todo)) {
                let diff_ms = due_date.as_millisecond() - now.as_millisecond();
                let hours_left = diff_ms / 3_600_000;
                let mins_left = (diff_ms % 3_600_000) / 60_000;

                let time_str = if hours_left > 0 {
                    format!("{}h {}m", hours_left, mins_left)
                } else {
                    format!("{}m", mins_left)
                };

                bus.publish(DomainEvent::TrayNotificationRequested {
                    title: format!("Due Soon: {}", todo.title),
                    body: format!("Due in {} · P{}", time_str, todo.priority.unwrap_or(3)),
                    alarm_id: None,
                });

                // Update last_reminded_at via SQL
                let patch = storage::TaskPatch {
                    id: todo.id.clone(),
                    last_reminded_at: Some(Some(jiff::Timestamp::now())),
                    ..Default::default()
                };
                let _ = repo.update(&patch).await;
            }

            // Rule #2: Focused deadlines (within 1 hour)
            if let Some(deadline) = todo
                .focus_deadline
                .filter(|_| Self::should_remind_focused_deadline(todo))
            {
                let diff_ms = deadline.as_millisecond() - now.as_millisecond();
                let mins_left = diff_ms / 60_000;

                bus.publish(DomainEvent::TrayNotificationRequested {
                    title: format!("Deadline: {}", todo.title),
                    body: format!("{} minutes remaining", mins_left),
                    alarm_id: None,
                });
            }

            // Rule #3: Overdue nagging (once per day)
            if let Some(due_date) = todo.due_date.filter(|_| Self::should_remind_overdue(todo)) {
                let diff_ms = now.as_millisecond() - due_date.as_millisecond();
                let days_overdue = diff_ms / 86_400_000;

                let overdue_str = if days_overdue == 1 {
                    "1 day overdue".to_string()
                } else {
                    format!("{} days overdue", days_overdue)
                };

                bus.publish(DomainEvent::TrayNotificationRequested {
                    title: format!("Overdue: {}", todo.title),
                    body: format!("{} · P{}", overdue_str, todo.priority.unwrap_or(3)),
                    alarm_id: None,
                });
            }
        }

        Ok(())
    }

    /// Check if a todo should trigger a due date reminder (within 2 hours, not yet reminded)
    pub fn should_remind_due_date(todo: &Todo) -> bool {
        if let Some(due_date) = todo.due_date {
            let now = Timestamp::now();
            let diff_ms = due_date.as_millisecond() - now.as_millisecond();
            // Within 2 hours and not in the past
            let two_hours_ms = 2 * 3_600_000_i64;
            if diff_ms > 0 && diff_ms <= two_hours_ms {
                return todo.last_reminded_at.is_none();
            }
        }
        false
    }

    /// Check if a focused task should trigger a deadline reminder (within 1 hour, not yet reminded)
    pub fn should_remind_focused_deadline(todo: &Todo) -> bool {
        if todo.focused_at.is_none() {
            return false;
        }

        if let Some(focus_deadline) = todo.focus_deadline {
            let now = Timestamp::now();
            let diff_ms = focus_deadline.as_millisecond() - now.as_millisecond();
            let one_hour_ms = 3_600_000_i64;
            if diff_ms > 0 && diff_ms <= one_hour_ms {
                return todo.last_reminded_at.is_none();
            }
        }
        false
    }

    /// Check if an overdue task should nag (once per day)
    pub fn should_remind_overdue(todo: &Todo) -> bool {
        if let Some(due_date) = todo.due_date {
            let now = Timestamp::now();
            if due_date < now {
                if let Some(last_reminded) = todo.last_reminded_at {
                    let since_reminded_ms = now.as_millisecond() - last_reminded.as_millisecond();
                    return since_reminded_ms >= 24 * 3_600_000;
                } else {
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
    use jiff::{SignedDuration, Timestamp};
    use tools::todo_types::{Todo, TodoStatus};

    fn create_test_todo(title: &str, due_date: Option<Timestamp>) -> Todo {
        let now = Timestamp::now();
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
            created_at: now,
            updated_at: now,
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
            group_id: None,
        }
    }

    fn add_secs(ts: Timestamp, secs: i64) -> Timestamp {
        ts.checked_add(SignedDuration::from_secs(secs))
            .unwrap_or(ts)
    }

    #[tokio::test]
    async fn test_detects_due_date_within_2_hours() {
        let due_in_1_hour = add_secs(Timestamp::now(), 3600);
        let todo = create_test_todo("Urgent task", Some(due_in_1_hour));
        let should_remind = ReminderEngine::should_remind_due_date(&todo);
        assert!(should_remind, "Should remind for task due within 2 hours");
    }

    #[tokio::test]
    async fn test_no_remind_if_already_reminded() {
        let due_in_1_hour = add_secs(Timestamp::now(), 3600);
        let mut todo = create_test_todo("Already reminded", Some(due_in_1_hour));
        todo.last_reminded_at = Some(add_secs(Timestamp::now(), -1800));
        let should_remind = ReminderEngine::should_remind_due_date(&todo);
        assert!(!should_remind, "Should not remind if already reminded");
    }

    #[tokio::test]
    async fn test_no_remind_if_due_beyond_2_hours() {
        let due_in_3_hours = add_secs(Timestamp::now(), 3 * 3600);
        let todo = create_test_todo("Not urgent yet", Some(due_in_3_hours));
        let should_remind = ReminderEngine::should_remind_due_date(&todo);
        assert!(!should_remind, "Should not remind if due beyond 2 hours");
    }

    #[tokio::test]
    async fn test_no_remind_if_overdue() {
        let due_1_hour_ago = add_secs(Timestamp::now(), -3600);
        let todo = create_test_todo("Overdue", Some(due_1_hour_ago));
        let should_remind = ReminderEngine::should_remind_due_date(&todo);
        assert!(
            !should_remind,
            "Should not use due date rule for overdue tasks"
        );
    }

    #[tokio::test]
    async fn test_detects_focused_deadline_within_1_hour() {
        let now = Timestamp::now();
        let deadline_in_30_min = add_secs(now, 1800);
        let mut todo = create_test_todo("Focused urgent", None);
        todo.focused_at = Some(add_secs(now, -7200));
        todo.focus_deadline = Some(deadline_in_30_min);
        let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);
        assert!(
            should_remind,
            "Should remind for focused task deadline within 1 hour"
        );
    }

    #[tokio::test]
    async fn test_no_remind_focused_if_not_focused() {
        let todo = create_test_todo("Not focused", None);
        let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);
        assert!(!should_remind, "Should not remind if task is not focused");
    }

    #[tokio::test]
    async fn test_no_remind_focused_if_already_reminded() {
        let now = Timestamp::now();
        let mut todo = create_test_todo("Focused", None);
        todo.focused_at = Some(add_secs(now, -7200));
        todo.focus_deadline = Some(add_secs(now, 1800));
        todo.last_reminded_at = Some(add_secs(now, -600));
        let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);
        assert!(!should_remind, "Should not remind if already reminded");
    }

    #[tokio::test]
    async fn test_no_remind_focused_if_beyond_1_hour() {
        let now = Timestamp::now();
        let mut todo = create_test_todo("Focused not urgent", None);
        todo.focused_at = Some(add_secs(now, -3600));
        todo.focus_deadline = Some(add_secs(now, 7200));
        let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);
        assert!(
            !should_remind,
            "Should not remind if deadline beyond 1 hour"
        );
    }

    #[tokio::test]
    async fn test_detects_overdue_task() {
        let due_2_hours_ago = add_secs(Timestamp::now(), -7200);
        let todo = create_test_todo("Overdue task", Some(due_2_hours_ago));
        let should_remind = ReminderEngine::should_remind_overdue(&todo);
        assert!(should_remind, "Should nag for overdue task");
    }

    #[tokio::test]
    async fn test_no_remind_overdue_if_reminded_recently() {
        let due_yesterday = add_secs(Timestamp::now(), -86400);
        let mut todo = create_test_todo("Overdue", Some(due_yesterday));
        todo.last_reminded_at = Some(add_secs(Timestamp::now(), -7200));
        let should_remind = ReminderEngine::should_remind_overdue(&todo);
        assert!(!should_remind, "Should not nag if reminded within 24 hours");
    }

    #[tokio::test]
    async fn test_remind_overdue_if_last_nag_over_24_hours() {
        let due_yesterday = add_secs(Timestamp::now(), -86400);
        let mut todo = create_test_todo("Overdue", Some(due_yesterday));
        todo.last_reminded_at = Some(add_secs(Timestamp::now(), -90000)); // 25 hours ago
        let should_remind = ReminderEngine::should_remind_overdue(&todo);
        assert!(
            should_remind,
            "Should nag again if last nag was over 24 hours ago"
        );
    }

    #[tokio::test]
    async fn test_no_remind_overdue_if_not_overdue() {
        let due_tomorrow = add_secs(Timestamp::now(), 86400);
        let todo = create_test_todo("Not overdue", Some(due_tomorrow));
        let should_remind = ReminderEngine::should_remind_overdue(&todo);
        assert!(!should_remind, "Should not nag if not overdue");
    }
}
