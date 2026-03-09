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
    state
        .productivity_pomodoro_start(work_mins, break_mins)
        .await
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

// ── Focus Timer (tray-driven) ──────────────────────────────────────────

use crate::focus_timer::{FocusTimer, TimerMode};
use desktop_shared::commands::FocusTimerStatusResponse;

#[allow(clippy::too_many_arguments)]
#[tauri::command(rename_all = "snake_case")]
pub async fn focus_timer_start(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    mode: String,
    work_mins: u64,
    break_mins: Option<u64>,
    action_id: Option<String>,
    action_title: Option<String>,
) -> Result<FocusSessionResponse, ApiError> {
    let timer_mode = match mode.as_str() {
        "pomodoro" => TimerMode::Pomodoro,
        _ => TimerMode::Focus,
    };

    // Start the persistent session first
    let session = if timer_mode == TimerMode::Pomodoro {
        state
            .productivity_pomodoro_start_with_action(
                action_id,
                None,
                Some(work_mins as i64),
                break_mins.map(|b| b as i64),
            )
            .await?
    } else {
        state
            .productivity_focus_start(action_id, None, Some(work_mins as i64))
            .await?
    };

    // Then start the desktop timer (tray title + countdown)
    timer
        .start(app, timer_mode, work_mins, break_mins, action_title)
        .await
        .map_err(|e| ApiError::new("TIMER_ERROR", e.to_string()))?;

    Ok(session)
}

#[tauri::command]
pub async fn focus_timer_stop(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    notes: Option<String>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    timer.stop(&app).await;
    // During a break there's no active focus session — don't error in that case
    Ok(state.productivity_focus_end(notes).await.unwrap_or(None))
}

