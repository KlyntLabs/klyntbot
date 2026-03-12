//! Conversion from `DomainEvent` to `Signal`.

use chrono::{DateTime, Utc};

use bus::DomainEvent;

use super::types::{Signal, SignalMetadata};

/// Convert a domain event into a signal.
pub(super) fn event_to_signal(event: &DomainEvent, timestamp: DateTime<Utc>) -> Signal {
    let (event_type, metadata) = match event {
        DomainEvent::DistractionDetected { app, .. } => (
            "DistractionDetected",
            SignalMetadata {
                app: Some(app.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::FocusSessionEnded { .. } => ("FocusSessionEnded", SignalMetadata::default()),
        DomainEvent::ProductivityScoreComputed { .. } => {
            ("ProductivityScoreComputed", SignalMetadata::default())
        }
        DomainEvent::TaskCompleted { task_id, .. } => (
            "TaskCompleted",
            SignalMetadata {
                task_id: Some(task_id.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::TaskDeferred { task_id, .. } => (
            "TaskDeferred",
            SignalMetadata {
                task_id: Some(task_id.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::BudgetAlert { category, .. } => (
            "BudgetAlert",
            SignalMetadata {
                category: Some(category.clone()),
                ..Default::default()
            },
        ),
        DomainEvent::TransactionRecorded {
            category, amount, ..
        } => (
            "TransactionRecorded",
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
            "SessionCreated",
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
            "SessionEnded",
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
            "QualityScored",
            SignalMetadata {
                category: Some(score_date.clone()),
                amount: Some(*overall_score),
                ..Default::default()
            },
        ),
        DomainEvent::PredictiveAlert {
            forecast_type,
            predicted_value,
            ..
        } => (
            "PredictiveAlert",
            SignalMetadata {
                category: Some(forecast_type.clone()),
                amount: Some(*predicted_value),
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
