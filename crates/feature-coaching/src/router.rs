//! Intervention router — routes coaching decisions to delivery channels
//! with rate limiting, cooldown, and adaptive tolerance.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::reasoner::{CoachingDecision, InterventionType};

/// Delivered intervention record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveredIntervention {
    pub id: String,
    pub intervention_type: InterventionType,
    pub message: String,
    pub delivered_at: DateTime<Utc>,
    pub trigger_name: String,
}

/// Routing decision after rate limiting.
#[derive(Debug, Clone)]
pub enum RoutingResult {
    /// Intervention delivered to the specified channel.
    Delivered(DeliveredIntervention),
    /// Intervention suppressed by rate limiting.
    RateLimited { reason: String },
    /// No intervention needed.
    Skipped,
}

/// Configuration for the intervention router.
#[derive(Debug, Clone)]
pub struct RouterConfig {
    /// Maximum interventions per hour.
    pub max_per_hour: usize,
    /// Maximum interventions per day.
    pub max_per_day: usize,
    /// Cooldown in seconds after a dismissed intervention type.
    pub dismiss_cooldown_secs: i64,
    /// Base cooldown multiplier for exponential backoff on dismissed types.
    pub backoff_multiplier: f64,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            max_per_hour: 3,
            max_per_day: 10,
            dismiss_cooldown_secs: 1800, // 30min
            backoff_multiplier: 2.0,
        }
    }
}

/// Delivery channel abstraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InterventionChannel {
    DashboardCard,
    ChatMessage,
    Notification,
    Overlay,
}

impl From<&InterventionType> for InterventionChannel {
    fn from(t: &InterventionType) -> Self {
        match t {
            InterventionType::DashboardCard => InterventionChannel::DashboardCard,
            InterventionType::ChatMessage => InterventionChannel::ChatMessage,
            InterventionType::Notification => InterventionChannel::Notification,
            InterventionType::Overlay => InterventionChannel::Overlay,
            InterventionType::None => InterventionChannel::DashboardCard,
        }
    }
}

/// Routes coaching decisions to delivery channels with rate limiting.
pub struct InterventionRouter {
    config: RouterConfig,
    /// Recent deliveries for rate limiting.
    recent_deliveries: Vec<DeliveredIntervention>,
    /// Dismissal counts per intervention type for exponential backoff.
    dismissal_counts: HashMap<String, i32>,
    /// Last dismissal time per trigger name.
    last_dismissed: HashMap<String, DateTime<Utc>>,
}

impl InterventionRouter {
    pub fn new(config: RouterConfig) -> Self {
        Self {
            config,
            recent_deliveries: Vec::new(),
            dismissal_counts: HashMap::new(),
            last_dismissed: HashMap::new(),
        }
    }

    /// Route a coaching decision. Returns the routing result.
    pub fn route(&mut self, decision: &CoachingDecision, trigger_name: &str) -> RoutingResult {
        if !decision.should_intervene {
            return RoutingResult::Skipped;
        }

        let now = Utc::now();

        // Check dismiss cooldown with exponential backoff
        if let Some(last_dismiss) = self.last_dismissed.get(trigger_name) {
            let dismiss_count = self
                .dismissal_counts
                .get(trigger_name)
                .copied()
                .unwrap_or(0);
            let backoff = self.config.dismiss_cooldown_secs as f64
                * self.config.backoff_multiplier.powi(dismiss_count);
            let elapsed = (now - *last_dismiss).num_seconds() as f64;
            if elapsed < backoff {
                return RoutingResult::RateLimited {
                    reason: format!(
                        "Dismissed type '{trigger_name}' in cooldown ({:.0}s remaining)",
                        backoff - elapsed
                    ),
                };
            }
        }

        // Check hourly rate limit
        let hour_ago = now - chrono::Duration::hours(1);
        let hourly_count = self
            .recent_deliveries
            .iter()
            .filter(|d| d.delivered_at > hour_ago)
            .count();
        if hourly_count >= self.config.max_per_hour {
            return RoutingResult::RateLimited {
                reason: format!("Hourly limit ({}) reached", self.config.max_per_hour),
            };
        }

        // Check daily rate limit
        let day_ago = now - chrono::Duration::hours(24);
        let daily_count = self
            .recent_deliveries
            .iter()
            .filter(|d| d.delivered_at > day_ago)
            .count();
        if daily_count >= self.config.max_per_day {
            return RoutingResult::RateLimited {
                reason: format!("Daily limit ({}) reached", self.config.max_per_day),
            };
        }

        // Deliver the intervention
        let intervention = DeliveredIntervention {
            id: uuid::Uuid::new_v4().to_string(),
            intervention_type: decision.intervention_type.clone(),
            message: decision.message.clone().unwrap_or_default(),
            delivered_at: now,
            trigger_name: trigger_name.to_string(),
        };

        debug!(
            "Routing intervention '{}' via {:?}",
            trigger_name, decision.intervention_type
        );

        self.recent_deliveries.push(intervention.clone());

        // Prune old deliveries (keep last 24h)
        self.recent_deliveries.retain(|d| d.delivered_at > day_ago);

        RoutingResult::Delivered(intervention)
    }

