//! Row struct for the `calendar_sync_state` table.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

/// Row struct for the `calendar_sync_state` table.
#[derive(Debug, Clone, FromRow)]
pub struct CalendarSyncStateRow {
    pub provider_id: String,
    pub sync_token: Option<String>,
    pub last_sync_at: Option<DateTime<Utc>>,
}
