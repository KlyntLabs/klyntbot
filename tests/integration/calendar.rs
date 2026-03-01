//! Calendar integration tests — reconciliation, conflict detection, and edge cases.

use calendar::{
    detect_conflict, resolve_conflict, CalendarEvent, ConflictResolutionStrategy, EventSource,
    SyncState,
};
use chrono::{TimeZone, Utc};

use super::common::create_test_todo;
use agent::calendar_reconcile::reconcile_calendar_events;
use tools::todo_types::{Todo, TodoStatus};

// ── Helpers ────────────────────────────────────────────────────

/// Create a migrated in-memory ActionRepo for testing, with the default test area pre-created.
async fn test_action_repo() -> Option<storage::ActionRepo> {
    let pool = super::common::test_pool().await;
    super::common::ensure_test_area(&pool).await;
    Some(storage::ActionRepo::new(pool.inner().clone()))
}

/// Generate a unique event UID to prevent parallel test contamination.
fn unique_event_uid(prefix: &str) -> String {
    format!("{prefix}-{}", &uuid::Uuid::new_v4().to_string()[..8])
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

/// Build a pair of otherwise-identical events differing only in the given field.
fn conflict_event_pair() -> (CalendarEvent, CalendarEvent) {
    let base = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: Some("etag-1".to_string()),
        status: None,
    };
    let mut local = base.clone();
    local.source = EventSource::TodoItem;
    (base, local)
}

// ── Reconciliation ─────────────────────────────────────────────

#[tokio::test]
async fn reconcile_updates_todo_on_time_change() {
    let Some(repo) = test_action_repo().await else {
        return;
    };

    let original_due = Utc.with_ymd_and_hms(2026, 2, 20, 14, 0, 0).unwrap();
    let event_uid = unique_event_uid("ev-time");
    let mut todo = create_test_todo("Meeting");
    todo.due_date = Some(original_due);
    todo.calendar_event_uid = Some(event_uid.clone());
    let todo_id = todo.id.clone();
    let row: storage::ActionRow = (&todo).into();
    repo.add(&row).await.unwrap();

    let new_time = Utc.with_ymd_and_hms(2026, 2, 20, 16, 0, 0).unwrap();
    let events = vec![create_test_event(&event_uid, new_time, None)];

    let report = reconcile_calendar_events(&repo, events).await.unwrap();

    assert!(report.checked >= 1);
    assert_eq!(report.due_dates_updated, 1);
    assert_eq!(report.todos_completed, 0);

    let updated_row = repo.get(&todo_id).await.unwrap().unwrap();
    let updated_todo = Todo::from(updated_row);
    assert_eq!(updated_todo.due_date, Some(new_time));

    let _ = repo.delete(&todo_id).await;
}

#[tokio::test]
async fn reconcile_marks_todo_done_on_completed_event() {
    let Some(repo) = test_action_repo().await else {
        return;
    };

    let event_uid = unique_event_uid("ev-done");
    let mut todo = create_test_todo("Task");
    todo.status = TodoStatus::Todo;
    todo.calendar_event_uid = Some(event_uid.clone());
    todo.due_date = Some(Utc::now());
    let todo_id = todo.id.clone();
    let row: storage::ActionRow = (&todo).into();
    repo.add(&row).await.unwrap();

    let events = vec![create_test_event(
        &event_uid,
        Utc::now(),
        Some("COMPLETED".to_string()),
    )];

    let report = reconcile_calendar_events(&repo, events).await.unwrap();

    assert!(report.checked >= 1);
    assert_eq!(report.todos_completed, 1);
    assert_eq!(report.due_dates_updated, 0);

    let updated_row = repo.get(&todo_id).await.unwrap().unwrap();
    let updated = Todo::from(updated_row);
    assert_eq!(updated.status, TodoStatus::Done);

    let _ = repo.delete(&todo_id).await;
}

