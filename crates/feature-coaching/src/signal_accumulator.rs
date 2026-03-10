//! Signal accumulator — subscribes to DomainEventBus, maintains a rolling
//! event window, and evaluates heuristic trigger conditions against UserSituation.

use std::collections::VecDeque;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use bus::DomainEvent;
use cognitive::situation::UserSituation;

/// A timestamped signal derived from a domain event.
#[derive(Debug, Clone)]
pub struct Signal {
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: SignalMetadata,
}

/// Extra data attached to a signal for trigger evaluation.
#[derive(Debug, Clone, Default)]
pub struct SignalMetadata {
    pub app: Option<String>,
    pub task_id: Option<String>,
    pub category: Option<String>,
    pub amount: Option<f64>,
}

/// Named trigger conditions with cooldown tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCondition {
    pub name: String,
    pub cooldown_secs: i64,
}

impl TriggerCondition {
    pub fn new(name: &str, cooldown_secs: i64) -> Self {
        Self {
            name: name.to_string(),
            cooldown_secs,
        }
    }
}

/// Result of evaluating triggers.
#[derive(Debug, Clone)]
pub struct TriggerFired {
    pub condition_name: String,
    pub confidence: f64,
    pub context: String,
}

/// Rolling window signal accumulator with heuristic trigger evaluation.
pub struct SignalAccumulator {
    /// Rolling window of recent signals (last 30 minutes).
    window: VecDeque<Signal>,
    /// Window duration in seconds.
    window_secs: i64,
    /// Last time each trigger condition fired (for cooldown).
    last_fired: std::collections::HashMap<String, DateTime<Utc>>,
    /// Registered trigger conditions.
    conditions: Vec<TriggerCondition>,
}

