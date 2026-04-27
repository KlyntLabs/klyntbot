//! Productivity IPC commands — thin Tauri delegates to `AppCore`.

use std::sync::Arc;

use desktop_shared::commands::{
    ActivityCategoryResponse, ActivityTimelineResponse, CategoryRulesResponse, DistractionResponse,
    FocusSessionResponse, GoalProgressResponse, HourlyBreakdownResponse, InsightCardResponse,
    IntelligenceSessionResponse, ProductivityPatternsResponse, ProductivityProjectResponse,
    ProductivitySummaryResponse, TimeEntryResponse, TrackedAppResponse, WeeklyAssessmentResponse,
};
use desktop_shared::errors::ApiError;
use desktop_shared::events::AutoFocusPayload;
use feature_productivity::auto_focus::AutoFocusEvent;
use tauri::State;

use crate::app_core::AppCore;
use crate::focus_timer::FocusTimer;

#[tauri::command]
#[specta::specta]
pub async fn productivity_today(
    state: State<'_, Arc<AppCore>>,
) -> Result<Option<ProductivitySummaryResponse>, ApiError> {
    state.productivity_today().await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_timeline(
    state: State<'_, Arc<AppCore>>,
    date: String,
    limit: Option<i64>,
    offset: Option<i64>,
    tz_offset_mins: Option<i32>,
) -> Result<Vec<ActivityTimelineResponse>, ApiError> {
    state
        .productivity_timeline(date, limit, offset, tz_offset_mins)
        .await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
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
#[specta::specta]
pub async fn productivity_focus_end(
    state: State<'_, Arc<AppCore>>,
    notes: Option<String>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    state.productivity_focus_end(notes).await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_focus_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    state.productivity_focus_status().await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_sessions(
    state: State<'_, Arc<AppCore>>,
    date: String,
) -> Result<Vec<FocusSessionResponse>, ApiError> {
    state.productivity_sessions(date).await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_intelligence_sessions(
    state: State<'_, Arc<AppCore>>,
    date: String,
    tz_offset_mins: Option<i32>,
) -> Result<Vec<IntelligenceSessionResponse>, ApiError> {
    state
        .productivity_intelligence_sessions(date, tz_offset_mins)
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_weekly(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
    state.productivity_weekly().await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_categories(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ActivityCategoryResponse>, ApiError> {
    state.productivity_categories().await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_tracked_apps(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<TrackedAppResponse>, ApiError> {
    state.productivity_tracked_apps().await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_summary_range(
    state: State<'_, Arc<AppCore>>,
    start_date: String,
    end_date: String,
) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
    state.productivity_summary_range(start_date, end_date).await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_activity_feed(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<ActivityTimelineResponse>, ApiError> {
    state.productivity_activity_feed(limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_goals(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<GoalProgressResponse>, ApiError> {
    state.productivity_goals().await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
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
#[specta::specta]
pub async fn productivity_time_entries(
    state: State<'_, Arc<AppCore>>,
    date: String,
) -> Result<Vec<TimeEntryResponse>, ApiError> {
    state.productivity_time_entries(date).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
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
#[specta::specta]
pub async fn productivity_goal_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    state.productivity_goal_delete(id).await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_goal_toggle(
    state: State<'_, Arc<AppCore>>,
    id: i64,
    enabled: bool,
) -> Result<(), ApiError> {
    state.productivity_goal_toggle(id, enabled).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
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
#[specta::specta]
pub async fn productivity_time_entry_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    state.productivity_time_entry_delete(id).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_category_upsert(
    state: State<'_, Arc<AppCore>>,
    id: String,
    name: String,
    category_type: String,
    color: Option<String>,
    icon: Option<String>,
    rules: Option<CategoryRulesResponse>,
) -> Result<ActivityCategoryResponse, ApiError> {
    state
        .productivity_category_upsert(id, name, category_type, color, icon, rules)
        .await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_category_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<bool, ApiError> {
    state.productivity_category_delete(id).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_recategorize_app(
    state: State<'_, Arc<AppCore>>,
    app_name: String,
    site_name: Option<String>,
    new_category_id: String,
) -> Result<u64, ApiError> {
    state
        .productivity_recategorize_app(app_name, site_name, new_category_id)
        .await
}

// ── V2: Insights & Auto-Focus ─────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn productivity_insights(
    state: State<'_, Arc<AppCore>>,
    date: Option<String>,
) -> Result<Vec<InsightCardResponse>, ApiError> {
    state.productivity_insights(date).await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_insight_dismiss(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<(), ApiError> {
    state.productivity_insight_dismiss(id).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_auto_focus_start(
    state: State<'_, Arc<AppCore>>,
) -> Result<FocusSessionResponse, ApiError> {
    state.productivity_auto_focus_start().await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_auto_focus_end(
    state: State<'_, Arc<AppCore>>,
    event: AutoFocusEvent,
) -> Result<FocusSessionResponse, ApiError> {
    state.productivity_auto_focus_end(event).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_auto_focus_confirm(
    state: State<'_, Arc<AppCore>>,
    payload: AutoFocusPayload,
) -> Result<FocusSessionResponse, ApiError> {
    state.productivity_auto_focus_confirm(payload).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn distraction_respond(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    response: DistractionResponse,
) -> Result<(), ApiError> {
    let app_name = response.app_name.unwrap_or_default();
    match response.action.as_str() {
        "back_to_work" => {
            state.distraction_dismiss(app_name).await?;
        }
        "not_distraction" => {
            state
                .distraction_allow_session(app_name, None, "productive".to_string())
                .await?;
        }
        "snooze" => {
            state.distraction_allow_temp(app_name).await?;
        }
        "end_focus" => {
            timer.stop(&app).await;
            let _ = state.productivity_focus_end(None).await;
        }
        other => {
            tracing::warn!("unknown distraction_respond action: {other}");
        }
    }
    Ok(())
}

// ── V3: Project Tracking ─────────────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn productivity_projects_list(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ProductivityProjectResponse>, ApiError> {
    state.productivity_projects_list().await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
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
#[specta::specta]
pub async fn productivity_project_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<(), ApiError> {
    state.productivity_project_delete(id).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_weekly_assessment(
    state: State<'_, Arc<AppCore>>,
    week_start: String,
) -> Result<WeeklyAssessmentResponse, ApiError> {
    state.productivity_weekly_assessment(week_start).await
}

#[tauri::command]
#[specta::specta]
pub async fn productivity_calendar_events(
    state: State<'_, Arc<AppCore>>,
    date: String,
) -> Result<Vec<feature_productivity::types::CalendarEvent>, ApiError> {
    state.productivity_calendar_events(date).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn calendar_sync_events(
    state: State<'_, Arc<AppCore>>,
    events: Vec<desktop_shared::commands::CalendarEventInput>,
) -> Result<Vec<feature_productivity::types::CalendarEvent>, ApiError> {
    state.calendar_sync_events(events).await
}

// ── Focus Session (tray-driven) ──────────────────────────────────────

use crate::focus_timer::{FocusSessionConfig, SessionCommand};
use desktop_shared::commands::FocusSessionStatusResponse;

#[allow(clippy::too_many_arguments)]
#[tauri::command(rename_all = "snake_case")]
pub async fn focus_session_start(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    work_secs: u64,
    short_break_secs: u64,
    long_break_secs: u64,
    long_break_after: u32,
    action_id: Option<String>,
    action_title: Option<String>,
    dnd_enabled: Option<bool>,
    sound_enabled: Option<bool>,
    notification_enabled: Option<bool>,
) -> Result<FocusSessionResponse, ApiError> {
    let config = FocusSessionConfig {
        work_secs,
        short_break_secs,
        long_break_secs,
        long_break_after,
    };

    // Clean up any orphaned DB sessions (from app restart or stale timer)
    let _ = state.productivity_focus_end(None).await;
    let _ = state.productivity_break_end().await;

    // Start persistent session in AppCore
    let session = state
        .productivity_focus_start(action_id.clone(), None, Some(work_secs as i64 / 60))
        .await?;

    // Start the desktop timer (phase state machine)
    timer
        .start(
            app,
            config,
            action_id,
            action_title,
            dnd_enabled.unwrap_or(false),
            sound_enabled.unwrap_or(true),
            notification_enabled.unwrap_or(true),
        )
        .await
        .map_err(|e| ApiError::new("TIMER_ERROR", e.to_string()))?;

    Ok(session)
}

#[tauri::command]
#[specta::specta]
pub async fn focus_session_stop(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    notes: Option<String>,
) -> Result<Option<FocusSessionResponse>, ApiError> {
    timer.stop(&app).await;
    // End whichever session is active (focus or break)
    let focus_result = state.productivity_focus_end(notes).await.unwrap_or(None);
    if focus_result.is_some() {
        return Ok(focus_result);
    }
    Ok(state.productivity_break_end().await.unwrap_or(None))
}

#[tauri::command]
#[specta::specta]
pub async fn focus_session_status(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<FocusSessionStatusResponse, ApiError> {
    let session = state.productivity_focus_status().await?;
    let config = timer.status().await;

    Ok(FocusSessionStatusResponse {
        active: config.is_some(),
        sync: None, // Sync is pushed via events, not polled
        session,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn focus_session_pause(timer: State<'_, Arc<FocusTimer>>) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::Pause).await)
}

#[tauri::command]
#[specta::specta]
pub async fn focus_session_resume(timer: State<'_, Arc<FocusTimer>>) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::Resume).await)
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn focus_session_extend(
    timer: State<'_, Arc<FocusTimer>>,
    extra_secs: u64,
) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::Extend(extra_secs)).await)
}

#[tauri::command]
#[specta::specta]
pub async fn focus_session_start_break(
    timer: State<'_, Arc<FocusTimer>>,
) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::StartBreak).await)
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn focus_session_extend_work(
    timer: State<'_, Arc<FocusTimer>>,
    extra_mins: u64,
) -> Result<bool, ApiError> {
    Ok(timer
        .send_command(SessionCommand::ExtendWork(extra_mins * 60))
        .await)
}

#[tauri::command]
#[specta::specta]
pub async fn focus_session_skip_break(timer: State<'_, Arc<FocusTimer>>) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::SkipBreak).await)
}

#[tauri::command]
#[specta::specta]
pub async fn focus_session_take_break(timer: State<'_, Arc<FocusTimer>>) -> Result<bool, ApiError> {
    Ok(timer.send_command(SessionCommand::TakeBreak).await)
}

// ── Patterns & Hourly Breakdown ─────────────────────────────────────

#[tauri::command]
#[specta::specta]
pub async fn productivity_patterns(
    state: State<'_, Arc<AppCore>>,
    days: Option<u32>,
) -> Result<ProductivityPatternsResponse, ApiError> {
    state.productivity_patterns(days).await
}

#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_hourly_breakdown(
    state: State<'_, Arc<AppCore>>,
    start_date: String,
    end_date: String,
    tz_offset_mins: Option<i32>,
) -> Result<Vec<HourlyBreakdownResponse>, ApiError> {
    state
        .productivity_hourly_breakdown(start_date, end_date, tz_offset_mins)
        .await
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
    "productivity_intelligence_sessions",
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
    "productivity_tracked_apps",
    "productivity_category_delete",
    "productivity_recategorize_app",
    "productivity_insights",
    "productivity_insight_dismiss",
    "productivity_projects_list",
    "productivity_project_upsert",
    "productivity_project_delete",
    "productivity_weekly_assessment",
    "productivity_calendar_events",
    "calendar_sync_events",
    "productivity_auto_focus_start",
    "productivity_auto_focus_end",
    "productivity_auto_focus_confirm",
    "distraction_respond",
    "productivity_patterns",
    "productivity_hourly_breakdown",
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
                core.productivity_timeline(
                    date,
                    dev::get(body, "limit"),
                    dev::get(body, "offset"),
                    dev::get(body, "tzOffsetMins"),
                )
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
        "productivity_intelligence_sessions" => {
            let date = try_field!(dev::get_str(body, "date"));
            dev::val(
                core.productivity_intelligence_sessions(date, dev::get(body, "tzOffsetMins"))
                    .await,
            )
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
                    dev::get(body, "rules"),
                )
                .await,
            )
        }
        "productivity_tracked_apps" => dev::val(core.productivity_tracked_apps().await),
        "productivity_category_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.productivity_category_delete(id).await)
        }
        "productivity_recategorize_app" => {
            let app_name = try_field!(dev::get_str(body, "app_name"));
            let site_name: Option<String> = dev::get(body, "site_name");
            let new_category_id = try_field!(dev::get_str(body, "new_category_id"));
            dev::val(
                core.productivity_recategorize_app(app_name, site_name, new_category_id)
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
        "productivity_weekly_assessment" => {
            let week_start = try_field!(dev::get_str(body, "week_start"));
            dev::val(core.productivity_weekly_assessment(week_start).await)
        }
        "productivity_calendar_events" => {
            let date = try_field!(dev::get_str(body, "date"));
            dev::val(core.productivity_calendar_events(date).await)
        }
        "calendar_sync_events" => dev::val(
            core.calendar_sync_events(try_field!(dev::parse_params(body)))
                .await,
        ),
        "productivity_auto_focus_start" => dev::val(core.productivity_auto_focus_start().await),
        "productivity_auto_focus_end" => dev::val(
            core.productivity_auto_focus_end(try_field!(dev::parse_params(body)))
                .await,
        ),
        "productivity_auto_focus_confirm" => dev::val(
            core.productivity_auto_focus_confirm(try_field!(dev::parse_params(body)))
                .await,
        ),
        "distraction_respond" => {
            let response: DistractionResponse = try_field!(dev::parse_params(body));
            let app_name = response.app_name.unwrap_or_default();
            match response.action.as_str() {
                "back_to_work" => dev::val(core.distraction_dismiss(app_name).await),
                "not_distraction" => dev::val(
                    core.distraction_allow_session(app_name, None, "productive".to_string())
                        .await,
                ),
                "snooze" => dev::val(core.distraction_allow_temp(app_name).await),
                "end_focus" => dev::val(core.productivity_focus_end(None).await.map(|_| ())),
                _ => Ok(serde_json::Value::Null),
            }
        }
        "productivity_patterns" => {
            dev::val(core.productivity_patterns(dev::get(body, "days")).await)
        }
        "productivity_hourly_breakdown" => {
            let start_date = try_field!(dev::get_str(body, "start_date"));
            let end_date = try_field!(dev::get_str(body, "end_date"));
            dev::val(
                core.productivity_hourly_breakdown(
                    start_date,
                    end_date,
                    dev::get(body, "tz_offset_mins"),
                )
                .await,
            )
        }
        _ => return None,
    })
}
