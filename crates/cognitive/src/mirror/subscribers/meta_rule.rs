//! MetaRuleDetector — proposes meta-rules from correction streaks and
//! low-confidence routing patterns.

use std::collections::HashMap;

use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};
use uuid::Uuid;

use bus::DomainEvent;
use chrono::Utc;

use crate::mirror::{
    snippet_from_alert, MetaRule, MetaRuleAction, MetaRuleSource, MetaRuleStatus, MirrorAlert,
    MirrorRepo,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const LOW_CONFIDENCE_THRESHOLD: f64 = 0.4;
const SAME_SESSION_CORRECTION_THRESHOLD: u32 = 2;
const CROSS_SESSION_CORRECTION_THRESHOLD: u32 = 3;
const LOW_CONFIDENCE_STREAK_THRESHOLD: u32 = 3;

// ---------------------------------------------------------------------------
// MetaRuleDetector
// ---------------------------------------------------------------------------

/// Detects correction streaks and low-confidence routing patterns, proposing
/// [`MetaRule`]s when thresholds are exceeded.
pub struct MetaRuleDetector {
    /// Per-session correction count.
    session_corrections: HashMap<String, u32>,
    /// Per-skill cross-session correction count.
    skill_corrections: HashMap<String, u32>,
    /// Consecutive low-confidence routing streak.
    low_confidence_streak: u32,
    /// Repo for persisting proposed meta-rules and snippets.
    repo: Option<MirrorRepo>,
}

impl MetaRuleDetector {
    /// Create a detector backed by a real [`MirrorRepo`].
    pub fn new(repo: MirrorRepo) -> Self {
        Self {
            session_corrections: HashMap::new(),
            skill_corrections: HashMap::new(),
            low_confidence_streak: 0,
            repo: Some(repo),
        }
    }

    /// Create a test instance with no repo (handle_alert is a no-op).
    #[cfg(test)]
    pub fn new_for_test() -> Self {
        Self {
            session_corrections: HashMap::new(),
            skill_corrections: HashMap::new(),
            low_confidence_streak: 0,
            repo: None,
        }
    }

    // -----------------------------------------------------------------------
    // Correction tracking
    // -----------------------------------------------------------------------

    /// Record a user correction. Returns a [`MirrorAlert`] if same-session or
    /// cross-session thresholds are exceeded.
    pub fn record_correction(
        &mut self,
        session_key: &str,
        skill_name: &str,
    ) -> Option<MirrorAlert> {
        // Track per-session corrections.
        let session_count = self
            .session_corrections
            .entry(session_key.to_string())
            .or_insert(0);
        *session_count += 1;

        if *session_count >= SAME_SESSION_CORRECTION_THRESHOLD {
            let rule_id = Uuid::new_v4();
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id,
                rule_text: format!(
                    "User corrected me {session_count} times in the same session \
                     — ask for clarification before responding"
                ),
                source: MetaRuleSource::CorrectionDerived,
            });
        }

        // Track per-skill cross-session corrections.
        let skill_count = self
            .skill_corrections
            .entry(skill_name.to_string())
            .or_insert(0);
        *skill_count += 1;

        if *skill_count >= CROSS_SESSION_CORRECTION_THRESHOLD {
            let rule_id = Uuid::new_v4();
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id,
                rule_text: format!(
                    "User has corrected me {skill_count} times across sessions for the \
                     '{skill_name}' skill — adjust routing or response strategy"
                ),
                source: MetaRuleSource::CorrectionDerived,
            });
        }

        None
    }

    // -----------------------------------------------------------------------
    // Low-confidence routing tracking
    // -----------------------------------------------------------------------

    /// Record a low-confidence routing event. Returns a [`MirrorAlert`] if
    /// the streak reaches the threshold, then resets.
    pub fn record_low_confidence(
        &mut self,
        skill: &str,
        confidence: f64,
    ) -> Option<MirrorAlert> {
        if confidence >= LOW_CONFIDENCE_THRESHOLD {
            return None;
        }

        self.low_confidence_streak += 1;

        if self.low_confidence_streak >= LOW_CONFIDENCE_STREAK_THRESHOLD {
            self.low_confidence_streak = 0;
            let rule_id = Uuid::new_v4();
            return Some(MirrorAlert::MetaRuleProposed {
                rule_id,
                rule_text: format!(
                    "Routing confidence has been consistently low (latest: {confidence:.2} \
                     for '{skill}') — consider adding trigger keywords or adjusting skill scopes"
                ),
                source: MetaRuleSource::ReflectionGenerated,
            });
        }

        None
    }

    /// Reset the low-confidence streak after a high-confidence routing event.
    pub fn record_high_confidence(&mut self) {
        self.low_confidence_streak = 0;
    }

    // -----------------------------------------------------------------------
    // Alert handling
    // -----------------------------------------------------------------------

    /// Persist a proposed meta-rule and its narrative snippet to the repo.
    pub async fn handle_alert(&self, alert: &MirrorAlert) {
        let Some(repo) = &self.repo else { return };

        if let MirrorAlert::MetaRuleProposed {
            rule_id,
            rule_text,
            source,
        } = alert
        {
            let rule = MetaRule {
                id: *rule_id,
                trigger_condition: rule_text.clone(),
                action: MetaRuleAction::ForceClarification,
                source: source.clone(),
                effectiveness_score: 0.5,
                status: MetaRuleStatus::Pending,
                signal_count: 1,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            };

            if let Err(e) = repo.insert_meta_rule(&rule).await {
                warn!("MetaRuleDetector: failed to insert meta-rule: {e}");
            }

            let snippet = snippet_from_alert(alert);
            if let Err(e) = repo.insert_snippet(&snippet).await {
                warn!("MetaRuleDetector: failed to insert snippet: {e}");
            }
        }
    }

    // -----------------------------------------------------------------------
    // Run loop
    // -----------------------------------------------------------------------

    /// Start the event-driven detector loop.
    pub async fn run(
        mut self,
        mut rx: broadcast::Receiver<DomainEvent>,
        shutdown: CancellationToken,
    ) {
        loop {
            tokio::select! {
                _ = shutdown.cancelled() => {
                    info!("MetaRuleDetector: shutdown received");
                    break;
                }
                result = rx.recv() => {
                    match result {
                        Ok(DomainEvent::UserCorrectedAI {
                            session_key,
                            active_skill,
                            ..
                        }) => {
                            let skill = active_skill
                                .as_deref()
                                .unwrap_or("unknown");
                            if let Some(alert) =
                                self.record_correction(&session_key, skill)
                            {
                                self.handle_alert(&alert).await;
                            }
                        }
                        Ok(DomainEvent::SkillRouted {
                            confidence,
                            skill_name,
                            ..
                        }) => {
                            if confidence < LOW_CONFIDENCE_THRESHOLD {
                                if let Some(alert) =
                                    self.record_low_confidence(&skill_name, confidence)
                                {
                                    self.handle_alert(&alert).await;
                                }
                            } else {
                                self.record_high_confidence();
                            }
                        }
                        Ok(_) => {
                            // Not an event we care about — ignore.
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("MetaRuleDetector: lagged, skipped {n} events");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("MetaRuleDetector: event channel closed");
                            break;
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correction_streak_same_session() {
        let mut detector = MetaRuleDetector::new_for_test();

        let result1 = detector.record_correction("session-1", "general");
        assert!(result1.is_none(), "First correction should not trigger");

        let result2 = detector.record_correction("session-1", "general");
        assert!(result2.is_some(), "Second correction in same session should trigger");

        match result2.unwrap() {
            MirrorAlert::MetaRuleProposed { rule_text, source, .. } => {
                assert!(rule_text.contains("2 times"));
                assert_eq!(source, MetaRuleSource::CorrectionDerived);
            }
            other => panic!("Expected MetaRuleProposed, got {other:?}"),
        }
    }

    #[test]
    fn test_correction_single_no_trigger() {
        let mut detector = MetaRuleDetector::new_for_test();

        let result = detector.record_correction("session-1", "finance");
        assert!(result.is_none(), "Single correction should not trigger any alert");
    }

    #[test]
    fn test_low_confidence_streak() {
        let mut detector = MetaRuleDetector::new_for_test();

        let r1 = detector.record_low_confidence("general", 0.3);
        assert!(r1.is_none());

        let r2 = detector.record_low_confidence("general", 0.2);
        assert!(r2.is_none());

        let r3 = detector.record_low_confidence("general", 0.1);
        assert!(r3.is_some(), "Third consecutive low-confidence should trigger");

        match r3.unwrap() {
            MirrorAlert::MetaRuleProposed { rule_text, source, .. } => {
                assert!(rule_text.contains("consistently low"));
                assert_eq!(source, MetaRuleSource::ReflectionGenerated);
            }
            other => panic!("Expected MetaRuleProposed, got {other:?}"),
        }
    }

    #[test]
    fn test_low_confidence_resets_on_high() {
        let mut detector = MetaRuleDetector::new_for_test();

        detector.record_low_confidence("general", 0.3);
        detector.record_low_confidence("general", 0.2);

        // High confidence resets the streak.
        detector.record_high_confidence();

        // Start new streak — need 3 more.
        let r1 = detector.record_low_confidence("general", 0.3);
        assert!(r1.is_none());
        let r2 = detector.record_low_confidence("general", 0.2);
        assert!(r2.is_none());
        let r3 = detector.record_low_confidence("general", 0.1);
        assert!(r3.is_some(), "Should trigger after new streak of 3");
    }

    #[test]
    fn test_cross_session_correction_streak() {
        let mut detector = MetaRuleDetector::new_for_test();

        // Different sessions, same skill.
        let r1 = detector.record_correction("session-1", "finance");
        assert!(r1.is_none());

        let r2 = detector.record_correction("session-2", "finance");
        assert!(r2.is_none());

        let r3 = detector.record_correction("session-3", "finance");
        assert!(r3.is_some(), "Third cross-session correction for same skill should trigger");

        match r3.unwrap() {
            MirrorAlert::MetaRuleProposed { rule_text, source, .. } => {
                assert!(rule_text.contains("finance"));
                assert!(rule_text.contains("3 times across sessions"));
                assert_eq!(source, MetaRuleSource::CorrectionDerived);
            }
            other => panic!("Expected MetaRuleProposed, got {other:?}"),
        }
    }
}
