//! Heuristic InsightEngine — generates daily insight cards by comparing
//! today's metrics against a rolling 14-day baseline.

use chrono::Utc;

use crate::repos::ProductivityRepos;
use crate::types::{InsightCard, InsightType, Sentiment};

pub struct InsightEngine {
    repos: ProductivityRepos,
}

struct Baseline {
    avg_deep_work_blocks: f64,
    avg_distraction_rate: f64,
    avg_recovery_secs: f64,
    max_score_30d: f64,
    /// Pre-fetched 14-day summaries, reused by check_streak to avoid redundant query.
    summaries_14: Vec<crate::types::DailySummary>,
}

impl InsightEngine {
    pub fn new(repos: ProductivityRepos) -> Self {
        Self { repos }
    }

    /// Generate all applicable insights for a given date.
    pub async fn generate_for_date(&self, date: &str) -> common::Result<Vec<InsightCard>> {
        let today = match self.repos.summaries.get(date).await? {
            Some(s) => s,
            None => return Ok(vec![]),
        };

        let baseline = self.compute_baseline(date).await?;

        let today_distraction_rate = if today.total_active_secs > 0 {
            today.distracting_secs as f64 / today.total_active_secs as f64
        } else {
            0.0
        };
        let score = today.productivity_score.unwrap_or(0.0);

        // All checks are independent — run in a single try_join!
        let (deep_work, distraction, personal_best, streak, fatigue, recovery) = tokio::try_join!(
            self.check_deep_work_trend(date, today.deep_work_blocks, &baseline),
            self.check_distraction_spike(date, today_distraction_rate, &baseline),
            self.check_new_personal_best(date, score, &baseline),
            self.check_streak(date, 60.0, &baseline.summaries_14),
            self.check_fatigue_warning(date),
            self.check_recovery_improvement(date, &today, &baseline),
        )?;

        let cards: Vec<InsightCard> = [
            deep_work,
            distraction,
            personal_best,
            streak,
            fatigue,
            recovery,
        ]
        .into_iter()
        .flatten()
        .collect();

        Ok(cards)
    }

    /// Returns true if an insight of this type already exists for the date.
    async fn already_exists(&self, t: InsightType, date: &str) -> common::Result<bool> {
        self.repos.insights.exists_for_date(t, date).await
    }

    async fn compute_baseline(&self, end_date: &str) -> common::Result<Baseline> {
        let end = chrono::NaiveDate::parse_from_str(end_date, "%Y-%m-%d")
            .map_err(|e| common::ToolError::InvalidParams(format!("invalid date: {e}")))?;
        let start_14 = (end - chrono::Duration::days(14))
            .format("%Y-%m-%d")
            .to_string();
        let start_30 = (end - chrono::Duration::days(30))
            .format("%Y-%m-%d")
            .to_string();

        // Fetch 30-day once; partition in memory to get the 14-day subset
        let summaries_30 = self.repos.summaries.list_range(&start_30, end_date).await?;
        let summaries_14: Vec<_> = summaries_30
            .iter()
            .filter(|s| s.date.as_str() >= start_14.as_str())
            .cloned()
            .collect();

        let n = summaries_14.len().max(1) as f64;

        let avg_deep_work_blocks = summaries_14
            .iter()
            .map(|s| s.deep_work_blocks as f64)
            .sum::<f64>()
            / n;

        let avg_distraction_rate = summaries_14
            .iter()
            .map(|s| {
                if s.total_active_secs > 0 {
                    s.distracting_secs as f64 / s.total_active_secs as f64
                } else {
                    0.0
                }
            })
            .sum::<f64>()
            / n;

        let avg_recovery_secs = summaries_14
            .iter()
            .filter_map(|s| s.avg_recovery_secs)
            .sum::<f64>()
            / summaries_14
                .iter()
                .filter(|s| s.avg_recovery_secs.is_some())
                .count()
                .max(1) as f64;

        let max_score_30d = summaries_30
            .iter()
            .filter_map(|s| s.productivity_score)
            .fold(0.0f64, f64::max);

        Ok(Baseline {
            avg_deep_work_blocks,
            avg_distraction_rate,
            avg_recovery_secs,
            max_score_30d,
            summaries_14,
        })
    }

    async fn check_deep_work_trend(
        &self,
        date: &str,
        today_blocks: i64,
        baseline: &Baseline,
    ) -> common::Result<Option<InsightCard>> {
        if self.already_exists(InsightType::DeepWorkTrend, date).await? {
            return Ok(None);
        }

        if today_blocks as f64 > baseline.avg_deep_work_blocks + 1.0 {
            let card = InsightCard {
                id: format!("deep-work-{date}"),
                insight_type: InsightType::DeepWorkTrend,
                title: "Strong deep work day".to_string(),
                body: format!(
                    "You had {} deep work blocks today — above your {:.1} average.",
                    today_blocks, baseline.avg_deep_work_blocks
                ),
                sentiment: Sentiment::Positive,
                metric_value: Some(today_blocks as f64),
                baseline_value: Some(baseline.avg_deep_work_blocks),
                date: date.to_string(),
                dismissed: false,
                generated_at: Utc::now(),
            };
            self.repos.insights.upsert(&card).await?;
            return Ok(Some(card));
        }
        Ok(None)
    }

