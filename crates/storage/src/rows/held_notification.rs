//! Row for `held_notifications` (quiet-hours-suppressed deliveries).
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldNotificationRow {
    pub id: String,
    pub alarm_id: String,
    pub channels: serde_json::Value,
    pub payload: serde_json::Value,
    pub release_at_ms: i64,
    pub released: bool,
    pub held_at_ms: i64,
}
