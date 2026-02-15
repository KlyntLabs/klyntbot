//! Edge case tests for reminder engine and time tracking
//!
//! Tests boundary conditions and edge cases:
//! - Reminder deduplication
//! - Time tracking edge cases
//! - Boundary time calculations
//! - Multiple reminder triggers

use agent::reminders::ReminderEngine;
use chrono::{Duration, Utc};
use tools::todo_types::{TimeEntry, TimeEntrySource, Todo, TodoStatus};

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

// ── Reminder Rule Edge Cases ──────────────────────────────────────

#[tokio::test]
async fn test_due_date_exactly_2_hours() {
    // Boundary: exactly 2 hours should trigger
    let due_in_2h = Utc::now() + Duration::hours(2);
    let todo = Todo {
        due_date: Some(due_in_2h),
        ..create_test_todo("Boundary task")
    };

    let should_remind = ReminderEngine::should_remind_due_date(&todo);
    assert!(should_remind, "Exactly 2 hours should trigger");
}

#[tokio::test]
async fn test_due_date_just_over_2_hours() {
    // Boundary: 2 hours + 1 second should NOT trigger
    let due_in_2h_plus = Utc::now() + Duration::hours(2) + Duration::seconds(1);
    let todo = Todo {
        due_date: Some(due_in_2h_plus),
        ..create_test_todo("Just over boundary")
    };

    let should_remind = ReminderEngine::should_remind_due_date(&todo);
    assert!(!should_remind, "Just over 2 hours should not trigger");
}

#[tokio::test]
async fn test_due_date_exactly_at_now() {
    // Edge case: due exactly now (0 duration remaining)
    let due_now = Utc::now();
    let todo = Todo {
        due_date: Some(due_now),
        ..create_test_todo("Due right now")
    };

    let should_remind = ReminderEngine::should_remind_due_date(&todo);
    assert!(
        !should_remind,
        "Due exactly now (0 duration) should not trigger - past threshold"
    );
}

#[tokio::test]
async fn test_due_date_1_second_in_future() {
    // Edge case: due in 1 second (very short time remaining)
    let due_soon = Utc::now() + Duration::seconds(1);
    let todo = Todo {
        due_date: Some(due_soon),
        ..create_test_todo("Due in 1 second")
    };

    let should_remind = ReminderEngine::should_remind_due_date(&todo);
    assert!(
        should_remind,
        "1 second in future should trigger (within 2h window)"
    );
}

#[tokio::test]
async fn test_focused_deadline_exactly_1_hour() {
    // Boundary: exactly 1 hour should trigger
    let now = Utc::now();
    let todo = Todo {
        focused_at: Some(now - Duration::hours(3)),
        focus_deadline: Some(now + Duration::hours(1)),
        ..create_test_todo("Boundary focus")
    };

    let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);
    assert!(should_remind, "Exactly 1 hour should trigger");
}

#[tokio::test]
async fn test_focused_deadline_just_over_1_hour() {
    // Boundary: 1 hour + 1 second should NOT trigger
    let now = Utc::now();
    let todo = Todo {
        focused_at: Some(now - Duration::hours(2)),
        focus_deadline: Some(now + Duration::hours(1) + Duration::seconds(1)),
        ..create_test_todo("Just over boundary")
    };

    let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);
    assert!(!should_remind, "Just over 1 hour should not trigger");
}

#[tokio::test]
async fn test_overdue_exactly_24_hours_since_last_nag() {
    // Boundary: exactly 24 hours since last nag should trigger
    let now = Utc::now();
    let todo = Todo {
        due_date: Some(now - Duration::days(1)),
        last_reminded_at: Some(now - Duration::hours(24)),
        ..create_test_todo("24h since nag")
    };

    let should_remind = ReminderEngine::should_remind_overdue(&todo);
    assert!(should_remind, "Exactly 24 hours since nag should trigger");
}

#[tokio::test]
async fn test_overdue_just_under_24_hours_since_last_nag() {
    // Boundary: 23h 59m since last nag should NOT trigger
    let now = Utc::now();
    let todo = Todo {
        due_date: Some(now - Duration::days(1)),
        last_reminded_at: Some(now - Duration::hours(23) - Duration::minutes(59)),
        ..create_test_todo("Just under 24h")
    };

    let should_remind = ReminderEngine::should_remind_overdue(&todo);
    assert!(!should_remind, "Just under 24 hours should not trigger");
}

#[tokio::test]
async fn test_calendar_event_exactly_30_minutes() {
    // Boundary: exactly 30 minutes should trigger
    use agent::reminders::CalendarEvent as ReminderCalendarEvent;

    let event = ReminderCalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        start: Utc::now() + Duration::minutes(30),
        last_reminded_at: None,
    };

    let should_remind = ReminderEngine::should_remind_calendar_event(&event);
    assert!(should_remind, "Exactly 30 minutes should trigger");
}

