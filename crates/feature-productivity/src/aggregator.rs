//! Daily summary aggregation — computes daily productivity summaries
//! from activity events and focus sessions.

use std::collections::HashMap;

use jiff::SignedDuration;

use std::sync::Arc;

use bus::DomainEventBus;

use crate::events::ProductivityEvent;
use crate::handler::ProductivityHandler;
use crate::intelligence::quality_scorer::QualityScorer;
use crate::repos::ProductivityRepos;
use crate::types::{AppUsage, CategoryUsage, DailySummary, ProjectUsage};

pub struct DailyAggregator {
    repos: ProductivityRepos,
    handler: Option<Arc<dyn ProductivityHandler>>,
    domain_bus: Option<Arc<DomainEventBus>>,
    quality_scorer: Option<QualityScorer>,
}

impl DailyAggregator {
    pub fn new(repos: ProductivityRepos) -> Self {
        Self {
            repos,
            handler: None,
            domain_bus: None,
            quality_scorer: None,
        }
    }

    pub fn with_quality_scorer(mut self, scorer: QualityScorer) -> Self {
        self.quality_scorer = Some(scorer);
        self
    }

    pub fn with_handler(mut self, handler: Arc<dyn ProductivityHandler>) -> Self {
        self.handler = Some(handler);
        self
    }

    pub fn with_domain_bus(mut self, bus: Arc<DomainEventBus>) -> Self {
        self.domain_bus = Some(bus);
        self
    }

