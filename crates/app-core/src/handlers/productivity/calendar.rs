//! Calendar event handlers — list, sync, weekly assessment.

use chrono::Utc;
use desktop_shared::commands::WeeklyAssessmentResponse;
use desktop_shared::errors::ApiError;

use super::converters::assessment_to_response;
use crate::errors::{map_prod_err, parse_date_or_err};
use crate::state::AppCore;

impl AppCore {
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

    /// Get the next upcoming calendar event (for tray countdown).
    pub async fn next_upcoming_event(
        &self,
    ) -> Option<feature_productivity::types::CalendarEvent> {
        let repos = self.productivity_repos().ok()?;
        let now = Utc::now().to_rfc3339();
        repos.calendar_events.next_upcoming(&now).await.ok().flatten()
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
        let total_focus_mins = summaries
            .iter()
            .map(|s| s.total_focus_secs / 60)
            .sum::<i64>();
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