#[tauri::command]
pub async fn focus_timer_status(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<FocusTimerStatusResponse, ApiError> {
    let session = state.productivity_focus_status().await?;
    let timer_info = timer.status().await;

    Ok(FocusTimerStatusResponse {
        active: timer_info.is_some(),
        mode: timer_info.map(|(m, _)| m.as_str().to_string()),
        remaining_secs: None, // Remaining is pushed via events, not polled
        total_secs: timer_info.map(|(_, t)| t),
        session,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn focus_break_start(
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    break_mins: u64,
) -> Result<(), ApiError> {
    timer
        .start_break(app, break_mins)
        .await
        .map_err(|e| ApiError::new("TIMER_ERROR", e.to_string()))
}

#[tauri::command(rename_all = "snake_case")]
pub async fn focus_timer_extend(
    timer: State<'_, Arc<FocusTimer>>,
    extra_secs: u64,
) -> Result<bool, ApiError> {
    Ok(timer.extend(extra_secs).await)
}

#[tauri::command]
pub async fn focus_timer_pause(timer: State<'_, Arc<FocusTimer>>) -> Result<bool, ApiError> {
    Ok(timer.pause().await)
}

#[tauri::command]
pub async fn focus_timer_resume(timer: State<'_, Arc<FocusTimer>>) -> Result<bool, ApiError> {
    Ok(timer.resume().await)
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "productivity_today",
    "productivity_timeline",
    "productivity_focus_start",
    "productivity_focus_end",
    "productivity_focus_status",
    "productivity_sessions",
    "productivity_weekly",
    "productivity_categories",
    "productivity_summary_range",
    "productivity_activity_feed",
    "productivity_goals",
    "productivity_pomodoro_start",
    "productivity_time_entries",
    "productivity_goal_create",
    "productivity_goal_delete",
    "productivity_goal_toggle",
    "productivity_time_entry_create",
    "productivity_time_entry_delete",
    "productivity_category_upsert",
    "productivity_insights",
    "productivity_insight_dismiss",
    "productivity_auto_focus_confirm",
    "productivity_projects_list",
    "productivity_project_upsert",
    "productivity_project_delete",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "productivity_today" => dev::val(core.productivity_today().await),
        "productivity_timeline" => {
            let date = try_field!(dev::get_str(body, "date"));
            dev::val(
                core.productivity_timeline(date, dev::get(body, "limit"), dev::get(body, "offset"))
                    .await,
            )
        }
        "productivity_focus_start" => dev::val(
            core.productivity_focus_start(
                dev::get(body, "action_id"),
                dev::get(body, "project_id"),
                dev::get(body, "target_mins"),
            )
            .await,
        ),
        "productivity_focus_end" => {
            dev::val(core.productivity_focus_end(dev::get(body, "notes")).await)
        }
        "productivity_focus_status" => dev::val(core.productivity_focus_status().await),
        "productivity_sessions" => {
            let date = try_field!(dev::get_str(body, "date"));
            dev::val(core.productivity_sessions(date).await)
        }
        "productivity_weekly" => dev::val(core.productivity_weekly().await),
        "productivity_categories" => dev::val(core.productivity_categories().await),
        "productivity_summary_range" => {
            let start_date = try_field!(dev::get_str(body, "startDate"));
            let end_date = try_field!(dev::get_str(body, "endDate"));
            dev::val(core.productivity_summary_range(start_date, end_date).await)
        }
        "productivity_activity_feed" => dev::val(
            core.productivity_activity_feed(dev::get(body, "limit"))
                .await,
        ),
        "productivity_goals" => dev::val(core.productivity_goals().await),
        "productivity_pomodoro_start" => dev::val(
            core.productivity_pomodoro_start(
                dev::get(body, "work_mins"),
                dev::get(body, "break_mins"),
            )
            .await,
        ),
        "productivity_time_entries" => {
            let date = try_field!(dev::get_str(body, "date"));
            dev::val(core.productivity_time_entries(date).await)
        }
        "productivity_goal_create" => {
            let goal_type = try_field!(dev::get_str(body, "goal_type"));
            let metric = try_field!(dev::get_str(body, "metric"));
            let target_value: f64 = try_field!(dev::require(body, "target_value"));
            dev::val(
                core.productivity_goal_create(goal_type, metric, target_value)
                    .await,
            )
        }
        "productivity_goal_delete" => {
            let id: i64 = try_field!(dev::require(body, "id"));
            dev::val(core.productivity_goal_delete(id).await)
        }
        "productivity_goal_toggle" => {
            let id: i64 = try_field!(dev::require(body, "id"));
            let enabled: bool = try_field!(dev::require(body, "enabled"));
            dev::val(core.productivity_goal_toggle(id, enabled).await)
        }
        "productivity_time_entry_create" => {
            let description = try_field!(dev::get_str(body, "description"));
            let duration_mins: i64 = try_field!(dev::require(body, "duration_mins"));
            dev::val(
                core.productivity_time_entry_create(
                    description,
                    duration_mins,
                    dev::get(body, "category_id"),
                    dev::get(body, "project_id"),
                )
                .await,
            )
        }
        "productivity_time_entry_delete" => {
            let id: i64 = try_field!(dev::require(body, "id"));
            dev::val(core.productivity_time_entry_delete(id).await)
        }
        "productivity_category_upsert" => {
            let id = try_field!(dev::get_str(body, "id"));
            let name = try_field!(dev::get_str(body, "name"));
            let category_type = try_field!(dev::get_str(body, "category_type"));
            dev::val(
                core.productivity_category_upsert(
                    id,
                    name,
                    category_type,
                    dev::get(body, "color"),
                    dev::get(body, "icon"),
                )
                .await,
            )
        }
        "productivity_insights" => {
            dev::val(core.productivity_insights(dev::get(body, "date")).await)
        }
        "productivity_insight_dismiss" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.productivity_insight_dismiss(id).await)
        }
        "productivity_auto_focus_confirm" => dev::val(
            core.productivity_auto_focus_confirm(try_field!(dev::parse_params(body)))
                .await,
        ),
        "productivity_projects_list" => dev::val(core.productivity_projects_list().await),
        "productivity_project_upsert" => {
            let id = try_field!(dev::get_str(body, "id"));
            let display_name = try_field!(dev::get_str(body, "display_name"));
            let path = try_field!(dev::get_str(body, "path"));
            dev::val(
                core.productivity_project_upsert(
                    id,
                    display_name,
                    path,
                    dev::get(body, "url_patterns"),
                    dev::get(body, "color"),
                )
                .await,
            )
        }
        "productivity_project_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.productivity_project_delete(id).await)
        }
        _ => return None,
    })
}
