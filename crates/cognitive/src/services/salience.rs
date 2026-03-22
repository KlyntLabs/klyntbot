//! Salience filter: classifies DomainEvents for cognitive processing priority.

use bus::DomainEvent;

use crate::types::SalienceVerdict;

/// Deviation percentage threshold above which estimation events are promoted to Extract.
/// Shared with `background::event_to_observation` for consistent importance scaling.
pub const HIGH_DEVIATION_THRESHOLD: f64 = 50.0;

/// Evaluate the salience of a domain event.
///
/// - `Extract`: high-value events processed immediately (user-explicit, threshold-crossing)
/// - `Accumulate`: routine events buffered for pattern detection
/// - `Discard`: low-value events ignored
pub fn evaluate_salience(event: &DomainEvent) -> SalienceVerdict {
    match event {
        // User-explicit events are always high-value
        DomainEvent::UserStatedFact { .. } => SalienceVerdict::Extract,
        DomainEvent::UserCorrectedAI { .. } => SalienceVerdict::Extract,

        // Chat turns — extract facts from conversation content
        DomainEvent::ChatTurnCompleted { .. } => SalienceVerdict::Extract,

        // Threshold-crossing events
        DomainEvent::BudgetAlert { .. } => SalienceVerdict::Extract,
        DomainEvent::TransactionRecorded {
            is_over_budget: true,
            ..
        } => SalienceVerdict::Extract,

        // Coaching feedback is always important
        DomainEvent::CoachingFeedback { .. } => SalienceVerdict::Extract,

        // Intelligence layer events
        DomainEvent::SessionCreated { .. } => SalienceVerdict::Discard,
        DomainEvent::SessionEnded {
            quality_score: Some(q),
            ..
        } if *q >= 80.0 => SalienceVerdict::Extract,
        DomainEvent::SessionEnded { .. } => SalienceVerdict::Accumulate,
        DomainEvent::QualityScored { overall_score, .. }
            if *overall_score >= 85.0 || *overall_score <= 30.0 =>
        {
            SalienceVerdict::Extract
        }
        DomainEvent::QualityScored { .. } => SalienceVerdict::Accumulate,
        DomainEvent::NarrativeGenerated { .. } => SalienceVerdict::Extract,
        DomainEvent::PredictiveAlert { .. } => SalienceVerdict::Accumulate,
        DomainEvent::RuleEvolved { .. } => SalienceVerdict::Discard,
        DomainEvent::VoiceJournalProcessed { .. } => SalienceVerdict::Extract,

        // Routine events — accumulated for pattern detection
        DomainEvent::ProductivityScoreComputed { .. } => SalienceVerdict::Accumulate,
        DomainEvent::ActivitySessionCompleted { .. } => SalienceVerdict::Accumulate,
        DomainEvent::FocusSessionStarted { .. } => SalienceVerdict::Accumulate,
        DomainEvent::FocusSessionEnded { .. } => SalienceVerdict::Accumulate,
        DomainEvent::DistractionDetected { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskCreated { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskCompleted { deviation_pct, .. } => {
            if deviation_pct.is_some_and(|d| d.abs() > HIGH_DEVIATION_THRESHOLD) {
                SalienceVerdict::Extract
            } else {
                SalienceVerdict::Accumulate
            }
        }
        DomainEvent::TaskDeferred { .. } => SalienceVerdict::Accumulate,
        DomainEvent::GoalProgress { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TransactionRecorded {
            is_over_budget: false,
            ..
        } => SalienceVerdict::Accumulate,
        DomainEvent::NoteCreated { .. } => SalienceVerdict::Accumulate,
        DomainEvent::NoteUpdated { .. } => SalienceVerdict::Accumulate,
        DomainEvent::ToolCallExecuted { .. } => SalienceVerdict::Accumulate,
        DomainEvent::BehavioralPatternDetected { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskFocusStarted { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskFocusEnded { .. } => SalienceVerdict::Accumulate,
        DomainEvent::EstimationRecorded { deviation_pct, .. } => {
            if deviation_pct.abs() > HIGH_DEVIATION_THRESHOLD {
                SalienceVerdict::Extract
            } else {
                SalienceVerdict::Accumulate
            }
        }
        DomainEvent::TaskExecutionStarted { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskExecutionCompleted { .. } => SalienceVerdict::Extract,
        DomainEvent::TaskExecutionFailed { .. } => SalienceVerdict::Extract,
        DomainEvent::TaskExecutionProgress { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskDecomposed { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskBlocked { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskUnblocked { .. } => SalienceVerdict::Accumulate,
        DomainEvent::DayPlanGenerated { .. } => SalienceVerdict::Accumulate,
        DomainEvent::ProactiveSuggestionCreated { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskStatusChanged { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskPriorityChanged { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TaskFieldUpdated { .. } => SalienceVerdict::Accumulate,

        // Autotuner decisions — always extract for cognitive processing
        DomainEvent::AutotunerDecision { .. } => SalienceVerdict::Extract,

        // Contradiction detection — always extract for cognitive processing
        DomainEvent::ContradictionDetected { .. } => SalienceVerdict::Extract,

        // Knowledge Atoms
        DomainEvent::KnowledgeAtomAccepted { .. } => SalienceVerdict::Extract,
        DomainEvent::RetentionMilestoneReached { .. } => SalienceVerdict::Extract,
        DomainEvent::AtomFlashcardReviewed { .. } => SalienceVerdict::Accumulate,
        DomainEvent::TranslationCompleted { .. } => SalienceVerdict::Accumulate,
        DomainEvent::AtomReinforced { .. } => SalienceVerdict::Accumulate,
        DomainEvent::NoteStudied { .. } => SalienceVerdict::Accumulate,
        DomainEvent::PracticeUnitCompleted { .. } => SalienceVerdict::Accumulate,
        DomainEvent::PracticeSessionCompleted { .. } => SalienceVerdict::Accumulate,
        DomainEvent::KnowledgeTransferDetected { .. } => SalienceVerdict::Accumulate,
        DomainEvent::KnowledgeAtomCreated { .. } => SalienceVerdict::Discard,
        DomainEvent::KnowledgeAtomArchived { .. } => SalienceVerdict::Discard,
        DomainEvent::AtomInteracted { .. } => SalienceVerdict::Discard,
        DomainEvent::CoachingLearningDigest { .. } => SalienceVerdict::Discard,

        // BookIndex events — discarded from cognitive extraction (handled by BookIndex updater)
        DomainEvent::NoteContentChanged { .. } => SalienceVerdict::Discard,
        DomainEvent::NoteDeleted { .. } => SalienceVerdict::Discard,
        DomainEvent::TaskHierarchyChanged { .. } => SalienceVerdict::Discard,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_stated_fact_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::UserStatedFact {
            fact: "I work best in mornings".into(),
            domain: "productivity".into(),
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_user_correction_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::UserCorrectedAI {
            original: "You like afternoon work".into(),
            correction: "No, I prefer mornings".into(),
            kind: bus::CorrectionKind::Reaction,
            strength: 1.0,
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_budget_alert_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::BudgetAlert {
            category: "food".into(),
            spent: 450.0,
            limit: 500.0,
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_over_budget_transaction_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::TransactionRecorded {
            category: "food".into(),
            amount: 50.0,
            is_over_budget: true,
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_normal_transaction_is_accumulate() {
        let verdict = evaluate_salience(&DomainEvent::TransactionRecorded {
            category: "food".into(),
            amount: 12.0,
            is_over_budget: false,
        });
        assert_eq!(verdict, SalienceVerdict::Accumulate);
    }

    #[test]
    fn test_normal_productivity_score_is_accumulate() {
        let verdict = evaluate_salience(&DomainEvent::ProductivityScoreComputed {
            date: "2026-03-06".into(),
            score: 72.0,
        });
        assert_eq!(verdict, SalienceVerdict::Accumulate);
    }

    #[test]
    fn test_task_completed_is_accumulate() {
        let verdict = evaluate_salience(&DomainEvent::TaskCompleted {
            task_id: "t1".into(),
            actual_duration_mins: Some(30),
            estimated_duration_mins: Some(45),
            deviation_pct: None,
        });
        assert_eq!(verdict, SalienceVerdict::Accumulate);
    }

    #[test]
    fn test_focus_session_is_accumulate() {
        let verdict = evaluate_salience(&DomainEvent::FocusSessionEnded {
            duration_secs: 3600,
            quality: 0.85,
            interruptions: 2,
        });
        assert_eq!(verdict, SalienceVerdict::Accumulate);
    }

    #[test]
    fn test_chat_turn_completed_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::ChatTurnCompleted {
            user_message: "My favorite language is Rust".into(),
            session_key: "test-session".into(),
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_coaching_feedback_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::CoachingFeedback {
            intervention_id: "i1".into(),
            response: bus::FeedbackResponse::Helpful,
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_task_completed_large_deviation_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::TaskCompleted {
            task_id: "t1".into(),
            actual_duration_mins: Some(90),
            estimated_duration_mins: Some(30),
            deviation_pct: Some(200.0),
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_task_completed_small_deviation_is_accumulate() {
        let verdict = evaluate_salience(&DomainEvent::TaskCompleted {
            task_id: "t1".into(),
            actual_duration_mins: Some(35),
            estimated_duration_mins: Some(30),
            deviation_pct: Some(16.7),
        });
        assert_eq!(verdict, SalienceVerdict::Accumulate);
    }

    #[test]
    fn test_estimation_recorded_large_deviation_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::EstimationRecorded {
            task_id: "t1".into(),
            estimated_mins: 30,
            actual_mins: 90,
            deviation_pct: 200.0,
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_estimation_recorded_small_deviation_is_accumulate() {
        let verdict = evaluate_salience(&DomainEvent::EstimationRecorded {
            task_id: "t1".into(),
            estimated_mins: 30,
            actual_mins: 35,
            deviation_pct: 16.7,
        });
        assert_eq!(verdict, SalienceVerdict::Accumulate);
    }

    #[test]
    fn test_task_execution_completed_is_extract() {
        let verdict = evaluate_salience(&DomainEvent::TaskExecutionCompleted {
            task_id: "t1".into(),
            execution_id: "e1".into(),
            tokens_used: 1000,
            cost_usd: Some(0.05),
            artifacts_count: 2,
        });
        assert_eq!(verdict, SalienceVerdict::Extract);
    }

    #[test]
    fn test_behavioral_pattern_detected_is_accumulate() {
        let verdict = evaluate_salience(&DomainEvent::BehavioralPatternDetected {
            pattern_type: "day_of_week".into(),
            pattern_key: "monday_task".into(),
            sample_count: 15,
            detail: "User uses task agent frequently on Mondays".into(),
        });
        assert_eq!(verdict, SalienceVerdict::Accumulate);
    }
}
