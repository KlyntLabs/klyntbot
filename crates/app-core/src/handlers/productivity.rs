//! Productivity handlers — daily summaries, focus sessions, activity timeline,
//! goals, time entries, insights, auto-focus, and project tracking.

use chrono::Utc;
use desktop_shared::commands::{
    ActivityCategoryResponse, ActivityTimelineResponse, AppUsageResponse, CategoryRulesResponse,
    CategoryUsageResponse, FocusSessionResponse, GoalProgressResponse, InsightCardResponse,
    ProductivityProjectResponse, ProductivitySummaryResponse, ProjectUsageResponse,
    TimeEntryResponse, TrackedAppResponse, WeeklyAssessmentResponse,
};
use desktop_shared::errors::ApiError;
use feature_productivity::auto_focus::AutoFocusSession;
use feature_productivity::types::{DailySummary, FocusSession, InsightCard};

use crate::errors::{map_prod_err, parse_date_or_err, parse_local_day_range};
use crate::state::AppCore;

// ── Converters ────────────────────────────────────────────────────────

fn rules_to_response(r: feature_productivity::types::CategoryRules) -> CategoryRulesResponse {
    CategoryRulesResponse {
        app_names: r.app_names,
        bundle_ids: r.bundle_ids,
        url_patterns: r.url_patterns,
    }
}

fn rules_from_response(r: CategoryRulesResponse) -> feature_productivity::types::CategoryRules {
    feature_productivity::types::CategoryRules {
        app_names: r.app_names,
        bundle_ids: r.bundle_ids,
        url_patterns: r.url_patterns,
    }
}

pub fn summary_to_response(s: DailySummary) -> ProductivitySummaryResponse {
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
                category_id: c.category_id,
                category: c.category,
                category_type: c.category_type,
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
        score_trend: None,
        focus_time_trend: None,
        active_time_trend: None,
    }
}

