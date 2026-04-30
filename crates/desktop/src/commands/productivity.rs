//! Productivity IPC commands — thin Tauri delegates to `AppCore`.

use std::sync::Arc;

use desktop_macros::{klynt_command, klynt_raw_command};

use desktop_shared::commands::{
    ActivityCategoryResponse, ActivityTimelineResponse, CategoryRulesResponse, DistractionResponse,
    FocusSessionResponse, GoalProgressResponse, HourlyBreakdownResponse, InsightCardResponse,
    IntelligenceSessionResponse, ProductivityPatternsResponse, ProductivityProjectResponse,
    ProductivitySummaryResponse, TimeEntryResponse, TrackedAppResponse, WeeklyAssessmentResponse,
};
use desktop_shared::events::AutoFocusPayload;
use desktop_shared::{errors::ApiError, CommandResult};
use tauri::State;

use crate::app_core::AppCore;
use crate::focus_timer::FocusTimer;

#[klynt_command]
pub async fn productivity_today() -> Option<ProductivitySummaryResponse> {
    state.productivity_today().await
}

#[klynt_command]
pub async fn productivity_timeline(
    date: String,
    limit: Option<i64>,
    offset: Option<i64>,
    tz_offset_mins: Option<i32>,
) -> Vec<ActivityTimelineResponse> {
    state
        .productivity_timeline(date, limit, offset, tz_offset_mins)
        .await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_focus_start(
    state: State<'_, Arc<AppCore>>,
    action_id: Option<String>,
    project_id: Option<String>,
    target_mins: Option<i64>,
) -> CommandResult<FocusSessionResponse> {
    state
        .productivity_focus_start(action_id, project_id, target_mins)
        .await
}

#[klynt_command]
pub async fn productivity_focus_end(notes: Option<String>) -> Option<FocusSessionResponse> {
    state.productivity_focus_end(notes).await
}

#[klynt_command]
pub async fn productivity_focus_status() -> Option<FocusSessionResponse> {
    state.productivity_focus_status().await
}

#[klynt_command]
pub async fn productivity_sessions(date: String) -> Vec<FocusSessionResponse> {
    state.productivity_sessions(date).await
}

#[klynt_command]
pub async fn productivity_intelligence_sessions(
    date: String,
    tz_offset_mins: Option<i32>,
) -> Vec<IntelligenceSessionResponse> {
    state
        .productivity_intelligence_sessions(date, tz_offset_mins)
        .await
}

#[klynt_command]
pub async fn productivity_weekly() -> Vec<ProductivitySummaryResponse> {
    state.productivity_weekly().await
}

#[klynt_command]
pub async fn productivity_categories() -> Vec<ActivityCategoryResponse> {
    state.productivity_categories().await
}

#[klynt_command]
pub async fn productivity_tracked_apps() -> Vec<TrackedAppResponse> {
    state.productivity_tracked_apps().await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_summary_range(
    state: State<'_, Arc<AppCore>>,
    start_date: String,
    end_date: String,
) -> CommandResult<Vec<ProductivitySummaryResponse>> {
    state.productivity_summary_range(start_date, end_date).await
}

#[klynt_command]
pub async fn productivity_activity_feed(limit: Option<i64>) -> Vec<ActivityTimelineResponse> {
    state.productivity_activity_feed(limit).await
}

#[klynt_command]
pub async fn productivity_goals() -> Vec<GoalProgressResponse> {
    state.productivity_goals().await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_pomodoro_start(
    state: State<'_, Arc<AppCore>>,
    work_mins: Option<i64>,
    break_mins: Option<i64>,
) -> CommandResult<FocusSessionResponse> {
    state
        .productivity_pomodoro_start(work_mins, break_mins)
        .await
}

#[klynt_command]
pub async fn productivity_time_entries(date: String) -> Vec<TimeEntryResponse> {
    state.productivity_time_entries(date).await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_goal_create(
    state: State<'_, Arc<AppCore>>,
    goal_type: String,
    metric: String,
    target_value: f64,
) -> CommandResult<GoalProgressResponse> {
    state
        .productivity_goal_create(goal_type, metric, target_value)
        .await
}

#[klynt_command]
pub async fn productivity_goal_delete(id: i64) -> () {
    state.productivity_goal_delete(id).await
}

#[klynt_command]
pub async fn productivity_goal_toggle(id: i64, enabled: bool) -> () {
    state.productivity_goal_toggle(id, enabled).await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_time_entry_create(
    state: State<'_, Arc<AppCore>>,
    description: String,
    duration_mins: i64,
    category_id: Option<String>,
    project_id: Option<String>,
) -> CommandResult<TimeEntryResponse> {
    state
        .productivity_time_entry_create(description, duration_mins, category_id, project_id)
        .await
}

#[klynt_command]
pub async fn productivity_time_entry_delete(id: i64) -> () {
    state.productivity_time_entry_delete(id).await
}

#[klynt_raw_command]
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
) -> CommandResult<ActivityCategoryResponse> {
    state
        .productivity_category_upsert(id, name, category_type, color, icon, rules)
        .await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_category_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> CommandResult<bool> {
    state.productivity_category_delete(id).await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_recategorize_app(
    state: State<'_, Arc<AppCore>>,
    app_name: String,
    site_name: Option<String>,
    new_category_id: String,
) -> CommandResult<u64> {
    state
        .productivity_recategorize_app(app_name, site_name, new_category_id)
        .await
}

// ── V2: Insights & Auto-Focus ─────────────────────────────────────────

#[klynt_command]
pub async fn productivity_insights(date: Option<String>) -> Vec<InsightCardResponse> {
    state.productivity_insights(date).await
}

#[klynt_command]
pub async fn productivity_insight_dismiss(id: String) -> () {
    state.productivity_insight_dismiss(id).await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_auto_focus_start(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<FocusSessionResponse> {
    state.productivity_auto_focus_start().await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_auto_focus_end(
    state: State<'_, Arc<AppCore>>,
    event: desktop_shared::specta_helpers::JsonValueWrapper,
) -> CommandResult<FocusSessionResponse> {
    let event: feature_productivity::auto_focus::AutoFocusEvent =
        serde_json::from_value(event.0).map_err(|e| ApiError::new("BAD_REQUEST", e.to_string()))?;
    state.productivity_auto_focus_end(event).await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_auto_focus_confirm(
    state: State<'_, Arc<AppCore>>,
    payload: AutoFocusPayload,
) -> CommandResult<FocusSessionResponse> {
    state.productivity_auto_focus_confirm(payload).await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn distraction_respond(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    response: DistractionResponse,
) -> CommandResult<()> {
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
            ::tracing::warn!("unknown distraction_respond action: {other}");
        }
    }
    Ok(())
}

// ── V3: Project Tracking ─────────────────────────────────────────────

#[klynt_command]
pub async fn productivity_projects_list() -> Vec<ProductivityProjectResponse> {
    state.productivity_projects_list().await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_project_upsert(
    state: State<'_, Arc<AppCore>>,
    id: String,
    display_name: String,
    path: String,
    url_patterns: Option<Vec<String>>,
    color: Option<String>,
) -> CommandResult<ProductivityProjectResponse> {
    state
        .productivity_project_upsert(id, display_name, path, url_patterns, color)
        .await
}

#[klynt_command]
pub async fn productivity_project_delete(id: String) -> () {
    state.productivity_project_delete(id).await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_weekly_assessment(
    state: State<'_, Arc<AppCore>>,
    week_start: String,
) -> CommandResult<WeeklyAssessmentResponse> {
    state.productivity_weekly_assessment(week_start).await
}

#[klynt_command]
pub async fn productivity_calendar_events(
    date: String,
) -> Vec<feature_productivity::types::CalendarEvent> {
    state.productivity_calendar_events(date).await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn calendar_sync_events(
    state: State<'_, Arc<AppCore>>,
    events: Vec<desktop_shared::commands::CalendarEventInput>,
) -> CommandResult<Vec<feature_productivity::types::CalendarEvent>> {
    state.calendar_sync_events(events).await
}

// ── Focus Session (tray-driven) ──────────────────────────────────────

use crate::focus_timer::{FocusSessionConfig, SessionCommand};
use desktop_shared::commands::FocusSessionStatusResponse;

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn focus_session_start(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    params: desktop_shared::commands::FocusSessionStartParams,
) -> CommandResult<FocusSessionResponse> {
    let config = FocusSessionConfig {
        work_secs: params.work_secs,
        short_break_secs: params.short_break_secs,
        long_break_secs: params.long_break_secs,
        long_break_after: params.long_break_after,
    };

    // Clean up any orphaned DB sessions (from app restart or stale timer)
    let _ = state.productivity_focus_end(None).await;
    let _ = state.productivity_break_end().await;

    // Start persistent session in AppCore
    let session = state
        .productivity_focus_start(
            params.action_id.clone(),
            None,
            Some(params.work_secs as i64 / 60),
        )
        .await?;

    // Start the desktop timer (phase state machine)
    timer
        .start(
            app,
            config,
            params.action_id,
            params.action_title,
            params.dnd_enabled.unwrap_or(false),
            params.sound_enabled.unwrap_or(true),
            params.notification_enabled.unwrap_or(true),
        )
        .await
        .map_err(|e| ApiError::new("TIMER_ERROR", e.to_string()))?;

    Ok(session)
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn focus_session_stop(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    app: tauri::AppHandle,
    notes: Option<String>,
) -> CommandResult<Option<FocusSessionResponse>> {
    timer.stop(&app).await;
    // End whichever session is active (focus or break)
    let focus_result = state.productivity_focus_end(notes).await.unwrap_or(None);
    if focus_result.is_some() {
        return Ok(focus_result);
    }
    Ok(state.productivity_break_end().await.unwrap_or(None))
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn focus_session_status(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
) -> CommandResult<FocusSessionStatusResponse> {
    let session = state.productivity_focus_status().await?;
    let config = timer.status().await;

    Ok(FocusSessionStatusResponse {
        active: config.is_some(),
        sync: None, // Sync is pushed via events, not polled
        session,
    })
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn focus_session_pause(timer: State<'_, Arc<FocusTimer>>) -> CommandResult<bool> {
    Ok(timer.send_command(SessionCommand::Pause).await)
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn focus_session_resume(timer: State<'_, Arc<FocusTimer>>) -> CommandResult<bool> {
    Ok(timer.send_command(SessionCommand::Resume).await)
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn focus_session_extend(
    timer: State<'_, Arc<FocusTimer>>,
    extra_secs: u64,
) -> CommandResult<bool> {
    Ok(timer.send_command(SessionCommand::Extend(extra_secs)).await)
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn focus_session_start_break(timer: State<'_, Arc<FocusTimer>>) -> CommandResult<bool> {
    Ok(timer.send_command(SessionCommand::StartBreak).await)
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn focus_session_extend_work(
    timer: State<'_, Arc<FocusTimer>>,
    extra_mins: u64,
) -> CommandResult<bool> {
    Ok(timer
        .send_command(SessionCommand::ExtendWork(extra_mins * 60))
        .await)
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn focus_session_skip_break(timer: State<'_, Arc<FocusTimer>>) -> CommandResult<bool> {
    Ok(timer.send_command(SessionCommand::SkipBreak).await)
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn focus_session_take_break(timer: State<'_, Arc<FocusTimer>>) -> CommandResult<bool> {
    Ok(timer.send_command(SessionCommand::TakeBreak).await)
}

// ── Patterns & Hourly Breakdown ─────────────────────────────────────

#[klynt_command]
pub async fn productivity_patterns(days: Option<u32>) -> ProductivityPatternsResponse {
    state.productivity_patterns(days).await
}

#[klynt_raw_command]
#[tauri::command(rename_all = "snake_case")]
#[specta::specta]
pub async fn productivity_hourly_breakdown(
    state: State<'_, Arc<AppCore>>,
    start_date: String,
    end_date: String,
    tz_offset_mins: Option<i32>,
) -> CommandResult<Vec<HourlyBreakdownResponse>> {
    state
        .productivity_hourly_breakdown(start_date, end_date, tz_offset_mins)
        .await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
