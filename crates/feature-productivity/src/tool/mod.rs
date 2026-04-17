//! ProductivityTool — manage focus sessions, view activity data, and configure categories.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::Value;

use std::fmt::Write;

use common::{Result, ToolError};
use tools_core::{ParamExtractor, RoutingContext, Tool};
use tracing::warn;

use crate::aggregator::DailyAggregator;
use crate::focus::FocusManager;
use crate::repos::ProductivityRepos;
use crate::types::{CategoryRules, CategoryType};

pub struct ProductivityTool {
    repos: ProductivityRepos,
    focus_manager: Arc<FocusManager>,
    aggregator: Arc<DailyAggregator>,
}

impl ProductivityTool {
    pub fn new(
        repos: ProductivityRepos,
        focus_manager: Arc<FocusManager>,
        aggregator: Arc<DailyAggregator>,
    ) -> Self {
        Self {
            repos,
            focus_manager,
            aggregator,
        }
    }

    // ── Focus actions ──────────────────────────────────────────────

    async fn handle_focus_start(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let action_id = p.optional_str("action_id")?.map(|s| s.to_string());
        let project_id = p.optional_str("project_id")?.map(|s| s.to_string());
        let duration_mins = p.optional_i64("duration_mins")?;

        let session = self
            .focus_manager
            .start_session(action_id, project_id, duration_mins)
            .await?;

        let target = session.target_mins.unwrap_or(45);
        Ok(format!(
            "Focus session started ({}min target). Session ID: {}.\nStarted at: {}",
            target,
            session.id,
            session.started_at.format("%H:%M")
        ))
    }

    async fn handle_focus_end(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let notes = p.optional_str("notes")?.map(|s| s.to_string());

        match self.focus_manager.end_session(notes).await? {
            Some(session) => {
                let quality = session
                    .quality_score
                    .map(|q| format!("{:.0}%", q * 100.0))
                    .unwrap_or_else(|| "N/A".into());
                let actual = session.actual_mins.unwrap_or(0);
                let completed = if session.completed { "Yes" } else { "No" };
                Ok(format!(
                    "Focus session ended.\n- Duration: {}min\n- Completed: {}\n- Quality: {}\n- Interruptions: {}",
                    actual, completed, quality, session.interruptions
                ))
            }
            None => Ok("No active focus session to end.".into()),
        }
    }

    async fn handle_focus_status(&self) -> Result<String> {
        match self.focus_manager.get_active().await? {
            Some(session) => {
                let elapsed = (Utc::now() - session.started_at).num_minutes();
                let target = session.target_mins.unwrap_or(45);
                let remaining = (target - elapsed).max(0);
                Ok(format!(
                    "Active focus session:\n- Elapsed: {}min / {}min target\n- Remaining: {}min\n- Interruptions: {}\n- Started: {}",
                    elapsed,
                    target,
                    remaining,
                    session.interruptions,
                    session.started_at.format("%H:%M")
                ))
            }
            None => Ok("No active focus session.".into()),
        }
    }

    // ── Activity actions ───────────────────────────────────────────

    async fn handle_activity_today(&self) -> Result<String> {
        let summary = self.aggregator.compute_today().await?;
        Ok(format_summary(&summary))
    }

