//! Coaching reasoner — LLM-powered decision engine for interventions.
//!
//! When a trigger fires, the reasoner assembles context (situation, patterns,
//! memories, history) and asks the LLM whether and how to intervene.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use cognitive::situation::UserSituation;

use crate::pattern_detector::DetectedPattern;
use crate::signal_accumulator::TriggerFired;

/// The reasoner's decision about whether and how to coach.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoachingDecision {
    /// Whether to intervene.
    pub should_intervene: bool,
    /// Confidence in the decision (0.0–1.0).
    pub confidence: f64,
    /// The intervention message (if should_intervene).
    pub message: Option<String>,
    /// Intervention type hint for routing.
    pub intervention_type: InterventionType,
    /// Reasoning for the decision (for logging/feedback).
    pub reasoning: String,
    /// Observations to feed back to the cognitive layer.
    pub observations: Vec<String>,
}

/// Types of coaching intervention.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InterventionType {
    /// Subtle — show on dashboard card.
    DashboardCard,
    /// Moderate — send as chat message.
    ChatMessage,
    /// Urgent — system notification.
    Notification,
    /// Critical — overlay (e.g., distraction block).
    Overlay,
    /// No intervention.
    None,
}

impl InterventionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DashboardCard => "DashboardCard",
            Self::ChatMessage => "ChatMessage",
            Self::Notification => "Notification",
            Self::Overlay => "Overlay",
            Self::None => "None",
        }
    }
}

impl std::fmt::Display for InterventionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Input context assembled for the reasoner.
#[derive(Debug, Clone)]
pub struct ReasonerInput {
    pub situation: UserSituation,
    pub trigger: TriggerFired,
    pub patterns: Vec<DetectedPattern>,
    pub relevant_memories: Vec<String>,
    pub recent_interventions: Vec<String>,
}

/// Trait for LLM-backed coaching reasoning.
///
/// Defined here (feature-coaching, L4), implemented in agent (L5).
#[async_trait]
pub trait CoachingReasonerHandler: Send + Sync {
    /// Given the assembled context, decide whether and how to intervene.
    async fn reason(&self, input: &ReasonerInput) -> common::Result<CoachingDecision>;
}

