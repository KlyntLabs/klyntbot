//! Domain event types and broadcast bus for cross-feature communication.
//!
//! The cognitive layer subscribes to all events. Feature crates emit
//! via `DomainEventBus::publish()` without knowing about consumers.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Events emitted by feature crates for cross-domain communication.
///
/// The cognitive layer subscribes to all events to extract facts,
/// detect patterns, and drive proactive coaching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    // -- Productivity --
    ActivitySessionCompleted {
        date: String,
        total_active_secs: i64,
        productive_secs: i64,
        distracting_secs: i64,
    },
    FocusSessionEnded {
        duration_secs: i64,
        quality: f64,
        interruptions: i32,
    },
    DistractionDetected {
        app: String,
        duration_secs: Option<i64>,
        context: String,
    },
    ProductivityScoreComputed {
        date: String,
        score: f64,
    },

    // -- Tasks --
    TaskCreated {
        task_id: String,
        project: Option<String>,
        estimate_mins: Option<i64>,
    },
    TaskCompleted {
        task_id: String,
        actual_duration_mins: Option<i64>,
        estimated_duration_mins: Option<i64>,
    },
    TaskDeferred {
        task_id: String,
        times_deferred: i32,
    },
    GoalProgress {
        objective_id: String,
        progress: f64,
        target: f64,
    },

    // -- Finance --
    TransactionRecorded {
        category: String,
        amount: f64,
        is_over_budget: bool,
    },
    BudgetAlert {
        category: String,
        spent: f64,
        limit: f64,
    },

    // -- Cross-domain --
    UserStatedFact {
        fact: String,
        domain: String,
    },
    UserCorrectedAI {
        original: String,
        correction: String,
    },

    // -- Coaching feedback --
    CoachingFeedback {
        intervention_id: String,
        response: FeedbackResponse,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackResponse {
    Helpful,
    Dismissed,
    StopSuggesting,
}

/// Broadcast bus for DomainEvents.
///
/// The inner `broadcast::Sender` is reference-counted, so wrapping in
/// `Arc<DomainEventBus>` is the intended sharing pattern.
pub struct DomainEventBus {
    tx: broadcast::Sender<DomainEvent>,
}

impl DomainEventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    pub fn publish(&self, event: DomainEvent) {
        if let Err(e) = self.tx.send(event) {
            tracing::warn!("DomainEventBus: no receivers for event: {e}");
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_domain_event_bus_publish_subscribe() {
        let bus = DomainEventBus::new(16);
        let mut rx = bus.subscribe();

        bus.publish(DomainEvent::ProductivityScoreComputed {
            date: "2026-03-06".into(),
            score: 74.0,
        });

        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, DomainEvent::ProductivityScoreComputed { score, .. } if score == 74.0)
        );
    }

    #[tokio::test]
    async fn test_domain_event_bus_multiple_subscribers() {
        let bus = DomainEventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(DomainEvent::TaskCompleted {
            task_id: "t1".into(),
            actual_duration_mins: Some(30),
            estimated_duration_mins: Some(45),
        });

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[test]
    fn test_domain_event_serialization() {
        let event = DomainEvent::UserStatedFact {
            fact: "I prefer morning work".into(),
            domain: "productivity".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, DomainEvent::UserStatedFact { .. }));
    }
}