    async fn handle_activity_summary(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let start_date = p.required_str("start_date")?;
        let end_date = p.required_str("end_date")?;
        let group_by = p.optional_str("group_by")?;

        let summaries = self
            .repos
            .summaries
            .list_range(start_date, end_date)
            .await?;

        if summaries.is_empty() {
            return Ok(format!("No data for {start_date} to {end_date}."));
        }

        match group_by {
            Some("day") => {
                let mut lines = vec![format!("Daily breakdown ({start_date} to {end_date}):")];
                for s in &summaries {
                    let score = s
                        .productivity_score
                        .map(|v| format!("{:.0}", v))
                        .unwrap_or_else(|| "N/A".into());
                    lines.push(format!(
                        "- {}: active {} | productive {} | score {}",
                        s.date,
                        format_duration(s.total_active_secs),
                        format_duration(s.productive_secs),
                        score,
                    ));
                }
                Ok(lines.join("\n"))
            }
            Some("week") => Ok(format_grouped_breakdown(
                "Weekly",
                start_date,
                end_date,
                &summaries,
                |s| {
                    s.date
                        .parse::<jiff::civil::Date>()
                        .map(|d| {
                            let w = d.iso_week_date();
                            format!("{}-W{:02}", w.year(), w.week())
                        })
                        .unwrap_or_else(|_| "unknown".into())
                },
            )),
            Some("month") => Ok(format_grouped_breakdown(
                "Monthly",
                start_date,
                end_date,
                &summaries,
                |s| s.date.get(..7).unwrap_or("unknown").to_string(),
            )),
            Some("project") => {
                let results = self
                    .repos
                    .events
                    .aggregate_by_project(start_date, end_date)
                    .await?;
                let mut lines = vec![format!("By project ({start_date} to {end_date}):")];
                for (pid, secs) in &results {
                    lines.push(format!("- {pid}: {}", format_duration(*secs)));
                }
                if results.is_empty() {
                    lines.push("  No project data found.".into());
                }
                Ok(lines.join("\n"))
            }
            Some(other) => Err(ToolError::InvalidParams(format!(
                "group_by must be 'day', 'week', 'month', or 'project', got '{other}'"
            ))
            .into()),
            None => {
                let total_active: i64 = summaries.iter().map(|s| s.total_active_secs).sum();
                let total_productive: i64 = summaries.iter().map(|s| s.productive_secs).sum();
                let total_distracting: i64 = summaries.iter().map(|s| s.distracting_secs).sum();
                let total_focus: i64 = summaries.iter().map(|s| s.focus_sessions_count).sum();
                Ok(format!(
                    "Activity summary ({} to {}):\n- Days tracked: {}\n- Total active: {}\n- Productive: {}\n- Distracting: {}\n- Focus sessions: {}",
                    start_date,
                    end_date,
                    summaries.len(),
                    format_duration(total_active),
                    format_duration(total_productive),
                    format_duration(total_distracting),
                    total_focus,
                ))
            }
        }
    }

    async fn handle_activity_week(&self) -> Result<String> {
        let today = Utc::now().date_naive();
        let week_ago = today - Duration::days(7);
        let start_date = week_ago.format("%Y-%m-%d").to_string();
        let end_date = today.format("%Y-%m-%d").to_string();

        // Compute today's summary first (best-effort — weekly still works with stale today)
        if let Err(e) = self.aggregator.compute_today().await {
            warn!("failed to recompute today's summary for weekly report: {e}");
        }

        let summaries = self
            .repos
            .summaries
            .list_range(&start_date, &end_date)
            .await?;

        if summaries.is_empty() {
            return Ok("No activity data for the past week.".into());
        }

        let total_active: i64 = summaries.iter().map(|s| s.total_active_secs).sum();
        let total_productive: i64 = summaries.iter().map(|s| s.productive_secs).sum();
        let total_focus_sessions: i64 = summaries.iter().map(|s| s.focus_sessions_count).sum();
        let avg_quality = avg_scores(summaries.iter().filter_map(|s| s.avg_session_quality));

        let quality_str = avg_quality
            .map(|q| format!("{:.0}%", q * 100.0))
            .unwrap_or_else(|| "N/A".into());

        Ok(format!(
            "Weekly summary ({} to {}):\n- Days with data: {}\n- Total active: {}\n- Productive: {}\n- Focus sessions: {}\n- Avg quality: {}",
            start_date,
            end_date,
            summaries.len(),
            format_duration(total_active),
            format_duration(total_productive),
            total_focus_sessions,
            quality_str,
        ))
    }

    // ── Pomodoro actions ─────────────────────────────────────────

    async fn handle_pomodoro_start(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let action_id = p.optional_str("action_id")?.map(|s| s.to_string());
        let project_id = p.optional_str("project_id")?.map(|s| s.to_string());
        let work_mins = p.optional_i64("work_mins")?;
        let break_mins = p.optional_i64("break_mins")?;

        let session = self
            .focus_manager
            .start_pomodoro(action_id, project_id, work_mins, break_mins)
            .await?;
        let work = session.target_mins.unwrap_or(25);
        Ok(format!(
            "Pomodoro started ({work}min work). Session ID: {}",
            session.id
        ))
    }

    // ── Score action ─────────────────────────────────────────────

