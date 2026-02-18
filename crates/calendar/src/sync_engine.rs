// SyncEngine - Two-way calendar synchronization with conflict resolution

use crate::types::CalendarEvent;

/// Detect if two events conflict (same UID but different content)
pub fn detect_conflict(server_event: &CalendarEvent, local_event: &CalendarEvent) -> bool {
    if server_event.uid != local_event.uid {
        return false; // Different events, no conflict
    }

    // Same UID - check if content differs
    server_event.summary != local_event.summary
        || server_event.description != local_event.description
        || server_event.start != local_event.start
        || server_event.end != local_event.end
        || server_event.etag != local_event.etag
        || server_event.status != local_event.status
}

/// Resolve conflict using server-wins strategy
/// Returns the server version of the event
pub fn resolve_conflict(
    server_event: &CalendarEvent,
    _local_event: &CalendarEvent,
) -> CalendarEvent {
    // Server-wins conflict resolution: always use server version
    server_event.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CalendarEvent, EventSource, SyncState};
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_sync_state_initialization() {
        let state = SyncState {
            sync_token: None,
            last_sync: None,
        };

        assert!(state.sync_token.is_none());
        assert!(state.last_sync.is_none());
    }

    #[test]
    fn test_sync_state_update() {
        let mut state = SyncState {
            sync_token: None,
            last_sync: None,
        };

        let new_token = "token-abc".to_string();
        let now = Utc::now();

        state.sync_token = Some(new_token.clone());
        state.last_sync = Some(now);

        assert_eq!(state.sync_token, Some(new_token));
        assert_eq!(state.last_sync, Some(now));
    }

    #[test]
    fn test_detect_conflict_no_conflict() {
        let server_event = CalendarEvent {
            uid: "event-1".to_string(),
            summary: "Meeting".to_string(),
            description: None,
            start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
            source: EventSource::CalDAV,
            etag: Some("etag-1".to_string()),
            status: None,
        };

        let local_event = CalendarEvent {
            uid: "event-2".to_string(),
            summary: "Task".to_string(),
            description: None,
            start: Utc.with_ymd_and_hms(2026, 3, 2, 14, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 2, 15, 0, 0).unwrap(),
            source: EventSource::TodoItem,
            etag: None,
            status: None,
        };

        let conflict = detect_conflict(&server_event, &local_event);
        assert!(!conflict); // Different UIDs = no conflict
    }

    #[test]
    fn test_detect_conflict_same_uid() {
        let server_event = CalendarEvent {
            uid: "event-1".to_string(),
            summary: "Server Version".to_string(),
            description: Some("Changed on server".to_string()),
            start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
            source: EventSource::CalDAV,
            etag: Some("etag-new".to_string()),
            status: None,
        };

        let local_event = CalendarEvent {
            uid: "event-1".to_string(),
            summary: "Local Version".to_string(),
            description: Some("Changed locally".to_string()),
            start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
            source: EventSource::TodoItem,
            etag: Some("etag-old".to_string()),
            status: None,
        };

        let conflict = detect_conflict(&server_event, &local_event);
        assert!(conflict); // Same UID + different content = conflict
    }

    #[test]
    fn test_resolve_conflict_server_wins() {
        let server_event = CalendarEvent {
            uid: "event-1".to_string(),
            summary: "Server Version".to_string(),
            description: None,
            start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
            source: EventSource::CalDAV,
            etag: Some("etag-new".to_string()),
            status: None,
        };

        let local_event = CalendarEvent {
            uid: "event-1".to_string(),
            summary: "Local Version".to_string(),
            description: None,
            start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
            source: EventSource::TodoItem,
            etag: Some("etag-old".to_string()),
            status: None,
        };

        let resolved = resolve_conflict(&server_event, &local_event);

        // Server-wins strategy: should return server version
        assert_eq!(resolved.uid, server_event.uid);
        assert_eq!(resolved.summary, "Server Version");
        assert_eq!(resolved.etag, Some("etag-new".to_string()));
    }
}
