use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::events::FocusSyncPayload;

// ── Productivity ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivitySummaryResponse {
    pub date: String,
    pub total_active_secs: i64,
    pub total_focus_secs: i64,
    pub total_break_secs: i64,
    pub total_idle_secs: i64,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub focus_sessions_count: i64,
    pub avg_session_quality: Option<f64>,
    pub interruptions_count: i64,
    pub context_switches: i64,
    pub top_apps: Vec<AppUsageResponse>,
    pub top_categories: Vec<CategoryUsageResponse>,
    pub top_projects: Vec<ProjectUsageResponse>,
    pub ai_summary: Option<String>,
    pub productivity_score: Option<f64>,
    pub score_trend: Option<f64>,
    pub focus_time_trend: Option<f64>,
    pub active_time_trend: Option<f64>,
    pub deep_work_blocks: i64,
    pub deep_work_secs: i64,
    pub avg_recovery_secs: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsageResponse {
    pub app_name: String,
    pub duration_secs: i64,
    pub category: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryUsageResponse {
    pub category_id: String,
    pub category: String,
    pub category_type: String,
    pub duration_secs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedAppResponse {
    pub display_name: String,
    pub app_name: String,
    pub site_name: Option<String>,
    pub category_id: Option<String>,
    pub category_name: Option<String>,
    pub total_secs: i64,
    pub event_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUsageResponse {
    pub project_id: String,
    pub display_name: String,
    pub duration_secs: i64,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityProjectResponse {
    pub id: String,
    pub display_name: String,
    pub path: String,
    pub url_patterns: Vec<String>,
    pub color: Option<String>,
    pub is_auto_detected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSessionResponse {
    pub id: String,
    pub action_id: Option<String>,
    pub project_id: Option<String>,
    pub session_type: String,
    pub target_mins: Option<i64>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub actual_mins: Option<i64>,
    pub interruptions: i64,
    pub quality_score: Option<f64>,
    pub completed: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSessionStatusResponse {
    pub active: bool,
    pub sync: Option<FocusSyncPayload>,
    pub session: Option<FocusSessionResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistractionResponse {
    pub action: String,
    pub app_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntelligenceSessionResponse {
    pub id: String,
    pub session_type: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_secs: Option<i64>,
    pub dominant_category: Option<String>,
    pub category_purity: Option<f64>,
    pub quality_score: Option<f64>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub app_breakdown: Option<String>,
    pub context_switches: i64,
    pub distraction_count: i64,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityTimelineResponse {
    pub app_name: String,
    pub window_title: Option<String>,
    pub site_name: Option<String>,
    pub category_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_secs: Option<i64>,
    pub is_idle: bool,
    pub project_id: Option<String>,
    pub focus_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityCategoryResponse {
    pub id: String,
    pub name: String,
    pub category_type: String,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub is_system: bool,
    pub rules: Option<CategoryRulesResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRulesResponse {
    pub app_names: Vec<String>,
    pub bundle_ids: Vec<String>,
    pub url_patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoalProgressResponse {
    pub id: i64,
    pub goal_type: String,
    pub metric: String,
    pub target_value: f64,
    pub current_value: f64,
    pub met: bool,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntryResponse {
    pub id: i64,
    pub description: String,
    pub category_id: Option<String>,
    pub project_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub duration_secs: i64,
    pub source: String,
}

// ── Insight Cards ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightCardResponse {
    pub id: String,
    pub insight_type: String,
    pub title: String,
    pub body: String,
    pub sentiment: String,
    pub metric_value: Option<f64>,
    pub baseline_value: Option<f64>,
    pub date: String,
    pub dismissed: bool,
    pub generated_at: DateTime<Utc>,
}

// ── Weekly Assessment ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeeklyAssessmentResponse {
    pub id: String,
    pub week_start: String,
    pub week_end: String,
    pub avg_score: Option<f64>,
    pub total_focus_mins: Option<i64>,
    pub total_productive_secs: Option<i64>,
    pub total_distracting_secs: Option<i64>,
    pub top_apps: Option<String>,
    pub summary: Option<String>,
}

// ── Productivity Patterns ────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityPatternsResponse {
    pub peak_focus_hours: Vec<u32>,
    pub avg_session_mins: f64,
    pub productive_ratio: f64,
    pub avg_context_switches: f64,
    pub best_day_of_week: Option<String>,
    pub days_analyzed: usize,
}

// ── Hourly Breakdown ─────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlyBreakdownResponse {
    pub hour: u32,
    pub productive_secs: i64,
    pub neutral_secs: i64,
    pub distracting_secs: i64,
    pub idle_secs: i64,
    pub total_secs: i64,
    pub productive_ratio: f64,
}

// ── Distraction ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearnedRuleResponse {
    pub id: i64,
    pub pattern: String,
    pub pattern_type: String,
    pub classification: String,
    pub confidence: f64,
    pub hit_count: i64,
    pub last_used_at: String,
    pub created_at: String,
}