#[tokio::test]
async fn reconcile_clears_uid_on_cancelled_event() {
    let Some(repo) = test_action_repo().await else {
        return;
    };

    let event_uid = unique_event_uid("ev-cancel");
    let mut todo = create_test_todo("Cancelled meeting");
    todo.calendar_event_uid = Some(event_uid.clone());
    let todo_id = todo.id.clone();
    let row: storage::ActionRow = (&todo).into();
    repo.add(&row).await.unwrap();

    let events = vec![create_test_event(
        &event_uid,
        Utc::now(),
        Some("CANCELLED".to_string()),
    )];

    let report = reconcile_calendar_events(&repo, events).await.unwrap();

    assert!(report.checked >= 1);
    assert!(report.links_cleared >= 1);
    assert_eq!(report.todos_completed, 0);
    assert_eq!(report.due_dates_updated, 0);

    let updated = Todo::from(repo.get(&todo_id).await.unwrap().unwrap());
    assert!(
        updated.calendar_event_uid.is_none(),
        "calendar_event_uid should be cleared after CANCELLED event"
    );

    let _ = repo.delete(&todo_id).await;
}

#[tokio::test]
async fn reconcile_clears_uid_on_deleted_event() {
    let Some(repo) = test_action_repo().await else {
        return;
    };

    let event_uid = unique_event_uid("ev-del");
    let mut todo = create_test_todo("Deleted event");
    todo.calendar_event_uid = Some(event_uid.clone());
    let todo_id = todo.id.clone();
    let row: storage::ActionRow = (&todo).into();
    repo.add(&row).await.unwrap();

    let events: Vec<CalendarEvent> = vec![];

    let report = reconcile_calendar_events(&repo, events).await.unwrap();

    assert!(report.checked >= 1);
    assert!(report.links_cleared >= 1);

    let updated = Todo::from(repo.get(&todo_id).await.unwrap().unwrap());
    assert!(
        updated.calendar_event_uid.is_none(),
        "calendar_event_uid should be cleared when event is not found"
    );

    let _ = repo.delete(&todo_id).await;
}

#[tokio::test]
async fn reconcile_handles_multiple_changes_in_batch() {
    let Some(repo) = test_action_repo().await else {
        return;
    };

    let original_time = Utc.with_ymd_and_hms(2026, 2, 20, 14, 0, 0).unwrap();
    let new_time = Utc.with_ymd_and_hms(2026, 2, 20, 16, 0, 0).unwrap();
    let unchanged_time = Utc.with_ymd_and_hms(2026, 2, 20, 10, 0, 0).unwrap();

    let mut ids = Vec::new();

    let uid1 = unique_event_uid("ev-multi1");
    let uid2 = unique_event_uid("ev-multi2");
    let uid3 = unique_event_uid("ev-multi3");
    let uid4 = unique_event_uid("ev-multi4");
    let uid5 = unique_event_uid("ev-multi5");

    // Todo 1: Time changed
    let mut todo1 = create_test_todo("Todo 1");
    todo1.calendar_event_uid = Some(uid1.clone());
    todo1.due_date = Some(original_time);
    ids.push(todo1.id.clone());
    repo.add(&storage::ActionRow::from(&todo1)).await.unwrap();

    // Todo 2: Completed
    let mut todo2 = create_test_todo("Todo 2");
    todo2.calendar_event_uid = Some(uid2.clone());
    todo2.status = TodoStatus::Todo;
    todo2.due_date = Some(unchanged_time);
    ids.push(todo2.id.clone());
    repo.add(&storage::ActionRow::from(&todo2)).await.unwrap();

    // Todo 3: Cancelled
    let mut todo3 = create_test_todo("Todo 3");
    todo3.calendar_event_uid = Some(uid3.clone());
    ids.push(todo3.id.clone());
    repo.add(&storage::ActionRow::from(&todo3)).await.unwrap();

    // Todo 4: Unchanged
    let mut todo4 = create_test_todo("Todo 4");
    todo4.calendar_event_uid = Some(uid4.clone());
    todo4.due_date = Some(unchanged_time);
    ids.push(todo4.id.clone());
    repo.add(&storage::ActionRow::from(&todo4)).await.unwrap();

    // Todo 5: Event deleted
    let mut todo5 = create_test_todo("Todo 5");
    todo5.calendar_event_uid = Some(uid5.clone());
    ids.push(todo5.id.clone());
    repo.add(&storage::ActionRow::from(&todo5)).await.unwrap();

    let events = vec![
        create_test_event(&uid1, new_time, None),
        create_test_event(&uid2, unchanged_time, Some("COMPLETED".to_string())),
        create_test_event(&uid3, unchanged_time, Some("CANCELLED".to_string())),
        create_test_event(&uid4, unchanged_time, None),
        // uid5 not in list (deleted)
    ];

    let report = reconcile_calendar_events(&repo, events).await.unwrap();

    assert!(report.checked >= 5);
    assert_eq!(report.due_dates_updated, 1); // todo1
    assert_eq!(report.todos_completed, 1); // todo2
    assert!(report.links_cleared >= 2); // todo3 + todo5

    for id in &ids {
        let _ = repo.delete(id).await;
    }
}