    /// Record a dismissal for a trigger type (increases backoff).
    pub fn record_dismissal(&mut self, trigger_name: &str) {
        let count = self
            .dismissal_counts
            .entry(trigger_name.to_string())
            .or_insert(0);
        *count += 1;
        self.last_dismissed
            .insert(trigger_name.to_string(), Utc::now());
    }

    /// Reset dismissal tracking for a trigger type.
    pub fn reset_dismissals(&mut self, trigger_name: &str) {
        self.dismissal_counts.remove(trigger_name);
        self.last_dismissed.remove(trigger_name);
    }

    /// Get the count of interventions delivered in the last hour.
    pub fn hourly_count(&self) -> usize {
        let hour_ago = Utc::now() - chrono::Duration::hours(1);
        self.recent_deliveries
            .iter()
            .filter(|d| d.delivered_at > hour_ago)
            .count()
    }
}

impl Default for InterventionRouter {
    fn default() -> Self {
        Self::new(RouterConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoner::InterventionType;

    fn test_decision(intervene: bool, msg: &str) -> CoachingDecision {
        CoachingDecision {
            should_intervene: intervene,
            confidence: 0.8,
            message: Some(msg.into()),
            intervention_type: InterventionType::ChatMessage,
            reasoning: "test".into(),
            observations: vec![],
        }
    }

    #[test]
    fn test_routes_intervention() {
        let mut router = InterventionRouter::default();
        let decision = test_decision(true, "Take a break!");
        let result = router.route(&decision, "distraction_streak");
        assert!(matches!(result, RoutingResult::Delivered(_)));
    }

    #[test]
    fn test_skips_no_intervention() {
        let mut router = InterventionRouter::default();
        let decision = test_decision(false, "");
        let result = router.route(&decision, "test");
        assert!(matches!(result, RoutingResult::Skipped));
    }

    #[test]
    fn test_hourly_rate_limit() {
        let config = RouterConfig {
            max_per_hour: 2,
            ..Default::default()
        };
        let mut router = InterventionRouter::new(config);
        let decision = test_decision(true, "msg");

        // First two should succeed
        assert!(matches!(
            router.route(&decision, "t1"),
            RoutingResult::Delivered(_)
        ));
        assert!(matches!(
            router.route(&decision, "t2"),
            RoutingResult::Delivered(_)
        ));

        // Third should be rate limited
        assert!(matches!(
            router.route(&decision, "t3"),
            RoutingResult::RateLimited { .. }
        ));
    }

    #[test]
    fn test_dismissal_cooldown() {
        let mut router = InterventionRouter::default();

        // Record a dismissal
        router.record_dismissal("distraction_streak");

        // Try to route same trigger — should be rate limited
        let decision = test_decision(true, "msg");
        let result = router.route(&decision, "distraction_streak");
        assert!(matches!(result, RoutingResult::RateLimited { .. }));

        // Different trigger should work
        let result = router.route(&decision, "other_trigger");
        assert!(matches!(result, RoutingResult::Delivered(_)));
    }

    #[test]
    fn test_reset_dismissals() {
        let config = RouterConfig {
            dismiss_cooldown_secs: 0, // No cooldown for this test
            ..Default::default()
        };
        let mut router = InterventionRouter::new(config);

        router.record_dismissal("test_trigger");
        router.reset_dismissals("test_trigger");

        let decision = test_decision(true, "msg");
        let result = router.route(&decision, "test_trigger");
        assert!(matches!(result, RoutingResult::Delivered(_)));
    }

    #[test]
    fn test_hourly_count() {
        let mut router = InterventionRouter::default();
        assert_eq!(router.hourly_count(), 0);

        let decision = test_decision(true, "msg");
        router.route(&decision, "t1");
        assert_eq!(router.hourly_count(), 1);
    }
}
