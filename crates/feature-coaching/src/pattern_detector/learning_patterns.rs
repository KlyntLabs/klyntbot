//! Learning pattern detectors — detect study streaks, retention decay,
//! learning momentum imbalance, domain gaps, and knowledge transfer events.

use std::collections::{BTreeSet, VecDeque};

use jiff::{SignedDuration, Timestamp};

use crate::pattern_detector::DetectedPattern;

/// Streak milestones that trigger a celebration pattern.
const STREAK_MILESTONES: &[i64] = &[3, 7, 14, 30, 60, 100];

/// Detect consecutive-day study streaks and celebrate milestones.
///
/// Counts distinct review days backwards from the most recent entry.
/// Fires at milestones (3/7/14/30/60/100 days) and when a streak >= 3 is
/// at risk (last review was yesterday, none today).
pub fn detect_study_streak(reviews: &VecDeque<(Timestamp, String)>) -> Option<DetectedPattern> {
    if reviews.is_empty() {
        return None;
    }

    let today = Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC).date();

    // Collect distinct review dates.
    let review_dates: BTreeSet<_> = reviews
        .iter()
        .map(|(ts, _)| ts.to_zoned(jiff::tz::TimeZone::UTC).date())
        .collect();

    // Count consecutive days backwards from the most recent review date.
    let most_recent = *review_dates.iter().next_back()?;
    let mut streak = 1i64;
    let mut check_date = most_recent;
    loop {
        let prev = check_date - jiff::Span::new().days(1);
        if review_dates.contains(&prev) {
            streak += 1;
            check_date = prev;
        } else {
            break;
        }
    }

    // Check if streak is at risk: last review was yesterday (not today) and streak >= 3.
    let yesterday = today - jiff::Span::new().days(1);
    if most_recent == yesterday && streak >= 3 {
        return Some(DetectedPattern {
            name: "study_streak_at_risk".into(),
            confidence: 0.85,
            signal_count: streak as i32,
            description: format!("{streak}-day study streak at risk — no review yet today"),
            domain: "learning".into(),
        });
    }

    // Celebrate milestone streaks (only if most recent review is today or yesterday).
    if most_recent < yesterday {
        return None; // Streak is already broken.
    }

    // Find the highest milestone reached.
    let milestone = STREAK_MILESTONES.iter().rev().find(|&&m| streak >= m);

    milestone.map(|&m| DetectedPattern {
        name: format!("study_streak_{m}_days"),
        confidence: (0.7 + (streak as f64 / 100.0)).min(0.95),
        signal_count: streak as i32,
        description: format!("{streak}-day study streak! Milestone: {m} days"),
        domain: "learning".into(),
    })
}

/// Detect retention decay — triggered when recent digest signals indicate
/// fading knowledge atoms.
pub fn detect_retention_decay(digests: &VecDeque<(Timestamp, String)>) -> Option<DetectedPattern> {
    if digests.is_empty() {
        return None;
    }

    let cutoff = Timestamp::now() - SignedDuration::from_hours(24);
    let recent: Vec<_> = digests.iter().filter(|(ts, _)| *ts > cutoff).collect();

    if recent.is_empty() {
        return None;
    }

    Some(DetectedPattern {
        name: "retention_decay_detected".into(),
        confidence: (0.6 + (recent.len() as f64 * 0.1)).min(0.9),
        signal_count: recent.len() as i32,
        description: format!(
            "{} learning digest signal(s) in the last 24h indicate fading atoms",
            recent.len()
        ),
        domain: "learning".into(),
    })
}

/// Detect learning momentum imbalance by comparing creation vs review counts
/// in the last 7 days.
pub fn detect_learning_momentum(
    creations: &VecDeque<(Timestamp, String)>,
    reviews: &VecDeque<(Timestamp, String)>,
) -> Option<DetectedPattern> {
    let cutoff = Timestamp::now() - SignedDuration::from_hours(7 * 24);

    let created_count = creations.iter().filter(|(ts, _)| *ts > cutoff).count();
    let reviewed_count = reviews.iter().filter(|(ts, _)| *ts > cutoff).count();

    // Create-heavy: creating much more than reviewing.
    if created_count >= 5 && reviewed_count == 0 {
        return Some(DetectedPattern {
            name: "learning_momentum_create_heavy".into(),
            confidence: 0.8,
            signal_count: created_count as i32,
            description: format!(
                "Created {created_count} atoms but reviewed {reviewed_count} in the last 7 days — review backlog growing"
            ),
            domain: "learning".into(),
        });
    }

    if created_count >= 5 {
        let ratio = created_count as f64 / reviewed_count.max(1) as f64;
        if ratio > 3.0 {
            return Some(DetectedPattern {
                name: "learning_momentum_create_heavy".into(),
                confidence: (0.6 + (ratio / 20.0)).min(0.9),
                signal_count: created_count as i32,
                description: format!(
                    "Created {created_count} atoms but only reviewed {reviewed_count} in the last 7 days — review backlog growing"
                ),
                domain: "learning".into(),
            });
        }
    }

    // Review-strong: reviewing much more than creating.
    if reviewed_count >= 10 {
        let ratio = if created_count == 0 {
            0.0
        } else {
            created_count as f64 / reviewed_count as f64
        };
        if ratio < 0.3 {
            return Some(DetectedPattern {
                name: "learning_momentum_review_strong".into(),
                confidence: (0.6 + (reviewed_count as f64 / 50.0)).min(0.9),
                signal_count: reviewed_count as i32,
                description: format!(
                    "Reviewed {reviewed_count} atoms vs {created_count} created in the last 7 days — strong review habit"
                ),
                domain: "learning".into(),
            });
        }
    }

    None
}