    async fn handle_activity_score(&self) -> Result<String> {
        let summary = self.aggregator.compute_today().await?;
        let score = summary.productivity_score.unwrap_or(0.0);

        let total = summary.total_active_secs as f64;
        let productive_pct = if total > 0.0 {
            (summary.productive_secs as f64 / total * 100.0).round()
        } else {
            0.0
        };
        let distracting_pct = if total > 0.0 {
            (summary.distracting_secs as f64 / total * 100.0).round()
        } else {
            0.0
        };

        Ok(format!(
            "Productivity score: {:.0}/100\n- Productive: {:.0}%\n- Distracting: {:.0}%\n- Focus sessions: {}\n- Context switches: {}",
            score, productive_pct, distracting_pct, summary.focus_sessions_count, summary.context_switches
        ))
    }

    // ── Comparison action ────────────────────────────────────────

    async fn handle_activity_compare(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let period = p.optional_str("period")?.unwrap_or("day");

        let today = Utc::now().date_naive();
        let (current_start, previous_start, previous_end, label) = match period {
            "day" => {
                let yesterday = today - Duration::days(1);
                (
                    today.format("%Y-%m-%d").to_string(),
                    yesterday.format("%Y-%m-%d").to_string(),
                    yesterday.format("%Y-%m-%d").to_string(),
                    "Today vs Yesterday",
                )
            }
            "week" => {
                let week_ago = today - Duration::days(7);
                let two_weeks_ago = today - Duration::days(14);
                (
                    week_ago.format("%Y-%m-%d").to_string(),
                    two_weeks_ago.format("%Y-%m-%d").to_string(),
                    (week_ago - Duration::days(1))
                        .format("%Y-%m-%d")
                        .to_string(),
                    "This week vs Last week",
                )
            }
            _ => {
                return Err(
                    ToolError::InvalidParams("period must be 'day' or 'week'".into()).into(),
                )
            }
        };

        // Ensure today is computed, then fetch both periods in parallel
        let today_str = today.format("%Y-%m-%d").to_string();
        let _ = self.aggregator.get_or_compute(&today_str).await;

        let (current, previous) = tokio::try_join!(
            self.repos.summaries.list_range(&current_start, &today_str),
            self.repos
                .summaries
                .list_range(&previous_start, &previous_end),
        )?;

        let cur_productive: i64 = current.iter().map(|s| s.productive_secs).sum();
        let prev_productive: i64 = previous.iter().map(|s| s.productive_secs).sum();
        let cur_score = avg_scores(current.iter().filter_map(|s| s.productivity_score));
        let prev_score = avg_scores(previous.iter().filter_map(|s| s.productivity_score));

        let productive_change = if prev_productive > 0 {
            let pct = ((cur_productive as f64 - prev_productive as f64) / prev_productive as f64
                * 100.0)
                .round();
            if pct >= 0.0 {
                format!("+{:.0}%", pct)
            } else {
                format!("{:.0}%", pct)
            }
        } else {
            "N/A".into()
        };

        let score_line = match (cur_score, prev_score) {
            (Some(c), Some(p)) => {
                let diff = c - p;
                let sign = if diff >= 0.0 { "+" } else { "" };
                format!("\n- Score: {:.0} vs {:.0} ({}{:.0})", c, p, sign, diff)
            }
            _ => String::new(),
        };

        Ok(format!(
            "{label}:\n- Productive time: {} vs {} ({productive_change}){score_line}",
            format_duration(cur_productive),
            format_duration(prev_productive),
        ))
    }

    // ── Goal actions ────────────────────────────────────────────

    async fn handle_set_goal(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let metric_str = p.required_str("metric")?;
        let metric: crate::types::GoalMetric = metric_str.parse()?;
        let target = p
            .optional_f64("target_value")?
            .ok_or_else(|| ToolError::InvalidParams("target_value is required".into()))?;
        let goal_type_str = p.optional_str("goal_type")?.unwrap_or("daily");
        let goal_type: crate::types::GoalType = goal_type_str.parse()?;

        let goal = crate::types::ProductivityGoal {
            id: None,
            goal_type,
            metric,
            target_value: target,
            enabled: true,
            project_id: None,
            created_at: Utc::now(),
        };
        let id = self.repos.goals.insert(&goal).await?;
        Ok(format!(
            "Goal set: {} {} {} ({goal_type}). ID: {id}",
            metric,
            metric.operator(),
            target
        ))
    }

    async fn handle_check_goals(&self) -> Result<String> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let results = self.aggregator.check_goals(&today).await?;

