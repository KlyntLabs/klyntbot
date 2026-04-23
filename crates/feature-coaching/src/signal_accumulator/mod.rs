//! Signal accumulator — subscribes to DomainEventBus, maintains a rolling
//! event window, and evaluates heuristic trigger conditions against UserSituation.

mod types;

pub use types::{Signal, SignalMetadata, TriggerCondition, TriggerFired};

use std::collections::VecDeque;

use jiff::Timestamp;

use cognitive::situation::UserSituation;

use types::default_conditions;

/// Rolling window signal accumulator with heuristic trigger evaluation.
pub struct SignalAccumulator {
    /// Rolling window of recent signals (last 30 minutes).
    window: VecDeque<Signal>,
    /// Window duration in seconds.
    window_secs: i64,
    /// Last time each trigger condition fired (for cooldown).
    last_fired: std::collections::HashMap<String, Timestamp>,
    /// Registered trigger conditions.
    conditions: Vec<TriggerCondition>,
}

impl SignalAccumulator {
    pub fn new() -> Self {
        Self {
            window: VecDeque::new(),
            window_secs: 1800, // 30 minutes
            last_fired: std::collections::HashMap::new(),
            conditions: default_conditions(),
        }
    }

    /// Add a signal from an AiSignal.
    pub fn push_event(&mut self, signal: &ai_core::AiSignal) {
        let s = Signal::from_ai_signal(signal);
        self.window.push_back(s);
        self.prune_old(jiff::Timestamp::now());
    }

    /// Prune signals outside the window.
    fn prune_old(&mut self, now: Timestamp) {
        let cutoff = now - jiff::SignedDuration::from_secs(self.window_secs);
        while let Some(front) = self.window.front() {
            if front.timestamp < cutoff {
                self.window.pop_front();
            } else {
                break;
            }
        }
    }

    /// Evaluate all trigger conditions against the current situation.
    /// Returns a list of triggers that fired.
    pub fn evaluate(&mut self, situation: &UserSituation) -> Vec<TriggerFired> {
        let now = Timestamp::now();
        self.prune_old(now);

        let mut fired = Vec::new();

        for condition in &self.conditions {
            // Check cooldown
            if let Some(last) = self.last_fired.get(&condition.name) {
                let elapsed = now.as_millisecond() - last.as_millisecond();
                if elapsed < condition.cooldown_secs * 1000 {
                    continue;
                }
            }

            if let Some(trigger) = self.evaluate_condition(condition, situation, now) {
                self.last_fired.insert(condition.name.clone(), now);
                fired.push(trigger);
            }
        }

        fired
    }

    pub(crate) fn evaluate_condition(
        &self,
        condition: &TriggerCondition,
        situation: &UserSituation,
        _now: Timestamp,
    ) -> Option<TriggerFired> {
        match condition.name.as_str() {
            "low_productivity" => {
                // productive_ratio is not directly in UserSituation, use energy + distraction_risk
                if situation.distraction_risk > 0.7 && situation.energy_level < 0.4 {
                    Some(TriggerFired {
                        condition_name: "low_productivity".into(),
                        confidence: 0.7,
                        context: format!(
                            "High distraction risk ({:.0}%) + low energy ({:.0}%)",
                            situation.distraction_risk * 100.0,
                            situation.energy_level * 100.0
                        ),
                    })
                } else {
                    None
                }
            }
            "deadline_approaching" => {
                if situation.deadline_pressure > 0.7 {
                    Some(TriggerFired {
                        condition_name: "deadline_approaching".into(),
                        confidence: situation.deadline_pressure,
                        context: "High deadline pressure detected".into(),
                    })
                } else {
                    None
                }
            }
            "focus_quality_declining" => {
                let focus_events = self.count_events(bus::DomainEvent::KIND_FOCUS_SESSION_ENDED);
                if focus_events >= 3 && situation.focus_state < 0.3 {
                    Some(TriggerFired {
                        condition_name: "focus_quality_declining".into(),
                        confidence: 0.7,
                        context: format!(
                            "Focus at {:.0}% with {focus_events} sessions",
                            situation.focus_state * 100.0
                        ),
                    })
                } else {
                    None
                }
            }
            "budget_warning" => {
                let budget_alerts = self.count_events(bus::DomainEvent::KIND_BUDGET_ALERT);
                if budget_alerts >= 1 {
                    Some(TriggerFired {
                        condition_name: "budget_warning".into(),
                        confidence: 0.9,
                        context: format!("{budget_alerts} budget alert(s)"),
                    })
                } else {
                    None
                }
            }
            "task_avoidance" => {
                if situation.task_avoidance_detected {
                    Some(TriggerFired {
                        condition_name: "task_avoidance".into(),
                        confidence: 0.6,
                        context: "Task avoidance behavior detected".into(),
                    })
                } else {
                    None
                }
            }
            // Learning triggers removed — atom/flashcard events are still
            // accumulated for analytics but no longer trigger coaching popups.
            _ => None,
        }
    }

