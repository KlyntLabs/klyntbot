//! Pattern detection layer — aggregates accumulated signals into higher-level
//! patterns that are input to the coaching reasoner.

mod learning_patterns;

use std::collections::{HashMap, VecDeque};

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

use crate::signal_accumulator::TriggerFired;

/// A detected behavioral pattern with confidence and evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    pub name: String,
    pub confidence: f64,
    pub signal_count: i32,
    pub description: String,
    pub domain: ai_core::RecallDomain,
    pub rule_text: String,
}

/// Tracks historical trigger data to detect recurring patterns.
pub struct PatternDetector {
    /// Historical trigger firings: condition_name → list of (timestamp, context).
    history: HashMap<String, VecDeque<(Timestamp, String)>>,
    /// Maximum history entries per condition.
    max_history: usize,
}

impl PatternDetector {
    pub fn new() -> Self {
        Self {
            history: HashMap::new(),
            max_history: 100,
        }
    }

    /// Record a trigger firing for pattern detection.
    pub fn record_trigger(&mut self, trigger: &TriggerFired) {
        let entry = self
            .history
            .entry(trigger.condition_name.clone())
            .or_default();
        entry.push_back((Timestamp::now(), trigger.context.clone()));
        if entry.len() > self.max_history {
            entry.pop_front();
        }
    }

    /// Detect patterns from accumulated trigger history.
    pub fn detect_patterns(&self) -> Vec<DetectedPattern> {
        let mut patterns = Vec::new();

        // Afternoon energy drop: distraction streaks consistently after 3pm local time
        if let Some(distractions) = self.history.get("distraction_streak") {
            let afternoon_count = distractions
                .iter()
                .filter(|(ts, _)| ts.to_zoned(jiff::tz::TimeZone::system()).hour() >= 15)
                .count();
            let total = distractions.len().max(1);
            if afternoon_count >= 3 && (afternoon_count as f64 / total as f64) > 0.5 {
                patterns.push(DetectedPattern {
                    name: "afternoon_energy_drop".into(),
                    confidence: (afternoon_count as f64 / total as f64).min(0.95),
                    signal_count: afternoon_count as i32,
                    description: format!(
                        "{afternoon_count}/{total} distraction streaks occur after 3pm"
                    ),
                    domain: ai_core::RecallDomain::Productivity,
                    rule_text: "Schedule demanding tasks in the morning; take breaks in the afternoon when energy drops".into(),
                });
            }
        }

        // Task avoidance pattern: repeated task_avoidance triggers
        if let Some(avoidance) = self.history.get("task_avoidance") {
            if avoidance.len() >= 3 {
                patterns.push(DetectedPattern {
                    name: "chronic_task_avoidance".into(),
                    confidence: (avoidance.len() as f64 / 10.0).min(0.9),
                    signal_count: avoidance.len() as i32,
                    description: format!("Task avoidance detected {} times", avoidance.len()),
                    domain: ai_core::RecallDomain::Tasks,
                    rule_text: "Break avoided tasks into smaller steps to overcome procrastination".into(),
                });
            }
        }

        // Context switch overload pattern
        if let Some(switches) = self.history.get("context_switch_overload") {
            if switches.len() >= 3 {
                patterns.push(DetectedPattern {
                    name: "habitual_context_switching".into(),
                    confidence: (switches.len() as f64 / 10.0).min(0.85),
                    signal_count: switches.len() as i32,
                    description: format!(
                        "Context switch overload triggered {} times",
                        switches.len()
                    ),
                    domain: ai_core::RecallDomain::Productivity,
                    rule_text: "Batch similar tasks together to reduce context switching overhead".into(),
                });
            }
        }

        // Budget overspending pattern
        if let Some(budget) = self.history.get("budget_warning") {
            if budget.len() >= 2 {
                patterns.push(DetectedPattern {
                    name: "recurring_budget_pressure".into(),
                    confidence: (budget.len() as f64 / 5.0).min(0.9),
                    signal_count: budget.len() as i32,
                    description: format!("Budget warnings triggered {} times", budget.len()),
                    domain: ai_core::RecallDomain::Finance,
                    rule_text: "Review spending patterns when budget pressure is detected".into(),
                });
            }
        }

        // Focus quality declining pattern
        if let Some(focus) = self.history.get("focus_quality_declining") {
            if focus.len() >= 3 {
                patterns.push(DetectedPattern {
                    name: "declining_focus_quality".into(),
                    confidence: (focus.len() as f64 / 10.0).min(0.8),
                    signal_count: focus.len() as i32,
                    description: format!("Focus quality declining triggered {} times", focus.len()),
                    domain: ai_core::RecallDomain::Productivity,
                    rule_text: "Take a break when focus quality starts declining".into(),
                });
            }
        }

        // --- Learning patterns ---

        // Study streak detection (milestones + at-risk).
        if let Some(reviews) = self.history.get("flashcard_reviewed") {
            if let Some(p) = learning_patterns::detect_study_streak(reviews) {
                patterns.push(p);
            }
        }

        // Retention decay from digest signals.
        if let Some(digests) = self.history.get("coaching_learning_digest") {
            if let Some(p) = learning_patterns::detect_retention_decay(digests) {
                patterns.push(p);
            }
        }

        // Learning momentum: compare atom creation vs review rates.
        {
            let empty = VecDeque::new();
            let creations = self.history.get("atom_created").unwrap_or(&empty);
            let reviews = self.history.get("flashcard_reviewed").unwrap_or(&empty);
            if let Some(p) = learning_patterns::detect_learning_momentum(creations, reviews) {
                patterns.push(p);
            }
        }

        // Domain retention gap from repeated digest signals.
        if let Some(digests) = self.history.get("coaching_learning_digest") {
            if let Some(p) = learning_patterns::detect_domain_gap(digests) {
                patterns.push(p);
            }
        }

        // Knowledge transfer events.
        if let Some(transfers) = self.history.get("knowledge_transfer") {
            if let Some(p) = learning_patterns::detect_knowledge_transfer(transfers) {
                patterns.push(p);
            }
        }

        patterns
    }

