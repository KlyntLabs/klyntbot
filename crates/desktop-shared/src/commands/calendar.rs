use serde::{Deserialize, Serialize};

// ── Calendar ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventInput {
    pub title: String,
    pub started_at: String,
    pub ended_at: String,
    pub external_uid: String,
    pub calendar_id: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub attendees_count: Option<i64>,
    pub is_recurring: Option<bool>,
    pub recurrence_id: Option<String>,
    pub source: Option<String>,
    pub color: Option<String>,
}
