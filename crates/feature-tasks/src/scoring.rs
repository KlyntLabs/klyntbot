//! Scoring utilities for task prioritization.
//!
//! Ported from feature-todo's tool scoring helpers. These are standalone functions
//! (not methods on a tool struct) for use in planning and suggestion generation.

use chrono::{DateTime, Utc};

use crate::types::Task;

/// Calculate urgency score based on due date proximity.
///
/// Returns:
/// - 10 if overdue
/// - 5 if due today
/// - 3 if due tomorrow
/// - 1 otherwise or no due date
pub fn calculate_urgency(due_date: Option<DateTime<Utc>>, now: DateTime<Utc>) -> u32 {
    match due_date {
        None => 1,
        Some(due) => {
            let now_date = now.date_naive();
            let due_date = due.date_naive();
            if due_date < now_date {
                10
            } else if due_date == now_date {
                5
            } else if due_date == now_date + chrono::Duration::days(1) {
                3
            } else {
                1
            }
        }
    }
}

/// Map a priority value (1=highest, 5=lowest) to a weight.
pub fn priority_weight(priority: Option<i16>) -> u32 {
    match priority {
        Some(1) => 5,
        Some(2) => 4,
        Some(3) => 3,
        Some(4) => 2,
        Some(5) => 1,
        None => 3,
        _ => 3,
    }
}

/// Calculate the age of a task in fractional days.
pub fn calculate_age_days(created_at: DateTime<Utc>, now: DateTime<Utc>) -> f64 {
    let duration = now.signed_duration_since(created_at);
    (duration.num_seconds().max(0) as f64) / 86400.0
}

/// Calculate a composite score for a task based on urgency, priority, and age.
///
/// Higher scores indicate higher priority for scheduling.
pub fn calculate_score(task: &Task, now: DateTime<Utc>) -> f64 {
    let urgency = calculate_urgency(task.due_date, now) as f64;
    let priority_wt = priority_weight(task.priority) as f64;
    let age_days = calculate_age_days(task.created_at, now);
    (urgency * priority_wt) + (age_days * 0.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_urgency_no_due_date() {
        let now = Utc::now();
        assert_eq!(calculate_urgency(None, now), 1);
    }

    #[test]
    fn test_urgency_overdue() {
        let now = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();
        let due = Utc.with_ymd_and_hms(2025, 6, 14, 12, 0, 0).unwrap();
        assert_eq!(calculate_urgency(Some(due), now), 10);
    }

    #[test]
    fn test_urgency_due_today() {
        let now = Utc.with_ymd_and_hms(2025, 6, 15, 9, 0, 0).unwrap();
        let due = Utc.with_ymd_and_hms(2025, 6, 15, 23, 0, 0).unwrap();
        assert_eq!(calculate_urgency(Some(due), now), 5);
    }

    #[test]
    fn test_urgency_due_tomorrow() {
        let now = Utc.with_ymd_and_hms(2025, 6, 15, 9, 0, 0).unwrap();
        let due = Utc.with_ymd_and_hms(2025, 6, 16, 9, 0, 0).unwrap();
        assert_eq!(calculate_urgency(Some(due), now), 3);
    }

    #[test]
    fn test_urgency_future() {
        let now = Utc.with_ymd_and_hms(2025, 6, 15, 9, 0, 0).unwrap();
        let due = Utc.with_ymd_and_hms(2025, 6, 20, 9, 0, 0).unwrap();
        assert_eq!(calculate_urgency(Some(due), now), 1);
    }

    #[test]
    fn test_priority_weights() {
        assert_eq!(priority_weight(Some(1)), 5);
        assert_eq!(priority_weight(Some(2)), 4);
        assert_eq!(priority_weight(Some(3)), 3);
        assert_eq!(priority_weight(Some(4)), 2);
        assert_eq!(priority_weight(Some(5)), 1);
        assert_eq!(priority_weight(None), 3);
        assert_eq!(priority_weight(Some(99)), 3);
    }

    #[test]
    fn test_age_days() {
        let now = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();
        let created = Utc.with_ymd_and_hms(2025, 6, 14, 12, 0, 0).unwrap();
        let age = calculate_age_days(created, now);
        assert!((age - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_calculate_score() {
        let mut task = crate::types::Task::default_instance();
        task.area_id = "area-1".to_string();
        task.priority = Some(1);
        let now = Utc::now();
        let score = calculate_score(&task, now);
        // urgency=1 (no due date) * priority_weight=5 + small age component
        assert!(score >= 5.0);
    }

    #[test]
    fn test_score_overdue_high_priority_beats_future_low() {
        let now = Utc.with_ymd_and_hms(2025, 6, 15, 12, 0, 0).unwrap();

        let mut high = crate::types::Task::default_instance();
        high.area_id = "a".to_string();
        high.priority = Some(1);
        high.due_date = Some(Utc.with_ymd_and_hms(2025, 6, 14, 12, 0, 0).unwrap());
        high.created_at = now;

        let mut low = crate::types::Task::default_instance();
        low.area_id = "a".to_string();
        low.priority = Some(5);
        low.due_date = Some(Utc.with_ymd_and_hms(2025, 7, 1, 12, 0, 0).unwrap());
        low.created_at = now;

        assert!(calculate_score(&high, now) > calculate_score(&low, now));
    }
}