    /// Compute (or recompute) the daily summary for a given date string (YYYY-MM-DD).
    ///
    /// Uses pre-computed 5-minute bucket aggregation for day-level totals (fast path).
    /// Falls back to raw event queries for the detailed per-app/per-category breakdowns.
    pub async fn compute_for_date(&self, date: &str) -> common::Result<DailySummary> {
        let date_parsed = date
            .parse::<jiff::civil::Date>()
            .map_err(|e| common::ToolError::InvalidParams(format!("invalid date '{date}': {e}")))?;

        let start = date_parsed
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .map_err(|e| common::ToolError::InvalidParams(format!("tz error: {e}")))?
            .timestamp();
        let end = date_parsed
            .tomorrow()
            .map_err(|e| common::ToolError::InvalidParams(format!("date overflow: {e}")))?
            .at(0, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .map_err(|e| common::ToolError::InvalidParams(format!("tz error: {e}")))?
            .timestamp();

        // Gather data — bucket aggregation for totals, raw events for detail breakdowns.
        // Include existing summary fetch in the join to parallelise the AI-summary cache lookup.
        let (
            bucket_agg,
            category_agg,
            categories,
            top_app_rows,
            top_project_rows,
            all_projects,
            sessions,
            time_entries,
            avg_recovery_secs,
            existing_summary,
        ) = tokio::try_join!(
            self.repos.buckets.aggregate_day(date),
            self.repos.events.aggregate_by_category(&start, &end),
            self.repos.categories.list_all(),
            self.repos.events.top_apps(&start, &end, 10),
            self.repos.events.top_projects(&start, &end, 10),
            self.repos.projects.list_all(),
            self.repos.sessions.list_range(&start, &end, None),
            self.repos.time_entries.list_range(&start, &end),
            self.repos
                .distraction_patterns
                .avg_recovery_secs(date, date),
            self.repos.summaries.get(date),
        )?;

        // Bucket-based totals (pre-computed by BucketAggregator)
        let (b_productive, b_neutral, b_distracting, total_idle_secs, context_switches) =
            bucket_agg.unwrap_or((0, 0, 0, 0, 0));
        let has_bucket_data = b_productive + b_neutral + b_distracting + total_idle_secs > 0;

        // Include manual time entries in totals
        let manual_secs: i64 = time_entries.iter().map(|e| e.duration_secs).sum();

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
                        category_id: cat.id.clone(),
                        category: cat.name.clone(),
                        category_type: cat.category_type.to_string(),
                        duration_secs: *secs,
                    });
                } else {
                    neutral_secs += secs;
                }
            } else {
                neutral_secs += secs;
            }
        }

        // Use bucket totals when available (pre-aggregated, faster); derive from
        // per-category event breakdown otherwise (handles tests and early-day edge case).
        if has_bucket_data {
            productive_secs = b_productive;
            neutral_secs = b_neutral;
            distracting_secs = b_distracting;
        }

        let total_active_secs = productive_secs + neutral_secs + distracting_secs + manual_secs;

        // Top apps
        let top_apps: Vec<AppUsage> = top_app_rows
            .into_iter()
            .map(|(app_name, duration_secs, category)| AppUsage {
                app_name,
                duration_secs,
                category,
            })
            .collect();

        // Build project lookup for display names and colors
        let project_map: HashMap<&str, &crate::types::ProductivityProject> =
            all_projects.iter().map(|p| (p.id.as_str(), p)).collect();

        let top_projects: Vec<ProjectUsage> = top_project_rows
            .into_iter()
            .map(|(pid, secs)| {
                let (name, color) = project_map
                    .get(pid.as_str())
                    .map(|p| (p.display_name.clone(), p.color.clone()))
                    .unwrap_or_else(|| (pid.clone(), None));
                ProjectUsage {
                    project_id: pid,
                    display_name: name,
                    duration_secs: secs,
                    color,
                }
            })
            .collect();
        let focus_sessions_count = sessions
            .iter()
            .filter(|s| s.session_type != crate::types::SessionType::Break)
            .count() as i64;
        let total_focus_secs: i64 = sessions
            .iter()
            .filter(|s| s.session_type != crate::types::SessionType::Break)
            .filter_map(|s| s.actual_mins)
            .sum::<i64>()
            * 60;
        let total_break_secs: i64 = sessions
            .iter()
            .filter(|s| s.session_type == crate::types::SessionType::Break)
            .filter_map(|s| s.actual_mins)
            .sum::<i64>()
            * 60;
        let interruptions_count: i64 = sessions.iter().map(|s| s.interruptions).sum();
        let avg_session_quality = {
            // Try focus session quality scores first
            let scores: Vec<f64> = sessions.iter().filter_map(|s| s.quality_score).collect();
            if !scores.is_empty() {
                Some(scores.iter().sum::<f64>() / scores.len() as f64)
            } else if total_active_secs > 0 {
                // Derive quality from session metrics: productive ratio + low distraction
                let productive_ratio = productive_secs as f64 / total_active_secs as f64;
                let distraction_ratio = distracting_secs as f64 / total_active_secs as f64;
                Some((productive_ratio * 0.7 + (1.0 - distraction_ratio) * 0.3).clamp(0.0, 1.0))
            } else {
                None
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
            top_projects,
            productivity_score: None,
            ai_summary: None,
            deep_work_blocks,
            deep_work_secs,
            avg_recovery_secs,
        };

        // Prefer the intelligence layer's unified quality score when available,
        // falling back to the legacy formula when no scored sessions exist.
        let score = if let Some(ref scorer) = self.quality_scorer {
            match scorer.score_day(date).await {
                Ok(Some(daily_score)) => {
                    // Also use intelligence quality for the "Quality" metric
                    // when focus sessions don't have their own quality scores.
                    // Normalize from 0-100 to 0-1 for the ScoreBar.
                    if summary.avg_session_quality.is_none() {
                        summary.avg_session_quality = Some(daily_score.overall_score / 100.0);
                    }
                    daily_score.overall_score
                }
                _ => compute_productivity_score(&summary),
            }
        } else {
            compute_productivity_score(&summary)
        };
        summary.productivity_score = Some(score);

        // Preserve existing AI summary to avoid redundant LLM calls on recompute.
        let existing_ai = existing_summary.and_then(|s| s.ai_summary);
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

        if let Some(ref bus) = self.domain_bus {
            if let Some(score) = summary.productivity_score {
                bus.publish(
                    ProductivityEvent::ProductivityScoreComputed {
                        date: summary.date.clone(),
                        score,
                    }
                    .into(),
                );
            }
        }

        Ok(summary)
    }

    /// Compute the daily summary for today.
    pub async fn compute_today(&self) -> common::Result<DailySummary> {
        let today = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
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
                crate::types::GoalMetric::ProjectHours => {
                    // Look up project time from summary's top_projects
                    goal.project_id
                        .as_ref()
                        .and_then(|pid| {
                            summary
                                .top_projects
                                .iter()
                                .find(|p| &p.project_id == pid)
                                .map(|p| p.duration_secs as f64 / 3600.0)
                        })
                        .unwrap_or(0.0)
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
        let raw_cutoff = jiff::Timestamp::now()
            .checked_sub(SignedDuration::from_secs(raw_days as i64 * 86_400))
            .unwrap_or_else(|_| jiff::Timestamp::now());
        let raw_purged = self.repos.events.purge_before(&raw_cutoff).await?;
        let bucket_cutoff = (jiff::Timestamp::now()
            - SignedDuration::from_secs(bucket_days as i64 * 86_400))
        .strftime("%Y-%m-%d")
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
pub fn compute_productivity_score(summary: &DailySummary) -> f64 {
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
        let today_date = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
        let today_parsed = today_date.parse::<jiff::civil::Date>().unwrap();
        let noon = today_parsed
            .at(12, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp();
        let today = today_date;
        let start = noon
            .checked_sub(jiff::SignedDuration::from_hours(2))
            .unwrap();
        let end = noon
            .checked_sub(jiff::SignedDuration::from_hours(1))
            .unwrap();

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
            project_id: None,
            focus_session_id: None,
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

        let today = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
        let summary = aggregator.compute_for_date(&today).await.unwrap();
        assert_eq!(summary.focus_sessions_count, 1);
    }

    #[tokio::test]
    async fn test_aggregate_top_apps() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let aggregator = DailyAggregator::new(repos.clone());

        let today_date = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
        let today_parsed = today_date.parse::<jiff::civil::Date>().unwrap();
        let noon = today_parsed
            .at(12, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp();
        let today = today_date;
        let start = noon
            .checked_sub(jiff::SignedDuration::from_hours(2))
            .unwrap();

        // Two different apps
        for (app, secs) in &[("VS Code", 7200i64), ("Safari", 1800i64)] {
            let event = ActivityEvent {
                id: None,
                app_name: app.to_string(),
                window_title: None,
                site_name: None,
                bundle_id: None,
                url: None,
                category_id: None,
                started_at: start,
                ended_at: Some(
                    start
                        .checked_add(jiff::SignedDuration::from_secs(*secs))
                        .unwrap(),
                ),
                duration_secs: Some(*secs),
                is_idle: false,
                metadata: None,
                project_id: None,
                focus_session_id: None,
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

        let today_date = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();
        let today_parsed = today_date.parse::<jiff::civil::Date>().unwrap();
        let noon = today_parsed
            .at(12, 0, 0, 0)
            .to_zoned(jiff::tz::TimeZone::UTC)
            .unwrap()
            .timestamp();
        let today = today_date;
        let start = noon
            .checked_sub(jiff::SignedDuration::from_hours(4))
            .unwrap();

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
                ended_at: Some(
                    start
                        .checked_add(jiff::SignedDuration::from_hours(3))
                        .unwrap(),
                ),
                duration_secs: Some(10800),
                is_idle: false,
                metadata: None,
                project_id: None,
                focus_session_id: None,
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
                started_at: start
                    .checked_add(jiff::SignedDuration::from_hours(3))
                    .unwrap(),
                ended_at: Some(
                    start
                        .checked_add(jiff::SignedDuration::from_hours(4))
                        .unwrap(),
                ),
                duration_secs: Some(3600),
                is_idle: false,
                metadata: None,
                project_id: None,
                focus_session_id: None,
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

        let today = jiff::Timestamp::now().strftime("%Y-%m-%d").to_string();

        // First call computes and stores
        let s1 = aggregator.get_or_compute(&today).await.unwrap();
        // Second call returns cached
        let s2 = aggregator.get_or_compute(&today).await.unwrap();
        assert_eq!(s1.date, s2.date);
    }
}