#[tokio::test]
async fn test_calendar_event_just_over_30_minutes() {
    // Boundary: 30 min + 1 sec should NOT trigger
    use agent::reminders::CalendarEvent as ReminderCalendarEvent;

    let event = ReminderCalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        start: Utc::now() + Duration::minutes(30) + Duration::seconds(1),
        last_reminded_at: None,
    };

    let should_remind = ReminderEngine::should_remind_calendar_event(&event);
    assert!(!should_remind, "Just over 30 minutes should not trigger");
}

// ── Time Tracking Edge Cases ──────────────────────────────────────

#[test]
fn test_time_entry_with_zero_duration() {
    // Edge case: time entry that started and ended at same instant
    let now = Utc::now();
    let entry = TimeEntry {
        id: "entry-1".to_string(),
        started_at: now,
        ended_at: Some(now),
        duration_secs: Some(0),
        note: Some("Zero duration entry".to_string()),
        source: TimeEntrySource::Focus,
    };

    assert_eq!(entry.duration_secs, Some(0));
    assert_eq!(entry.started_at, entry.ended_at.unwrap());
}

#[test]
fn test_time_entry_with_negative_duration() {
    // Edge case: time entry with end before start (invalid but structurally possible)
    let now = Utc::now();
    let entry = TimeEntry {
        id: "entry-1".to_string(),
        started_at: now,
        ended_at: Some(now - Duration::hours(1)), // Invalid: end before start
        duration_secs: None,                      // Duration not calculated
        note: None,
        source: TimeEntrySource::Focus,
    };

    assert!(entry.ended_at.unwrap() < entry.started_at);
}

#[test]
fn test_time_entry_running_without_end() {
    // Normal case: time entry still running (no end time)
    let entry = TimeEntry {
        id: "running-1".to_string(),
        started_at: Utc::now() - Duration::hours(2),
        ended_at: None,
        duration_secs: None,
        note: Some("Still working".to_string()),
        source: TimeEntrySource::Focus,
    };

    assert!(entry.ended_at.is_none());
    assert!(entry.duration_secs.is_none());
}

#[test]
fn test_time_entry_with_very_long_duration() {
    // Edge case: time entry spanning multiple days
    let start = Utc::now() - Duration::days(5);
    let end = Utc::now();
    let duration_secs = 5 * 24 * 3600; // 5 days in seconds

    let entry = TimeEntry {
        id: "long-entry".to_string(),
        started_at: start,
        ended_at: Some(end),
        duration_secs: Some(duration_secs),
        note: Some("Multi-day work".to_string()),
        source: TimeEntrySource::Focus,
    };

    assert_eq!(entry.duration_secs, Some(432000)); // 5 days
}

#[test]
fn test_time_entry_with_very_long_note() {
    // Edge case: time entry with very long note
    let long_note = "Note: ".to_string() + &"x".repeat(1000);
    let entry = TimeEntry {
        id: "entry-1".to_string(),
        started_at: Utc::now(),
        ended_at: None,
        duration_secs: None,
        note: Some(long_note.clone()),
        source: TimeEntrySource::Focus,
    };

    assert_eq!(entry.note.as_ref().unwrap().len(), 1006);
}

#[test]
fn test_time_entry_with_empty_note() {
    // Edge case: time entry with empty string note
    let entry = TimeEntry {
        id: "entry-1".to_string(),
        started_at: Utc::now(),
        ended_at: None,
        duration_secs: None,
        note: Some(String::new()),
        source: TimeEntrySource::Focus,
    };

    assert_eq!(entry.note, Some(String::new()));
}

#[test]
fn test_time_entry_id_uniqueness() {
    // Verify different time entries have unique IDs
    let entry1 = TimeEntry {
        id: "entry-1".to_string(),
        started_at: Utc::now(),
        ended_at: None,
        duration_secs: None,
        note: None,
        source: TimeEntrySource::Focus,
    };

    let entry2 = TimeEntry {
        id: "entry-2".to_string(),
        started_at: Utc::now(),
        ended_at: None,
        duration_secs: None,
        note: None,
        source: TimeEntrySource::Focus,
    };

    assert_ne!(entry1.id, entry2.id);
}

// ── Reminder Deduplication Edge Cases ──────────────────────────────

#[tokio::test]
async fn test_multiple_reminder_conditions_same_task() {
    // Edge case: task triggers multiple reminder rules simultaneously
    let now = Utc::now();
    let todo = Todo {
        due_date: Some(now + Duration::minutes(30)), // Due date reminder
        focused_at: Some(now - Duration::hours(2)),
        focus_deadline: Some(now + Duration::minutes(30)), // Focus reminder
        ..create_test_todo("Multiple triggers")
    };

    let due_remind = ReminderEngine::should_remind_due_date(&todo);
    let focus_remind = ReminderEngine::should_remind_focused_deadline(&todo);

    assert!(due_remind, "Should trigger due date reminder");
    assert!(focus_remind, "Should trigger focus reminder");
    // Both triggers fire - deduplication is handled by last_reminded_at field
}

