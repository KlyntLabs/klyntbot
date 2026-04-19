//! Row for the `notification_log` idempotency-gate table.
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationLogRow {
    pub alarm_id: String,
    pub channel: String,
    pub sent_at_ms: i64,
    pub ack_at_ms: Option<i64>,
    pub error: Option<String>,
}
