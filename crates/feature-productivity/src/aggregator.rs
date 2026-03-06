//! Daily summary aggregation — computes daily productivity summaries
//! from activity events and focus sessions.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use std::sync::Arc;

use crate::handler::ProductivityHandler;
use crate::repos::ProductivityRepos;
use crate::types::{AppUsage, CategoryUsage, DailySummary};

pub struct DailyAggregator {
    repos: ProductivityRepos,
    handler: Option<Arc<dyn ProductivityHandler>>,
}

impl DailyAggregator {
    pub fn new(repos: ProductivityRepos) -> Self {
        Self {
            repos,
            handler: None,
        }
    }

    pub fn with_handler(mut self, handler: Arc<dyn ProductivityHandler>) -> Self {
        self.handler = Some(handler);
        self
    }

    /// Compute (or recompute) the daily summary for a given date string (YYYY-MM-DD).
    pub async fn compute_for_date(&self, date: &str) -> common::Result<DailySummary> {
        let naive = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|e| common::ToolError::InvalidParams(format!("invalid date '{date}': {e}")))?;

        let start: DateTime<Utc> = Utc.from_utc_datetime(&naive.and_hms_opt(0, 0, 0).unwrap());
        let end: DateTime<Utc> = Utc.from_utc_datetime(
            &(naive + chrono::Duration::days(1))
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        );

        // Gather data from repos — parallelize independent queries
        let (
            total_active_secs,
            total_idle_secs,
            context_switches,
            category_agg,
            categories,
            top_app_rows,
            sessions,
            time_entries,
            avg_recovery_secs,
        ) = tokio::try_join!(
            self.repos.events.total_active_secs(&start, &end),
            self.repos.events.total_idle_secs(&start, &end),
            self.repos.events.count_context_switches(&start, &end),
            self.repos.events.aggregate_by_category(&start, &end),
            self.repos.categories.list_all(),
            self.repos.events.top_apps(&start, &end, 10),
            self.repos.sessions.list_range(&start, &end, None),
            self.repos.time_entries.list_range(&start, &end),
            self.repos.distraction_patterns.avg_recovery_secs(date, date),
        )?;

        // Include manual time entries in totals
        let manual_secs: i64 = time_entries.iter().map(|e| e.duration_secs).sum();
        let total_active_secs = total_active_secs + manual_secs;

        // Category breakdown — O(1) lookups via HashMap
        let cat_map: HashMap<&str, &crate::types::ActivityCategory> =
            categories.iter().map(|c| (c.id.as_str(), c)).collect();

        let mut productive_secs: i64 = 0;
        let mut neutral_secs: i64 = 0;
        let mut distracting_secs: i64 = 0;
        let mut top_categories = Vec::new();

        for (cat_id, secs) in &category_agg {
            if let Some(cat_id) = cat_id {
                if let Some(cat) = cat_map.get(cat_id.as_str()) {
                    match cat.category_type {
                        crate::types::CategoryType::Productive => productive_secs += secs,
                        crate::types::CategoryType::Neutral => neutral_secs += secs,
                        crate::types::CategoryType::Distracting => distracting_secs += secs,
                    }
                    top_categories.push(CategoryUsage {
                        category: cat.name.clone(),
                        duration_secs: *secs,
                    });
                } else {
                    neutral_secs += secs;
                }
            } else {
                neutral_secs += secs;
            }
        }