/// Detect domain-level retention gaps via repeated digest signals.
pub fn detect_domain_gap(digests: &VecDeque<(Timestamp, String)>) -> Option<DetectedPattern> {
    let cutoff = Timestamp::now() - SignedDuration::from_hours(7 * 24);
    let recent_count = digests.iter().filter(|(ts, _)| *ts > cutoff).count();

    if recent_count >= 2 {
        Some(DetectedPattern {
            name: "domain_retention_gap".into(),
            confidence: (0.5 + (recent_count as f64 * 0.1)).min(0.9),
            signal_count: recent_count as i32,
            description: format!(
                "{recent_count} digest signals in the last 7 days suggest a domain retention gap"
            ),
            domain: "learning".into(),
        })
    } else {
        None
    }
}

/// Detect knowledge transfer events in the last 24 hours.
pub fn detect_knowledge_transfer(
    transfers: &VecDeque<(Timestamp, String)>,
) -> Option<DetectedPattern> {
    let cutoff = Timestamp::now() - SignedDuration::from_hours(24);
    let recent: Vec<_> = transfers.iter().filter(|(ts, _)| *ts > cutoff).collect();

    if recent.is_empty() {
        return None;
    }

    Some(DetectedPattern {
        name: "knowledge_transfer_detected".into(),
        confidence: 0.75,
        signal_count: recent.len() as i32,
        description: format!(
            "{} knowledge transfer event(s) detected in the last 24h",
            recent.len()
        ),
        domain: "learning".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_history(days_ago: &[i64]) -> VecDeque<(Timestamp, String)> {
        let now = Timestamp::now();
        days_ago
            .iter()
            .map(|&d| (now - SignedDuration::from_hours(d * 24), "test".into()))
            .collect()
    }

    fn make_recent_history(count: usize) -> VecDeque<(Timestamp, String)> {
        let now = Timestamp::now();
        (0..count)
            .map(|i| (now - SignedDuration::from_hours(i as i64), "test".into()))
            .collect()
    }

    #[test]
    fn test_streak_milestone_7_days() {
        // Reviews on each of the last 7 days (today through 6 days ago).
        let history = make_history(&[0, 1, 2, 3, 4, 5, 6]);
        let pattern = detect_study_streak(&history);
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.name, "study_streak_7_days");
        assert_eq!(p.signal_count, 7);
        assert_eq!(p.domain, "learning");
    }

    #[test]
    fn test_streak_at_risk() {
        // Reviews yesterday and the 2 days before (3-day streak), but NOT today.
        let history = make_history(&[1, 2, 3]);
        let pattern = detect_study_streak(&history);
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.name, "study_streak_at_risk");
        assert_eq!(p.signal_count, 3);
    }

    #[test]
    fn test_momentum_create_heavy() {
        // 6 creations, 1 review in the last 7 days.
        let creations = make_recent_history(6);
        let reviews = make_recent_history(1);
        let pattern = detect_learning_momentum(&creations, &reviews);
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.name, "learning_momentum_create_heavy");
    }

    #[test]
    fn test_empty_history_returns_none() {
        let empty: VecDeque<(Timestamp, String)> = VecDeque::new();
        assert!(detect_study_streak(&empty).is_none());
        assert!(detect_retention_decay(&empty).is_none());
        assert!(detect_learning_momentum(&empty, &empty).is_none());
        assert!(detect_domain_gap(&empty).is_none());
        assert!(detect_knowledge_transfer(&empty).is_none());
    }

    #[test]
    fn test_retention_decay_recent_signals() {
        let digests = make_recent_history(2);
        let pattern = detect_retention_decay(&digests);
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.name, "retention_decay_detected");
        assert_eq!(p.signal_count, 2);
    }

    #[test]
    fn test_domain_gap_detected() {
        let digests = make_recent_history(3);
        let pattern = detect_domain_gap(&digests);
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.name, "domain_retention_gap");
    }

    #[test]
    fn test_knowledge_transfer_detected() {
        let transfers = make_recent_history(1);
        let pattern = detect_knowledge_transfer(&transfers);
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.name, "knowledge_transfer_detected");
    }

    #[test]
    fn test_streak_broken_returns_none() {
        // Last review was 3 days ago — streak is broken.
        let history = make_history(&[3, 4, 5]);
        let pattern = detect_study_streak(&history);
        assert!(pattern.is_none());
    }

    #[test]
    fn test_momentum_review_strong() {
        let creations = make_recent_history(2);
        let reviews = make_recent_history(12);
        let pattern = detect_learning_momentum(&creations, &reviews);
        assert!(pattern.is_some());
        let p = pattern.unwrap();
        assert_eq!(p.name, "learning_momentum_review_strong");
    }

    #[test]
    fn test_no_streak_below_milestone() {
        // Only 2 days — below the minimum milestone of 3.
        let history = make_history(&[0, 1]);
        let pattern = detect_study_streak(&history);
        assert!(pattern.is_none());
    }
}
