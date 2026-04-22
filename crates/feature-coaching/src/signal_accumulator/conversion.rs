//! Conversion from `DomainEvent` to `Signal`.

use jiff::Timestamp;

use bus::DomainEvent;

use super::types::{Signal, SignalMetadata};

/// Convert a domain event into a signal.
pub(super) fn event_to_signal(event: &DomainEvent, timestamp: Timestamp) -> Signal {
    let (event_type, metadata) = match event {
        DomainEvent::DistractionDetected { app, .. } => (
            bus::DomainEvent::KIND_DISTRACTION_DETECTED,
            SignalMetadata {
                app: Some(app.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::FocusSessionEnded { .. } => (bus::DomainEvent::KIND_FOCUS_SESSION_ENDED, SignalMetadata::default()),
        DomainEvent::ProductivityScoreComputed { .. } => {
            (bus::DomainEvent::KIND_PRODUCTIVITY_SCORE_COMPUTED, SignalMetadata::default())
        }
        DomainEvent::TaskCompleted { task_id, .. } => (
            bus::DomainEvent::KIND_TASK_COMPLETED,
            SignalMetadata {
                task_id: Some(task_id.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::TaskDeferred { task_id, .. } => (
            bus::DomainEvent::KIND_TASK_DEFERRED,
            SignalMetadata {
                task_id: Some(task_id.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::BudgetAlert { category, .. } => (
            bus::DomainEvent::KIND_BUDGET_ALERT,
            SignalMetadata {
                category: Some(category.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::TransactionRecorded {
            category, amount, ..
        } => (
            bus::DomainEvent::KIND_TRANSACTION_RECORDED,
            SignalMetadata {
                category: Some(category.clone()),
                amount: Some(*amount),
                ..Default::default()
            },
        ),
        // -- Intelligence layer events --
        DomainEvent::SessionCreated {
            session_type,
            dominant_category,
            ..
        } => (
            bus::DomainEvent::KIND_SESSION_CREATED,
            SignalMetadata {
                category: Some(format!("{session_type}:{dominant_category}")),
                ..Default::default()
            },
        ),
        DomainEvent::SessionEnded {
            duration_secs,
            quality_score,
            session_type,
            ..
        } => (
            bus::DomainEvent::KIND_SESSION_ENDED,
            SignalMetadata {
                category: Some(session_type.clone()),
                amount: quality_score.or(Some(*duration_secs as f64)),
                ..Default::default()
            },
        ),
        DomainEvent::QualityScored {
            overall_score,
            score_date,
            ..
        } => (
            bus::DomainEvent::KIND_QUALITY_SCORED,
            SignalMetadata {
                category: Some(score_date.clone()),
                amount: Some(*overall_score),
                ..Default::default()
            },
        ),

        DomainEvent::AtomFlashcardReviewed { quality, .. } => (
            bus::DomainEvent::KIND_ATOM_FLASHCARD_REVIEWED,
            SignalMetadata {
                amount: Some(*quality as f64),
                ..Default::default()
            },
        ),
        DomainEvent::KnowledgeAtomCreated { domain, .. } => (
            bus::DomainEvent::KIND_KNOWLEDGE_ATOM_CREATED,
            SignalMetadata {
                category: Some(domain.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::KnowledgeAtomArchived { reason, .. } => (
            bus::DomainEvent::KIND_KNOWLEDGE_ATOM_ARCHIVED,
            SignalMetadata {
                category: Some(reason.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::RetentionMilestoneReached {
            new_retention_pct, ..
        } => (
            bus::DomainEvent::KIND_RETENTION_MILESTONE_REACHED,
            SignalMetadata {
                amount: Some(*new_retention_pct),
                ..Default::default()
            },
        ),
        DomainEvent::KnowledgeTransferDetected {
            from_domain,
            to_domain,
            confidence,
            ..
        } => (
            bus::DomainEvent::KIND_KNOWLEDGE_TRANSFER_DETECTED,
            SignalMetadata {
                category: Some(format!("{from_domain}->{to_domain}")),
                amount: Some(*confidence),
                ..Default::default()
            },
        ),
        DomainEvent::CoachingLearningDigest { fading_count, .. } => (
            bus::DomainEvent::KIND_COACHING_LEARNING_DIGEST,
            SignalMetadata {
                amount: Some(*fading_count as f64),
                ..Default::default()
            },
        ),
        _ => ("Other", SignalMetadata::default()),
    };

    Signal {
        event_type: event_type.to_string(),
        timestamp,
        metadata,
    }
}
