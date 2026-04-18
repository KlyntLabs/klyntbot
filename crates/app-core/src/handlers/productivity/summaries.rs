//! Productivity summary handlers — today, timeline, weekly, date ranges, activity feed.

use desktop_shared::commands::{
    ActivityTimelineResponse, HourlyBreakdownResponse, ProductivityPatternsResponse,
    ProductivitySummaryResponse,
};
use desktop_shared::errors::ApiError;

use super::converters::{event_to_timeline, summary_to_response};
use crate::errors::{map_prod_err, parse_local_day_range};
use crate::state::AppCore;

impl AppCore {
    pub async fn productivity_today(
        &self,
    ) -> Result<Option<ProductivitySummaryResponse>, ApiError> {
        let aggregator = self.aggregator()?;
        let summary = aggregator.compute_today().await.map_err(map_prod_err)?;
        let mut resp = summary_to_response(summary);

        // Compute trend deltas vs 4-week rolling average
        let repos = self.productivity_repos()?;
        let today_str = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
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

    pub async fn productivity_weekly(&self) -> Result<Vec<ProductivitySummaryResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let now = jiff::Timestamp::now();
        let today_str = now.strftime("%Y-%m-%d").to_string();
        let week_start_str = now
            .checked_sub(jiff::SignedDuration::from_secs(6 * 86400))
            .unwrap_or(now)
            .strftime("%Y-%m-%d")
            .to_string();
        let mut summaries = repos
            .summaries
            .list_range(&week_start_str, &today_str)
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
        let today = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
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

    pub async fn productivity_patterns(
        &self,
        days: Option<u32>,
    ) -> Result<ProductivityPatternsResponse, ApiError> {
        let repos = self.productivity_repos()?;
        let analyzer = feature_productivity::ProductivityPatternAnalyzer::new(repos.clone());
        let patterns = analyzer
            .analyze(days.unwrap_or(14))
            .await
            .map_err(map_prod_err)?;
        Ok(ProductivityPatternsResponse {
            peak_focus_hours: patterns.peak_focus_hours,
            avg_session_mins: patterns.avg_session_mins,
            productive_ratio: patterns.productive_ratio,
            avg_context_switches: patterns.avg_context_switches,
            best_day_of_week: patterns.best_day_of_week.map(|d| d.to_string()),
            days_analyzed: patterns.days_analyzed,
        })
    }

    pub async fn productivity_hourly_breakdown(
        &self,
        start_date: String,
        end_date: String,
        tz_offset_mins: Option<i32>,
    ) -> Result<Vec<HourlyBreakdownResponse>, ApiError> {
        let repos = self.productivity_repos()?;
        let rows = repos
            .buckets
            .aggregate_by_hour(&start_date, &end_date, tz_offset_mins.unwrap_or(0))
            .await
            .map_err(map_prod_err)?;
        Ok(rows
            .into_iter()
            .map(|r| HourlyBreakdownResponse {
                hour: r.hour as u32,
                productive_secs: r.productive_secs,
                neutral_secs: r.neutral_secs,
                distracting_secs: r.distracting_secs,
                idle_secs: r.idle_secs,
                total_secs: r.total_secs,
                productive_ratio: if r.total_secs > 0 {
                    r.productive_secs as f64 / r.total_secs as f64
                } else {
                    0.0
                },
            })
            .collect())
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
}