        if results.is_empty() {
            return Ok("No goals set. Use set_goal to create one.".into());
        }

        let mut lines = vec!["Goal progress:".to_string()];
        for (goal, current, met) in &results {
            let status = if *met { "MET" } else { "IN PROGRESS" };
            lines.push(format!(
                "- {} {}: {:.1}/{:.1} [{}]",
                goal.goal_type, goal.metric, current, goal.target_value, status
            ));
        }
        Ok(lines.join("\n"))
    }

    async fn handle_list_goals(&self) -> Result<String> {
        let goals = self.repos.goals.list_enabled().await?;
        if goals.is_empty() {
            return Ok("No goals set.".into());
        }
        let mut lines = vec!["Active goals:".to_string()];
        for g in &goals {
            lines.push(format!(
                "- [{}] {} {} {} (id: {})",
                g.goal_type,
                g.metric,
                g.metric.operator(),
                g.target_value,
                g.id.unwrap_or(0)
            ));
        }
        Ok(lines.join("\n"))
    }

    async fn handle_remove_goal(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_i64("goal_id")?;
        if self.repos.goals.delete(id).await? {
            Ok(format!("Goal {id} removed."))
        } else {
            Ok(format!("Goal {id} not found."))
        }
    }

    // ── Manual time entry action ────────────────────────────────

    async fn handle_log_time(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let description = p.required_str("description")?;
        let duration_mins = p.required_i64("duration_mins")?;
        let category_id = p.optional_str("category_id")?.map(|s| s.to_string());
        let project_id = p.optional_str("project_id")?.map(|s| s.to_string());

        let entry = crate::types::TimeEntry {
            id: None,
            description: description.to_string(),
            category_id,
            project_id,
            started_at: Utc::now() - Duration::minutes(duration_mins),
            duration_secs: duration_mins * 60,
            source: "manual".into(),
            created_at: Utc::now(),
        };

        let id = self.repos.time_entries.insert(&entry).await?;
        Ok(format!(
            "Logged {}min: '{}' (id: {id})",
            duration_mins, description
        ))
    }

    // ── Export action ────────────────────────────────────────────

    async fn handle_activity_export(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let start_date = p.required_str("start_date")?;
        let end_date = p.required_str("end_date")?;
        let format = p.optional_str("format")?.unwrap_or("csv");

        let start = chrono::NaiveDate::parse_from_str(start_date, "%Y-%m-%d")
            .map_err(|e| ToolError::InvalidParams(format!("invalid start_date: {e}")))?
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let end = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
            .map_err(|e| ToolError::InvalidParams(format!("invalid end_date: {e}")))?
            .and_hms_opt(23, 59, 59)
            .unwrap()
            .and_utc();

        let events = self
            .repos
            .events
            .list_range(&start, &end, Some(50_000))
            .await?;

        match format {
            "csv" => {
                let mut csv = String::from(
                    "app_name,window_title,category_id,started_at,duration_secs,is_idle\n",
                );
                for e in &events {
                    let _ = writeln!(
                        csv,
                        "{},{},{},{},{},{}",
                        escape_csv(&e.app_name),
                        escape_csv(e.window_title.as_deref().unwrap_or("")),
                        escape_csv(e.category_id.as_deref().unwrap_or("")),
                        e.started_at,
                        e.duration_secs.unwrap_or(0),
                        e.is_idle,
                    );
                }
                Ok(format!(
                    "Exported {} events ({start_date} to {end_date}):\n\n{csv}",
                    events.len()
                ))
            }
            "json" => {
                let json = serde_json::to_string_pretty(&events)
                    .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
                Ok(format!(
                    "Exported {} events ({start_date} to {end_date}):\n\n{json}",
                    events.len()
                ))
            }
            _ => Err(ToolError::InvalidParams("format must be 'csv' or 'json'".into()).into()),
        }
    }

    // ── Category actions ───────────────────────────────────────────

    async fn handle_list_categories(&self) -> Result<String> {
        let categories = self.repos.categories.list_all().await?;
        if categories.is_empty() {
            return Ok("No categories configured.".into());
        }

        let mut lines = vec!["Activity categories:".to_string()];
        for cat in &categories {
            let system_tag = if cat.is_system { " (system)" } else { "" };
            lines.push(format!(
                "- {} [{}]{}",
                cat.name, cat.category_type, system_tag
            ));
        }
        Ok(lines.join("\n"))
    }

    async fn handle_set_category(&self, p: &ParamExtractor<'_>) -> Result<String> {
        let id = p.required_str("id")?;
        let name = p.required_str("name")?;
        let category_type_str = p.required_str("category_type")?;
        let category_type: CategoryType = category_type_str.parse()?;
        let color = p.optional_str("color")?.map(|s| s.to_string());
        let icon = p.optional_str("icon")?.map(|s| s.to_string());

        let app_names = p.string_array_or_empty("app_names")?;
        let bundle_ids = p.string_array_or_empty("bundle_ids")?;
        let url_patterns = p.string_array_or_empty("url_patterns")?;

        let rules = CategoryRules {
            app_names,
            bundle_ids,
            url_patterns,
        };

        let category = crate::types::ActivityCategory {
            id: id.to_string(),
            name: name.to_string(),
            category_type,
            color,
            icon,
            rules: Some(rules),
            is_system: false,
        };

        self.repos.categories.upsert(&category).await?;
        Ok(format!("Category '{}' ({}) saved.", name, category_type))
    }
}

