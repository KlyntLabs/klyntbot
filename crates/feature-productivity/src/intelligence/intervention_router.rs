use std::collections::VecDeque;

use jiff::Timestamp;

use crate::types::*;

use super::session_aggregator::ClassifiedTick;

/// Maximum interventions allowed within the rate-limit window.
const MAX_INTERVENTIONS_PER_HOUR: usize = 3;
/// Context switches within this window trigger a storm alert.
const STORM_WINDOW_TICKS: usize = 36; // 3 min at 5s intervals
/// Number of context switches within the storm window to trigger.
const STORM_THRESHOLD: usize = 5;
/// Continuous focus ticks before suggesting a break (90 min at 5s).
const BREAK_REMINDER_TICKS: usize = 1080;

struct InterventionRecord {
    timestamp: Timestamp,
    _intervention_type: InterventionType,
}

pub struct InterventionRouter {
    recent_ticks: VecDeque<ClassifiedTick>,
    recent_interventions: VecDeque<InterventionRecord>,
    continuous_focus_ticks: usize,
    in_meeting: bool,
}

impl Default for InterventionRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl InterventionRouter {
    pub fn new() -> Self {
        Self {
            recent_ticks: VecDeque::new(),
            recent_interventions: VecDeque::new(),
            continuous_focus_ticks: 0,
            in_meeting: false,
        }
    }

    /// Evaluate the latest tick and return an intervention if warranted.
    pub fn evaluate(&mut self, tick: &ClassifiedTick) -> Option<ProductivityIntervention> {
        self.recent_ticks.push_back(tick.clone());
        if self.recent_ticks.len() > STORM_WINDOW_TICKS {
            self.recent_ticks.pop_front();
        }

        // Track meeting state
        self.in_meeting = tick.session_type == IntelligenceSessionType::Meeting;

        // No interventions during meetings
        if self.in_meeting {
            self.continuous_focus_ticks = 0;
            return None;
        }

        // Track continuous focus
        if tick.session_type == IntelligenceSessionType::Focus && !tick.is_idle {
            self.continuous_focus_ticks += 1;
        } else {
            self.continuous_focus_ticks = 0;
        }

        // Prune old interventions (keep only last hour) — only compute now() when needed
        if !self.recent_interventions.is_empty() {
            let one_hour_ago = Timestamp::now() - jiff::SignedDuration::from_hours(1);
            while self
                .recent_interventions
                .front()
                .is_some_and(|r| r.timestamp < one_hour_ago)
            {
                self.recent_interventions.pop_front();
            }
        }

        // Rate limit check
        if self.recent_interventions.len() >= MAX_INTERVENTIONS_PER_HOUR {
            return None;
        }

        // Check 1: Context-switch storm
        if let Some(intervention) = self.check_context_switch_storm(tick.timestamp) {
            return Some(intervention);
        }

        // Check 2: Break reminder after prolonged focus
        if let Some(intervention) = self.check_break_reminder(tick.timestamp) {
            return Some(intervention);
        }

        None
    }

    fn check_context_switch_storm(
        &mut self,
        now: Timestamp,
    ) -> Option<ProductivityIntervention> {
        let switches = self
            .recent_ticks
            .iter()
            .filter(|t| t.is_context_switch)
            .count();

        if switches >= STORM_THRESHOLD {
            let intervention = ProductivityIntervention {
                intervention_type: InterventionType::FocusModeActivation,
                message: format!(
                    "High context switching detected ({} switches in 3 min). Consider entering focus mode.",
                    switches
                ),
                suggested_action: SuggestedAction::ReduceDistractions,
                urgency: Urgency::High,
                timestamp: now,
            };
            self.record_intervention(now, InterventionType::FocusModeActivation);
            // Clear recent ticks to avoid re-triggering immediately
            self.recent_ticks.clear();
            Some(intervention)
        } else {
            None
        }
    }

    fn check_break_reminder(&mut self, now: Timestamp) -> Option<ProductivityIntervention> {
        if self.continuous_focus_ticks >= BREAK_REMINDER_TICKS {
            let mins = self.continuous_focus_ticks * 5 / 60;
            let intervention = ProductivityIntervention {
                intervention_type: InterventionType::BreakReminder,
                message: format!(
                    "You've been focused for {}+ minutes. Time for a break!",
                    mins
                ),
                suggested_action: SuggestedAction::TakeBreak,
                urgency: Urgency::Medium,
                timestamp: now,
            };
            self.record_intervention(now, InterventionType::BreakReminder);
            self.continuous_focus_ticks = 0;
            Some(intervention)
        } else {
            None
        }
    }

    fn record_intervention(&mut self, timestamp: Timestamp, itype: InterventionType) {
        self.recent_interventions.push_back(InterventionRecord {
            timestamp,
            _intervention_type: itype,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tick(session_type: IntelligenceSessionType, is_context_switch: bool) -> ClassifiedTick {
        ClassifiedTick {
            timestamp: Timestamp::now(),
            app_name: "test".to_string(),
            category: "coding".to_string(),
            session_type,
            confidence: 1.0,
            is_idle: false,
            is_context_switch,
        }
    }

    #[test]
    fn test_context_switch_storm_triggers_focus() {
        let mut router = InterventionRouter::new();

        // Feed 5 context-switching ticks within the storm window
        for _ in 0..5 {
            let tick = make_tick(IntelligenceSessionType::Focus, true);
            let result = router.evaluate(&tick);
            if let Some(ref intervention) = result {
                assert_eq!(
                    intervention.intervention_type,
                    InterventionType::FocusModeActivation
                );
                assert_eq!(intervention.urgency, Urgency::High);
                return;
            }
        }
        panic!("Expected FocusModeActivation intervention after 5 context switches");
    }

    #[test]
    fn test_rate_limiting() {
        let mut router = InterventionRouter::new();

        // Trigger 3 interventions
        for _ in 0..3 {
            for _ in 0..5 {
                let tick = make_tick(IntelligenceSessionType::Focus, true);
                router.evaluate(&tick);
            }
        }

        // 4th attempt should be rate-limited
        for _ in 0..5 {
            let tick = make_tick(IntelligenceSessionType::Focus, true);
            let result = router.evaluate(&tick);
            assert!(result.is_none(), "Expected rate-limited (no intervention)");
        }
    }

    #[test]
    fn test_break_reminder_after_90min() {
        let mut router = InterventionRouter::new();

        // Simulate 1080 focus ticks (90 min)
        let mut found_break = false;
        for _ in 0..BREAK_REMINDER_TICKS + 1 {
            let tick = make_tick(IntelligenceSessionType::Focus, false);
            if let Some(intervention) = router.evaluate(&tick) {
                assert_eq!(
                    intervention.intervention_type,
                    InterventionType::BreakReminder
                );
                found_break = true;
                break;
            }
        }
        assert!(found_break, "Expected BreakReminder after 90min of focus");
    }

    #[test]
    fn test_no_intervention_during_meeting() {
        let mut router = InterventionRouter::new();

        // Even with context switches, meeting suppresses interventions
        for _ in 0..10 {
            let tick = make_tick(IntelligenceSessionType::Meeting, true);
            let result = router.evaluate(&tick);
            assert!(result.is_none(), "Expected no intervention during meeting");
        }
    }
}