        // Top apps
        let top_apps: Vec<AppUsage> = top_app_rows
            .into_iter()
            .map(|(app_name, duration_secs)| AppUsage {
                app_name,
                duration_secs,
                category: None,
            })
            .collect();
        let focus_sessions_count = sessions.len() as i64;
        let total_focus_secs: i64 = sessions.iter().filter_map(|s| s.actual_mins).sum::<i64>() * 60;
        let total_break_secs: i64 = sessions
            .iter()
            .filter(|s| s.session_type == crate::types::SessionType::Break)
            .filter_map(|s| s.actual_mins)
            .sum::<i64>()
            * 60;
        let interruptions_count: i64 = sessions.iter().map(|s| s.interruptions).sum();
        let avg_session_quality = if sessions.is_empty() {
            None
        } else {
            let scores: Vec<f64> = sessions.iter().filter_map(|s| s.quality_score).collect();
            if scores.is_empty() {
                None
            } else {
                Some(scores.iter().sum::<f64>() / scores.len() as f64)
            }
        };

        let (deep_work_blocks, deep_work_secs) = sessions
            .iter()
            .filter(|s| s.session_type != crate::types::SessionType::Break)
            .filter(|s| s.actual_mins.unwrap_or(0) >= 25)
            .fold((0i64, 0i64), |(blocks, secs), s| {
                (blocks + 1, secs + s.actual_mins.unwrap_or(0) * 60)
            });

        let mut summary = DailySummary {
            date: date.to_string(),
            total_active_secs,
            total_focus_secs,
            total_break_secs,
            total_idle_secs,
            productive_secs,
            neutral_secs,
            distracting_secs,
            focus_sessions_count,
            avg_session_quality,
            interruptions_count,
            context_switches,
            top_apps,
            top_categories,
            productivity_score: None,
            ai_summary: None,
            deep_work_blocks,
            deep_work_secs,
            avg_recovery_secs,
        };

        let score = compute_productivity_score(&summary);
        summary.productivity_score = Some(score);

        // Preserve existing AI summary to avoid redundant LLM calls on recompute.
        // Only generate a new one if no cached summary exists with an AI summary.
        let existing_ai = self
            .repos
            .summaries
            .get(date)
            .await?
            .and_then(|s| s.ai_summary);
        if let Some(cached) = existing_ai {
            summary.ai_summary = Some(cached);
        } else if let Some(ref handler) = self.handler {
            let context = format!(
                "Date: {}. Active: {:.1}h. Productive: {:.1}h. Distracting: {:.1}h. Focus sessions: {}. Context switches: {}. Score: {:.0}/100. Top apps: {}.",
                summary.date,
                summary.total_active_secs as f64 / 3600.0,
                summary.productive_secs as f64 / 3600.0,
                summary.distracting_secs as f64 / 3600.0,
                summary.focus_sessions_count,
                summary.context_switches,
                summary.productivity_score.unwrap_or(0.0),
                summary.top_apps.iter().take(3).map(|a| format!("{} ({}m)", a.app_name, a.duration_secs / 60)).collect::<Vec<_>>().join(", "),
            );
            match handler.generate_daily_summary(&context).await {
                Ok(ai_summary) => summary.ai_summary = Some(ai_summary),
                Err(e) => tracing::warn!("AI summary generation failed: {e}"),
            }
        }