// ── Conflict detection ─────────────────────────────────────────

#[test]
fn conflict_detect_returns_false_for_identical_events() {
    let (event1, event2) = conflict_event_pair();

    // Different description sources but identical content
    let mut e1 = event1;
    e1.description = Some("Discussion".to_string());
    let mut e2 = event2;
    e2.description = Some("Discussion".to_string());

    let conflict = detect_conflict(&e1, &e2);
    assert!(!conflict, "Identical events should not conflict");
}

#[test]
fn conflict_detect_returns_true_on_summary_diff() {
    let (mut event1, event2) = conflict_event_pair();
    event1.summary = "Old Summary".to_string();
    let mut e2 = event2;
    e2.summary = "New Summary".to_string();

    assert!(
        detect_conflict(&event1, &e2),
        "Different summaries should conflict"
    );
}

#[test]
fn conflict_detect_returns_true_on_description_diff() {
    let (mut event1, mut event2) = conflict_event_pair();
    event1.description = Some("Original notes".to_string());
    event2.description = Some("Updated notes".to_string());

    assert!(
        detect_conflict(&event1, &event2),
        "Different descriptions should conflict"
    );
}

#[test]
fn conflict_detect_returns_true_on_none_vs_some_description() {
    let (mut event1, mut event2) = conflict_event_pair();
    event1.description = None;
    event2.description = Some("Added notes".to_string());

    assert!(
        detect_conflict(&event1, &event2),
        "None vs Some description should conflict"
    );
}

#[test]
fn conflict_detect_returns_true_on_start_time_diff() {
    let (event1, mut event2) = conflict_event_pair();
    event2.start = Utc.with_ymd_and_hms(2026, 3, 1, 10, 30, 0).unwrap();

    assert!(
        detect_conflict(&event1, &event2),
        "Different start times should conflict"
    );
}

#[test]
fn conflict_detect_returns_true_on_end_time_diff() {
    let (event1, mut event2) = conflict_event_pair();
    event2.end = Utc.with_ymd_and_hms(2026, 3, 1, 11, 30, 0).unwrap();

    assert!(
        detect_conflict(&event1, &event2),
        "Different end times should conflict"
    );
}

#[test]
fn conflict_detect_returns_true_on_etag_diff() {
    let (mut event1, mut event2) = conflict_event_pair();
    event1.etag = Some("etag-old".to_string());
    event2.etag = Some("etag-new".to_string());

    assert!(
        detect_conflict(&event1, &event2),
        "Different etags should conflict"
    );
}

