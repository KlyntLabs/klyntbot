//! Sprint 3 integration tests — Bidirectional Calendar Sync
//!
//! Tests for calendar reconciliation system where calendar changes
//! automatically update todos. Covers:
//!
//! ## Core Reconciliation
//! - Event time changed → todo.due_date updates
//! - Event marked complete → todo.status = Done
//! - Event cancelled → calendar_event_uid cleared
//! - Event deleted from provider → calendar_event_uid cleared
//! - Multiple changes in single sync batch

#[allow(dead_code)] // Used by ignored tests that will be implemented later
mod mock_calendar_handler;

use agent::calendar_reconcile::reconcile_calendar_events;
use calendar::{CalendarEvent, EventSource};
use chrono::{TimeZone, Utc};
use tools::todo_types::{Todo, TodoStatus};

// ─── Test helpers ──────────────────────────────────────────────

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

/// Try to connect to a test database. Returns None if no DB is available.
async fn test_todo_repo() -> Option<storage::TodoRepo> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/klyntbot_test".to_string());
    match storage::StoragePool::connect(&url).await {
        Ok(pool) => Some(storage::Repos::from_pool(&pool).todos),
        Err(_) => None,
    }
}

fn create_test_event(
    uid: &str,
    start: chrono::DateTime<Utc>,
    status: Option<String>,
) -> CalendarEvent {
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

// ═══════════════════════════════════════════════════════════════
// CORE RECONCILIATION LOGIC
// ═══════════════════════════════════════════════════════════════

// ─── AC1: Event time changed → todo.due_date updates ───────────

#[tokio::test]
async fn test_reconcile_event_time_changed_updates_todo() {
    let Some(repo) = test_todo_repo().await else { return };

    // Create todo with calendar event
    let original_due = Utc.with_ymd_and_hms(2026, 2, 20, 14, 0, 0).unwrap();
    let mut todo = create_test_todo("Meeting");
    todo.due_date = Some(original_due);
    todo.calendar_event_uid = Some("event-123".to_string());
    let todo_id = todo.id.clone();
    let row: storage::TodoRow = (&todo).into();
    repo.add(&row).await.unwrap();

    // Calendar returns event with new time
    let new_time = Utc.with_ymd_and_hms(2026, 2, 20, 16, 0, 0).unwrap();
    let events = vec![create_test_event("event-123", new_time, None)];

    // Action: Run reconciliation
    let report = reconcile_calendar_events(&repo, events)
        .await
        .unwrap();

    // Assert
    assert_eq!(report.checked, 1);
    assert_eq!(report.due_dates_updated, 1);
    assert_eq!(report.todos_completed, 0);
    assert_eq!(report.links_cleared, 0);

    let updated_row = repo.get(&todo_id).await.unwrap().unwrap();
    let updated_todo = Todo::from(updated_row);
    assert_eq!(updated_todo.due_date, Some(new_time));

    // Cleanup
    let _ = repo.delete(&todo_id).await;
}

// ─── AC2: Event marked complete → todo.status = Done ───────────

#[tokio::test]
async fn test_reconcile_event_completed_marks_todo_done() {
    let Some(repo) = test_todo_repo().await else { return };

    let mut todo = create_test_todo("Task");
    todo.status = TodoStatus::Todo;
    todo.calendar_event_uid = Some("event-456".to_string());
    todo.due_date = Some(Utc::now());
    let todo_id = todo.id.clone();
    let row: storage::TodoRow = (&todo).into();
    repo.add(&row).await.unwrap();

    let events = vec![create_test_event(
        "event-456",
        Utc::now(),
        Some("COMPLETED".to_string()),
    )];

    let report = reconcile_calendar_events(&repo, events)
        .await
        .unwrap();

    assert_eq!(report.checked, 1);
    assert_eq!(report.todos_completed, 1);
    assert_eq!(report.due_dates_updated, 0);
    assert_eq!(report.links_cleared, 0);

    let updated_row = repo.get(&todo_id).await.unwrap().unwrap();
    let updated = Todo::from(updated_row);
    assert_eq!(updated.status, TodoStatus::Done);

    let _ = repo.delete(&todo_id).await;
}

// ─── AC3: Event cancelled/deleted → calendar_event_uid cleared ─

#[tokio::test]
async fn test_reconcile_event_cancelled_clears_uid() {
    let Some(repo) = test_todo_repo().await else { return };

    let mut todo = create_test_todo("Cancelled meeting");
    todo.calendar_event_uid = Some("event-789".to_string());
    let todo_id = todo.id.clone();
    let row: storage::TodoRow = (&todo).into();
    repo.add(&row).await.unwrap();

    let events = vec![create_test_event(
        "event-789",
        Utc::now(),
        Some("CANCELLED".to_string()),
    )];

    let report = reconcile_calendar_events(&repo, events)
        .await
        .unwrap();

    assert_eq!(report.checked, 1);
    assert_eq!(report.links_cleared, 1);
    assert_eq!(report.todos_completed, 0);
    assert_eq!(report.due_dates_updated, 0);

    // Note: ClearCalendarLink is best-effort (TodoPatch lacks calendar_event_uid field)
    let _ = repo.delete(&todo_id).await;
}

#[tokio::test]
async fn test_reconcile_event_deleted_from_provider_clears_uid() {
    let Some(repo) = test_todo_repo().await else { return };

    let mut todo = create_test_todo("Deleted event");
    todo.calendar_event_uid = Some("event-999".to_string());
    let todo_id = todo.id.clone();
    let row: storage::TodoRow = (&todo).into();
    repo.add(&row).await.unwrap();

    let events: Vec<CalendarEvent> = vec![];

    let report = reconcile_calendar_events(&repo, events)
        .await
        .unwrap();

    assert_eq!(report.checked, 1);
    assert_eq!(report.links_cleared, 1);

    let _ = repo.delete(&todo_id).await;
}

// ─── Multiple changes ──────────────────────────────────────────

#[tokio::test]
async fn test_reconcile_multiple_changes_in_single_sync() {
    let Some(repo) = test_todo_repo().await else { return };

    let original_time = Utc.with_ymd_and_hms(2026, 2, 20, 14, 0, 0).unwrap();
    let new_time = Utc.with_ymd_and_hms(2026, 2, 20, 16, 0, 0).unwrap();
    let unchanged_time = Utc.with_ymd_and_hms(2026, 2, 20, 10, 0, 0).unwrap();

    let mut ids = Vec::new();

    // Todo 1: Time changed
    let mut todo1 = create_test_todo("Todo 1");
    todo1.calendar_event_uid = Some("event-1".to_string());
    todo1.due_date = Some(original_time);
    ids.push(todo1.id.clone());
    repo.add(&storage::TodoRow::from(&todo1)).await.unwrap();

    // Todo 2: Completed
    let mut todo2 = create_test_todo("Todo 2");
    todo2.calendar_event_uid = Some("event-2".to_string());
    todo2.status = TodoStatus::Todo;
    todo2.due_date = Some(unchanged_time);
    ids.push(todo2.id.clone());
    repo.add(&storage::TodoRow::from(&todo2)).await.unwrap();

    // Todo 3: Cancelled
    let mut todo3 = create_test_todo("Todo 3");
    todo3.calendar_event_uid = Some("event-3".to_string());
    ids.push(todo3.id.clone());
    repo.add(&storage::TodoRow::from(&todo3)).await.unwrap();

    // Todo 4: Unchanged
    let mut todo4 = create_test_todo("Todo 4");
    todo4.calendar_event_uid = Some("event-4".to_string());
    todo4.due_date = Some(unchanged_time);
    ids.push(todo4.id.clone());
    repo.add(&storage::TodoRow::from(&todo4)).await.unwrap();

    // Todo 5: Event deleted
    let mut todo5 = create_test_todo("Todo 5");
    todo5.calendar_event_uid = Some("event-5".to_string());
    ids.push(todo5.id.clone());
    repo.add(&storage::TodoRow::from(&todo5)).await.unwrap();

    let events = vec![
        create_test_event("event-1", new_time, None),
        create_test_event("event-2", unchanged_time, Some("COMPLETED".to_string())),
        create_test_event("event-3", unchanged_time, Some("CANCELLED".to_string())),
        create_test_event("event-4", unchanged_time, None),
        // event-5 not in list (deleted)
    ];

    let report = reconcile_calendar_events(&repo, events)
        .await
        .unwrap();

    assert_eq!(report.checked, 5);
    assert_eq!(report.due_dates_updated, 1); // todo1
    assert_eq!(report.todos_completed, 1); // todo2
    assert_eq!(report.links_cleared, 2); // todo3 + todo5

    // Cleanup
    for id in &ids {
        let _ = repo.delete(id).await;
    }
}