#[tokio::test]
async fn test_last_reminded_blocks_all_rules() {
    // Edge case: last_reminded_at blocks both due date and focus reminders
    let now = Utc::now();
    let mut todo = Todo {
        due_date: Some(now + Duration::minutes(30)),
        focused_at: Some(now - Duration::hours(2)),
        focus_deadline: Some(now + Duration::minutes(30)),
        last_reminded_at: Some(now - Duration::minutes(10)), // Recently reminded
        ..create_test_todo("Recently reminded")
    };

    let due_remind = ReminderEngine::should_remind_due_date(&todo);
    let focus_remind = ReminderEngine::should_remind_focused_deadline(&todo);

    assert!(
        !due_remind,
        "last_reminded_at should block due date reminder"
    );
    assert!(
        !focus_remind,
        "last_reminded_at should block focus reminder"
    );

    // But overdue nag has its own 24-hour logic
    todo.due_date = Some(now - Duration::days(1)); // Make it overdue
    let overdue_remind = ReminderEngine::should_remind_overdue(&todo);
    assert!(
        !overdue_remind,
        "last_reminded_at within 24h should block overdue"
    );
}

#[tokio::test]
async fn test_calendar_event_reminder_deduplication() {
    use agent::reminders::CalendarEvent as ReminderCalendarEvent;

    // Edge case: event already reminded, should not remind again
    let event = ReminderCalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        start: Utc::now() + Duration::minutes(15),
        last_reminded_at: Some(Utc::now() - Duration::minutes(5)),
    };

    let should_remind = ReminderEngine::should_remind_calendar_event(&event);
    assert!(!should_remind, "Already reminded event should not trigger");
}

// ── Priority and Status Edge Cases ──────────────────────────────────

#[tokio::test]
async fn test_reminder_for_done_task() {
    // Edge case: completed task with due date (should reminders still fire?)
    let now = Utc::now();
    let todo = Todo {
        status: TodoStatus::Done,
        due_date: Some(now + Duration::minutes(30)),
        completed_at: Some(now - Duration::hours(1)),
        ..create_test_todo("Completed but has due date")
    };

    // Current implementation doesn't filter by status - reminder will fire
    // This might be a bug or expected behavior - test documents current state
    let should_remind = ReminderEngine::should_remind_due_date(&todo);
    // Note: Implementation doesn't check status, so this will be true
    // In production, the check_and_send_reminders might filter by status
    assert!(
        should_remind,
        "Current implementation: reminders fire regardless of status"
    );
}

#[tokio::test]
async fn test_reminder_for_archived_task() {
    // Edge case: archived task with due date
    let now = Utc::now();
    let todo = Todo {
        status: TodoStatus::Archived,
        due_date: Some(now + Duration::minutes(30)),
        ..create_test_todo("Archived task")
    };

    let should_remind = ReminderEngine::should_remind_due_date(&todo);
    assert!(
        should_remind,
        "Current implementation: reminders fire for archived tasks"
    );
}

#[tokio::test]
async fn test_no_due_date_no_reminder() {
    // Edge case: task without due date should never trigger due date reminder
    let todo = create_test_todo("No due date");

    let should_remind = ReminderEngine::should_remind_due_date(&todo);
    assert!(!should_remind, "No due date should not trigger");
}

#[tokio::test]
async fn test_no_focus_no_reminder() {
    // Edge case: unfocused task should not trigger focus reminder
    let todo = create_test_todo("Not focused");

    let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);
    assert!(!should_remind, "Unfocused task should not trigger");
}

#[tokio::test]
async fn test_focused_but_no_deadline() {
    // Edge case: focused task without deadline
    let now = Utc::now();
    let todo = Todo {
        focused_at: Some(now - Duration::hours(2)),
        focus_deadline: None, // No deadline set
        ..create_test_todo("Focused without deadline")
    };

    let should_remind = ReminderEngine::should_remind_focused_deadline(&todo);
    assert!(
        !should_remind,
        "No deadline should not trigger even if focused"
    );
}

// ── Time Calculation Edge Cases ──────────────────────────────────

#[tokio::test]
async fn test_overdue_by_many_days() {
    // Edge case: task overdue by many days
    let now = Utc::now();
    let todo = Todo {
        due_date: Some(now - Duration::days(365)), // Overdue by a year
        ..create_test_todo("Very overdue")
    };

    let should_remind = ReminderEngine::should_remind_overdue(&todo);
    assert!(should_remind, "Very overdue task should nag");
}

#[tokio::test]
async fn test_due_far_in_future() {
    // Edge case: task due far in future
    let now = Utc::now();
    let todo = Todo {
        due_date: Some(now + Duration::days(365)), // Due in a year
        ..create_test_todo("Far future")
    };

    let should_remind = ReminderEngine::should_remind_due_date(&todo);
    assert!(!should_remind, "Far future due date should not remind");
}

#[tokio::test]
async fn test_focus_expired_many_times() {
    // Edge case: task that expired focus many times
    let todo = Todo {
        focus_expired_count: 100,
        ..create_test_todo("Chronic procrastinator")
    };

    assert_eq!(todo.focus_expired_count, 100);
    // The count itself doesn't affect reminders, but documents edge case
}
