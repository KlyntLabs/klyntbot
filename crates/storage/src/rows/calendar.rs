//! Row structs for calendar tables (`calendar_sync_state`, `calendar_event_cache`).

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `calendar_sync_state` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarSyncStateRow {
    pub provider_id: String,
    pub sync_token: Option<String>,
    pub last_sync_at: Option<DateTime<Utc>>,
}

/// Row struct for the `calendar_event_cache` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventCacheRow {
    pub uid: String,
    pub provider_id: String,
    pub summary: String,
    pub description: Option<String>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
    pub source: String,
    pub etag: Option<String>,
    pub status: Option<String>,
    pub cached_at: DateTime<Utc>,
}
