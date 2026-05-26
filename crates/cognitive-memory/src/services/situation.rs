//! UserSituation — the derived world model used by the coaching engine.
//!
//! Computed from current productivity state, task pressure, user model facts,
//! and recent coaching history. Recomputed periodically or on significant events.

use serde::{Deserialize, Serialize};

/// The structured user situation — queryable by the coaching engine.
///
/// Each field is a 0.0–1.0 normalized signal.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserSituation {
    /// Current energy level estimate (0.0 = depleted, 1.0 = peak).
    pub energy_level: f64,
    /// Current focus state (0.0 = scattered, 1.0 = deep focus).
    pub focus_state: f64,
    /// Deadline pressure (0.0 = relaxed, 1.0 = critical).
    pub deadline_pressure: f64,
    /// Risk of distraction based on recent patterns (0.0 = low, 1.0 = high).
    pub distraction_risk: f64,
    /// Estimated receptivity to coaching interventions (0.0 = unreceptive, 1.0 = open).
    pub coaching_receptivity: f64,
    /// Whether task avoidance behavior is detected.
    pub task_avoidance_detected: bool,
    /// Hours active today.
    pub hours_active_today: f64,
    /// Minutes since last break.
    pub mins_since_break: f64,
    /// Current hour of day (0-23).
    pub hour_of_day: u32,
    /// Number of context switches in the last 30 minutes.
    pub recent_context_switches: i32,
}

/// Inputs for computing the user situation.
///
/// Feature crates provide these values; the cognitive layer assembles them.
#[derive(Debug, Clone, Default)]
pub struct SituationInputs {
    // Productivity signals
    pub hours_active_today: f64,
    pub mins_since_break: f64,
    pub productive_ratio_today: f64,
    pub distraction_count_last_30min: i32,
    pub context_switches_last_30min: i32,
    pub is_in_focus_session: bool,
    pub focus_quality: Option<f64>,

    // Task signals
    pub overdue_task_count: i32,
    pub tasks_due_within_24h: i32,
    pub times_deferred_max: i32,

    // User model signals (from semantic facts)
    pub peak_hour_match: bool,
    pub historical_avg_context_switches: f64,

    // Coaching signals
    pub recent_dismissals: i32,
    pub coaching_intensity: f64,

    // Time
    pub hour_of_day: u32,
}

