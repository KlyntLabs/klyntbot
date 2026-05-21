//! Signal types, trigger conditions, and default condition definitions.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// A timestamped signal derived from a domain event.
#[derive(Debug, Clone)]
pub struct Signal {
    pub event_type: String,
    pub timestamp: Timestamp,
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

impl SignalMetadata {
    pub fn from_ai_signal(sig: &ai_core::AiSignal) -> Self {
        Self {
            app: sig.metrics.app.clone(),
            task_id: sig
                .entity
                .as_ref()
                .filter(|e| e.entity_type == "task")
                .map(|e| e.id.clone()),
            category: sig.metrics.category.clone(),
            amount: sig.metrics.amount,
        }
    }
}

impl Signal {
    pub fn from_ai_signal(sig: &ai_core::AiSignal) -> Self {
        Self {
            event_type: sig.event_kind.to_string(),
            timestamp: sig.timestamp,
            metadata: SignalMetadata::from_ai_signal(sig),
        }
    }
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

/// Default built-in trigger conditions. Each condition here MUST have a
/// matching arm in `SignalAccumulator::evaluate_condition`. Conditions that
/// lost their arm during v1.5 (learning triggers, distraction_streak,
/// context_switch_overload) are handled by pattern_detector alone — keep
/// them out of this list to avoid no-op evaluations.
pub(super) fn default_conditions() -> Vec<TriggerCondition> {
    vec![
        TriggerCondition::new("low_productivity", 1800), // 30min cooldown
        TriggerCondition::new("deadline_approaching", 3600), // 1h cooldown
        TriggerCondition::new("focus_quality_declining", 1800),
        TriggerCondition::new("budget_warning", 3600),
        TriggerCondition::new("focus_warning", 3600),
        TriggerCondition::new("task_avoidance", 1800),
    ]
}
