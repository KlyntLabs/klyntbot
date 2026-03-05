//! Daily summary aggregation — computes daily productivity summaries
//! from activity events and focus sessions.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

use crate::repos::ProductivityRepos;
use crate::types::{AppUsage, CategoryUsage, DailySummary};

pub struct DailyAggregator {
    repos: ProductivityRepos,
}

impl DailyAggregator {
    pub fn new(repos: ProductivityRepos) -> Self {
        Self { repos }
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
        ) = tokio::try_join!(
            self.repos.events.total_active_secs(&start, &end),
            self.repos.events.total_idle_secs(&start, &end),
            self.repos.events.count_context_switches(&start, &end),
            self.repos.events.aggregate_by_category(&start, &end),
            self.repos.categories.list_all(),
            self.repos.events.top_apps(&start, &end, 10),
            self.repos.sessions.list_range(&start, &end, None),
        )?;

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

        let summary = DailySummary {
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
            ai_summary: None,
        };

        self.repos.summaries.upsert(&summary).await?;
        Ok(summary)
    }

    /// Compute the daily summary for today.
    pub async fn compute_today(&self) -> common::Result<DailySummary> {
        let today = Utc::now().format("%Y-%m-%d").to_string();
        self.compute_for_date(&today).await
    }

    /// Get a cached summary for a date, or compute it if missing.
    pub async fn get_or_compute(&self, date: &str) -> common::Result<DailySummary> {
        if let Some(existing) = self.repos.summaries.get(date).await? {
            return Ok(existing);
        }
        self.compute_for_date(date).await
    }
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

        // Insert activity events for today
        let today = Utc::now().format("%Y-%m-%d").to_string();
        let start = Utc::now() - chrono::Duration::hours(2);
        let end = Utc::now() - chrono::Duration::hours(1);

        let event = ActivityEvent {
            id: None,
            app_name: "Visual Studio Code".into(),
            window_title: Some("main.rs".into()),
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

        let today = Utc::now().format("%Y-%m-%d").to_string();
        let start = Utc::now() - chrono::Duration::hours(2);

        // Two different apps
        for (app, secs) in &[("VS Code", 7200), ("Safari", 1800)] {
            let event = ActivityEvent {
                id: None,
                app_name: app.to_string(),
                window_title: None,
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
