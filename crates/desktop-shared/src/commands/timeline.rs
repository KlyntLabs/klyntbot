use serde::{Deserialize, Serialize};

// ── Timeline / Dashboard ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineQuery {
    pub start_date: String,
    pub end_date: String,
    pub sources: Option<Vec<TimelineSource>>,
    pub include_point_events: Option<bool>,
    /// JS-style timezone offset in minutes (e.g. -420 for UTC+7).
    /// Used to shift day boundaries so local-time events appear on the correct date.
    pub tz_offset_mins: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineResponse {
    pub entries: Vec<TimelineEntry>,
    pub summary: TimelineSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    pub id: String,
    pub source: TimelineSource,
    pub entry_type: TimelineEntryType,
    pub title: String,
    pub description: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_secs: Option<i64>,
    pub entity_id: Option<String>,
    pub entity_route: Option<String>,
    pub color: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimelineSource {
    Productivity,
    Focus,
    Task,
    Todo,
    Note,
    Finance,
    System,
    Calendar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TimelineEntryType {
    AppUsage,
    FocusSession,
    TaskTimeEntry,
    TaskCreated,
    TaskCompleted,
    TaskUpdated,
    TaskDue,
    NoteCreated,
    NoteUpdated,
    TransactionRecorded,
    ExpenseRecorded,
    IncomeRecorded,
    SystemEvent,
    CalendarEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineSummary {
    pub total_tracked_secs: i64,
    pub focus_secs: i64,
    pub tasks_completed: i64,
    pub tasks_created: i64,
    pub notes_touched: i64,
    pub transactions_count: i64,
    pub top_apps: Vec<TopAppSummary>,
    pub source_breakdown: Vec<SourceBreakdown>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TopAppSummary {
    pub app_name: String,
    pub duration_secs: i64,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceBreakdown {
    pub source: TimelineSource,
    pub duration_secs: i64,
    pub count: i64,
}
