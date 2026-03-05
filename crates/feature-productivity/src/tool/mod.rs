//! ProductivityTool — manage focus sessions, view activity data, and configure categories.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::Value;

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
        let summaries = self
            .repos
            .summaries
            .list_range(start_date, end_date)
            .await?;

        if summaries.is_empty() {
            return Ok(format!("No data for {start_date} to {end_date}."));
        }

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
        let avg_quality: Option<f64> = {
            let quals: Vec<f64> = summaries
                .iter()
                .filter_map(|s| s.avg_session_quality)
                .collect();
            if quals.is_empty() {
                None
            } else {
                Some(quals.iter().sum::<f64>() / quals.len() as f64)
            }
        };

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
            color: None,
            icon: None,
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
        "Track productivity, manage focus sessions, and view activity data. Actions: focus_start, focus_end, focus_status, activity_today, activity_summary, activity_week, activity_score, list_categories, set_category."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": [
                        "focus_start", "focus_end", "focus_status",
                        "activity_today", "activity_summary", "activity_week",
                        "activity_score",
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
                }
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
            "activity_today" => self.handle_activity_today().await,
            "activity_summary" => self.handle_activity_summary(&p).await,
            "activity_week" => self.handle_activity_week().await,
            "activity_score" => self.handle_activity_score().await,
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
