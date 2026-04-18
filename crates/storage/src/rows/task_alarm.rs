//! Row struct for the `task_alarms` table.
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAlarmRow {
    pub id: String,
    pub task_id: String,
    pub rule_type: String,
    pub offset_secs: Option<i64>,
    pub day_offset: Option<i64>,
    pub time_of_day: Option<String>,
    pub iana_tz: Option<String>,
    pub absolute_fire_at_ms: Option<i64>,
    pub channel_mask: i64,
    pub priority_override: Option<String>,
    pub misfire_policy: Option<String>,
    pub grace_window_secs: Option<i64>,
    pub created_at_ms: i64,
}