        self.repos.summaries.upsert(&summary).await?;
        Ok(summary)
    }

    /// Compute the daily summary for today.
    pub async fn compute_today(&self) -> common::Result<DailySummary> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        self.compute_for_date(&today).await
    }

    /// Check goal progress for a given date.
    pub async fn check_goals(
        &self,
        date: &str,
    ) -> common::Result<Vec<(crate::types::ProductivityGoal, f64, bool)>> {
        let (summary, goals) =
            tokio::try_join!(self.get_or_compute(date), self.repos.goals.list_enabled(),)?;

        let mut results = Vec::new();
        for goal in goals {
            let current = match goal.metric {
                crate::types::GoalMetric::ProductiveHours => {
                    summary.productive_secs as f64 / 3600.0
                }
                crate::types::GoalMetric::FocusSessions => summary.focus_sessions_count as f64,
                crate::types::GoalMetric::ProductivityScore => {
                    summary.productivity_score.unwrap_or(0.0)
                }
                crate::types::GoalMetric::MaxDistractingMins => {
                    summary.distracting_secs as f64 / 60.0
                }
            };
            let met = match goal.metric {
                crate::types::GoalMetric::MaxDistractingMins => current <= goal.target_value,
                _ => current >= goal.target_value,
            };
            results.push((goal, current, met));
        }
        Ok(results)
    }

    /// Dual-tier purge: raw events after `raw_days`, buckets after `bucket_days`.
    pub async fn purge_old_data(
        &self,
        raw_days: u64,
        bucket_days: u64,
    ) -> common::Result<(u64, u64)> {
        let raw_cutoff = Utc::now() - chrono::Duration::days(raw_days as i64);
        let raw_purged = self.repos.events.purge_before(&raw_cutoff).await?;
        let bucket_cutoff = (Utc::now() - chrono::Duration::days(bucket_days as i64))
            .format("%Y-%m-%d")
            .to_string();
        let bucket_purged = self.repos.buckets.purge_before(&bucket_cutoff).await?;
        Ok((raw_purged, bucket_purged))
    }

    /// Get a cached summary for a date, or compute it if missing.
    pub async fn get_or_compute(&self, date: &str) -> common::Result<DailySummary> {
        if let Some(existing) = self.repos.summaries.get(date).await? {
            return Ok(existing);
        }
        self.compute_for_date(date).await
    }
}

