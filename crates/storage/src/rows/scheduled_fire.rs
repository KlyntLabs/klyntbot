//! Row struct for the `scheduled_fires` table — the canonical "when to fire" store.
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledFireRow {
    pub id: String,
    pub fire_at_ms: i64,
    pub kind: String,
    pub ref_id: Option<String>,
    pub payload: serde_json::Value,
    pub dedup_prefix: Option<String>,
    pub fired: bool,
    pub firing_started_at_ms: Option<i64>,
    pub fired_at_ms: Option<i64>,
    pub suppressed_by: Option<String>,
    pub created_at_ms: i64,
}