/// Default built-in trigger conditions.
fn default_conditions() -> Vec<TriggerCondition> {
    vec![
        TriggerCondition::new("distraction_streak", 900), // 15min cooldown
        TriggerCondition::new("low_productivity", 1800),  // 30min cooldown
        TriggerCondition::new("deadline_approaching", 3600), // 1h cooldown
        TriggerCondition::new("focus_quality_declining", 1800),
        TriggerCondition::new("context_switch_overload", 900),
        TriggerCondition::new("budget_warning", 3600),
        TriggerCondition::new("task_avoidance", 1800),
    ]
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

    /// Add a signal from a domain event.
    pub fn push_event(&mut self, event: &DomainEvent) {
        let now = Utc::now();
        let signal = event_to_signal(event, now);
        self.window.push_back(signal);
        self.prune_old(now);
    }

    /// Prune signals outside the window.
    fn prune_old(&mut self, now: DateTime<Utc>) {
        let cutoff = now - chrono::Duration::seconds(self.window_secs);
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
        let now = Utc::now();
        self.prune_old(now);

        let mut fired = Vec::new();

        for condition in &self.conditions {
            // Check cooldown
            if let Some(last) = self.last_fired.get(&condition.name) {
                if (now - *last).num_seconds() < condition.cooldown_secs {
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

    fn evaluate_condition(
        &self,
        condition: &TriggerCondition,
        situation: &UserSituation,
        _now: DateTime<Utc>,
    ) -> Option<TriggerFired> {
        match condition.name.as_str() {
            "distraction_streak" => {
                let distraction_count = self.count_events("DistractionDetected");
                if distraction_count >= 3 {
                    Some(TriggerFired {
                        condition_name: "distraction_streak".into(),
                        confidence: 0.8,
                        context: format!("{distraction_count} distractions in last 30min"),
                    })
                } else {
                    None
                }
            }
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
                let focus_events = self.count_events("FocusSessionEnded");
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
            "context_switch_overload" => {
                if situation.recent_context_switches > 10 {
                    Some(TriggerFired {
                        condition_name: "context_switch_overload".into(),
                        confidence: 0.75,
                        context: format!(
                            "{} context switches in last 30min",
                            situation.recent_context_switches
                        ),
                    })
                } else {
                    None
                }
            }
            "budget_warning" => {
                let budget_alerts = self.count_events("BudgetAlert");
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
    pub fn last_fired(&self) -> &std::collections::HashMap<String, DateTime<Utc>> {
        &self.last_fired
    }
}

impl Default for SignalAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a domain event into a signal.
fn event_to_signal(event: &DomainEvent, timestamp: DateTime<Utc>) -> Signal {
    let (event_type, metadata) = match event {
        DomainEvent::DistractionDetected { app, .. } => (
            "DistractionDetected",
            SignalMetadata {
                app: Some(app.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::FocusSessionEnded { .. } => ("FocusSessionEnded", SignalMetadata::default()),
        DomainEvent::ProductivityScoreComputed { .. } => {
            ("ProductivityScoreComputed", SignalMetadata::default())
        }
        DomainEvent::TaskCompleted { task_id, .. } => (
            "TaskCompleted",
            SignalMetadata {
                task_id: Some(task_id.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::TaskDeferred { task_id, .. } => (
            "TaskDeferred",
            SignalMetadata {
                task_id: Some(task_id.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::BudgetAlert { category, .. } => (
            "BudgetAlert",
            SignalMetadata {
                category: Some(category.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::TransactionRecorded {
            category, amount, ..
        } => (
            "TransactionRecorded",
            SignalMetadata {
                category: Some(category.clone()),
                amount: Some(*amount),
                ..Default::default()
            },
        ),
        // -- Intelligence layer events --
        DomainEvent::SessionCreated {
            session_type,
            dominant_category,
            ..
        } => (
            "SessionCreated",
            SignalMetadata {
                category: Some(format!("{session_type}:{dominant_category}")),
                ..Default::default()
            },
        ),
        DomainEvent::SessionEnded {
            duration_secs,
            quality_score,
            session_type,
            ..
        } => (
            "SessionEnded",
            SignalMetadata {
                category: Some(session_type.clone()),
                amount: quality_score.or(Some(*duration_secs as f64)),
                ..Default::default()
            },
        ),
        DomainEvent::QualityScored {
            overall_score,
            score_date,
            ..
        } => (
            "QualityScored",
            SignalMetadata {
                category: Some(score_date.clone()),
                amount: Some(*overall_score),
                ..Default::default()
            },
        ),
        DomainEvent::PredictiveAlert {
            forecast_type,
            predicted_value,
            ..
        } => (
            "PredictiveAlert",
            SignalMetadata {
                category: Some(forecast_type.clone()),
                amount: Some(*predicted_value),
                ..Default::default()
            },
        ),
        _ => ("Other", SignalMetadata::default()),
    };

    Signal {
        event_type: event_type.to_string(),
        timestamp,
        metadata,
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

    #[test]
    fn test_distraction_streak_fires() {
        let mut acc = SignalAccumulator::new();

        for _ in 0..3 {
            acc.push_event(&DomainEvent::DistractionDetected {
                app: "reddit".into(),
                duration_secs: None,
                context: "test".into(),
            });
        }

        let sit = situation_with(0.5, 0.5, 0.2);
        let fired = acc.evaluate(&sit);
        assert!(fired
            .iter()
            .any(|t| t.condition_name == "distraction_streak"));
    }

    #[test]
    fn test_no_trigger_below_threshold() {
        let mut acc = SignalAccumulator::new();

        acc.push_event(&DomainEvent::DistractionDetected {
            app: "reddit".into(),
            duration_secs: None,
            context: "test".into(),
        });

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

        for _ in 0..5 {
            acc.push_event(&DomainEvent::DistractionDetected {
                app: "reddit".into(),
                duration_secs: None,
                context: "test".into(),
            });
        }

        let sit = situation_with(0.5, 0.5, 0.2);

        // First evaluation fires
        let fired1 = acc.evaluate(&sit);
        assert!(!fired1.is_empty());

        // Second evaluation — cooldown blocks
        let fired2 = acc.evaluate(&sit);
        let distraction_fired = fired2
            .iter()
            .any(|t| t.condition_name == "distraction_streak");
        assert!(!distraction_fired);
    }

    #[test]
    fn test_budget_warning_trigger() {
        let mut acc = SignalAccumulator::new();

        acc.push_event(&DomainEvent::BudgetAlert {
            category: "food".into(),
            spent: 450.0,
            limit: 500.0,
        });

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

        acc.push_event(&DomainEvent::SessionEnded {
            session_id: "s1".into(),
            session_type: "focus".into(),
            duration_secs: 3600,
            quality_score: Some(85.0),
            category_purity: 0.9,
        });

        assert_eq!(acc.window_size(), 1);
        let signal = acc.signals().front().unwrap();
        assert_eq!(signal.event_type, "SessionEnded");
        assert_eq!(signal.metadata.amount, Some(85.0));
    }

    #[test]
    fn test_quality_scored_signal() {
        let mut acc = SignalAccumulator::new();

        acc.push_event(&DomainEvent::QualityScored {
            score_date: "2026-03-09".into(),
            session_id: None,
            overall_score: 25.0,
            components: "{}".into(),
        });

        let signal = acc.signals().front().unwrap();
        assert_eq!(signal.event_type, "QualityScored");
        assert_eq!(signal.metadata.amount, Some(25.0));
    }

    #[test]
    fn test_predictive_alert_signal() {
        let mut acc = SignalAccumulator::new();

        acc.push_event(&DomainEvent::PredictiveAlert {
            forecast_type: "energy".into(),
            window_start: "14:00".into(),
            window_end: "16:00".into(),
            predicted_value: 0.3,
            suggested_action: Some("Take a break".into()),
        });

        let signal = acc.signals().front().unwrap();
        assert_eq!(signal.event_type, "PredictiveAlert");
        assert_eq!(signal.metadata.category, Some("energy".into()));
    }

    #[test]
    fn test_context_switch_overload() {
        let mut acc = SignalAccumulator::new();
        let sit = UserSituation {
            recent_context_switches: 15,
            ..situation_with(0.0, 1.0, 0.0)
        };
        let fired = acc.evaluate(&sit);
        assert!(fired
            .iter()
            .any(|t| t.condition_name == "context_switch_overload"));
    }
}
