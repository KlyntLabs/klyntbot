//! Feedback tracker — three channels of feedback for coaching interventions.
//!
//! 1. **Explicit:** User responds (helpful/dismiss/stop) via DomainEvent::CoachingFeedback
//! 2. **Behavioral:** Monitor post-intervention behavior (productive switch = positive)
//! 3. **Outcome:** Compare daily scores on days with vs without interventions

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

use bus::FeedbackResponse;

use crate::router::DeliveredIntervention;

/// Aggregated feedback for a coaching strategy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyFeedback {
    pub strategy_type: String,
    pub domain: String,
    pub times_used: i32,
    pub times_accepted: i32,
    pub times_dismissed: i32,
    pub times_stopped: i32,
    pub behavioral_positive: i32,
    pub behavioral_negative: i32,
    pub times_led_to_improvement: i32,
    pub avg_improvement_magnitude: f64,
}

impl StrategyFeedback {
    /// Acceptance rate (0.0–1.0).
    pub fn acceptance_rate(&self) -> f64 {
        if self.times_used == 0 {
            0.5
        } else {
            self.times_accepted as f64 / self.times_used as f64
        }
    }

    /// Effectiveness score combining explicit and behavioral signals.
    pub fn effectiveness(&self) -> f64 {
        let total = self.times_used.max(1) as f64;
        let positive = (self.times_accepted + self.behavioral_positive) as f64;
        let negative =
            (self.times_dismissed + self.times_stopped + self.behavioral_negative) as f64;
        (positive - negative * 0.5) / total
    }
}

/// Tracks feedback across all coaching strategies.
pub struct FeedbackTracker {
    /// Strategy feedback indexed by trigger name.
    strategies: HashMap<String, StrategyFeedback>,
    /// Pending interventions awaiting behavioral feedback.
    pending_behavioral: Vec<PendingBehavioral>,
    /// Behavioral monitoring window in seconds.
    behavioral_window_secs: i64,
    /// Optional SQL persistence for coaching strategies.
    repo: Option<storage::CoachingStrategyRepo>,
}

#[derive(Debug, Clone)]
struct PendingBehavioral {
    intervention: DeliveredIntervention,
    expires_at: DateTime<Utc>,
}

impl FeedbackTracker {
    pub fn new() -> Self {
        Self {
            strategies: HashMap::new(),
            pending_behavioral: Vec::new(),
            behavioral_window_secs: 120, // 2 minutes
            repo: None,
        }
    }

    pub fn with_repo(mut self, repo: storage::CoachingStrategyRepo) -> Self {
        self.repo = Some(repo);
        self
    }

    /// Persist all strategy feedback to SQL. No-op if no repo is configured.
    pub async fn persist(&self) {
        let repo = match &self.repo {
            Some(r) => r,
            None => return,
        };
        for feedback in self.strategies.values() {
            let input = storage::UpsertCoachingStrategy {
                strategy_type: &feedback.strategy_type,
                domain: &feedback.domain,
                times_used: feedback.times_used,
                times_accepted: feedback.times_accepted,
                times_led_to_improvement: feedback.times_led_to_improvement,
                avg_improvement_magnitude: if feedback.avg_improvement_magnitude == 0.0 {
                    None
                } else {
                    Some(feedback.avg_improvement_magnitude)
                },
                confidence: feedback.effectiveness().max(0.0),
            };
            if let Err(e) = repo.upsert(&input).await {
                tracing::warn!(
                    "Failed to persist coaching strategy '{}': {e}",
                    feedback.strategy_type
                );
            }
        }
    }

    /// Load previously persisted strategies from SQL. No-op if no repo.
    pub async fn load_from_db(&mut self) {
        let repo = match &self.repo {
            Some(r) => r,
            None => return,
        };
        match repo.list_all().await {
            Ok(rows) => {
                for row in rows {
                    self.strategies
                        .entry(row.strategy_type.clone())
                        .or_insert_with(|| StrategyFeedback {
                            strategy_type: row.strategy_type,
                            domain: row.domain,
                            times_used: row.times_used,
                            times_accepted: row.times_accepted,
                            times_led_to_improvement: row.times_led_to_improvement,
                            avg_improvement_magnitude: row.avg_improvement_magnitude.unwrap_or(0.0),
                            ..Default::default()
                        });
                }
            }
            Err(e) => tracing::warn!("Failed to load coaching strategies from DB: {e}"),
        }
    }

