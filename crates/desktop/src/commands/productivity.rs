//! Productivity IPC commands — daily summaries, focus sessions, activity timeline.

use std::sync::Arc;

use chrono::Utc;
use desktop_shared::commands::{
    ActivityCategoryResponse, ActivityTimelineResponse, AppUsageResponse, CategoryUsageResponse,
    FocusSessionResponse, GoalProgressResponse, ProductivitySummaryResponse, TimeEntryResponse,
};
use desktop_shared::errors::ApiError;
use feature_productivity::types::{DailySummary, FocusSession};
use tauri::State;

use crate::app_core::AppCore;

// ── Helpers ────────────────────────────────────────────────────────────

use super::map_prod_err;

fn summary_to_response(s: DailySummary) -> ProductivitySummaryResponse {
    ProductivitySummaryResponse {
        date: s.date,
        total_active_secs: s.total_active_secs,
        total_focus_secs: s.total_focus_secs,
        total_break_secs: s.total_break_secs,
        total_idle_secs: s.total_idle_secs,
        productive_secs: s.productive_secs,
        neutral_secs: s.neutral_secs,
        distracting_secs: s.distracting_secs,
        focus_sessions_count: s.focus_sessions_count,
        avg_session_quality: s.avg_session_quality,
        interruptions_count: s.interruptions_count,
        context_switches: s.context_switches,
        top_apps: s
            .top_apps
            .into_iter()
            .map(|a| AppUsageResponse {
                app_name: a.app_name,
                duration_secs: a.duration_secs,
                category: a.category,
            })
            .collect(),
        top_categories: s
            .top_categories
            .into_iter()
            .map(|c| CategoryUsageResponse {
                category: c.category,
                duration_secs: c.duration_secs,
            })
            .collect(),
        ai_summary: s.ai_summary,
        productivity_score: s.productivity_score,
    }
}

fn session_to_response(s: FocusSession) -> FocusSessionResponse {
    FocusSessionResponse {
        id: s.id,
        action_id: s.action_id,
        project_id: s.project_id,
        session_type: s.session_type.to_string(),
        target_mins: s.target_mins,
        started_at: s.started_at,
        ended_at: s.ended_at,
        actual_mins: s.actual_mins,
        interruptions: s.interruptions,
        quality_score: s.quality_score,
        completed: s.completed,
        notes: s.notes,
    }
}

// ── Commands ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn productivity_today(
    state: State<'_, Arc<AppCore>>,
) -> Result<Option<ProductivitySummaryResponse>, ApiError> {
    let aggregator = state.aggregator()?;
    let summary = aggregator.compute_today().await.map_err(map_prod_err)?;
    Ok(Some(summary_to_response(summary)))
}

#[tauri::command]
pub async fn productivity_timeline(
    state: State<'_, Arc<AppCore>>,
    date: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<ActivityTimelineResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let start = super::parse_date_or_err(&date)?;
    let end = start + chrono::Duration::days(1);
    let cap = limit.unwrap_or(10_000).min(10_000);
    let events = repos
        .events
        .list_range_offset(&start, &end, Some(cap), offset)
        .await
        .map_err(map_prod_err)?;
    Ok(events
        .into_iter()
        .map(|e| ActivityTimelineResponse {
            app_name: e.app_name,
            window_title: e.window_title,
            site_name: e.site_name,
            category_id: e.category_id,
            started_at: e.started_at,
            duration_secs: e.duration_secs,
            is_idle: e.is_idle,
        })
        .collect())
}

#[tauri::command]
pub async fn productivity_focus_start(
    state: State<'_, Arc<AppCore>>,
    action_id: Option<String>,
    project_id: Option<String>,
    target_mins: Option<i64>,
) -> Result<FocusSessionResponse, ApiError> {
    let focus_mgr = state.focus_manager()?;
    let session = focus_mgr
        .start_session(action_id, project_id, target_mins)
        .await
        .map_err(map_prod_err)?;
    Ok(session_to_response(session))
}

#[tauri::command]
pub async fn productivity_focus_end(
    state: State<'_, Arc<AppCore>>,
    notes: Option<String>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    let focus_mgr = state.focus_manager()?;
    let session = focus_mgr.end_session(notes).await.map_err(map_prod_err)?;

    // Clear interceptor session state (whitelist + temp passes)
    if let Ok(interceptor) = state.distraction_interceptor() {
        let mut guard = interceptor.lock().await;
        guard.reset_session();
    }

    Ok(session.map(session_to_response))
}

#[tauri::command]
pub async fn productivity_focus_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    let focus_mgr = state.focus_manager()?;
    let session = focus_mgr.get_active().await.map_err(map_prod_err)?;
    Ok(session.map(session_to_response))
}