#[async_trait]
impl Tool for ProductivityTool {
    fn name(&self) -> &str {
        "productivity"
    }

    fn description(&self) -> &str {
        "Track daily productivity and focus. Start/end focus sessions (Pomodoro), log time, set productivity goals, and view activity summaries. NOT for financial goals or task management."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "focus_start", "focus_end", "focus_status",
                        "pomodoro_start",
                        "activity_today", "activity_summary", "activity_week",
                        "activity_score", "activity_compare",
                        "set_goal", "check_goals", "list_goals", "remove_goal",
                        "log_time", "activity_export",
                        "list_categories", "set_category"
                    ],
                    "description": "Action to perform"
                },
                "action_id": { "type": "string", "description": "Task/action ID to link the focus session to" },
                "project_id": { "type": "string", "description": "Project ID for the focus session" },
                "duration_mins": { "type": "integer", "description": "Target duration in minutes (default: 45)" },
                "notes": { "type": "string", "description": "Notes when ending a focus session" },
                "start_date": { "type": "string", "description": "Start date (YYYY-MM-DD) for activity_summary" },
                "end_date": { "type": "string", "description": "End date (YYYY-MM-DD) for activity_summary" },
                "group_by": {
                    "type": "string",
                    "enum": ["day", "week", "month", "project"],
                    "description": "Group results by time period or project (optional, for activity_summary)"
                },
                "id": { "type": "string", "description": "Category ID for set_category" },
                "name": { "type": "string", "description": "Category name for set_category" },
                "category_type": {
                    "type": "string",
                    "enum": ["productive", "neutral", "distracting"],
                    "description": "Category type for set_category"
                },
                "app_names": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "App names for category matching"
                },
                "bundle_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Bundle IDs for category matching"
                },
                "url_patterns": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "URL patterns for category matching"
                },
                "work_mins": { "type": "integer", "description": "Pomodoro work duration in minutes (default: 25)" },
                "break_mins": { "type": "integer", "description": "Pomodoro break duration in minutes (default: 5)" },
                "period": { "type": "string", "enum": ["day", "week"], "description": "Comparison period for activity_compare" },
                "metric": { "type": "string", "enum": ["productive_hours", "focus_sessions", "productivity_score", "max_distracting_mins"], "description": "Goal metric for set_goal" },
                "target_value": { "type": "number", "description": "Target value for set_goal" },
                "goal_type": { "type": "string", "enum": ["daily", "weekly"], "description": "Goal type for set_goal (default: daily)" },
                "goal_id": { "type": "integer", "description": "Goal ID for remove_goal" },
                "description": { "type": "string", "description": "Description for log_time" },
                "category_id": { "type": "string", "description": "Category ID for log_time" },
                "format": { "type": "string", "enum": ["csv", "json"], "description": "Export format (default: csv)" }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, args: Value, _ctx: &RoutingContext) -> Result<String> {
        let p = ParamExtractor::new(&args);
        let action = p.required_str("action")?;

        match action {
            "focus_start" => self.handle_focus_start(&p).await,
            "focus_end" => self.handle_focus_end(&p).await,
            "focus_status" => self.handle_focus_status().await,
            "pomodoro_start" => self.handle_pomodoro_start(&p).await,
            "activity_today" => self.handle_activity_today().await,
            "activity_summary" => self.handle_activity_summary(&p).await,
            "activity_week" => self.handle_activity_week().await,
            "activity_score" => self.handle_activity_score().await,
            "activity_compare" => self.handle_activity_compare(&p).await,
            "set_goal" => self.handle_set_goal(&p).await,
            "check_goals" => self.handle_check_goals().await,
            "list_goals" => self.handle_list_goals().await,
            "remove_goal" => self.handle_remove_goal(&p).await,
            "log_time" => self.handle_log_time(&p).await,
            "activity_export" => self.handle_activity_export(&p).await,
            "list_categories" => self.handle_list_categories().await,
            "set_category" => self.handle_set_category(&p).await,
            _ => Err(ToolError::InvalidParams(format!("Unknown action: {action}")).into()),
        }
    }
}

