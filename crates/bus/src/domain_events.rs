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
    NoteContentChanged {
        note_id: String,
        content: String,
    },
    NoteDeleted {
        note_id: String,
    },

    // -- Task hierarchy (BookIndex) --
    TaskHierarchyChanged {
        project_id: String,
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
        kind: CorrectionKind,
        strength: f64,
        session_key: String,
        active_skill: Option<String>,
    },
    AutotunerDecision {
        trial_id: String,
        verdict: String,
        improvement_pct: f64,
        affected_params: Vec<String>,
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

    // -- Knowledge Atoms --
    KnowledgeAtomCreated {
        atom_id: String,
        atom_type: String,
        domain: String,
        source_note_id: Option<String>,
        personal_importance: f64,
    },
    KnowledgeAtomAccepted {
        atom_id: String,
        atom_type: String,
    },
    KnowledgeAtomArchived {
        atom_id: String,
        reason: String,
    },
    AtomFlashcardReviewed {
        atom_id: String,
        card_id: String,
        quality: u8,
        recall_speed_ms: u64,
        new_retention_pct: f64,
        source_note_id: Option<String>,
    },
    AtomReinforced {
        atom_id: String,
        referencing_note_id: String,
        new_salience: f64,
    },
    AtomInteracted {
        atom_id: String,
        interaction_type: String,
        note_id: Option<String>,
    },
    RetentionMilestoneReached {
        atom_id: String,
        topic_id: Option<String>,
        new_retention_pct: f64,
        milestone: String,
        previous_pct: f64,
    },
    TranslationCompleted {
        note_id: String,
        source_lang: String,
        target_lang: String,
        word_count: usize,
        is_selection: bool,
    },
    NoteStudied {
        note_id: String,
        duration_secs: u64,
        atoms_reviewed: usize,
        mode: String,
    },
    PracticeUnitCompleted {
        session_id: String,
        note_id: String,
        unit_index: u32,
        grade: String,
        scores: String,
        confidence_rating: u8,
        edited: bool,
    },
    PracticeSessionCompleted {
        session_id: String,
        note_id: String,
        units_completed: u32,
        average_score: f64,
        source_lang: String,
        target_lang: String,
        weak_unit_count: u32,
    },
    FlashcardSessionCompleted {
        session_id: String,
        cards_reviewed: usize,
        avg_score: f64,
        weak_domains: Vec<String>,
        propagation_count: usize,
    },
    KnowledgeTransferDetected {
        atom_id: String,
        from_domain: String,
        to_domain: String,
        confidence: f64,
    },
    CoachingLearningDigest {
        fading_count: usize,
        archived_count: usize,
        streak_days: usize,
        strongest_topic: Option<String>,
        weakest_topic: Option<String>,
    },

    // -- Productivity interventions --
    InterventionTriggered {
        intervention_type: String,
        urgency: String,
        message: String,
        suggested_action: String,
    },

    // -- Contradiction detection (Phase 3 prep) --
    ContradictionDetected {
        existing_subject: String,
        existing_predicate: String,
        existing_object: String,
        new_object: String,
        confidence: f64,
    },

    /// A memory write is below the confidence threshold and needs user confirmation.
    MemoryPendingConfirmation {
        fact_id: String,
        subject: String,
        predicate: String,
        object: String,
    },

    // -- Agent routing --
    /// Emitted when AgentRuntime selects an orchestrator skill for a message.
    SkillRouted {
        skill_name: String,
        confidence: f64,
        source: String,
        trigger_phrases: Vec<String>,
        session_key: String,
    },

    // -- Autotuner trials --
    /// Emitted when the autotuner creates a new trial for evaluation.
    TrialActivated {
        trial_id: String,
        hypothesis: String,
        params_summary: String,
    },

    // -- Mirror self-reflection --
    /// Emitted when user kills an experiment trial via the Mirror UI.
    MirrorTrialKilled {
        trial_id: String,
    },
    /// Emitted when the Mirror layer creates a new NarrativeSnippet for the user.
    MirrorSnippetCreated {
        snippet_id: String,
        headline: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedbackResponse {
    Helpful,
    Dismissed,
    StopSuggesting,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionKind {
    Reaction,
    KeywordPrefix,
    /// User indicated the AI forgot something they previously mentioned.
    MemoryMiss,
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

    #[test]
    fn correction_kind_roundtrip() {
        let kind = CorrectionKind::Reaction;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"reaction\"");
        let parsed: CorrectionKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, CorrectionKind::Reaction);

        let kind2 = CorrectionKind::KeywordPrefix;
        let json2 = serde_json::to_string(&kind2).unwrap();
        assert_eq!(json2, "\"keyword_prefix\"");

        let kind3 = CorrectionKind::MemoryMiss;
        let json3 = serde_json::to_string(&kind3).unwrap();
        assert_eq!(json3, "\"memory_miss\"");
    }

    #[test]
    fn user_corrected_ai_with_kind_roundtrip() {
        let event = DomainEvent::UserCorrectedAI {
            original: "test".into(),
            correction: "fixed".into(),
            kind: CorrectionKind::Reaction,
            strength: 1.0,
            session_key: "desktop:main".into(),
            active_skill: Some("general".into()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            DomainEvent::UserCorrectedAI {
                kind,
                strength,
                session_key,
                active_skill,
                ..
            } => {
                assert_eq!(kind, CorrectionKind::Reaction);
                assert!((strength - 1.0).abs() < f64::EPSILON);
                assert_eq!(session_key, "desktop:main");
                assert_eq!(active_skill, Some("general".to_string()));
            }
            _ => panic!("Expected UserCorrectedAI"),
        }
    }

    #[test]
    fn autotuner_decision_roundtrip() {
        let event = DomainEvent::AutotunerDecision {
            trial_id: "abc-123".into(),
            verdict: "promoted".into(),
            improvement_pct: 12.5,
            affected_params: vec!["heuristic_confidence_threshold".into()],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("promoted"));
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            DomainEvent::AutotunerDecision { verdict, .. } => {
                assert_eq!(verdict, "promoted");
            }
            _ => panic!("Expected AutotunerDecision"),
        }
    }
}
