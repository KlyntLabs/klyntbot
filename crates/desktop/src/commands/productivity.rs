//! Productivity IPC commands — thin Tauri delegates to `AppCore`.

use std::sync::Arc;

use desktop_shared::commands::{
    ActivityCategoryResponse, ActivityTimelineResponse, FocusSessionResponse, GoalProgressResponse,
    InsightCardResponse, ProductivityProjectResponse, ProductivitySummaryResponse,
    TimeEntryResponse,
};
use desktop_shared::errors::ApiError;
use feature_productivity::auto_focus::AutoFocusSession;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn productivity_today(
    state: State<'_, Arc<AppCore>>,
) -> Result<Option<ProductivitySummaryResponse>, ApiError> {
    state.productivity_today().await
}

#[tauri::command]
pub async fn productivity_timeline(
    state: State<'_, Arc<AppCore>>,
    date: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<ActivityTimelineResponse>, ApiError> {
    state.productivity_timeline(date, limit, offset).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_focus_start(
    state: State<'_, Arc<AppCore>>,
    action_id: Option<String>,
    project_id: Option<String>,
    target_mins: Option<i64>,
) -> Result<FocusSessionResponse, ApiError> {
    state
        .productivity_focus_start(action_id, project_id, target_mins)
        .await
}

#[tauri::command]
pub async fn productivity_focus_end(
    state: State<'_, Arc<AppCore>>,
    notes: Option<String>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    state.productivity_focus_end(notes).await
}

#[tauri::command]
pub async fn productivity_focus_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    state.productivity_focus_status().await
}

#[tauri::command]
pub async fn productivity_sessions(
    state: State<'_, Arc<AppCore>>,
    date: String,
) -> Result<Vec<FocusSessionResponse>, ApiError> {
    state.productivity_sessions(date).await
}

#[tauri::command]
pub async fn productivity_weekly(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
    state.productivity_weekly().await
}

#[tauri::command]
pub async fn productivity_categories(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ActivityCategoryResponse>, ApiError> {
    state.productivity_categories().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_summary_range(
    state: State<'_, Arc<AppCore>>,
    start_date: String,
    end_date: String,
) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
    state.productivity_summary_range(start_date, end_date).await
}

#[tauri::command]
pub async fn productivity_activity_feed(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<ActivityTimelineResponse>, ApiError> {
    state.productivity_activity_feed(limit).await
}

#[tauri::command]
pub async fn productivity_goals(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<GoalProgressResponse>, ApiError> {
    state.productivity_goals().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_pomodoro_start(
    state: State<'_, Arc<AppCore>>,
    work_mins: Option<i64>,
    break_mins: Option<i64>,
) -> Result<FocusSessionResponse, ApiError> {
    state.productivity_pomodoro_start(work_mins, break_mins).await
}

#[tauri::command]
pub async fn productivity_time_entries(
    state: State<'_, Arc<AppCore>>,
    date: String,
) -> Result<Vec<TimeEntryResponse>, ApiError> {
    state.productivity_time_entries(date).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_goal_create(
    state: State<'_, Arc<AppCore>>,
    goal_type: String,
    metric: String,
    target_value: f64,
) -> Result<GoalProgressResponse, ApiError> {
    state
        .productivity_goal_create(goal_type, metric, target_value)
        .await
}

#[tauri::command]
pub async fn productivity_goal_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    state.productivity_goal_delete(id).await
}

#[tauri::command]
pub async fn productivity_goal_toggle(
    state: State<'_, Arc<AppCore>>,
    id: i64,
    enabled: bool,
) -> Result<(), ApiError> {
    state.productivity_goal_toggle(id, enabled).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_time_entry_create(
    state: State<'_, Arc<AppCore>>,
    description: String,
    duration_mins: i64,
    category_id: Option<String>,
    project_id: Option<String>,
) -> Result<TimeEntryResponse, ApiError> {
    state
        .productivity_time_entry_create(description, duration_mins, category_id, project_id)
        .await
}

#[tauri::command]
pub async fn productivity_time_entry_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    state.productivity_time_entry_delete(id).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_category_upsert(
    state: State<'_, Arc<AppCore>>,
    id: String,
    name: String,
    category_type: String,
    color: Option<String>,
    icon: Option<String>,
) -> Result<ActivityCategoryResponse, ApiError> {
    state
        .productivity_category_upsert(id, name, category_type, color, icon)
        .await
}

// ── V2: Insights & Auto-Focus ─────────────────────────────────────────

#[tauri::command]
pub async fn productivity_insights(
    state: State<'_, Arc<AppCore>>,
    date: Option<String>,
) -> Result<Vec<InsightCardResponse>, ApiError> {
    state.productivity_insights(date).await
}

#[tauri::command]
pub async fn productivity_insight_dismiss(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<(), ApiError> {
    state.productivity_insight_dismiss(id).await
}

#[tauri::command]
pub async fn productivity_auto_focus_confirm(
    state: State<'_, Arc<AppCore>>,
    session: AutoFocusSession,
) -> Result<FocusSessionResponse, ApiError> {
    state.productivity_auto_focus_confirm(session).await
}

// ── V3: Project Tracking ─────────────────────────────────────────────

#[tauri::command]
pub async fn productivity_projects_list(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ProductivityProjectResponse>, ApiError> {
    state.productivity_projects_list().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_project_upsert(
    state: State<'_, Arc<AppCore>>,
    id: String,
    display_name: String,
    path: String,
    url_patterns: Option<Vec<String>>,
    color: Option<String>,
) -> Result<ProductivityProjectResponse, ApiError> {
    state
        .productivity_project_upsert(id, display_name, path, url_patterns, color)
        .await
}

#[tauri::command]
pub async fn productivity_project_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<(), ApiError> {
    state.productivity_project_delete(id).await
}