/// Compute the user situation from raw inputs.
pub fn compute_situation(inputs: &SituationInputs) -> UserSituation {
    // Energy: decreases with hours active, recovers after breaks, peaks at user's peak hours
    let base_energy = 1.0 - (inputs.hours_active_today / 12.0).clamp(0.0, 1.0);
    let break_penalty = (inputs.mins_since_break / 120.0).clamp(0.0, 0.3);
    let peak_bonus = if inputs.peak_hour_match { 0.2 } else { 0.0 };
    let energy_level = (base_energy - break_penalty + peak_bonus).clamp(0.0, 1.0);

    // Focus: high if in focus session with good quality, low if many context switches
    let focus_state = if inputs.is_in_focus_session {
        inputs.focus_quality.unwrap_or(0.7)
    } else {
        let switch_penalty = (inputs.context_switches_last_30min as f64 / 10.0).clamp(0.0, 0.8);
        (0.5 - switch_penalty).clamp(0.0, 1.0)
    };

    // Deadline pressure: rises with overdue tasks and approaching deadlines
    let overdue_pressure = (inputs.overdue_task_count as f64 / 5.0).clamp(0.0, 0.5);
    let upcoming_pressure = (inputs.tasks_due_within_24h as f64 / 3.0).clamp(0.0, 0.5);
    let deadline_pressure = (overdue_pressure + upcoming_pressure).clamp(0.0, 1.0);

    // Distraction risk: based on recent distraction count relative to average
    let avg = inputs.historical_avg_context_switches.max(1.0);
    let relative_switches = inputs.context_switches_last_30min as f64 / avg;
    let distraction_base = (inputs.distraction_count_last_30min as f64 / 5.0).clamp(0.0, 0.6);
    let switch_factor = ((relative_switches - 1.0) * 0.4).clamp(0.0, 0.4);
    let distraction_risk = (distraction_base + switch_factor).clamp(0.0, 1.0);

    // Coaching receptivity: lower when user has been dismissing, higher when open
    let dismiss_penalty = (inputs.recent_dismissals as f64 * 0.15).clamp(0.0, 0.6);
    let coaching_receptivity = (inputs.coaching_intensity - dismiss_penalty).clamp(0.0, 1.0);

    // Task avoidance: deferred many times + low productive ratio
    let task_avoidance_detected =
        inputs.times_deferred_max >= 3 && inputs.productive_ratio_today < 0.4;

    UserSituation {
        energy_level,
        focus_state,
        deadline_pressure,
        distraction_risk,
        coaching_receptivity,
        task_avoidance_detected,
        hours_active_today: inputs.hours_active_today,
        mins_since_break: inputs.mins_since_break,
        hour_of_day: inputs.hour_of_day,
        recent_context_switches: inputs.context_switches_last_30min,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_inputs() -> SituationInputs {
        SituationInputs {
            hours_active_today: 4.0,
            mins_since_break: 45.0,
            productive_ratio_today: 0.7,
            distraction_count_last_30min: 1,
            context_switches_last_30min: 3,
            is_in_focus_session: false,
            focus_quality: None,
            overdue_task_count: 0,
            tasks_due_within_24h: 1,
            times_deferred_max: 0,
            peak_hour_match: true,
            historical_avg_context_switches: 5.0,
            recent_dismissals: 0,
            coaching_intensity: 0.7,
            hour_of_day: 10,
        }
    }

    #[test]
    fn test_compute_situation_normal() {
        let sit = compute_situation(&default_inputs());
        assert!(sit.energy_level > 0.3 && sit.energy_level < 1.0);
        assert!(sit.focus_state > 0.0);
        assert!(sit.deadline_pressure > 0.0);
        assert!(!sit.task_avoidance_detected);
        assert!(sit.coaching_receptivity > 0.0);
    }

    #[test]
    fn test_energy_drops_with_hours() {
        let mut inputs = default_inputs();
        inputs.hours_active_today = 1.0;
        let fresh = compute_situation(&inputs);

        inputs.hours_active_today = 10.0;
        let tired = compute_situation(&inputs);

        assert!(fresh.energy_level > tired.energy_level);
    }

    #[test]
    fn test_focus_high_in_session() {
        let mut inputs = default_inputs();
        inputs.is_in_focus_session = true;
        inputs.focus_quality = Some(0.9);
        let sit = compute_situation(&inputs);
        assert!(sit.focus_state >= 0.9);
    }

    #[test]
    fn test_deadline_pressure_rises() {
        let mut inputs = default_inputs();
        inputs.overdue_task_count = 3;
        inputs.tasks_due_within_24h = 3;
        let sit = compute_situation(&inputs);
        assert!(sit.deadline_pressure > 0.5);
    }

    #[test]
    fn test_distraction_risk_with_many_distractions() {
        let mut inputs = default_inputs();
        inputs.distraction_count_last_30min = 5;
        inputs.context_switches_last_30min = 15;
        let sit = compute_situation(&inputs);
        assert!(sit.distraction_risk > 0.5);
    }

    #[test]
    fn test_coaching_receptivity_drops_with_dismissals() {
        let mut inputs = default_inputs();
        inputs.recent_dismissals = 0;
        let open = compute_situation(&inputs);

        inputs.recent_dismissals = 4;
        let closed = compute_situation(&inputs);

        assert!(open.coaching_receptivity > closed.coaching_receptivity);
    }

    #[test]
    fn test_task_avoidance_detection() {
        let mut inputs = default_inputs();
        inputs.times_deferred_max = 4;
        inputs.productive_ratio_today = 0.3;
        let sit = compute_situation(&inputs);
        assert!(sit.task_avoidance_detected);
    }

    #[test]
    fn test_no_task_avoidance_when_productive() {
        let mut inputs = default_inputs();
        inputs.times_deferred_max = 4;
        inputs.productive_ratio_today = 0.6;
        let sit = compute_situation(&inputs);
        assert!(!sit.task_avoidance_detected);
    }

    #[test]
    fn test_all_values_clamped_0_1() {
        // Extreme inputs
        let inputs = SituationInputs {
            hours_active_today: 100.0,
            mins_since_break: 1000.0,
            productive_ratio_today: 0.0,
            distraction_count_last_30min: 100,
            context_switches_last_30min: 100,
            is_in_focus_session: false,
            focus_quality: None,
            overdue_task_count: 100,
            tasks_due_within_24h: 100,
            times_deferred_max: 100,
            peak_hour_match: false,
            historical_avg_context_switches: 1.0,
            recent_dismissals: 100,
            coaching_intensity: 0.0,
            hour_of_day: 23,
        };
        let sit = compute_situation(&inputs);
        assert!(sit.energy_level >= 0.0 && sit.energy_level <= 1.0);
        assert!(sit.focus_state >= 0.0 && sit.focus_state <= 1.0);
        assert!(sit.deadline_pressure >= 0.0 && sit.deadline_pressure <= 1.0);
        assert!(sit.distraction_risk >= 0.0 && sit.distraction_risk <= 1.0);
        assert!(sit.coaching_receptivity >= 0.0 && sit.coaching_receptivity <= 1.0);
    }
}
