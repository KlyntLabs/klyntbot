//! Integration tests for /api/calendar endpoints.
//!
//! Covers AC-12.x: Events list, sync status, trigger-sync.

mod common;

#[tokio::test]
async fn get_calendar_events_returns_array() {
    // AC-12.1: GET /api/calendar/events returns Vec<CalendarEventCacheRow>
    // TODO: implement
}

#[tokio::test]
async fn get_calendar_events_filter_by_provider_id() {
    // AC-12.1: GET /api/calendar/events?providerId=icloud filters by provider
    // TODO: implement
}

#[tokio::test]
async fn get_calendar_events_limit_param() {
    // AC-12.1: GET /api/calendar/events?limit=10 caps results
    // TODO: implement
}

#[tokio::test]
async fn get_calendar_sync_status_returns_sync_states() {
    // AC-12.2: GET /api/calendar/sync-status returns Vec<CalendarSyncStateRow>
    // TODO: implement
}

#[tokio::test]
async fn post_calendar_sync_returns_202_accepted() {
    // AC-12.3: POST /api/calendar/sync triggers async sync, returns 202 Accepted
    // (sync is background — response is immediate, not blocking)
    // TODO: implement
}

#[tokio::test]
async fn calendar_event_response_fields_are_camel_case() {
    // AC-1.6: CalendarEventCacheRow response fields are camelCase
    // Verify: providerId, eventUid, startAt, endAt, createdAt, updatedAt
    // TODO: implement
}
