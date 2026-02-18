//! Edge case and error condition tests for calendar functionality
//!
//! Tests boundary conditions, error handling, and unusual scenarios:
//! - Empty sync states
//! - Conflict detection edge cases
//! - Invalid ETags
//! - CalDAV protocol edge cases

use calendar::{detect_conflict, resolve_conflict, CalendarEvent, EventSource, SyncState};
use chrono::{TimeZone, Utc};

#[test]
fn test_empty_sync_state_initialization() {
    let state = SyncState {
        sync_token: None,
        last_sync: None,
    };

    assert!(state.sync_token.is_none());
    assert!(state.last_sync.is_none());
}

#[test]
fn test_conflict_detection_identical_events() {
    // No conflict when events are identical except source
    let event1 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: Some("Discussion".to_string()),
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let event2 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: Some("Discussion".to_string()),
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::TodoItem,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let conflict = detect_conflict(&event1, &event2);
    assert!(!conflict, "Identical events should not conflict");
}

#[test]
fn test_conflict_detection_different_summary() {
    // Conflict when summary differs
    let event1 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Old Summary".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let event2 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "New Summary".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::TodoItem,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let conflict = detect_conflict(&event1, &event2);
    assert!(conflict, "Different summaries should conflict");
}

#[test]
fn test_conflict_detection_different_description() {
    // Conflict when description differs
    let event1 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: Some("Original notes".to_string()),
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let event2 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: Some("Updated notes".to_string()),
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::TodoItem,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let conflict = detect_conflict(&event1, &event2);
    assert!(conflict, "Different descriptions should conflict");
}

#[test]
fn test_conflict_detection_none_vs_some_description() {
    // Conflict when one has description and other doesn't
    let event1 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let event2 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: Some("Added notes".to_string()),
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::TodoItem,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let conflict = detect_conflict(&event1, &event2);
    assert!(conflict, "None vs Some description should conflict");
}

#[test]
fn test_conflict_detection_different_start_time() {
    // Conflict when start time differs
    let event1 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let event2 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 30, 0).unwrap(), // Different
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::TodoItem,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let conflict = detect_conflict(&event1, &event2);
    assert!(conflict, "Different start times should conflict");
}

#[test]
fn test_conflict_detection_different_end_time() {
    // Conflict when end time differs
    let event1 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let event2 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 30, 0).unwrap(), // Different
        source: EventSource::TodoItem,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let conflict = detect_conflict(&event1, &event2);
    assert!(conflict, "Different end times should conflict");
}

#[test]
fn test_conflict_detection_different_etag() {
    // Conflict when etag differs
    let event1 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: Some("etag-old".to_string()),
        status: None,
    };

    let event2 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::TodoItem,
        etag: Some("etag-new".to_string()),
        status: None,
    };

    let conflict = detect_conflict(&event1, &event2);
    assert!(conflict, "Different etags should conflict");
}

#[test]
fn test_conflict_detection_none_vs_some_etag() {
    // Conflict when one has etag and other doesn't
    let event1 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::CalDAV,
        etag: None,
        status: None,
    };

    let event2 = CalendarEvent {
        uid: "event-1".to_string(),
        summary: "Meeting".to_string(),
        description: None,
        start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
        end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
        source: EventSource::TodoItem,
        etag: Some("etag-1".to_string()),
        status: None,
    };

    let conflict = detect_conflict(&event1, &event2);
    assert!(conflict, "None vs Some etag should conflict");
}

#[test]
fn test_resolve_conflict_preserves_server_data() {
    // Server-wins: verify all server fields are preserved
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

    let resolved = resolve_conflict(&server_event, &local_event);

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

// CalDavClient tests removed - these tested private implementation details
// that are no longer exposed. The public API is tested through integration tests.

#[test]
fn test_event_source_variants() {
    // Verify EventSource enum variants
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
fn test_calendar_event_with_very_long_summary() {
    // Edge case: event with very long summary
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
fn test_calendar_event_with_zero_duration() {
    // Edge case: event where start == end (zero duration)
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
fn test_calendar_event_with_negative_duration() {
    // Edge case: event where end < start (invalid but structurally possible)
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
fn test_sync_state_with_very_long_token() {
    // Edge case: sync token that's very long
    let long_token = "token-".to_string() + &"x".repeat(1000);
    let state = SyncState {
        sync_token: Some(long_token.clone()),
        last_sync: Some(Utc::now()),
    };

    assert_eq!(state.sync_token.as_ref().unwrap().len(), 1006);
    assert_eq!(state.sync_token.unwrap(), long_token);
}
