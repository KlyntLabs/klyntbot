//! Productivity IPC commands — daily summaries, focus sessions, activity timeline.

use std::sync::Arc;

use chrono::Utc;
use desktop_shared::commands::{
    ActivityCategoryResponse, ActivityTimelineResponse, AppUsageResponse, CategoryUsageResponse,
    FocusSessionResponse, GoalProgressResponse, InsightCardResponse, ProductivityProjectResponse,
    ProductivitySummaryResponse, ProjectUsageResponse, TimeEntryResponse,
};
use desktop_shared::errors::ApiError;
use feature_productivity::types::{DailySummary, FocusSession, InsightCard};
use tauri::State;

use crate::app_core::AppCore;

// ── Helpers ────────────────────────────────────────────────────────────

use super::map_prod_err;

pub(crate) fn summary_to_response(s: DailySummary) -> ProductivitySummaryResponse {
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
        top_projects: s
            .top_projects
            .into_iter()
            .map(|p| ProjectUsageResponse {
                project_id: p.project_id,
                display_name: p.display_name,
                duration_secs: p.duration_secs,
                color: p.color,
            })
            .collect(),
        ai_summary: s.ai_summary,
        productivity_score: s.productivity_score,
    }
}

pub(crate) fn session_to_response(s: FocusSession) -> FocusSessionResponse {
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

pub(crate) fn project_to_response(
    p: feature_productivity::types::ProductivityProject,
) -> ProductivityProjectResponse {
    ProductivityProjectResponse {
        id: p.id,
        display_name: p.display_name,
        path: p.path,
        url_patterns: p.url_patterns,
        color: p.color,
        is_auto_detected: p.is_auto_detected,
    }
}

pub(crate) fn insight_to_response(c: InsightCard) -> InsightCardResponse {
    InsightCardResponse {
        id: c.id,
        insight_type: c.insight_type.to_string(),
        title: c.title,
        body: c.body,
        sentiment: c.sentiment.to_string(),
        metric_value: c.metric_value,
        baseline_value: c.baseline_value,
        date: c.date,
        dismissed: c.dismissed,
        generated_at: c.generated_at,
    }
}

fn event_to_timeline(e: feature_productivity::types::ActivityEvent) -> ActivityTimelineResponse {
    ActivityTimelineResponse {
        app_name: e.app_name,
        window_title: e.window_title,
        site_name: e.site_name,
        category_id: e.category_id,
        started_at: e.started_at,
        duration_secs: e.duration_secs,
        is_idle: e.is_idle,
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
    Ok(events.into_iter().map(event_to_timeline).collect())
}

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
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
    let cap = limit.unwrap_or(50).min(200);
    // list_recent returns newest-first (DESC), which is what the feed wants
    let events = repos.events.list_recent(cap).await.map_err(map_prod_err)?;
    Ok(events.into_iter().map(event_to_timeline).collect())
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

#[tauri::command(rename_all = "snake_case")]
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

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_goal_create(
    state: State<'_, Arc<AppCore>>,
    goal_type: String,
    metric: String,
    target_value: f64,
) -> Result<GoalProgressResponse, ApiError> {
    let repos = state.productivity_repos()?;
    let gt: feature_productivity::types::GoalType = goal_type
        .parse()
        .map_err(|_| ApiError::new("VALIDATION", "Invalid goal_type. Use: daily, weekly"))?;
    let gm: feature_productivity::types::GoalMetric = metric
        .parse()
        .map_err(|_| ApiError::new("VALIDATION", "Invalid metric. Use: productive_hours, focus_sessions, productivity_score, max_distracting_mins"))?;
    let goal = feature_productivity::types::ProductivityGoal {
        id: None,
        goal_type: gt,
        metric: gm,
        target_value,
        enabled: true,
        project_id: None,
        created_at: Utc::now(),
    };
    let id = repos.goals.insert(&goal).await.map_err(map_prod_err)?;
    Ok(GoalProgressResponse {
        id,
        goal_type: goal.goal_type.to_string(),
        metric: goal.metric.to_string(),
        target_value: goal.target_value,
        current_value: 0.0,
        met: false,
    })
}

#[tauri::command]
pub async fn productivity_goal_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos.goals.delete(id).await.map_err(map_prod_err)?;
    Ok(())
}

#[tauri::command]
pub async fn productivity_goal_toggle(
    state: State<'_, Arc<AppCore>>,
    id: i64,
    enabled: bool,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos
        .goals
        .set_enabled(id, enabled)
        .await
        .map_err(map_prod_err)?;
    Ok(())
}

#[tauri::command(rename_all = "snake_case")]
pub async fn productivity_time_entry_create(
    state: State<'_, Arc<AppCore>>,
    description: String,
    duration_mins: i64,
    category_id: Option<String>,
    project_id: Option<String>,
) -> Result<TimeEntryResponse, ApiError> {
    let repos = state.productivity_repos()?;
    let now = Utc::now();
    let started_at = now - chrono::Duration::minutes(duration_mins);
    let duration_secs = duration_mins * 60;
    let entry = feature_productivity::types::TimeEntry {
        id: None,
        description,
        category_id,
        project_id,
        started_at,
        duration_secs,
        source: "manual".to_string(),
        created_at: now,
    };
    let id = repos
        .time_entries
        .insert(&entry)
        .await
        .map_err(map_prod_err)?;
    Ok(TimeEntryResponse {
        id,
        description: entry.description,
        category_id: entry.category_id,
        project_id: entry.project_id,
        started_at,
        duration_secs,
        source: entry.source,
    })
}

#[tauri::command]
pub async fn productivity_time_entry_delete(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos.time_entries.delete(id).await.map_err(map_prod_err)?;
    Ok(())
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
    let repos = state.productivity_repos()?;
    let ct: feature_productivity::types::CategoryType = category_type.parse().map_err(|_| {
        ApiError::new(
            "VALIDATION",
            "Invalid category_type. Use: productive, neutral, distracting",
        )
    })?;
    let cat = feature_productivity::types::ActivityCategory {
        id,
        name,
        category_type: ct,
        color,
        icon,
        rules: None,
        is_system: false,
    };
    repos.categories.upsert(&cat).await.map_err(map_prod_err)?;
    Ok(ActivityCategoryResponse {
        id: cat.id,
        name: cat.name,
        category_type: cat.category_type.to_string(),
        color: cat.color,
        icon: cat.icon,
        is_system: false,
    })
}

// ── V2: Insights & Auto-Focus ─────────────────────────────────────────

#[tauri::command]
pub async fn productivity_insights(
    state: State<'_, Arc<AppCore>>,
    date: Option<String>,
) -> Result<Vec<InsightCardResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let date = date.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
    let engine = feature_productivity::insights::InsightEngine::new(repos.clone());
    let cards = engine
        .generate_for_date(&date)
        .await
        .map_err(map_prod_err)?;
    Ok(cards.into_iter().map(insight_to_response).collect())
}