#[tauri::command]
pub async fn productivity_sessions(
    state: State<'_, Arc<AppCore>>,
    date: String,
) -> Result<Vec<FocusSessionResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let start = super::parse_date_or_err(&date)?;
    let end = start + chrono::Duration::days(1);
    let sessions = repos
        .sessions
        .list_range(&start, &end, None)
        .await
        .map_err(map_prod_err)?;
    Ok(sessions.into_iter().map(session_to_response).collect())
}

#[tauri::command]
pub async fn productivity_weekly(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let today = Utc::now().date_naive();
    let week_start = today - chrono::Duration::days(6);
    let summaries = repos
        .summaries
        .list_range(
            &week_start.format("%Y-%m-%d").to_string(),
            &today.format("%Y-%m-%d").to_string(),
        )
        .await
        .map_err(map_prod_err)?;
    Ok(summaries.into_iter().map(summary_to_response).collect())
}

#[tauri::command]
pub async fn productivity_categories(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ActivityCategoryResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let categories = repos.categories.list_all().await.map_err(map_prod_err)?;
    Ok(categories
        .into_iter()
        .map(|c| ActivityCategoryResponse {
            id: c.id,
            name: c.name,
            category_type: c.category_type.to_string(),
            color: c.color,
            icon: c.icon,
            is_system: c.is_system,
        })
        .collect())
}

#[tauri::command]
pub async fn productivity_summary_range(
    state: State<'_, Arc<AppCore>>,
    start_date: String,
    end_date: String,
) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let mut summaries = repos
        .summaries
        .list_range(&start_date, &end_date)
        .await
        .map_err(map_prod_err)?;

    // Include today's live-computed summary if today falls within the range
    let today = Utc::now().format("%Y-%m-%d").to_string();
    if today >= start_date && today <= end_date {
        let has_today = summaries.iter().any(|s| s.date == today);
        if let Ok(aggregator) = state.aggregator() {
            if let Ok(live) = aggregator.compute_today().await {
                if has_today {
                    // Replace stored (possibly stale) with live data
                    summaries.retain(|s| s.date != today);
                }
                summaries.push(live);
                summaries.sort_by(|a, b| a.date.cmp(&b.date));
            }
        }
    }

    Ok(summaries.into_iter().map(summary_to_response).collect())
}

#[tauri::command]
pub async fn productivity_activity_feed(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<ActivityTimelineResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let now = Utc::now();
    let start = now - chrono::Duration::hours(24);
    let cap = limit.unwrap_or(50).min(200);
    let events = repos
        .events
        .list_range_offset(&start, &now, Some(cap), None)
        .await
        .map_err(map_prod_err)?;
    Ok(events
        .into_iter()
        .rev()
        .map(|e| ActivityTimelineResponse {
            app_name: e.app_name,
            window_title: e.window_title,
            site_name: e.site_name,
            category_id: e.category_id,
            started_at: e.started_at,
            duration_secs: e.duration_secs,
            is_idle: e.is_idle,
        })
        .collect())
}

#[tauri::command]
pub async fn productivity_goals(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<GoalProgressResponse>, ApiError> {
    let aggregator = state.aggregator()?;
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let results = aggregator.check_goals(&today).await.map_err(map_prod_err)?;
    Ok(results
        .into_iter()
        .map(|(goal, current, met)| GoalProgressResponse {
            id: goal.id.unwrap_or(0),
            goal_type: goal.goal_type.to_string(),
            metric: goal.metric.to_string(),
            target_value: goal.target_value,
            current_value: current,
            met,
        })
        .collect())
}

#[tauri::command]
pub async fn productivity_pomodoro_start(
    state: State<'_, Arc<AppCore>>,
    work_mins: Option<i64>,
    break_mins: Option<i64>,
) -> Result<FocusSessionResponse, ApiError> {
    let focus_mgr = state.focus_manager()?;
    let session = focus_mgr
        .start_pomodoro(None, None, work_mins, break_mins)
        .await
        .map_err(map_prod_err)?;
    Ok(session_to_response(session))
}

#[tauri::command]
pub async fn productivity_time_entries(
    state: State<'_, Arc<AppCore>>,
    date: String,
) -> Result<Vec<TimeEntryResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let start = super::parse_date_or_err(&date)?;
    let end = start + chrono::Duration::days(1);
    let entries = repos
        .time_entries
        .list_range(&start, &end)
        .await
        .map_err(map_prod_err)?;
    Ok(entries
        .into_iter()
        .map(|e| TimeEntryResponse {
            id: e.id.unwrap_or(0),
            description: e.description,
            category_id: e.category_id,
            project_id: e.project_id,
            started_at: e.started_at,
            duration_secs: e.duration_secs,
            source: e.source,
        })
        .collect())
}