/// Heuristic fallback reasoner for when LLM is unavailable.
///
/// Makes simple rule-based decisions based on trigger type and situation.
pub fn heuristic_reason(input: &ReasonerInput) -> CoachingDecision {
    let trigger = &input.trigger;
    let situation = &input.situation;

    // Don't intervene if coaching receptivity is very low
    if situation.coaching_receptivity < 0.2 {
        return CoachingDecision {
            should_intervene: false,
            confidence: 0.5,
            message: None,
            intervention_type: InterventionType::None,
            reasoning: "User coaching receptivity too low".into(),
            observations: vec![],
        };
    }

    // Don't interrupt deep focus with low-priority nudges
    if situation.focus_state > 0.8 && trigger.confidence < 0.8 {
        return CoachingDecision {
            should_intervene: false,
            confidence: 0.6,
            message: None,
            intervention_type: InterventionType::None,
            reasoning: "User in deep focus, trigger not urgent enough".into(),
            observations: vec![],
        };
    }

    let (should, msg, intervention_type, reasoning) = match trigger.condition_name.as_str() {
        "distraction_streak" => (
            true,
            Some(format!(
                "You've been distracted a few times recently. {}. Consider a 5-minute break or switching to a focused task.",
                trigger.context
            )),
            if situation.coaching_receptivity > 0.6 {
                InterventionType::ChatMessage
            } else {
                InterventionType::DashboardCard
            },
            "Distraction streak detected, suggesting break".to_string(),
        ),
        "deadline_approaching" => (
            true,
            Some("You have tasks with approaching deadlines. Would you like to focus on the most urgent one?".into()),
            InterventionType::Notification,
            "Deadline pressure is high".to_string(),
        ),
        "task_avoidance" => (
            true,
            Some("It looks like you might be avoiding a task. Sometimes breaking it into smaller pieces helps. Want me to suggest subtasks?".into()),
            InterventionType::ChatMessage,
            "Task avoidance behavior detected".to_string(),
        ),
        "context_switch_overload" => (
            true,
            Some(format!(
                "You've switched contexts {} times recently. Try batching similar tasks together.",
                situation.recent_context_switches
            )),
            InterventionType::DashboardCard,
            "Excessive context switching".to_string(),
        ),
        "budget_warning" => (
            true,
            Some("A budget category is approaching its limit. Check your spending dashboard.".into()),
            InterventionType::DashboardCard,
            "Budget threshold approaching".to_string(),
        ),
        "low_productivity" => (
            true,
            Some("Your energy seems low. A short walk or break might help restore focus.".into()),
            InterventionType::DashboardCard,
            "Low productivity detected".to_string(),
        ),
        _ => (
            false,
            None,
            InterventionType::None,
            format!("Unknown trigger: {}", trigger.condition_name),
        ),
    };

    CoachingDecision {
        should_intervene: should,
        confidence: trigger.confidence * situation.coaching_receptivity,
        message: msg,
        intervention_type,
        reasoning,
        observations: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_situation() -> UserSituation {
        UserSituation {
            energy_level: 0.6,
            focus_state: 0.4,
            deadline_pressure: 0.3,
            distraction_risk: 0.4,
            coaching_receptivity: 0.7,
            task_avoidance_detected: false,
            hours_active_today: 4.0,
            mins_since_break: 45.0,
            hour_of_day: 10,
            recent_context_switches: 5,
        }
    }

    fn trigger(name: &str) -> TriggerFired {
        TriggerFired {
            condition_name: name.into(),
            confidence: 0.8,
            context: "test context".into(),
        }
    }

    fn input(trigger_name: &str) -> ReasonerInput {
        ReasonerInput {
            situation: default_situation(),
            trigger: trigger(trigger_name),
            patterns: vec![],
            relevant_memories: vec![],
            recent_interventions: vec![],
        }
    }

    #[test]
    fn test_distraction_streak_intervenes() {
        let decision = heuristic_reason(&input("distraction_streak"));
        assert!(decision.should_intervene);
        assert!(decision.message.is_some());
        assert_ne!(decision.intervention_type, InterventionType::None);
    }

    #[test]
    fn test_deadline_approaching_notifies() {
        let mut inp = input("deadline_approaching");
        inp.situation.deadline_pressure = 0.9;
        let decision = heuristic_reason(&inp);
        assert!(decision.should_intervene);
        assert_eq!(decision.intervention_type, InterventionType::Notification);
    }

    #[test]
    fn test_low_receptivity_suppresses() {
        let mut inp = input("distraction_streak");
        inp.situation.coaching_receptivity = 0.1;
        let decision = heuristic_reason(&inp);
        assert!(!decision.should_intervene);
    }

    #[test]
    fn test_deep_focus_not_interrupted() {
        let mut inp = input("budget_warning");
        inp.situation.focus_state = 0.9;
        inp.trigger.confidence = 0.5; // Low urgency
        let decision = heuristic_reason(&inp);
        assert!(!decision.should_intervene);
    }

    #[test]
    fn test_task_avoidance_uses_chat() {
        let decision = heuristic_reason(&input("task_avoidance"));
        assert!(decision.should_intervene);
        assert_eq!(decision.intervention_type, InterventionType::ChatMessage);
    }

    #[test]
    fn test_confidence_modulated_by_receptivity() {
        let inp = input("distraction_streak");
        let decision = heuristic_reason(&inp);
        // confidence should be trigger.confidence * receptivity = 0.8 * 0.7 = 0.56
        assert!((decision.confidence - 0.56).abs() < 0.01);
    }

    #[test]
    fn test_unknown_trigger_no_intervention() {
        let decision = heuristic_reason(&input("unknown_trigger"));
        assert!(!decision.should_intervene);
    }
}
