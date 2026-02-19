// SyncEngine - Two-way calendar synchronization with conflict resolution

use crate::types::{CalendarEvent, ConflictResolutionStrategy};

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

/// Resolve a conflict between a server and local version of the same event.
///
/// The `strategy` controls which version wins:
/// - `ServerWins` — always return the server version.
/// - `ClientWins` — always return the local (client) version.
/// - `LastWriteWins` — return whichever version appears more recent, using CalDAV
///   ETag lexicographic ordering as a proxy for recency.
///   Falls back to server version when ordering is ambiguous.
/// - `Manual` — return the server version as a safe placeholder; callers
///   should detect this strategy and surface the conflict to the user.
pub fn resolve_conflict(
    server_event: &CalendarEvent,
    local_event: &CalendarEvent,
    strategy: ConflictResolutionStrategy,
) -> CalendarEvent {
    match strategy {
        ConflictResolutionStrategy::ServerWins => server_event.clone(),

        ConflictResolutionStrategy::ClientWins => local_event.clone(),

        ConflictResolutionStrategy::LastWriteWins => {
            // Use ETag as a recency proxy.  CalDAV servers commonly use monotonically
            // increasing values (integers, timestamps, or hash strings that sort by
            // recency).  Lexicographic comparison is a best-effort heuristic; when
            // ordering cannot be determined we fall back to the server version.
            match (&server_event.etag, &local_event.etag) {
                // Both have ETags: higher lexicographic value ⇒ more recent.
                (Some(s_etag), Some(l_etag)) => {
                    if l_etag > s_etag {
                        local_event.clone()
                    } else {
                        server_event.clone()
                    }
                }
                // Only the local event has an ETag: local was previously confirmed
                // by the server, so it might be the more recent copy.
                (None, Some(_)) => local_event.clone(),
                // Server has an ETag (or neither does): server is authoritative.
                _ => server_event.clone(),
            }
        }

        ConflictResolutionStrategy::Manual => {
            // Flag conflict for manual resolution.  Return the server version as a
            // safe placeholder so no local changes are accidentally pushed.  Callers
            // that need to surface this conflict to the user should check the
            // strategy before calling `resolve_conflict`.
            server_event.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CalendarEvent, ConflictResolutionStrategy, EventSource, SyncState};
    use chrono::{TimeZone, Utc};
    use std::str::FromStr;

    // ---- helpers ----

    fn make_server_event() -> CalendarEvent {
        CalendarEvent {
            uid: "event-1".to_string(),
            summary: "Server Version".to_string(),
            description: Some("Changed on server".to_string()),
            start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
            source: EventSource::CalDAV,
            etag: Some("etag-new".to_string()),
            status: None,
        }
    }

    fn make_local_event() -> CalendarEvent {
        CalendarEvent {
            uid: "event-1".to_string(),
            summary: "Local Version".to_string(),
            description: Some("Changed locally".to_string()),
            start: Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap(),
            end: Utc.with_ymd_and_hms(2026, 3, 1, 11, 0, 0).unwrap(),
            source: EventSource::TodoItem,
            etag: Some("etag-old".to_string()),
            status: None,
        }
    }

    // ---- SyncState tests ----

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

    // ---- detect_conflict tests ----

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

        assert!(!detect_conflict(&server_event, &local_event)); // Different UIDs = no conflict
    }

    #[test]
    fn test_detect_conflict_same_uid() {
        let server_event = make_server_event();
        let local_event = make_local_event();
        assert!(detect_conflict(&server_event, &local_event)); // Same UID + different content = conflict
    }

    // ---- resolve_conflict strategy tests ----

    #[test]
    fn test_resolve_conflict_server_wins() {
        let server = make_server_event();
        let local = make_local_event();

        let resolved = resolve_conflict(&server, &local, ConflictResolutionStrategy::ServerWins);

        assert_eq!(resolved.summary, "Server Version");
        assert_eq!(resolved.etag, Some("etag-new".to_string()));
        assert_eq!(resolved.source, EventSource::CalDAV);
    }

    #[test]
    fn test_resolve_conflict_client_wins() {
        let server = make_server_event();
        let local = make_local_event();

        let resolved = resolve_conflict(&server, &local, ConflictResolutionStrategy::ClientWins);

        assert_eq!(resolved.summary, "Local Version");
        assert_eq!(resolved.etag, Some("etag-old".to_string()));
        assert_eq!(resolved.source, EventSource::TodoItem);
    }

    #[test]
    fn test_resolve_conflict_last_write_wins_server_newer() {
        let mut server = make_server_event();
        let mut local = make_local_event();
        // Use numeric-style ETags where server > local → server wins
        server.etag = Some("etag-200".to_string());
        local.etag = Some("etag-100".to_string());

        let resolved = resolve_conflict(&server, &local, ConflictResolutionStrategy::LastWriteWins);

        assert_eq!(resolved.summary, "Server Version");
        assert_eq!(resolved.etag, Some("etag-200".to_string()));
    }

    #[test]
    fn test_resolve_conflict_last_write_wins_local_newer() {
        let mut server = make_server_event();
        let mut local = make_local_event();
        // Make local etag lexicographically greater → local wins
        server.etag = Some("etag-100".to_string());
        local.etag = Some("etag-200".to_string());

        let resolved = resolve_conflict(&server, &local, ConflictResolutionStrategy::LastWriteWins);

        assert_eq!(resolved.summary, "Local Version");
        assert_eq!(resolved.etag, Some("etag-200".to_string()));
    }

    #[test]
    fn test_resolve_conflict_last_write_wins_only_local_has_etag() {
        let mut server = make_server_event();
        let local = make_local_event();
        server.etag = None; // server lacks etag

        let resolved = resolve_conflict(&server, &local, ConflictResolutionStrategy::LastWriteWins);

        // Local has an etag (confirmed server-side previously) → local wins
        assert_eq!(resolved.summary, "Local Version");
    }

    #[test]
    fn test_resolve_conflict_last_write_wins_no_etags() {
        let mut server = make_server_event();
        let mut local = make_local_event();
        server.etag = None;
        local.etag = None;

        let resolved = resolve_conflict(&server, &local, ConflictResolutionStrategy::LastWriteWins);

        // Cannot determine recency → fall back to server
        assert_eq!(resolved.summary, "Server Version");
    }

    #[test]
    fn test_resolve_conflict_manual_returns_server() {
        let server = make_server_event();
        let local = make_local_event();

        // Manual strategy preserves server version as a safe placeholder.
        let resolved = resolve_conflict(&server, &local, ConflictResolutionStrategy::Manual);

        assert_eq!(resolved.summary, "Server Version");
        assert_eq!(resolved.etag, Some("etag-new".to_string()));
    }

    // ---- ConflictResolutionStrategy serde / FromStr tests ----

    #[test]
    fn test_strategy_serde_roundtrip() {
        use serde_json;

        let cases = [
            (ConflictResolutionStrategy::ServerWins, "\"serverWins\""),
            (ConflictResolutionStrategy::ClientWins, "\"clientWins\""),
            (
                ConflictResolutionStrategy::LastWriteWins,
                "\"lastWriteWins\"",
            ),
            (ConflictResolutionStrategy::Manual, "\"manual\""),
        ];

        for (strategy, expected_json) in cases {
            let serialized = serde_json::to_string(&strategy).unwrap();
            assert_eq!(
                serialized, expected_json,
                "Serialization mismatch for {:?}",
                strategy
            );

            let deserialized: ConflictResolutionStrategy =
                serde_json::from_str(&serialized).unwrap();
            assert_eq!(
                deserialized, strategy,
                "Deserialization mismatch for {:?}",
                strategy
            );
        }
    }

    #[test]
    fn test_strategy_from_str_camel_case() {
        assert_eq!(
            ConflictResolutionStrategy::from_str("serverWins").unwrap(),
            ConflictResolutionStrategy::ServerWins
        );
        assert_eq!(
            ConflictResolutionStrategy::from_str("clientWins").unwrap(),
            ConflictResolutionStrategy::ClientWins
        );
        assert_eq!(
            ConflictResolutionStrategy::from_str("lastWriteWins").unwrap(),
            ConflictResolutionStrategy::LastWriteWins
        );
        assert_eq!(
            ConflictResolutionStrategy::from_str("manual").unwrap(),
            ConflictResolutionStrategy::Manual
        );
    }

    #[test]
    fn test_strategy_from_str_snake_case_compat() {
        // Backward compat with old config string values
        assert_eq!(
            ConflictResolutionStrategy::from_str("server_wins").unwrap(),
            ConflictResolutionStrategy::ServerWins
        );
        assert_eq!(
            ConflictResolutionStrategy::from_str("client_wins").unwrap(),
            ConflictResolutionStrategy::ClientWins
        );
        assert_eq!(
            ConflictResolutionStrategy::from_str("last_write_wins").unwrap(),
            ConflictResolutionStrategy::LastWriteWins
        );
    }

    #[test]
    fn test_strategy_from_str_invalid() {
        assert!(ConflictResolutionStrategy::from_str("unknown").is_err());
        assert!(ConflictResolutionStrategy::from_str("").is_err());
    }

    #[test]
    fn test_strategy_default_is_server_wins() {
        assert_eq!(
            ConflictResolutionStrategy::default(),
            ConflictResolutionStrategy::ServerWins
        );
    }
}