    /// Seed test trigger data for debugging pattern detection UI.
    pub fn seed_test_triggers(&mut self, condition: &str, count: usize) {
        let entry = self.history.entry(condition.to_string()).or_default();
        for _ in 0..count {
            entry.push_back((Timestamp::now(), "seeded for testing".into()));
        }
    }

    /// Get the number of historical entries for a condition.
    pub fn history_count(&self, condition: &str) -> usize {
        self.history.get(condition).map(|v| v.len()).unwrap_or(0)
    }
}

impl Default for PatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trigger(name: &str, context: &str) -> TriggerFired {
        TriggerFired {
            condition_name: name.into(),
            confidence: 0.8,
            context: context.into(),
        }
    }

    #[test]
    fn test_no_patterns_with_few_signals() {
        let detector = PatternDetector::new();
        let patterns = detector.detect_patterns();
        assert!(patterns.is_empty());
    }

    #[test]
    fn test_task_avoidance_pattern() {
        let mut detector = PatternDetector::new();

        for i in 0..4 {
            detector.record_trigger(&trigger("task_avoidance", &format!("event {i}")));
        }

        let patterns = detector.detect_patterns();
        assert!(patterns.iter().any(|p| p.name == "chronic_task_avoidance"));
    }

    #[test]
    fn test_context_switch_pattern() {
        let mut detector = PatternDetector::new();

        for i in 0..5 {
            detector.record_trigger(&trigger(
                "context_switch_overload",
                &format!("{} switches", 12 + i),
            ));
        }

        let patterns = detector.detect_patterns();
        assert!(patterns
            .iter()
            .any(|p| p.name == "habitual_context_switching"));
    }

    #[test]
    fn test_budget_pattern() {
        let mut detector = PatternDetector::new();

        detector.record_trigger(&trigger("budget_warning", "food at 90%"));
        detector.record_trigger(&trigger("budget_warning", "entertainment at 85%"));

        let patterns = detector.detect_patterns();
        assert!(patterns
            .iter()
            .any(|p| p.name == "recurring_budget_pressure"));
    }

    #[test]
    fn test_history_count() {
        let mut detector = PatternDetector::new();
        assert_eq!(detector.history_count("task_avoidance"), 0);

        detector.record_trigger(&trigger("task_avoidance", "test"));
        assert_eq!(detector.history_count("task_avoidance"), 1);
    }

    #[test]
    fn test_confidence_clamps() {
        let mut detector = PatternDetector::new();

        // Add many signals
        for i in 0..50 {
            detector.record_trigger(&trigger("task_avoidance", &format!("event {i}")));
        }

        let patterns = detector.detect_patterns();
        let avoidance = patterns
            .iter()
            .find(|p| p.name == "chronic_task_avoidance")
            .unwrap();
        assert!(avoidance.confidence <= 0.9);
    }

    #[test]
    fn test_detected_pattern_carries_rule_text() {
        let mut detector = PatternDetector::new();
        for _ in 0..4 {
            detector.record_trigger(&trigger("task_avoidance", "x"));
        }
        let patterns = detector.detect_patterns();
        let p = patterns.iter().find(|p| p.name == "chronic_task_avoidance").unwrap();
        assert!(p.rule_text.contains("smaller steps"));
    }
}