/// Compute a 0-100 productivity score from daily metrics.
///
/// Formula:
/// - Productive ratio (40%): productive_secs / total_active_secs
/// - Focus quality (30%): avg_session_quality (or 0.5 if no sessions)
/// - Low distraction (20%): 1.0 - (distracting_secs / total_active_secs)
/// - Continuity (10%): 1.0 - (context_switches / expected_switches)
fn compute_productivity_score(summary: &DailySummary) -> f64 {
    let total = summary.total_active_secs as f64;
    if total < 60.0 {
        return 0.0;
    }

    let productive_ratio = summary.productive_secs as f64 / total;
    let focus_quality = summary.avg_session_quality.unwrap_or(0.5);
    let distraction_ratio = 1.0 - (summary.distracting_secs as f64 / total);
    let expected_switches = (total / 1800.0).max(1.0);
    let continuity = (1.0 - (summary.context_switches as f64 / expected_switches)).clamp(0.0, 1.0);

    let raw = (productive_ratio * 0.4)
        + (focus_quality * 0.3)
        + (distraction_ratio * 0.2)
        + (continuity * 0.1);

    (raw * 100.0).clamp(0.0, 100.0).round()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FocusConfig;
    use crate::focus::FocusManager;
    use crate::repos::ProductivityRepos;
    use crate::types::ActivityEvent;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &crate::ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    #[tokio::test]
    async fn test_aggregate_daily_summary() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let aggregator = DailyAggregator::new(repos.clone());

        // Use noon UTC today to avoid midnight-crossing flakiness
        let now = Utc::now();
        let noon = Utc.from_utc_datetime(&now.date_naive().and_hms_opt(12, 0, 0).unwrap());
        let today = noon.format("%Y-%m-%d").to_string();
        let start = noon - chrono::Duration::hours(2);
        let end = noon - chrono::Duration::hours(1);

        let event = ActivityEvent {
            id: None,
            app_name: "Visual Studio Code".into(),
            window_title: Some("main.rs".into()),
            site_name: None,
            bundle_id: Some("com.microsoft.VSCode".into()),
            url: None,
            category_id: Some("coding".into()),
            started_at: start,
            ended_at: Some(end),
            duration_secs: Some(3600),
            is_idle: false,
            metadata: None,
        };
        repos.events.insert(&event).await.unwrap();

        let summary = aggregator.compute_for_date(&today).await.unwrap();
        assert_eq!(summary.total_active_secs, 3600);
        assert_eq!(summary.productive_secs, 3600);
        assert_eq!(summary.date, today);
    }

    #[tokio::test]
    async fn test_aggregate_includes_focus_sessions() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let aggregator = DailyAggregator::new(repos.clone());
        let focus_mgr = FocusManager::new(repos.clone(), FocusConfig::default());

        // Start and end a quick focus session
        focus_mgr.start_session(None, None, Some(1)).await.unwrap();
        let ended = focus_mgr.end_session(Some("test".into())).await.unwrap();
        assert!(ended.is_some());

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let summary = aggregator.compute_for_date(&today).await.unwrap();
        assert_eq!(summary.focus_sessions_count, 1);
    }

    #[tokio::test]
    async fn test_aggregate_top_apps() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let aggregator = DailyAggregator::new(repos.clone());

        let now = Utc::now();
        let noon = Utc.from_utc_datetime(&now.date_naive().and_hms_opt(12, 0, 0).unwrap());
        let today = noon.format("%Y-%m-%d").to_string();
        let start = noon - chrono::Duration::hours(2);

        // Two different apps
        for (app, secs) in &[("VS Code", 7200), ("Safari", 1800)] {
            let event = ActivityEvent {
                id: None,
                app_name: app.to_string(),
                window_title: None,
                site_name: None,
                bundle_id: None,
                url: None,
                category_id: None,
                started_at: start,
                ended_at: Some(start + chrono::Duration::seconds(*secs)),
                duration_secs: Some(*secs),
                is_idle: false,
                metadata: None,
            };
            repos.events.insert(&event).await.unwrap();
        }

        let summary = aggregator.compute_for_date(&today).await.unwrap();
        assert_eq!(summary.top_apps.len(), 2);
        // VS Code should be first (more time)
        assert_eq!(summary.top_apps[0].app_name, "VS Code");
        assert_eq!(summary.top_apps[0].duration_secs, 7200);
    }

    #[tokio::test]
    async fn test_compute_productivity_score() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let aggregator = DailyAggregator::new(repos.clone());

        let now = Utc::now();
        let noon = Utc.from_utc_datetime(&now.date_naive().and_hms_opt(12, 0, 0).unwrap());
        let today = noon.format("%Y-%m-%d").to_string();
        let start = noon - chrono::Duration::hours(4);

        // 3h productive coding
        repos
            .events
            .insert(&ActivityEvent {
                id: None,
                app_name: "VS Code".into(),
                window_title: None,
                site_name: None,
                bundle_id: None,
                url: None,
                category_id: Some("coding".into()),
                started_at: start,
                ended_at: Some(start + chrono::Duration::hours(3)),
                duration_secs: Some(10800),
                is_idle: false,
                metadata: None,
            })
            .await
            .unwrap();

        // 1h distracting
        repos
            .events
            .insert(&ActivityEvent {
                id: None,
                app_name: "Chrome".into(),
                window_title: None,
                site_name: None,
                bundle_id: None,
                url: None,
                category_id: Some("entertainment".into()),
                started_at: start + chrono::Duration::hours(3),
                ended_at: Some(start + chrono::Duration::hours(4)),
                duration_secs: Some(3600),
                is_idle: false,
                metadata: None,
            })
            .await
            .unwrap();

        let summary = aggregator.compute_for_date(&today).await.unwrap();
        let score = summary
            .productivity_score
            .expect("score should be computed");
        assert!(score > 0.0 && score <= 100.0, "score {score} out of range");
        assert!(
            score > 40.0 && score < 85.0,
            "score {score} unexpected for 75% productive"
        );
    }

    #[tokio::test]
    async fn test_get_or_compute_caches() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let aggregator = DailyAggregator::new(repos);

        let today = Utc::now().format("%Y-%m-%d").to_string();

        // First call computes and stores
        let s1 = aggregator.get_or_compute(&today).await.unwrap();
        // Second call returns cached
        let s2 = aggregator.get_or_compute(&today).await.unwrap();
        assert_eq!(s1.date, s2.date);
    }
}
