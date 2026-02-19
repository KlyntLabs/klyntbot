//! Calendar reconciliation engine
//!
//! Reconciles calendar events with linked todos, detecting changes in:
//! - Due dates (event.start vs todo.due_date)
//! - Completion status (event.status == "COMPLETED")
//! - Cancellations (event.status == "CANCELLED" or event not found)

use calendar::CalendarEvent;
use chrono::{DateTime, Utc};
use common::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tools::todo_types::{Todo, TodoStatus};

/// Result of reconciling a single calendar event against its linked todo
#[derive(Debug, Clone, PartialEq)]
pub enum ReconcileAction {
    /// Update todo's due_date to match event start time
    UpdateDueDate {
        todo_id: String,
        old_due: Option<DateTime<Utc>>,
        new_due: DateTime<Utc>,
    },
    /// Mark todo as Done because event is COMPLETED
    CompleteTodo { todo_id: String },
    /// Clear calendar link because event was cancelled or deleted
    ClearCalendarLink { todo_id: String, event_uid: String },
    /// No changes needed
    NoChange { todo_id: String },
}

/// Summary of a reconciliation run
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileReport {
    pub due_dates_updated: u32,
    pub todos_completed: u32,
    pub links_cleared: u32,
    pub errors: Vec<String>,
    pub checked: u32,
    pub timestamp: DateTime<Utc>,
}

/// Determine what action to take for a todo given its linked calendar event.
///
/// Pure function with no side effects - just decision logic.
pub fn determine_action(event: &CalendarEvent, todo: &Todo) -> ReconcileAction {
    let todo_id = todo.id.clone();

    // Priority 1: Event cancelled → clear link
    if event.status.as_deref() == Some("CANCELLED") {
        return ReconcileAction::ClearCalendarLink {
            todo_id,
            event_uid: event.uid.clone(),
        };
    }

    // Priority 2: Event completed and todo not done → mark todo done
    if event.status.as_deref() == Some("COMPLETED") && todo.status != TodoStatus::Done {
        return ReconcileAction::CompleteTodo { todo_id };
    }

    // Priority 3: Due date mismatch → update todo
    if let Some(todo_due) = todo.due_date {
        if event.start != todo_due {
            return ReconcileAction::UpdateDueDate {
                todo_id,
                old_due: Some(todo_due),
                new_due: event.start,
            };
        }
    } else {
        // Todo has no due date but event exists → set due date
        return ReconcileAction::UpdateDueDate {
            todo_id,
            old_due: None,
            new_due: event.start,
        };
    }

    // No changes needed
    ReconcileAction::NoChange { todo_id }
}