    /// Record that an intervention was delivered (start tracking).
    pub fn record_delivery(&mut self, intervention: &DeliveredIntervention) {
        let strategy = self
            .strategies
            .entry(intervention.trigger_name.clone())
            .or_insert_with(|| StrategyFeedback {
                strategy_type: intervention.trigger_name.clone(),
                domain: "coaching".into(),
                ..Default::default()
            });
        strategy.times_used += 1;

        self.pending_behavioral.push(PendingBehavioral {
            intervention: intervention.clone(),
            expires_at: Utc::now() + chrono::Duration::seconds(self.behavioral_window_secs),
        });
    }

    /// Record explicit user feedback.
    pub fn record_explicit(&mut self, intervention_id: &str, response: &FeedbackResponse) {
        // Find the trigger name for this intervention
        let trigger_name = self
            .pending_behavioral
            .iter()
            .find(|p| p.intervention.id == intervention_id)
            .map(|p| p.intervention.trigger_name.clone());

        if let Some(trigger_name) = trigger_name {
            let strategy = self
                .strategies
                .entry(trigger_name.clone())
                .or_insert_with(|| StrategyFeedback {
                    strategy_type: trigger_name,
                    domain: "coaching".into(),
                    ..Default::default()
                });

            match response {
                FeedbackResponse::Helpful => {
                    strategy.times_accepted += 1;
                    debug!(
                        "Explicit feedback: helpful for '{}'",
                        strategy.strategy_type
                    );
                }
                FeedbackResponse::Dismissed => {
                    strategy.times_dismissed += 1;
                    debug!(
                        "Explicit feedback: dismissed for '{}'",
                        strategy.strategy_type
                    );
                }
                FeedbackResponse::StopSuggesting => {
                    strategy.times_stopped += 1;
                    debug!("Explicit feedback: stop for '{}'", strategy.strategy_type);
                }
            }

            // Remove from pending behavioral
            self.pending_behavioral
                .retain(|p| p.intervention.id != intervention_id);
        }
    }

    /// Record behavioral signal: user behavior changed after intervention.
    pub fn record_behavioral_positive(&mut self, trigger_name: &str) {
        if let Some(strategy) = self.strategies.get_mut(trigger_name) {
            strategy.behavioral_positive += 1;
            debug!("Behavioral positive for '{trigger_name}'");
        }
    }

    /// Record behavioral signal: no behavior change after intervention.
    pub fn record_behavioral_negative(&mut self, trigger_name: &str) {
        if let Some(strategy) = self.strategies.get_mut(trigger_name) {
            strategy.behavioral_negative += 1;
            debug!("Behavioral negative for '{trigger_name}'");
        }
    }

    /// Check for expired behavioral monitoring windows.
    /// Returns trigger names of interventions that expired without positive signal.
    pub fn check_expired_behavioral(&mut self) -> Vec<String> {
        let now = Utc::now();
        let expired: Vec<_> = self
            .pending_behavioral
            .iter()
            .filter(|p| p.expires_at < now)
            .map(|p| p.intervention.trigger_name.clone())
            .collect();

        for name in &expired {
            self.record_behavioral_negative(name);
        }

        self.pending_behavioral.retain(|p| p.expires_at >= now);
        expired
    }

    /// Get feedback for a specific strategy.
    pub fn get_strategy(&self, trigger_name: &str) -> Option<&StrategyFeedback> {
        self.strategies.get(trigger_name)
    }

    /// Get all strategy feedback (for weekly reflection).
    pub fn all_strategies(&self) -> Vec<&StrategyFeedback> {
        self.strategies.values().collect()
    }