#[test]
fn conflict_detect_returns_true_on_none_vs_some_etag() {
    let (mut event1, mut event2) = conflict_event_pair();
    event1.etag = None;
    event2.etag = Some("etag-1".to_string());

    assert!(
        detect_conflict(&event1, &event2),
        "None vs Some etag should conflict"
    );
}

// ── Conflict resolution ────────────────────────────────────────

#[test]
fn conflict_resolve_preserves_server_data_with_server_wins() {
    let server_event = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Server Summary".to_string(),
        description: Some("Server description".to_string()),
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: Some("server-etag".to_string()),
        status: None,
    };

    let local_event = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Local Summary".to_string(),
        description: Some("Local description".to_string()),
        start: Utc.with_ymd_and_hms(2026, 3, 1, 9, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        source: EventSource::TodoItem,
        etag: Some("local-etag".to_string()),
        status: None,
    };

    let resolved = resolve_conflict(
        &server_event,
        &local_event,
        ConflictResolutionStrategy::ServerWins,
    );

    assert_eq!(resolved.summary, "Server Summary");
    assert_eq!(resolved.description, Some("Server description".to_string()));
    assert_eq!(
        resolved.start,
        Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap()
    );
    assert_eq!(
        resolved.end,
        Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap()
    );
    assert_eq!(resolved.etag, Some("server-etag".to_string()));
    assert_eq!(resolved.source, EventSource::CalDAV);
}

// ── Edge cases ─────────────────────────────────────────────────

#[test]
fn event_source_distinguishes_caldav_from_todo() {
    let caldav_event = CalendarEvent {
        uid: "cal-1".to_string(),
        summary: "CalDAV Event".to_string(),
        description: None,
        start: Utc::now(),
        end: Utc::now() + chrono::Duration::hours(1),
        source: EventSource::CalDAV,
        etag: None,
        status: None,
    };

    let todo_event = CalendarEvent {
        uid: "todo-1".to_string(),
        summary: "Todo Event".to_string(),
        description: None,
        start: Utc::now(),
        end: Utc::now() + chrono::Duration::hours(1),
        source: EventSource::TodoItem,
        etag: None,
        status: None,
    };

    assert!(matches!(caldav_event.source, EventSource::CalDAV));
    assert!(matches!(todo_event.source, EventSource::TodoItem));
}

#[test]
fn event_accepts_very_long_summary() {
    let long_summary = "A".repeat(1000);
    let event = CalendarEvent {
        uid: "event-1".to_string(),
        summary: long_summary.clone(),
        description: None,
        start: Utc::now(),
        end: Utc::now() + chrono::Duration::hours(1),
        source: EventSource::CalDAV,
        etag: None,
        status: None,
    };

    assert_eq!(event.summary.len(), 1000);
    assert_eq!(event.summary, long_summary);
}

#[test]
fn event_allows_zero_duration() {
    let time = Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap();
    let event = CalendarEvent {
        uid: "instant-event".to_string(),
        summary: "Instant event".to_string(),
        description: None,
        start: time,
        end: time,
        source: EventSource::CalDAV,
        etag: None,
        status: None,
    };

    assert_eq!(event.start, event.end);
}

#[test]
fn event_allows_negative_duration() {
    let start = Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap();
    let end = Utc.with_ymd_and_hms(2026, 3, 1, 9, 0, 0).unwrap();

    let event = CalendarEvent {
        uid: "backwards-event".to_string(),
        summary: "Backwards event".to_string(),
        description: None,
        start,
        end,
        source: EventSource::CalDAV,
        etag: None,
        status: None,
    };

    assert!(event.end < event.start);
}

#[test]
fn sync_state_accepts_very_long_token() {
    let long_token = "token-".to_string() + &"x".repeat(1000);
    let state = SyncState {
        sync_token: Some(long_token.clone()),
        last_sync: Some(Utc::now()),
    };

    assert_eq!(state.sync_token.as_ref().unwrap().len(), 1006);
    assert_eq!(state.sync_token.unwrap(), long_token);
}