/// Reconcile all calendar-linked todos against pre-fetched events.
///
/// Takes events directly (already fetched during sync) to avoid duplicate fetching.
/// Returns a report summarizing changes made.
pub async fn reconcile_calendar_events(
    todo_repo: &storage::TodoRepo,
    events: Vec<CalendarEvent>,
) -> Result<ReconcileReport> {
    let mut report = ReconcileReport {
        timestamp: Utc::now(),
        ..Default::default()
    };

    // Build HashMap for O(1) event lookup
    let event_map: HashMap<String, CalendarEvent> =
        events.into_iter().map(|e| (e.uid.clone(), e)).collect();

    // Get all todos with calendar links
    let rows = match todo_repo.list(&storage::TodoFilter::default()).await {
        Ok(rows) => rows,
        Err(e) => {
            report.errors.push(format!("Failed to list todos: {}", e));
            return Ok(report);
        }
    };
    let todos: Vec<Todo> = rows.into_iter().map(Todo::from).collect();

    // Process each linked todo
    for todo in todos {
        let Some(event_uid) = &todo.calendar_event_uid else {
            continue; // Skip todos without calendar links
        };

        report.checked += 1;

        // Determine action based on whether event exists
        let action = if let Some(event) = event_map.get(event_uid) {
            determine_action(event, &todo)
        } else {
            // Event not found → treat as deleted/cancelled
            ReconcileAction::ClearCalendarLink {
                todo_id: todo.id.clone(),
                event_uid: event_uid.clone(),
            }
        };

        // Apply action
        let result = match action {
            ReconcileAction::UpdateDueDate { new_due, .. } => {
                let patch = storage::TodoPatch {
                    id: todo.id.clone(),
                    due_date: Some(Some(new_due)),
                    ..Default::default()
                };
                todo_repo
                    .update(&patch)
                    .await
                    .map(|_| {
                        report.due_dates_updated += 1;
                    })
                    .map_err(common::KlyntbotError::from)
            }
            ReconcileAction::CompleteTodo { .. } => {
                let patch = storage::TodoPatch {
                    id: todo.id.clone(),
                    status: Some("done".to_string()),
                    ..Default::default()
                };
                todo_repo
                    .update(&patch)
                    .await
                    .map(|_| {
                        report.todos_completed += 1;
                    })
                    .map_err(common::KlyntbotError::from)
            }
            ReconcileAction::ClearCalendarLink { .. } => {
                let patch = storage::TodoPatch {
                    id: todo.id.clone(),
                    calendar_event_uid: Some(None),
                    ..Default::default()
                };
                todo_repo
                    .update(&patch)
                    .await
                    .map(|_| {
                        report.links_cleared += 1;
                    })
                    .map_err(common::KlyntbotError::from)
            }
            ReconcileAction::NoChange { .. } => Ok(()),
        };

        if let Err(e) = result {
            report
                .errors
                .push(format!("Failed to update todo {}: {}", todo.id, e));
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use calendar::EventSource;
    use chrono::Utc;

    fn create_test_todo(id: &str, due_date: Option<DateTime<Utc>>) -> Todo {
        Todo {
            id: id.to_string(),
            title: "Test Todo".to_string(),
            description: None,
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
            attachments: vec![],
            time_entries: vec![],
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: Some("event-123".to_string()),
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
        }
    }

    fn create_test_event(uid: &str, start: DateTime<Utc>, status: Option<String>) -> CalendarEvent {
        CalendarEvent {
            uid: uid.to_string(),
            summary: "Test Event".to_string(),
            description: None,
            start,
            end: start + chrono::Duration::hours(1),
            source: EventSource::CalDAV,
            etag: None,
            status,
        }
    }

    #[test]
    fn test_determine_action_cancelled_event() {
        let todo = create_test_todo("todo-1", Some(Utc::now()));
        let event = create_test_event("event-123", Utc::now(), Some("CANCELLED".to_string()));

        let action = determine_action(&event, &todo);

        assert!(matches!(action, ReconcileAction::ClearCalendarLink { .. }));
    }

    #[test]
    fn test_determine_action_completed_event() {
        let todo = create_test_todo("todo-1", Some(Utc::now()));
        let event = create_test_event("event-123", Utc::now(), Some("COMPLETED".to_string()));

        let action = determine_action(&event, &todo);

        assert!(matches!(action, ReconcileAction::CompleteTodo { .. }));
    }

    #[test]
    fn test_determine_action_time_change() {
        let old_time = Utc::now();
        let new_time = old_time + chrono::Duration::hours(2);

        let todo = create_test_todo("todo-1", Some(old_time));
        let event = create_test_event("event-123", new_time, None);

        let action = determine_action(&event, &todo);

        match action {
            ReconcileAction::UpdateDueDate { new_due, .. } => {
                assert_eq!(new_due, new_time);
            }
            _ => panic!("Expected UpdateDueDate action"),
        }
    }

    #[test]
    fn test_determine_action_no_changes() {
        let time = Utc::now();
        let todo = create_test_todo("todo-1", Some(time));
        let event = create_test_event("event-123", time, None);

        let action = determine_action(&event, &todo);

        assert!(matches!(action, ReconcileAction::NoChange { .. }));
    }

    #[test]
    fn test_determine_action_todo_without_due_date() {
        let event_time = Utc::now();
        let todo = create_test_todo("todo-1", None);
        let event = create_test_event("event-123", event_time, None);

        let action = determine_action(&event, &todo);

        match action {
            ReconcileAction::UpdateDueDate {
                old_due, new_due, ..
            } => {
                assert_eq!(old_due, None);
                assert_eq!(new_due, event_time);
            }
            _ => panic!("Expected UpdateDueDate action"),
        }
    }

    #[test]
    fn test_determine_action_completed_event_already_done_todo() {
        let time = Utc::now();
        let mut todo = create_test_todo("todo-1", Some(time));
        todo.status = TodoStatus::Done;
        let event = create_test_event("event-123", time, Some("COMPLETED".to_string()));

        let action = determine_action(&event, &todo);

        // Should not try to complete an already-done todo
        assert!(matches!(action, ReconcileAction::NoChange { .. }));
    }
}