    /// Compute coaching receptivity adjustment based on recent feedback.
    /// Positive value = increase receptivity, negative = decrease.
    pub fn receptivity_adjustment(&self) -> f64 {
        let total_used: i32 = self.strategies.values().map(|s| s.times_used).sum();
        if total_used == 0 {
            return 0.0;
        }

        let total_accepted: f64 = self
            .strategies
            .values()
            .map(|s| s.times_accepted as f64)
            .sum();
        let total_dismissed: f64 = self
            .strategies
            .values()
            .map(|s| (s.times_dismissed + s.times_stopped) as f64)
            .sum();

        let acceptance_ratio = total_accepted / total_used as f64;
        let dismiss_ratio = total_dismissed / total_used as f64;

        // Range: -0.3 to +0.2
        (acceptance_ratio * 0.2 - dismiss_ratio * 0.3).clamp(-0.3, 0.2)
    }
}

impl Default for FeedbackTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoner::InterventionType;

    fn test_intervention(trigger: &str) -> DeliveredIntervention {
        DeliveredIntervention {
            id: uuid::Uuid::new_v4().to_string(),
            intervention_type: InterventionType::ChatMessage,
            message: "test".into(),
            delivered_at: Utc::now(),
            trigger_name: trigger.into(),
        }
    }

    #[test]
    fn test_record_delivery_increments_usage() {
        let mut tracker = FeedbackTracker::new();
        let intervention = test_intervention("distraction_streak");
        tracker.record_delivery(&intervention);

        let strategy = tracker.get_strategy("distraction_streak").unwrap();
        assert_eq!(strategy.times_used, 1);
    }

    #[test]
    fn test_explicit_helpful_feedback() {
        let mut tracker = FeedbackTracker::new();
        let intervention = test_intervention("distraction_streak");
        let id = intervention.id.clone();
        tracker.record_delivery(&intervention);

        tracker.record_explicit(&id, &FeedbackResponse::Helpful);

        let strategy = tracker.get_strategy("distraction_streak").unwrap();
        assert_eq!(strategy.times_accepted, 1);
        assert_eq!(strategy.acceptance_rate(), 1.0);
    }

    #[test]
    fn test_explicit_dismissed_feedback() {
        let mut tracker = FeedbackTracker::new();
        let intervention = test_intervention("distraction_streak");
        let id = intervention.id.clone();
        tracker.record_delivery(&intervention);

        tracker.record_explicit(&id, &FeedbackResponse::Dismissed);

        let strategy = tracker.get_strategy("distraction_streak").unwrap();
        assert_eq!(strategy.times_dismissed, 1);
        assert_eq!(strategy.acceptance_rate(), 0.0);
    }

    #[test]
    fn test_behavioral_positive() {
        let mut tracker = FeedbackTracker::new();
        let intervention = test_intervention("distraction_streak");
        tracker.record_delivery(&intervention);

        tracker.record_behavioral_positive("distraction_streak");

        let strategy = tracker.get_strategy("distraction_streak").unwrap();
        assert_eq!(strategy.behavioral_positive, 1);
    }

    #[test]
    fn test_effectiveness_score() {
        let mut tracker = FeedbackTracker::new();

        for _ in 0..5 {
            let intervention = test_intervention("good_strategy");
            let id = intervention.id.clone();
            tracker.record_delivery(&intervention);
            tracker.record_explicit(&id, &FeedbackResponse::Helpful);
        }

        let strategy = tracker.get_strategy("good_strategy").unwrap();
        assert!(strategy.effectiveness() > 0.5);
    }

    #[test]
    fn test_receptivity_adjustment_positive() {
        let mut tracker = FeedbackTracker::new();

        for _ in 0..5 {
            let intervention = test_intervention("good");
            let id = intervention.id.clone();
            tracker.record_delivery(&intervention);
            tracker.record_explicit(&id, &FeedbackResponse::Helpful);
        }

        assert!(tracker.receptivity_adjustment() > 0.0);
    }

    #[test]
    fn test_receptivity_adjustment_negative() {
        let mut tracker = FeedbackTracker::new();

        for _ in 0..5 {
            let intervention = test_intervention("bad");
            let id = intervention.id.clone();
            tracker.record_delivery(&intervention);
            tracker.record_explicit(&id, &FeedbackResponse::Dismissed);
        }

        assert!(tracker.receptivity_adjustment() < 0.0);
    }

    #[test]
    fn test_all_strategies() {
        let mut tracker = FeedbackTracker::new();
        tracker.record_delivery(&test_intervention("a"));
        tracker.record_delivery(&test_intervention("b"));
        assert_eq!(tracker.all_strategies().len(), 2);
    }
}
