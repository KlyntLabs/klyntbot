//! Feedback level decider — FSRS-driven adaptive feedback escalation.
//!
//! Determines whether pronunciation feedback should be shown as a post-turn
//! summary, a real-time overlay on weak spots, or silently in the background.

pub use config::schema::FeedbackLevel;

/// FSRS-like mastery state for a phoneme.
#[derive(Debug, Clone)]
pub struct PhonemeMastery {
    pub phoneme: String,
    /// FSRS stability (higher = more stable recall).
    pub stability: f32,
    /// Total number of encounters.
    pub encounters: u32,
    /// Number of errors in recent encounters.
    pub recent_errors: u32,
}

/// Decide the feedback level for a given set of phoneme mastery data.
///
/// If any phoneme has stability < `escalation_threshold` AND enough
/// encounters (>= `min_encounters`), escalates to `Overlay`.
/// Otherwise uses the configured default level.
pub fn decide_feedback_level(
    mastery: &[PhonemeMastery],
    default_level: FeedbackLevel,
    escalation_threshold: f32,
    min_encounters: u32,
) -> FeedbackLevel {
    let needs_escalation = mastery.iter().any(|m| {
        m.encounters >= min_encounters && m.stability < escalation_threshold && m.recent_errors > 0
    });

    if needs_escalation {
        FeedbackLevel::Overlay
    } else {
        default_level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_mastery_uses_default() {
        assert_eq!(
            decide_feedback_level(&[], FeedbackLevel::Summary, 0.3, 5),
            FeedbackLevel::Summary
        );
    }

    #[test]
    fn stable_phonemes_use_default() {
        let mastery = vec![PhonemeMastery {
            phoneme: "θ".to_string(),
            stability: 0.8,
            encounters: 10,
            recent_errors: 0,
        }];
        assert_eq!(
            decide_feedback_level(&mastery, FeedbackLevel::Silent, 0.3, 5),
            FeedbackLevel::Silent
        );
    }

    #[test]
    fn weak_phoneme_escalates_to_overlay() {
        let mastery = vec![PhonemeMastery {
            phoneme: "θ".to_string(),
            stability: 0.1,
            encounters: 8,
            recent_errors: 3,
        }];
        assert_eq!(
            decide_feedback_level(&mastery, FeedbackLevel::Summary, 0.3, 5),
            FeedbackLevel::Overlay
        );
    }

    #[test]
    fn weak_but_insufficient_encounters_stays_default() {
        let mastery = vec![PhonemeMastery {
            phoneme: "θ".to_string(),
            stability: 0.1,
            encounters: 2,
            recent_errors: 2,
        }];
        assert_eq!(
            decide_feedback_level(&mastery, FeedbackLevel::Summary, 0.3, 5),
            FeedbackLevel::Summary
        );
    }
}