#[tauri::command]
pub async fn productivity_insight_dismiss(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos.insights.dismiss(&id).await.map_err(map_prod_err)?;
    Ok(())
}

#[tauri::command]
pub async fn productivity_auto_focus_confirm(
    state: State<'_, Arc<AppCore>>,
    session: feature_productivity::auto_focus::AutoFocusSession,
) -> Result<FocusSessionResponse, ApiError> {
    let repos = state.productivity_repos()?;
    let actual_mins = session.total_secs / 60;
    let focus_session = FocusSession {
        id: uuid::Uuid::new_v4().to_string(),
        action_id: None,
        project_id: None,
        session_type: feature_productivity::types::SessionType::Focus,
        target_mins: None,
        started_at: session.started_at,
        ended_at: Some(session.ended_at),
        actual_mins: Some(actual_mins),
        interruptions: 0,
        distraction_events: vec![],
        quality_score: Some(session.productive_ratio),
        completed: true,
        notes: Some(format!("Auto-detected focus in {}", session.dominant_app)),
        source: feature_productivity::types::SessionSource::AutoDetected,
    };
    repos
        .sessions
        .create(&focus_session)
        .await
        .map_err(map_prod_err)?;
    Ok(session_to_response(focus_session))
}

// ── V3: Project Tracking ─────────────────────────────────────────────

#[tauri::command]
pub async fn productivity_projects_list(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<ProductivityProjectResponse>, ApiError> {
    let repos = state.productivity_repos()?;
    let projects = repos.projects.list_all().await.map_err(map_prod_err)?;
    Ok(projects.into_iter().map(project_to_response).collect())
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
    let repos = state.productivity_repos()?;
    let project = feature_productivity::types::ProductivityProject {
        id: id.clone(),
        display_name,
        path,
        url_patterns: url_patterns.unwrap_or_default(),
        color,
        is_auto_detected: false,
        created_at: Utc::now(),
    };
    repos.projects.upsert(&project).await.map_err(map_prod_err)?;
    Ok(project_to_response(project))
}

#[tauri::command]
pub async fn productivity_project_delete(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<(), ApiError> {
    let repos = state.productivity_repos()?;
    repos.projects.delete(&id).await.map_err(map_prod_err)?;
    Ok(())
}
