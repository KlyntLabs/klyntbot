//! Pattern analyzer — detects behavioral patterns from interaction logs.
//!
//! Runs periodically in the LearningService background loop. Analyzes:
//! - Day-of-week patterns (which agents are used on which days)
//! - Time-of-day patterns (morning/afternoon/evening usage)
//! - Agent usage frequency

use std::collections::HashMap;

use chrono::{Datelike, Timelike};
use tracing::warn;

/// Minimum interactions before patterns can be detected.
const MIN_INTERACTIONS_FOR_ANALYSIS: usize = 10;

/// Minimum occurrences for a pattern to be considered significant.
const MIN_PATTERN_OCCURRENCES: i32 = 5;

/// Analyzes interaction logs to detect behavioral patterns.
pub struct PatternAnalyzer {
    log_repo: storage::InteractionLogRepo,
    pattern_repo: storage::BehavioralPatternRepo,
}

impl PatternAnalyzer {
    pub fn new(
        log_repo: storage::InteractionLogRepo,
        pattern_repo: storage::BehavioralPatternRepo,
    ) -> Self {
        Self {
            log_repo,
            pattern_repo,
        }
    }

    /// Run all pattern analyses.
    pub async fn analyze(&self) -> common::Result<()> {
        let logs = self.log_repo.list_recent(1000).await?;

        if logs.len() < MIN_INTERACTIONS_FOR_ANALYSIS {
            return Ok(());
        }

        // Single pass: accumulate all three pattern types simultaneously
        let mut day_agent_counts: HashMap<(String, String), i32> = HashMap::new();
        let mut time_counts: HashMap<String, i32> = HashMap::new();

        for log in &logs {
            if let Ok(dt) =
                chrono::NaiveDateTime::parse_from_str(&log.timestamp, "%Y-%m-%d %H:%M:%S")
            {
                // Day-of-week × agent
                let day = match dt.weekday() {
                    chrono::Weekday::Mon => "monday",
                    chrono::Weekday::Tue => "tuesday",
                    chrono::Weekday::Wed => "wednesday",
                    chrono::Weekday::Thu => "thursday",
                    chrono::Weekday::Fri => "friday",
                    chrono::Weekday::Sat => "saturday",
                    chrono::Weekday::Sun => "sunday",
                };
                *day_agent_counts
                    .entry((day.to_string(), log.agent_name.clone()))
                    .or_default() += 1;

                // Time-of-day
                let period = match dt.hour() {
                    5..=11 => "morning",
                    12..=17 => "afternoon",
                    18..=22 => "evening",
                    _ => "night",
                };
                *time_counts.entry(period.to_string()).or_default() += 1;
            }
        }

        // Flush day-of-week patterns
        for ((day, agent), count) in &day_agent_counts {
            if *count >= MIN_PATTERN_OCCURRENCES {
                let key = format!("{}_{}", day, agent);
                let value = serde_json::json!({ "day": day, "agent": agent });
                if let Err(e) = self
                    .pattern_repo
                    .upsert("day_of_week", &key, &value, *count)
                    .await
                {
                    warn!("Failed to upsert day_of_week pattern: {}", e);
                }
            }
        }

        // Flush time-of-day patterns
        for (period, count) in &time_counts {
            if *count >= MIN_PATTERN_OCCURRENCES {
                let value = serde_json::json!({ "period": period, "count": count });
                if let Err(e) = self
                    .pattern_repo
                    .upsert("time_of_day", period, &value, *count)
                    .await
                {
                    warn!("Failed to upsert time_of_day pattern: {}", e);
                }
            }
        }

        // Agent usage: use the repo's SQL GROUP BY instead of in-memory counting
        // (correct across full table, not limited to the last 1000 rows)
        let agent_counts = self.log_repo.count_by_agent().await?;
        for (agent, count) in &agent_counts {
            let count_i32 = *count as i32;
            if count_i32 >= MIN_PATTERN_OCCURRENCES {
                let value = serde_json::json!({ "agent": agent, "total_uses": count });
                if let Err(e) = self
                    .pattern_repo
                    .upsert("agent_usage", agent, &value, count_i32)
                    .await
                {
                    warn!("Failed to upsert agent_usage pattern: {}", e);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_record_and_analyze_interactions() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);

        // Insert 15 interactions on a Monday with agent="task"
        for _ in 0..15 {
            repos
                .interaction_log
                .create_with_timestamp(
                    "task",
                    &["task"],
                    "telegram",
                    Some(100),
                    "2026-03-02 10:00:00", // Monday
                )
                .await
                .unwrap();
        }

        let analyzer = PatternAnalyzer::new(
            repos.interaction_log.clone(),
            repos.behavioral_patterns.clone(),
        );
        analyzer.analyze().await.unwrap();

        let patterns = repos
            .behavioral_patterns
            .list_by_type("day_of_week")
            .await
            .unwrap();
        assert!(!patterns.is_empty(), "Should detect day_of_week pattern");

        let agent_patterns = repos
            .behavioral_patterns
            .list_by_type("agent_usage")
            .await
            .unwrap();
        assert!(
            !agent_patterns.is_empty(),
            "Should detect agent_usage pattern"
        );

        let time_patterns = repos
            .behavioral_patterns
            .list_by_type("time_of_day")
            .await
            .unwrap();
        assert!(
            !time_patterns.is_empty(),
            "Should detect time_of_day pattern"
        );
    }

    #[tokio::test]
    async fn test_insufficient_data_skips_analysis() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);

        // Only 3 interactions — below threshold
        for _ in 0..3 {
            repos
                .interaction_log
                .create("task", &["task"], "telegram", Some(100))
                .await
                .unwrap();
        }

        let analyzer = PatternAnalyzer::new(
            repos.interaction_log.clone(),
            repos.behavioral_patterns.clone(),
        );
        analyzer.analyze().await.unwrap();

        let patterns = repos.behavioral_patterns.list_all().await.unwrap();
        assert!(
            patterns.is_empty(),
            "Should not detect patterns with insufficient data"
        );
    }
}
