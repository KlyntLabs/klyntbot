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
    FocusSessionStarted {
        session_type: String,
        target_mins: i64,
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

    // -- Productivity Intelligence Layer --
    SessionCreated {
        session_id: String,
        session_type: String,
        dominant_category: String,
        predicted_energy: Option<f64>,
    },
    SessionEnded {
        session_id: String,
        session_type: String,
        duration_secs: i64,
        quality_score: Option<f64>,
        category_purity: f64,
    },
    QualityScored {
        score_date: String,
        session_id: Option<String>,
        overall_score: f64,
        components: String,
    },
    PredictiveAlert {
        forecast_type: String,
        window_start: String,
        window_end: String,
        predicted_value: f64,
        suggested_action: Option<String>,
    },
    NarrativeGenerated {
        date: String,
        sentiment: String,
        excerpt: String,
    },
    RuleEvolved {
        rule_id: String,
        action: String,
        category: String,
        confidence: f64,
        source: String,
    },
    VoiceJournalProcessed {
        journal_id: String,
        extracted_fact_count: usize,
        sentiment: Option<String>,
    },

    // -- Tasks --
    TaskCreated {
        task_id: String,
        project: Option<String>,
        estimate_mins: Option<i64>,
        task_type: String,
    },
    TaskCompleted {
        task_id: String,
        actual_duration_mins: Option<i64>,
        estimated_duration_mins: Option<i64>,
        deviation_pct: Option<f64>,
    },
    TaskDeferred {
        task_id: String,
        times_deferred: i32,
    },

    // -- Tasks (agentic) --
    TaskDecomposed {
        source_task_id: String,
        subtask_ids: Vec<String>,
        total_estimated_mins: Option<i64>,
    },
    TaskExecutionStarted {
        task_id: String,
        execution_id: String,
        agent_profile: String,
    },
    TaskExecutionCompleted {
        task_id: String,
        execution_id: String,
        tokens_used: u64,
        cost_usd: Option<f64>,
        artifacts_count: u32,
    },
    TaskExecutionFailed {
        task_id: String,
        execution_id: String,
        error: String,
        retry_count: u32,
    },
    TaskBlocked {
        task_id: String,
        blocker_id: String,
    },
    TaskUnblocked {
        task_id: String,
        was_blocked_by: String,
    },
    TaskStatusChanged {
        task_id: String,
        from: String,
        to: String,
        actor: Option<String>,
    },
    TaskPriorityChanged {
        task_id: String,
        from: String,
        to: String,
        actor: Option<String>,
    },
    TaskFieldUpdated {
        task_id: String,
        field: String,
        from: String,
        to: String,
        actor: Option<String>,
    },
    DayPlanGenerated {
        task_count: u32,
        total_estimated_mins: u32,
    },
    ProactiveSuggestionCreated {
        suggestion_id: String,
        suggestion_type: String,
        task_id: Option<String>,
        confidence: f64,
    },
    TaskFocusStarted {
        task_id: String,
        energy_level: String,
    },
    TaskFocusEnded {
        task_id: String,
        duration_secs: u64,
    },
    EstimationRecorded {
        task_id: String,
        estimated_mins: u32,
        actual_mins: u32,
        deviation_pct: f64,
    },
    TaskExecutionProgress {
        task_id: String,
        execution_id: String,
        current_step: String,
        percentage: Option<u8>,
        latest_tool: Option<String>,
        reasoning_snippet: Option<String>,
        cost_so_far_usd: f64,
        elapsed_secs: u64,
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

    // -- Notes --
    NoteCreated {
        note_id: String,
        title: String,
    },
    NoteUpdated {
        note_id: String,
        title: String,
    },

    // -- Chat --
    ChatTurnCompleted {
        user_message: String,
        session_key: String,
    },

    // -- Tool execution --
    ToolCallExecuted {
        tool_name: String,
        args_preview: Option<String>,
        session_key: Option<String>,
        duration_ms: Option<i64>,
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

    // -- Behavioral learning --
    BehavioralPatternDetected {
        pattern_type: String,
        pattern_key: String,
        sample_count: i32,
        detail: String,
    },

    // -- Contradiction detection (Phase 3 prep) --
    ContradictionDetected {
        existing_subject: String,
        existing_predicate: String,
        existing_object: String,
        new_object: String,
        confidence: f64,
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

    /// Number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
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
            deviation_pct: None,
        });

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[test]
    fn test_behavioral_pattern_detected_serialization() {
        let event = DomainEvent::BehavioralPatternDetected {
            pattern_type: "day_of_week".into(),
            pattern_key: "monday_task".into(),
            sample_count: 15,
            detail: "User uses task agent frequently on Mondays (15 interactions)".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            DomainEvent::BehavioralPatternDetected {
                sample_count: 15,
                ..
            }
        ));
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
