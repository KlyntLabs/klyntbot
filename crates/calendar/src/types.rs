use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a calendar event from CalDAV or converted from a todo item
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEvent {
    /// Unique identifier (iCalendar UID)
    pub uid: String,
    /// Event summary/title
    pub summary: String,
    /// Optional detailed description
    pub description: Option<String>,
    /// Start time
    pub start: DateTime<Utc>,
    /// End time
    pub end: DateTime<Utc>,
    /// Source of the event
    pub source: EventSource,
    /// ETag for CalDAV sync (if from server)
    pub etag: Option<String>,
}

/// Source of a calendar event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventSource {
    /// Event from CalDAV server
    CalDAV,
    /// Event converted from a todo item
    TodoItem,
}

/// Sync state for incremental CalDAV sync (RFC 6578)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncState {
    /// Sync token from server (for incremental sync)
    pub sync_token: Option<String>,
    /// Last successful sync timestamp
    pub last_sync: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_calendar_event_creation() {
        let start_time = Utc::now();
        let end_time = start_time + chrono::Duration::hours(1);

        let event = CalendarEvent {
            uid: "event-123".to_string(),
            summary: "Team Meeting".to_string(),
            description: Some("Discuss Q1 plans".to_string()),
            start: start_time,
            end: end_time,
            source: EventSource::CalDAV,
            etag: Some("etag-456".to_string()),
        };

        assert_eq!(event.uid, "event-123");
        assert_eq!(event.summary, "Team Meeting");
        assert_eq!(event.description, Some("Discuss Q1 plans".to_string()));
        assert_eq!(event.start, start_time);
        assert_eq!(event.end, end_time);
        assert_eq!(event.source, EventSource::CalDAV);
        assert_eq!(event.etag, Some("etag-456".to_string()));
    }

    #[test]
    fn test_event_source_variants() {
        assert_eq!(EventSource::CalDAV, EventSource::CalDAV);
        assert_ne!(EventSource::CalDAV, EventSource::TodoItem);
    }

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
    fn test_sync_state_with_token() {
        let now = Utc::now();
        let state = SyncState {
            sync_token: Some("token-789".to_string()),
            last_sync: Some(now),
        };

        assert_eq!(state.sync_token, Some("token-789".to_string()));
        assert_eq!(state.last_sync, Some(now));
    }
}
