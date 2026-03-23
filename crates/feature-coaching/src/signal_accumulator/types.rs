//! Signal types, trigger conditions, and default condition definitions.

use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};

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

/// Default built-in trigger conditions.
pub(super) fn default_conditions() -> Vec<TriggerCondition> {
    vec![
        // distraction_streak removed — distraction data is still tracked for
        // pattern detection (afternoon_energy_drop) and insights, but no longer
        // triggers coaching popups. The distraction overlay handles real-time
        // intervention for distracting apps.
        TriggerCondition::new("low_productivity", 1800), // 30min cooldown
        TriggerCondition::new("deadline_approaching", 3600), // 1h cooldown
        TriggerCondition::new("focus_quality_declining", 1800),
        // context_switch_overload removed — context switch data is tracked for
        // analytics/insights but no longer triggers coaching popups. The distraction
        // overlay handles real-time notifications for distracting apps.
        TriggerCondition::new("budget_warning", 3600),
        TriggerCondition::new("task_avoidance", 1800),
        TriggerCondition::new("flashcard_reviewed", 0),
        TriggerCondition::new("retention_drop_important", 3600),
        TriggerCondition::new("learning_streak_milestone", 86400),
        TriggerCondition::new("learning_momentum_shift", 3600),
        TriggerCondition::new("domain_retention_decline", 86400),
        TriggerCondition::new("knowledge_transfer", 3600),
        TriggerCondition::new("atom_created", 0),
        TriggerCondition::new("coaching_learning_digest", 86400),
    ]
}