fn format_duration(secs: i64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else {
        format!("{}m", mins)
    }
}

fn avg_scores(iter: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, count) = iter.fold((0.0, 0usize), |(s, c), v| (s + v, c + 1));
    if count == 0 {
        None
    } else {
        Some(sum / count as f64)
    }
}

fn format_grouped_breakdown(
    label: &str,
    start_date: &str,
    end_date: &str,
    summaries: &[crate::types::DailySummary],
    key_fn: impl Fn(&crate::types::DailySummary) -> String,
) -> String {
    struct Accum {
        active: i64,
        productive: i64,
        distracting: i64,
        scores: Vec<Option<f64>>,
    }
    let mut groups: std::collections::BTreeMap<String, Accum> = std::collections::BTreeMap::new();
    for s in summaries {
        let entry = groups.entry(key_fn(s)).or_insert(Accum {
            active: 0,
            productive: 0,
            distracting: 0,
            scores: vec![],
        });
        entry.active += s.total_active_secs;
        entry.productive += s.productive_secs;
        entry.distracting += s.distracting_secs;
        entry.scores.push(s.productivity_score);
    }
    let mut lines = vec![format!("{label} breakdown ({start_date} to {end_date}):")];
    for (key, acc) in &groups {
        let score_str = avg_scores(acc.scores.iter().filter_map(|s| *s))
            .map(|v| format!("{v:.0}"))
            .unwrap_or_else(|| "N/A".into());
        lines.push(format!(
            "- {key}: active {} | productive {} | distracting {} | avg score {score_str}",
            format_duration(acc.active),
            format_duration(acc.productive),
            format_duration(acc.distracting),
        ));
    }
    lines.join("\n")
}

fn escape_csv(field: &str) -> std::borrow::Cow<'_, str> {
    if field.contains(',') || field.contains('"') || field.contains('\n') {
        std::borrow::Cow::Owned(format!("\"{}\"", field.replace('"', "\"\"")))
    } else {
        std::borrow::Cow::Borrowed(field)
    }
}

fn format_summary(summary: &crate::types::DailySummary) -> String {
    let mut lines = vec![format!("Activity for {}:", summary.date)];
    lines.push(format!(
        "- Active: {} | Idle: {}",
        format_duration(summary.total_active_secs),
        format_duration(summary.total_idle_secs)
    ));
    lines.push(format!(
        "- Productive: {} | Neutral: {} | Distracting: {}",
        format_duration(summary.productive_secs),
        format_duration(summary.neutral_secs),
        format_duration(summary.distracting_secs)
    ));
    if summary.focus_sessions_count > 0 {
        let quality = summary
            .avg_session_quality
            .map(|q| format!("{:.0}%", q * 100.0))
            .unwrap_or_else(|| "N/A".into());
        lines.push(format!(
            "- Focus sessions: {} (avg quality: {})",
            summary.focus_sessions_count, quality
        ));
    }
    if !summary.top_apps.is_empty() {
        lines.push("- Top apps:".into());
        for app in summary.top_apps.iter().take(5) {
            lines.push(format!(
                "  - {}: {}",
                app.app_name,
                format_duration(app.duration_secs)
            ));
        }
    }
    if let Some(ref ai) = summary.ai_summary {
        lines.push(format!("\n{}", ai));
    }
    if let Some(score) = summary.productivity_score {
        lines.push(format!("\nProductivity score: {:.0}/100", score));
    }
    lines.join("\n")
}