pub fn session_to_response(s: FocusSession) -> FocusSessionResponse {
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

pub fn project_to_response(
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

fn assessment_to_response(
    a: feature_productivity::types::WeeklyAssessment,
) -> WeeklyAssessmentResponse {
    WeeklyAssessmentResponse {
        id: a.id,
        week_start: a.week_start,
        week_end: a.week_end,
        avg_score: a.avg_score,
        total_focus_mins: a.total_focus_mins,
        total_productive_secs: a.total_productive_secs,
        total_distracting_secs: a.total_distracting_secs,
        top_apps: a.top_apps,
        summary: a.summary,
    }
}

pub fn insight_to_response(c: InsightCard) -> InsightCardResponse {
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

pub fn event_to_timeline(
    e: feature_productivity::types::ActivityEvent,
) -> ActivityTimelineResponse {
    ActivityTimelineResponse {
        app_name: e.app_name,
        window_title: e.window_title,
        site_name: e.site_name,
        category_id: e.category_id,
        started_at: e.started_at,
        duration_secs: e.duration_secs,
        is_idle: e.is_idle,
        project_id: e.project_id,
        focus_session_id: e.focus_session_id,
    }
}

// ── AppCore methods ───────────────────────────────────────────────────

impl AppCore {
    pub async fn productivity_today(
        &self,
    ) -> Result<Option<ProductivitySummaryResponse>, ApiError> {
        let aggregator = self.aggregator()?;
        let summary = aggregator.compute_today().await.map_err(map_prod_err)?;
        let mut resp = summary_to_response(summary);

        // Compute trend deltas vs 4-week rolling average
        let repos = self.productivity_repos()?;
        let today_str = Utc::now().format("%Y-%m-%d").to_string();
        if let Ok(baseline) = repos.summaries.rolling_averages(&today_str, 28).await {
            resp.score_trend = match (resp.productivity_score, baseline.avg_score) {
                (Some(current), Some(avg)) => Some(current - avg),
                _ => None,
            };
            resp.focus_time_trend = baseline
                .avg_focus_secs
                .map(|avg| resp.total_focus_secs as f64 - avg);
            resp.active_time_trend = baseline
                .avg_active_secs
                .map(|avg| resp.total_active_secs as f64 - avg);
        }

        Ok(Some(resp))
    }

    pub async fn productivity_timeline(
        &self,
        date: String,
        limit: Option<i64>,
        offset: Option<i64>,
        tz_offset_mins: Option<i32>,
    ) -> Result<Vec<ActivityTimelineResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let (start, end) = parse_local_day_range(&date, tz_offset_mins)?;
        let cap = limit.unwrap_or(10_000).min(10_000);
        let events = repos
            .events
            .list_range_offset(&start, &end, Some(cap), offset)
            .await
            .map_err(map_prod_err)?;
        Ok(events.into_iter().map(event_to_timeline).collect())
    }

    pub async fn productivity_focus_start(
        &self,
        action_id: Option<String>,
        project_id: Option<String>,
        target_mins: Option<i64>,
    ) -> Result<FocusSessionResponse, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr
            .start_session(action_id, project_id, target_mins)
            .await
            .map_err(map_prod_err)?;
        Ok(session_to_response(session))
    }

    pub async fn productivity_focus_end(
        &self,
        notes: Option<String>,
    ) -> Result<Option<FocusSessionResponse>, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr.end_session(notes).await.map_err(map_prod_err)?;

        // Clear interceptor session state (whitelist + temp passes)
        if let Ok(interceptor) = self.distraction_interceptor() {
            let mut guard = interceptor.lock().await;
            guard.reset_session();
        }

        Ok(session.map(session_to_response))
    }

    pub async fn productivity_focus_status(
        &self,
    ) -> Result<Option<FocusSessionResponse>, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr.get_active().await.map_err(map_prod_err)?;
        Ok(session.map(session_to_response))
    }

    pub async fn productivity_sessions(
        &self,
        date: String,
    ) -> Result<Vec<FocusSessionResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let start = parse_date_or_err(&date)?;
        let end = start + chrono::Duration::days(1);
        let sessions = repos
            .sessions
            .list_range(&start, &end, None)
            .await
            .map_err(map_prod_err)?;
        Ok(sessions.into_iter().map(session_to_response).collect())
    }

    pub async fn productivity_weekly(&self) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let today = Utc::now().date_naive();
        let today_str = today.format("%Y-%m-%d").to_string();
        let week_start = today - chrono::Duration::days(6);
        let mut summaries = repos
            .summaries
            .list_range(
                &week_start.format("%Y-%m-%d").to_string(),
                &today_str,
            )
            .await
            .map_err(map_prod_err)?;

        // Live-override today's summary
        if let Ok(aggregator) = self.aggregator() {
            if let Ok(live) = aggregator.compute_today().await {
                if let Some(idx) = summaries.iter().position(|s| s.date == today_str) {
                    summaries[idx] = live;
                } else {
                    summaries.push(live);
                }
            }
        }

        Ok(summaries.into_iter().map(summary_to_response).collect())
    }

    pub async fn productivity_categories(&self) -> Result<Vec<ActivityCategoryResponse>, ApiError> {
        let repos = self.productivity_repos()?;
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
                rules: c.rules.map(rules_to_response),
            })
            .collect())
    }

    pub async fn productivity_tracked_apps(&self) -> Result<Vec<TrackedAppResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let rows = repos.events.tracked_apps().await.map_err(map_prod_err)?;
        Ok(rows
            .into_iter()
            .map(|r| TrackedAppResponse {
                display_name: r.display_name,
                app_name: r.app_name,
                site_name: r.site_name,
                category_id: r.category_id,
                category_name: r.category_name,
                total_secs: r.total_secs,
                event_count: r.event_count,
            })
            .collect())
    }

    pub async fn productivity_summary_range(
        &self,
        start_date: String,
        end_date: String,
    ) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let mut summaries = repos
            .summaries
            .list_range(&start_date, &end_date)
            .await
            .map_err(map_prod_err)?;

        // Include today's live-computed summary if today falls within the range
        let today = Utc::now().format("%Y-%m-%d").to_string();
        if today >= start_date && today <= end_date {
            let has_today = summaries.iter().any(|s| s.date == today);
            if let Ok(aggregator) = self.aggregator() {
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

        let mut responses: Vec<ProductivitySummaryResponse> =
            summaries.into_iter().map(summary_to_response).collect();

        // Compute trend deltas for each day vs its own 28-day baseline
        for resp in &mut responses {
            if let Ok(baseline) = repos.summaries.rolling_averages(&resp.date, 28).await {
                resp.score_trend = match (resp.productivity_score, baseline.avg_score) {
                    (Some(current), Some(avg)) => Some(current - avg),
                    _ => None,
                };
                resp.focus_time_trend = baseline
                    .avg_focus_secs
                    .map(|avg| resp.total_focus_secs as f64 - avg);
                resp.active_time_trend = baseline
                    .avg_active_secs
                    .map(|avg| resp.total_active_secs as f64 - avg);
            }
        }

        Ok(responses)
    }

    pub async fn productivity_activity_feed(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<ActivityTimelineResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let cap = limit.unwrap_or(50).min(200);
        // list_recent returns newest-first (DESC), which is what the feed wants
        let events = repos.events.list_recent(cap).await.map_err(map_prod_err)?;
        Ok(events.into_iter().map(event_to_timeline).collect())
    }

    pub async fn productivity_goals(&self) -> Result<Vec<GoalProgressResponse>, ApiError> {
        let aggregator = self.aggregator()?;
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
                project_id: goal.project_id.clone(),
            })
            .collect())
    }

    pub async fn productivity_pomodoro_start(
        &self,
        work_mins: Option<i64>,
        break_mins: Option<i64>,
    ) -> Result<FocusSessionResponse, ApiError> {
        self.productivity_pomodoro_start_with_action(None, None, work_mins, break_mins)
            .await
    }

    pub async fn productivity_pomodoro_start_with_action(
        &self,
        action_id: Option<String>,
        project_id: Option<String>,
        work_mins: Option<i64>,
        break_mins: Option<i64>,
    ) -> Result<FocusSessionResponse, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr
            .start_pomodoro(action_id, project_id, work_mins, break_mins)
            .await
            .map_err(map_prod_err)?;
        Ok(session_to_response(session))
    }

    pub async fn productivity_break_start(
        &self,
        break_mins: i64,
    ) -> Result<FocusSessionResponse, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr
            .start_break_session(break_mins)
            .await
            .map_err(map_prod_err)?;
        Ok(session_to_response(session))
    }

    pub async fn productivity_break_end(&self) -> Result<Option<FocusSessionResponse>, ApiError> {
        let focus_mgr = self.focus_manager()?;
        let session = focus_mgr.end_break_session().await.map_err(map_prod_err)?;
        Ok(session.map(session_to_response))
    }

    pub async fn productivity_time_entries(
        &self,
        date: String,
    ) -> Result<Vec<TimeEntryResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let start = parse_date_or_err(&date)?;
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

    pub async fn productivity_goal_create(
        &self,
        goal_type: String,
        metric: String,
        target_value: f64,
    ) -> Result<GoalProgressResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let gt: feature_productivity::types::GoalType = goal_type
            .parse()
            .map_err(|_| ApiError::new("VALIDATION", "Invalid goal_type. Use: daily, weekly"))?;
        let gm: feature_productivity::types::GoalMetric = metric.parse().map_err(|_| {
            ApiError::new(
                "VALIDATION",
                "Invalid metric. Use: productive_hours, focus_sessions, productivity_score, max_distracting_mins",
            )
        })?;
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
            project_id: goal.project_id,
        })
    }

    pub async fn productivity_goal_delete(&self, id: i64) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.goals.delete(id).await.map_err(map_prod_err)?;
        Ok(())
    }

    pub async fn productivity_goal_toggle(&self, id: i64, enabled: bool) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos
            .goals
            .set_enabled(id, enabled)
            .await
            .map_err(map_prod_err)?;
        Ok(())
    }

    pub async fn productivity_time_entry_create(
        &self,
        description: String,
        duration_mins: i64,
        category_id: Option<String>,
        project_id: Option<String>,
    ) -> Result<TimeEntryResponse, ApiError> {
        let repos = self.productivity_repos()?;
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

    pub async fn productivity_time_entry_delete(&self, id: i64) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.time_entries.delete(id).await.map_err(map_prod_err)?;
        Ok(())
    }

    pub async fn productivity_category_upsert(
        &self,
        id: String,
        name: String,
        category_type: String,
        color: Option<String>,
        icon: Option<String>,
        rules: Option<CategoryRulesResponse>,
    ) -> Result<ActivityCategoryResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let ct: feature_productivity::types::CategoryType =
            category_type.parse().map_err(|_| {
                ApiError::new(
                    "VALIDATION",
                    "Invalid category_type. Use: productive, neutral, distracting",
                )
            })?;
        let cat_rules = rules.map(rules_from_response);
        let cat = feature_productivity::types::ActivityCategory {
            id,
            name,
            category_type: ct,
            color,
            icon,
            rules: cat_rules,
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
            rules: cat.rules.map(rules_to_response),
        })
    }

    pub async fn productivity_category_delete(&self, id: String) -> Result<bool, ApiError> {
        let repos = self.productivity_repos()?;
        repos.categories.delete(&id).await.map_err(map_prod_err)
    }

    // ── V2: Insights & Auto-Focus ─────────────────────────────────────

    pub async fn productivity_insights(
        &self,
        date: Option<String>,
    ) -> Result<Vec<InsightCardResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let date = date.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
        let engine = feature_productivity::insights::InsightEngine::new(repos.clone());
        // Generate any missing insights (idempotent)
        let _ = engine
            .generate_for_date(&date)
            .await
            .map_err(map_prod_err)?;
        // Always return ALL stored (non-dismissed) insights for the date
        let cards = repos
            .insights
            .list_for_date(&date)
            .await
            .map_err(map_prod_err)?;
        Ok(cards.into_iter().map(insight_to_response).collect())
    }

    pub async fn productivity_insight_dismiss(&self, id: String) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.insights.dismiss(&id).await.map_err(map_prod_err)?;
        Ok(())
    }

    pub async fn productivity_auto_focus_confirm(
        &self,
        session: AutoFocusSession,
    ) -> Result<FocusSessionResponse, ApiError> {
        let repos = self.productivity_repos()?;
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

    // ── V3: Project Tracking ──────────────────────────────────────────

    pub async fn productivity_projects_list(
        &self,
    ) -> Result<Vec<ProductivityProjectResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let projects = repos.projects.list_all().await.map_err(map_prod_err)?;
        Ok(projects.into_iter().map(project_to_response).collect())
    }

    pub async fn productivity_project_upsert(
        &self,
        id: String,
        display_name: String,
        path: String,
        url_patterns: Option<Vec<String>>,
        color: Option<String>,
    ) -> Result<ProductivityProjectResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let project = feature_productivity::types::ProductivityProject {
            id: id.clone(),
            display_name,
            path,
            url_patterns: url_patterns.unwrap_or_default(),
            color,
            is_auto_detected: false,
            created_at: Utc::now(),
        };
        repos
            .projects
            .upsert(&project)
            .await
            .map_err(map_prod_err)?;
        Ok(project_to_response(project))
    }

    pub async fn productivity_project_delete(&self, id: String) -> Result<(), ApiError> {
        let repos = self.productivity_repos()?;
        repos.projects.delete(&id).await.map_err(map_prod_err)?;
        Ok(())
    }

    // ── Calendar Events ──────────────────────────────────────────────

    pub async fn productivity_calendar_events(
        &self,
        date: String,
    ) -> Result<Vec<feature_productivity::types::CalendarEvent>, ApiError> {
        let repos = self.productivity_repos()?;
        let _ = parse_date_or_err(&date)?;
        repos
            .calendar_events
            .list_for_date(&date)
            .await
            .map_err(map_prod_err)
    }

    pub async fn calendar_sync_events(
        &self,
        events: Vec<desktop_shared::commands::CalendarEventInput>,
    ) -> Result<Vec<feature_productivity::types::CalendarEvent>, ApiError> {
        let repos = self.productivity_repos()?;
        let now = Utc::now().to_rfc3339();
        let mut results = Vec::new();

        for input in events {
            let event = feature_productivity::types::CalendarEvent {
                id: uuid::Uuid::new_v4().to_string(),
                calendar_id: input.calendar_id.unwrap_or_else(|| "primary".into()),
                title: input.title,
                description: input.description,
                started_at: input.started_at,
                ended_at: input.ended_at,
                location: input.location,
                attendees_count: input.attendees_count.unwrap_or(0),
                is_recurring: input.is_recurring.unwrap_or(false),
                recurrence_id: input.recurrence_id,
                source: input.source.unwrap_or_else(|| "google".into()),
                external_uid: input.external_uid,
                session_id: None,
                color: input.color,
                synced_at: now.clone(),
                created_at: now.clone(),
                updated_at: now.clone(),
            };
            repos
                .calendar_events
                .upsert(&event)
                .await
                .map_err(map_prod_err)?;
            results.push(event);
        }

        Ok(results)
    }

    // ── Weekly Assessment ───────────────────────────────────────────

    pub async fn productivity_weekly_assessment(
        &self,
        week_start: String,
    ) -> Result<WeeklyAssessmentResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let start_date = parse_date_or_err(&week_start)?;
        let end_date = start_date + chrono::Duration::days(6);
        let end_str = end_date.format("%Y-%m-%d").to_string();

        // Query daily summaries for the week
        let summaries = repos
            .summaries
            .list_range(&week_start, &end_str)
            .await
            .map_err(map_prod_err)?;

        // Aggregate
        let days = summaries.len() as f64;
        let total_focus_mins = summaries.iter().map(|s| s.total_focus_secs / 60).sum::<i64>();
        let total_productive_secs = summaries.iter().map(|s| s.productive_secs).sum::<i64>();
        let total_distracting_secs = summaries.iter().map(|s| s.distracting_secs).sum::<i64>();

        let scores: Vec<f64> = summaries
            .iter()
            .filter_map(|s| s.productivity_score)
            .collect();
        let avg_score = if scores.is_empty() {
            None
        } else {
            Some(scores.iter().sum::<f64>() / scores.len() as f64)
        };

        // Aggregate top apps across the week
        let mut app_map: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        for s in &summaries {
            for app in &s.top_apps {
                *app_map.entry(app.app_name.clone()).or_default() += app.duration_secs;
            }
        }
        let mut top_apps_vec: Vec<_> = app_map.into_iter().collect();
        top_apps_vec.sort_by(|a, b| b.1.cmp(&a.1));
        top_apps_vec.truncate(10);
        let top_apps_json = serde_json::to_string(&top_apps_vec).ok();

        let summary_text = if days > 0.0 {
            Some(format!(
                "{} days tracked, {:.0}h focus, {:.0}% productive",
                days as i64,
                total_focus_mins as f64 / 60.0,
                if total_productive_secs + total_distracting_secs > 0 {
                    total_productive_secs as f64
                        / (total_productive_secs + total_distracting_secs) as f64
                        * 100.0
                } else {
                    0.0
                }
            ))
        } else {
            None
        };

        let assessment = feature_productivity::types::WeeklyAssessment {
            id: format!("wa-{}", week_start),
            week_start: week_start.clone(),
            week_end: end_str,
            avg_score,
            total_focus_mins: Some(total_focus_mins),
            total_productive_secs: Some(total_productive_secs),
            total_distracting_secs: Some(total_distracting_secs),
            top_apps: top_apps_json,
            summary: summary_text,
            created_at: Utc::now().to_rfc3339(),
        };

        repos
            .weekly_assessments
            .upsert(&assessment)
            .await
            .map_err(map_prod_err)?;

        Ok(assessment_to_response(assessment))
    }
}