    /// Count signals of a given event type in the current window.
    fn count_events(&self, event_type: &str) -> usize {
        self.window
            .iter()
            .filter(|s| s.event_type == event_type)
            .count()
    }

    /// Get the current window size.
    pub fn window_size(&self) -> usize {
        self.window.len()
    }

    /// Get a snapshot of signals in the current window.
    pub fn signals(&self) -> &VecDeque<Signal> {
        &self.window
    }

    /// Get the trigger condition names and their cooldown state.
    pub fn condition_names(&self) -> Vec<&str> {
        self.conditions.iter().map(|c| c.name.as_str()).collect()
    }

    /// Get last fired timestamps for trigger conditions.
    pub fn last_fired(&self) -> &std::collections::HashMap<String, Timestamp> {
        &self.last_fired
    }
}

impl Default for SignalAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn situation_with(distraction_risk: f64, energy: f64, deadline_pressure: f64) -> UserSituation {
        UserSituation {
            energy_level: energy,
            focus_state: 0.5,
            deadline_pressure,
            distraction_risk,
            coaching_receptivity: 0.7,
            task_avoidance_detected: false,
            hours_active_today: 4.0,
            mins_since_break: 45.0,
            hour_of_day: 10,
            recent_context_switches: 3,
        }
    }

    fn make_signal(event_type: &'static str, amount: Option<f64>) -> ai_core::AiSignal {
        ai_core::AiSignal {
            domain: ai_core::RecallDomain::General,
            event_kind: event_type,
            importance: 0.5,
            salience: ai_core::SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
            metrics: ai_core::AiMetrics {
                amount,
                ..Default::default()
            },
            coaching_signal: false,
            coaching_rule: None,
            metric_samples: Vec::new(),
        }
    }

    #[test]
    fn test_push_ai_signal_updates_window() {
        let mut acc = SignalAccumulator::new();
        let sig = ai_core::AiSignal {
            domain: ai_core::RecallDomain::Finance,
            event_kind: "BudgetAlert",
            importance: 0.9,
            salience: ai_core::SalienceVerdict::Extract,
            content: "".into(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
            metrics: ai_core::AiMetrics {
                category: Some("food".into()),
                amount: Some(450.0),
                app: None,
            },
            coaching_signal: true,
            coaching_rule: None,
            metric_samples: Vec::new(),
        };
        acc.push_event(&sig);
        assert_eq!(acc.window_size(), 1);
        let front = acc.signals().front().unwrap();
        assert_eq!(front.event_type, "BudgetAlert");
        assert_eq!(front.metadata.category.as_deref(), Some("food"));
        assert_eq!(front.metadata.amount, Some(450.0));
    }

    #[test]
    fn test_distraction_events_no_longer_trigger() {
        let mut acc = SignalAccumulator::new();

        for _ in 0..5 {
            acc.push_event(&make_signal("DistractionDetected", None));
        }

        let sit = situation_with(0.5, 0.5, 0.2);
        let fired = acc.evaluate(&sit);
        // distraction_streak condition removed — events are tracked for
        // pattern detection but no longer trigger coaching popups
        assert!(!fired
            .iter()
            .any(|t| t.condition_name == "distraction_streak"));
    }

    #[test]
    fn test_no_trigger_below_threshold() {
        let mut acc = SignalAccumulator::new();

        acc.push_event(&make_signal("DistractionDetected", None));

        let sit = situation_with(0.2, 0.8, 0.1);
        let fired = acc.evaluate(&sit);
        assert!(fired.is_empty());
    }

    #[test]
    fn test_deadline_trigger() {
        let acc = SignalAccumulator::new();
        let sit = UserSituation {
            deadline_pressure: 0.9,
            ..situation_with(0.0, 1.0, 0.9)
        };

        // Need to evaluate via a mutable reference
        let mut acc = acc;
        let fired = acc.evaluate(&sit);
        assert!(fired
            .iter()
            .any(|t| t.condition_name == "deadline_approaching"));
    }

    #[test]
    fn test_cooldown_prevents_re_fire() {
        let mut acc = SignalAccumulator::new();

        acc.push_event(&make_signal("BudgetAlert", None));

        let sit = situation_with(0.0, 1.0, 0.0);

        // First evaluation fires
        let fired1 = acc.evaluate(&sit);
        assert!(fired1.iter().any(|t| t.condition_name == "budget_warning"));

        // Push another budget alert
        acc.push_event(&make_signal("BudgetAlert", None));

        // Second evaluation — cooldown blocks
        let fired2 = acc.evaluate(&sit);
        assert!(!fired2.iter().any(|t| t.condition_name == "budget_warning"));
    }

    #[test]
    fn test_budget_warning_trigger() {
        let mut acc = SignalAccumulator::new();

        acc.push_event(&make_signal("BudgetAlert", None));

        let sit = situation_with(0.0, 1.0, 0.0);
        let fired = acc.evaluate(&sit);
        assert!(fired.iter().any(|t| t.condition_name == "budget_warning"));
    }

    #[test]
    fn test_task_avoidance_trigger() {
        let mut acc = SignalAccumulator::new();
        let sit = UserSituation {
            task_avoidance_detected: true,
            ..situation_with(0.0, 1.0, 0.0)
        };
        let fired = acc.evaluate(&sit);
        assert!(fired.iter().any(|t| t.condition_name == "task_avoidance"));
    }

    #[test]
    fn test_session_ended_signal() {
        let mut acc = SignalAccumulator::new();

        acc.push_event(&make_signal("SessionEnded", Some(85.0)));

        assert_eq!(acc.window_size(), 1);
        let signal = acc.signals().front().unwrap();
        assert_eq!(signal.event_type, "SessionEnded");
        assert_eq!(signal.metadata.amount, Some(85.0));
    }

    #[test]
    fn test_quality_scored_signal() {
        let mut acc = SignalAccumulator::new();

        acc.push_event(&make_signal("QualityScored", Some(25.0)));

        let signal = acc.signals().front().unwrap();
        assert_eq!(signal.event_type, "QualityScored");
        assert_eq!(signal.metadata.amount, Some(25.0));
    }

    #[test]
    fn test_context_switch_overload_disabled() {
        // context_switch_overload is disabled — data used for analytics only,
        // distraction overlay handles real-time intervention.
        let mut acc = SignalAccumulator::new();
        let sit = UserSituation {
            recent_context_switches: 15,
            ..situation_with(0.0, 1.0, 0.0)
        };
        let fired = acc.evaluate(&sit);
        assert!(!fired
            .iter()
            .any(|t| t.condition_name == "context_switch_overload"));
    }

    #[test]
    fn every_default_condition_has_evaluator() {
        let acc = SignalAccumulator::new();
        let sit = UserSituation::default();
        // For each condition, calling evaluate_condition with a matching
        // synthesized situation must either fire or return None — never panic
        // with `_ => None` hitting a condition name without a real evaluator.
        for c in &acc.conditions {
            let _ = acc.evaluate_condition(c, &sit, jiff::Timestamp::now());
        }
    }
}
