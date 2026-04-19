//! Pattern analyzer — detects behavioral patterns from interaction logs.
//!
//! Runs periodically in the LearningService background loop. Analyzes:
//! - Day-of-week patterns (which agents are used on which days)
//! - Time-of-day patterns (morning/afternoon/evening usage)
//! - Agent usage frequency

use std::collections::HashMap;
use std::sync::Arc;

/// Minimum interactions before patterns can be detected.
const MIN_INTERACTIONS_FOR_ANALYSIS: usize = 10;

/// Minimum occurrences for a pattern to be considered significant.
const MIN_PATTERN_OCCURRENCES: i32 = 5;

/// Analyzes interaction logs to detect behavioral patterns.
///
/// Emits `DomainEvent::BehavioralPatternDetected` events instead of writing
/// to the (now-removed) `behavioral_patterns` table. The cognitive pipeline
/// processes these events into procedural rules via salience → extraction →
/// consolidation.
pub struct PatternAnalyzer {
    log_repo: storage::InteractionLogRepo,
    event_bus: Arc<bus::DomainEventBus>,
}

impl PatternAnalyzer {
    pub fn new(log_repo: storage::InteractionLogRepo, event_bus: Arc<bus::DomainEventBus>) -> Self {
        Self {
            log_repo,
            event_bus,
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
            // Parse timestamp as jiff civil datetime (local time, no tz)
            if let Ok(dt) = log
                .timestamp
                .parse::<jiff::civil::DateTime>()
                .or_else(|_| jiff::civil::DateTime::strptime("%Y-%m-%d %H:%M:%S", &log.timestamp))
            {
                // Day-of-week × agent
                let day = match dt.weekday() {
                    jiff::civil::Weekday::Monday => "monday",
                    jiff::civil::Weekday::Tuesday => "tuesday",
                    jiff::civil::Weekday::Wednesday => "wednesday",
                    jiff::civil::Weekday::Thursday => "thursday",
                    jiff::civil::Weekday::Friday => "friday",
                    jiff::civil::Weekday::Saturday => "saturday",
                    jiff::civil::Weekday::Sunday => "sunday",
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

        // Emit day-of-week patterns
        for ((day, agent), count) in &day_agent_counts {
            if *count >= MIN_PATTERN_OCCURRENCES {
                self.event_bus
                    .publish(bus::DomainEvent::BehavioralPatternDetected {
                        pattern_type: "day_of_week".into(),
                        pattern_key: format!("{}_{}", day, agent),
                        sample_count: *count,
                        detail: format!(
                            "User uses {} agent frequently on {}s ({} interactions)",
                            agent, day, count
                        ),
                    });
            }
        }

        // Emit time-of-day patterns
        for (period, count) in &time_counts {
            if *count >= MIN_PATTERN_OCCURRENCES {
                self.event_bus
                    .publish(bus::DomainEvent::BehavioralPatternDetected {
                        pattern_type: "time_of_day".into(),
                        pattern_key: period.clone(),
                        sample_count: *count,
                        detail: format!(
                            "User is most active in the {} ({} interactions)",
                            period, count
                        ),
                    });
            }
        }

        // Agent usage: use the repo's SQL GROUP BY
        let agent_counts = self.log_repo.count_by_agent().await?;
        for (agent, count) in &agent_counts {
            let count_i32 = *count as i32;
            if count_i32 >= MIN_PATTERN_OCCURRENCES {
                self.event_bus
                    .publish(bus::DomainEvent::BehavioralPatternDetected {
                        pattern_type: "agent_usage".into(),
                        pattern_key: agent.clone(),
                        sample_count: count_i32,
                        detail: format!(
                            "{} agent is used frequently ({} total uses)",
                            agent, count
                        ),
                    });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pattern_analyzer_emits_domain_events() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repos = storage::Repos::from_pool(&pool);

        // Insert 15 interactions on a Monday morning with agent="task"
        for _ in 0..15 {
            repos
                .interaction_log
                .create_with_timestamp(
                    "task",
                    &["task"],
                    "telegram",
                    Some(100),
                    "2026-03-02 10:00:00", // Monday morning
                )
                .await
                .unwrap();
        }

        let event_bus = Arc::new(bus::DomainEventBus::new(64));
        let mut rx = event_bus.subscribe();

        let analyzer = PatternAnalyzer::new(repos.interaction_log.clone(), Arc::clone(&event_bus));
        analyzer.analyze().await.unwrap();

        // Collect all published events
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        // Should have: day_of_week + time_of_day + agent_usage = at least 3 events
        assert!(
            events.len() >= 3,
            "Expected at least 3 events, got {}",
            events.len()
        );

        let has_day = events.iter().any(|e| {
            matches!(
                e, bus::DomainEvent::BehavioralPatternDetected { pattern_type, .. }
                if pattern_type == "day_of_week"
            )
        });
        assert!(has_day, "Expected a day_of_week pattern event");

        let has_time = events.iter().any(|e| {
            matches!(
                e, bus::DomainEvent::BehavioralPatternDetected { pattern_type, .. }
                if pattern_type == "time_of_day"
            )
        });
        assert!(has_time, "Expected a time_of_day pattern event");

        let has_agent = events.iter().any(|e| {
            matches!(
                e, bus::DomainEvent::BehavioralPatternDetected { pattern_type, .. }
                if pattern_type == "agent_usage"
            )
        });
        assert!(has_agent, "Expected an agent_usage pattern event");
    }

    #[tokio::test]
    async fn test_insufficient_data_emits_no_events() {
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

        let event_bus = Arc::new(bus::DomainEventBus::new(64));
        let mut rx = event_bus.subscribe();

        let analyzer = PatternAnalyzer::new(repos.interaction_log.clone(), Arc::clone(&event_bus));
        analyzer.analyze().await.unwrap();

        assert!(
            rx.try_recv().is_err(),
            "Should not emit events with insufficient data"
        );
    }
}
