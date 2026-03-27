//! Row struct for the `cron_jobs` table.

use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `cron_jobs` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CronJobRow {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub origin: String,
    pub schedule: serde_json::Value,
    pub payload: serde_json::Value,
    pub next_run_at_ms: Option<i64>,
    pub last_run_at_ms: Option<i64>,
    pub last_status: Option<String>,
    pub last_error: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub delete_after_run: bool,
    pub intent_window: Option<String>,
    pub intent_pending_since_ms: Option<i64>,
}