    async fn check_distraction_spike(
        &self,
        date: &str,
        today_rate: f64,
        baseline: &Baseline,
    ) -> common::Result<Option<InsightCard>> {
        if self.already_exists(InsightType::DistractionSpike, date).await? {
            return Ok(None);
        }

        if baseline.avg_distraction_rate > 0.0 && today_rate > baseline.avg_distraction_rate * 1.5 {
            let card = InsightCard {
                id: format!("distraction-spike-{date}"),
                insight_type: InsightType::DistractionSpike,
                title: "Higher distractions today".to_string(),
                body: format!(
                    "Distraction rate is {:.0}% — 1.5x your {:.0}% average.",
                    today_rate * 100.0,
                    baseline.avg_distraction_rate * 100.0
                ),
                sentiment: Sentiment::Warning,
                metric_value: Some(today_rate),
                baseline_value: Some(baseline.avg_distraction_rate),
                date: date.to_string(),
                dismissed: false,
                generated_at: Utc::now(),
            };
            self.repos.insights.upsert(&card).await?;
            return Ok(Some(card));
        }
        Ok(None)
    }

    async fn check_new_personal_best(
        &self,
        date: &str,
        today_score: f64,
        baseline: &Baseline,
    ) -> common::Result<Option<InsightCard>> {
        if self.already_exists(InsightType::NewPersonalBest, date).await? {
            return Ok(None);
        }

        if today_score > baseline.max_score_30d && baseline.max_score_30d > 0.0 {
            let card = InsightCard {
                id: format!("personal-best-{date}"),
                insight_type: InsightType::NewPersonalBest,
                title: "New personal best!".to_string(),
                body: format!(
                    "Score of {:.0} beats your 30-day record of {:.0}.",
                    today_score, baseline.max_score_30d
                ),
                sentiment: Sentiment::Positive,
                metric_value: Some(today_score),
                baseline_value: Some(baseline.max_score_30d),
                date: date.to_string(),
                dismissed: false,
                generated_at: Utc::now(),
            };
            self.repos.insights.upsert(&card).await?;
            return Ok(Some(card));
        }
        Ok(None)
    }

    async fn check_streak(
        &self,
        date: &str,
        threshold: f64,
        summaries: &[crate::types::DailySummary],
    ) -> common::Result<Option<InsightCard>> {
        if self.already_exists(InsightType::StreakAchieved, date).await? {
            return Ok(None);
        }

        // Count consecutive days from end that meet threshold
        let mut streak = 0u32;
        for s in summaries.iter().rev() {
            if s.productivity_score.unwrap_or(0.0) >= threshold {
                streak += 1;
            } else {
                break;
            }
        }

        if streak >= 3 {
            let card = InsightCard {
                id: format!("streak-{date}"),
                insight_type: InsightType::StreakAchieved,
                title: format!("{streak}-day streak!"),
                body: format!(
                    "You've scored {:.0}+ for {streak} consecutive days. Keep it up!",
                    threshold
                ),
                sentiment: Sentiment::Positive,
                metric_value: Some(streak as f64),
                baseline_value: None,
                date: date.to_string(),
                dismissed: false,
                generated_at: Utc::now(),
            };
            self.repos.insights.upsert(&card).await?;
            return Ok(Some(card));
        }
        Ok(None)
    }

    async fn check_fatigue_warning(&self, date: &str) -> common::Result<Option<InsightCard>> {
        if self.already_exists(InsightType::FatigueWarning, date).await? {
            return Ok(None);
        }

        let patterns = self
            .repos
            .distraction_patterns
            .list_range(date, date)
            .await?;
        if patterns.len() < 3 {
            return Ok(None);
        }

        // Check if distraction rate increased after 3+ hours active
        let late_distractions = patterns
            .iter()
            .filter(|p| p.hours_active_today > 3.0)
            .count();
        let early_distractions = patterns
            .iter()
            .filter(|p| p.hours_active_today <= 3.0)
            .count();

        if late_distractions > 0 && late_distractions > early_distractions * 2 {
            let card = InsightCard {
                id: format!("fatigue-{date}"),
                insight_type: InsightType::FatigueWarning,
                title: "Fatigue detected".to_string(),
                body: format!(
                    "Distractions increased after 3h of work ({late_distractions} vs {early_distractions} early). Consider taking breaks."
                ),
                sentiment: Sentiment::Warning,
                metric_value: Some(late_distractions as f64),
                baseline_value: Some(early_distractions as f64),
                date: date.to_string(),
                dismissed: false,
                generated_at: Utc::now(),
            };
            self.repos.insights.upsert(&card).await?;
            return Ok(Some(card));
        }
        Ok(None)
    }

    async fn check_recovery_improvement(
        &self,
        date: &str,
        today: &crate::types::DailySummary,
        baseline: &Baseline,
    ) -> common::Result<Option<InsightCard>> {
        if self.already_exists(InsightType::RecoveryImprovement, date).await? {
            return Ok(None);
        }

        // Use the pre-fetched avg_recovery_secs from the daily summary
        // instead of issuing a duplicate DB query.
        if let Some(today_recovery) = today.avg_recovery_secs {
            if baseline.avg_recovery_secs > 0.0 && today_recovery < baseline.avg_recovery_secs * 0.7
            {
                let card = InsightCard {
                    id: format!("recovery-{date}"),
                    insight_type: InsightType::RecoveryImprovement,
                    title: "Faster recovery!".to_string(),
                    body: format!(
                        "Average recovery time is {:.0}s — down from {:.0}s baseline.",
                        today_recovery, baseline.avg_recovery_secs
                    ),
                    sentiment: Sentiment::Positive,
                    metric_value: Some(today_recovery),
                    baseline_value: Some(baseline.avg_recovery_secs),
                    date: date.to_string(),
                    dismissed: false,
                    generated_at: Utc::now(),
                };
                self.repos.insights.upsert(&card).await?;
                return Ok(Some(card));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::ProductivityRepos;
    use crate::types::DailySummary;
    use crate::ProductivityFeature;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    fn make_summary(
        date: &str,
        score: f64,
        productive_secs: i64,
        deep_work_blocks: i64,
    ) -> DailySummary {
        DailySummary {
            date: date.to_string(),
            total_active_secs: 28800,
            total_focus_secs: 14400,
            total_break_secs: 3600,
            total_idle_secs: 3600,
            productive_secs,
            neutral_secs: 3600,
            distracting_secs: 3600,
            focus_sessions_count: 4,
            avg_session_quality: Some(0.8),
            interruptions_count: 5,
            context_switches: 20,
            top_apps: vec![],
            top_categories: vec![],
            top_projects: vec![],
            productivity_score: Some(score),
            ai_summary: None,
            deep_work_blocks,
            deep_work_secs: deep_work_blocks * 1500, // 25 min each
            avg_recovery_secs: Some(60.0),
        }
    }

    #[tokio::test]
    async fn test_no_insights_without_data() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);
        let engine = InsightEngine::new(repos);

        let cards = engine.generate_for_date("2026-03-06").await.unwrap();
        assert!(cards.is_empty());
    }

    #[tokio::test]
    async fn test_deep_work_insight() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);

        // Create baseline with low deep work (within 14-day lookback from 2026-03-06)
        for i in 20..=28 {
            let date = format!("2026-02-{:02}", i);
            let s = make_summary(&date, 70.0, 21600, 1);
            repos.summaries.upsert(&s).await.unwrap();
        }
        for i in 1..=5 {
            let date = format!("2026-03-{:02}", i);
            let s = make_summary(&date, 70.0, 21600, 1);
            repos.summaries.upsert(&s).await.unwrap();
        }

        // Today has high deep work
        let today = make_summary("2026-03-06", 80.0, 25200, 5);
        repos.summaries.upsert(&today).await.unwrap();

        let engine = InsightEngine::new(repos);
        let cards = engine.generate_for_date("2026-03-06").await.unwrap();

        let deep_work = cards
            .iter()
            .find(|c| c.insight_type == InsightType::DeepWorkTrend);
        assert!(deep_work.is_some());
        assert_eq!(deep_work.unwrap().sentiment, Sentiment::Positive);
    }

    #[tokio::test]
    async fn test_deduplication() {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool);

        for i in 20..=28 {
            let date = format!("2026-02-{:02}", i);
            repos
                .summaries
                .upsert(&make_summary(&date, 70.0, 21600, 1))
                .await
                .unwrap();
        }
        for i in 1..=5 {
            let date = format!("2026-03-{:02}", i);
            repos
                .summaries
                .upsert(&make_summary(&date, 70.0, 21600, 1))
                .await
                .unwrap();
        }
        repos
            .summaries
            .upsert(&make_summary("2026-03-06", 80.0, 25200, 5))
            .await
            .unwrap();

        let engine = InsightEngine::new(repos);

        let first = engine.generate_for_date("2026-03-06").await.unwrap();
        let second = engine.generate_for_date("2026-03-06").await.unwrap();

        // Second run should produce no new deep work insights
        let deep_work_second = second
            .iter()
            .filter(|c| c.insight_type == InsightType::DeepWorkTrend)
            .count();
        assert_eq!(deep_work_second, 0);
        assert!(first.len() >= second.len());
    }
}
