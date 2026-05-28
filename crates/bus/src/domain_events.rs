//! Domain event types and broadcast bus for cross-feature communication.
//!
//! The cognitive layer subscribes to all events. Feature crates emit
//! via `DomainEventBus::publish()` without knowing about consumers.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// How the user returned — from OS sleep or from idle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeType {
    FromSleep,
    FromIdle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Done,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum ConcurrencyClass {
    Safe,
    Sequential,
    Exclusive,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BashJobEvent {
    Started {
        job_id: String,
        thread_id: String,
        agent_id: String,
        command: String,
        description: String,
        started_at: jiff::Timestamp,
    },
    Completed {
        job_id: String,
        thread_id: String,
        agent_id: String,
        exit_code: i32,
        duration_ms: u64,
    },
    Failed {
        job_id: String,
        thread_id: String,
        agent_id: String,
        exit_code: Option<i32>,
        failure_kind: String,
        failure_detail: String,
    },
    Cancelled {
        job_id: String,
        thread_id: String,
        agent_id: String,
        reason: String,
    },
    Lost {
        job_id: String,
        thread_id: String,
        agent_id: String,
    },
    AttachStarted {
        job_id: String,
        thread_id: String,
        agent_id: String,
        timestamp: jiff::Timestamp,
    },
    AttachEnded {
        job_id: String,
        thread_id: String,
        agent_id: String,
        timestamp: jiff::Timestamp,
        duration_ms: u64,
    },
}

impl BashJobEvent {
    pub fn job_id(&self) -> &str {
        match self {
            Self::Started { job_id, .. }
            | Self::Completed { job_id, .. }
            | Self::Failed { job_id, .. }
            | Self::Cancelled { job_id, .. }
            | Self::Lost { job_id, .. }
            | Self::AttachStarted { job_id, .. }
            | Self::AttachEnded { job_id, .. } => job_id,
        }
    }

    pub fn thread_id(&self) -> &str {
        match self {
            Self::Started { thread_id, .. }
            | Self::Completed { thread_id, .. }
            | Self::Failed { thread_id, .. }
            | Self::Cancelled { thread_id, .. }
            | Self::Lost { thread_id, .. }
            | Self::AttachStarted { thread_id, .. }
            | Self::AttachEnded { thread_id, .. } => thread_id,
        }
    }

    pub fn agent_id(&self) -> &str {
        match self {
            Self::Started { agent_id, .. }
            | Self::Completed { agent_id, .. }
            | Self::Failed { agent_id, .. }
            | Self::Cancelled { agent_id, .. }
            | Self::Lost { agent_id, .. }
            | Self::AttachStarted { agent_id, .. }
            | Self::AttachEnded { agent_id, .. } => agent_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TodoEvent {
    StateChanged {
        thread_id: String,
        agent_id: String,
        agent_profile: String,
        item_id: String,
        from: TodoStatus,
        to: TodoStatus,
        concurrency: ConcurrencyClass,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        timestamp: jiff::Timestamp,
    },
    Cancelled {
        thread_id: String,
        agent_id: String,
        agent_profile: String,
        item_id: String,
        prior_status: TodoStatus,
        was_blocked_by: Vec<String>,
        timestamp: jiff::Timestamp,
    },
    PlanProposed {
        thread_id: String,
        plan_session_id: String,
        item_ids: Vec<String>,
        timestamp: jiff::Timestamp,
    },
    PlanRatified {
        thread_id: String,
        plan_session_id: String,
        ratified_count: usize,
        user_edited_count: usize,
        user_removed_count: usize,
        timestamp: jiff::Timestamp,
    },
    PlanCancelled {
        thread_id: String,
        plan_session_id: String,
        timestamp: jiff::Timestamp,
    },
}

/// Notification-domain events. Carried by `DomainEvent::Notification`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NotificationEvent {
    HeldNotificationReleased {
        held_id: String,
        alarm_id: String,
        channels: Vec<String>,
    },
    NotificationDeliveryFailed {
        alarm_id: String,
        channel: String,
        error: String,
        attempts: u32,
    },
    TrayNotificationRequested {
        title: String,
        body: String,
        alarm_id: Option<String>,
    },
}

impl NotificationEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::HeldNotificationReleased { .. } => "HeldNotificationReleased",
            Self::NotificationDeliveryFailed { .. } => "NotificationDeliveryFailed",
            Self::TrayNotificationRequested { .. } => "TrayNotificationRequested",
        }
    }
}

/// Scheduler alarm events. Carried by `DomainEvent::Alarm`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AlarmEvent {
    AlarmFired {
        fire_id: String,
        #[serde(rename = "alarm_kind")]
        kind: String,
        ref_id: Option<String>,
        payload_json: String,
        fired_at_ms: i64,
    },
    AlarmSnoozed {
        fire_id: String,
        new_fire_at_ms: i64,
    },
    AlarmCancelled {
        fire_id: String,
        reason: String,
    },
    MissedAlarms {
        fire_ids: Vec<String>,
        oldest_fire_at_ms: i64,
        newest_fire_at_ms: i64,
    },
}

impl AlarmEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::AlarmFired { .. } => "AlarmFired",
            Self::AlarmSnoozed { .. } => "AlarmSnoozed",
            Self::AlarmCancelled { .. } => "AlarmCancelled",
            Self::MissedAlarms { .. } => "MissedAlarms",
        }
    }
}

/// Task-domain events. Carried by `DomainEvent::Task`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TaskEvent {
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
    TaskFocusChanged {
        task_id: String,
        focus_deadline: Option<String>,
    },
    TaskFocusExpired {
        task_id: String,
        title: String,
    },
    EstimationRecorded {
        task_id: String,
        estimated_mins: u32,
        actual_mins: u32,
        deviation_pct: f64,
    },
}

impl TaskEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::TaskCreated { .. } => "TaskCreated",
            Self::TaskCompleted { .. } => "TaskCompleted",
            Self::TaskDeferred { .. } => "TaskDeferred",
            Self::TaskFocusChanged { .. } => "TaskFocusChanged",
            Self::TaskFocusExpired { .. } => "TaskFocusExpired",
            Self::EstimationRecorded { .. } => "EstimationRecorded",
        }
    }
}

/// Note-domain events. Carried by `DomainEvent::Note`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NoteEvent {
    NoteCreated { note_id: String, title: String },
    NoteUpdated { note_id: String, title: String },
    NoteContentChanged { note_id: String },
    NoteEditingFinished { note_id: String },
    NoteDeleted { note_id: String },
}

impl NoteEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::NoteCreated { .. } => "NoteCreated",
            Self::NoteUpdated { .. } => "NoteUpdated",
            Self::NoteContentChanged { .. } => "NoteContentChanged",
            Self::NoteEditingFinished { .. } => "NoteEditingFinished",
            Self::NoteDeleted { .. } => "NoteDeleted",
        }
    }
}

/// Tool-execution events. Carried by `DomainEvent::ToolExecution`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolExecutionEvent {
    ToolCallExecuted {
        tool_name: String,
        args_preview: Option<String>,
        session_key: Option<String>,
        duration_ms: Option<i64>,
    },
    ApprovalRequested {
        request_id: String,
        tool: String,
        args_hash: String,
        layer: String,
        repo_id: Option<String>,
    },
    ApprovalResolved {
        request_id: String,
        user_id: Option<String>,
        tool_name: String,
        path: Option<String>,
        decision: String,
        pattern_used: Option<String>,
        decided_by: String,
        occurred_at: i64,
    },
}

impl ToolExecutionEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::ToolCallExecuted { .. } => "ToolCallExecuted",
            Self::ApprovalRequested { .. } => "ApprovalRequested",
            Self::ApprovalResolved { .. } => "ApprovalResolved",
        }
    }
}

/// Coaching events. Carried by `DomainEvent::Coaching`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CoachingEvent {
    CoachingFeedback {
        intervention_id: String,
        response: FeedbackResponse,
    },
    CoachingStrategyApplied {
        strategy_id: String,
        rule_text: String,
        accepted: bool,
    },
    CoachingPatternDetected {
        pattern_name: String,
        confidence: f64,
        description: String,
        domain: String,
        signal_count: i32,
        rule_text: String,
    },
    CoachingLearningDigest {
        fading_count: usize,
        archived_count: usize,
        streak_days: usize,
        strongest_topic: Option<String>,
        weakest_topic: Option<String>,
    },
}

impl CoachingEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::CoachingFeedback { .. } => "CoachingFeedback",
            Self::CoachingStrategyApplied { .. } => "CoachingStrategyApplied",
            Self::CoachingPatternDetected { .. } => "CoachingPatternDetected",
            Self::CoachingLearningDigest { .. } => "CoachingLearningDigest",
        }
    }
}

/// Cross-domain events. Carried by `DomainEvent::CrossDomain`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CrossDomainEvent {
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
}

impl CrossDomainEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::UserStatedFact { .. } => "UserStatedFact",
            Self::UserCorrectedAI { .. } => "UserCorrectedAI",
            Self::AutotunerDecision { .. } => "AutotunerDecision",
        }
    }
}

/// Productivity events. Carried by `DomainEvent::Productivity`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProductivityEvent {
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
    ProductivitySessionEnded {
        session_id: String,
        quality: f64,
        duration_mins: u32,
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
}

impl ProductivityEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::ActivitySessionCompleted { .. } => "ActivitySessionCompleted",
            Self::FocusSessionStarted { .. } => "FocusSessionStarted",
            Self::FocusSessionEnded { .. } => "FocusSessionEnded",
            Self::ProductivitySessionEnded { .. } => "ProductivitySessionEnded",
            Self::DistractionDetected { .. } => "DistractionDetected",
            Self::ProductivityScoreComputed { .. } => "ProductivityScoreComputed",
            Self::SessionCreated { .. } => "SessionCreated",
            Self::SessionEnded { .. } => "SessionEnded",
            Self::QualityScored { .. } => "QualityScored",
        }
    }
}

/// Language-learning events. Carried by `DomainEvent::LanguageLearning`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LanguageLearningEvent {
    PronunciationScored {
        session_id: String,
        overall_score: f64,
        weak_phonemes: Vec<String>,
    },
    ExamAttempted {
        exam_id: String,
        score: u32,
        passed: bool,
    },
    PhoneticMasteryGained {
        phoneme: String,
        mastery_level: f64,
    },
    LanguagePracticeSessionCompleted {
        session_id: String,
        language: String,
        duration_secs: u64,
        success_rate: f64,
    },
}

impl LanguageLearningEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::PronunciationScored { .. } => "PronunciationScored",
            Self::ExamAttempted { .. } => "ExamAttempted",
            Self::PhoneticMasteryGained { .. } => "PhoneticMasteryGained",
            Self::LanguagePracticeSessionCompleted { .. } => "LanguagePracticeSessionCompleted",
        }
    }
}

/// Lifecycle events. Carried by `DomainEvent::Lifecycle`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LifecycleEvent {
    SystemWillSleep,
    SystemDidWake {
        away_secs: u64,
        wake_type: WakeType,
    },
    UserBecameIdle {
        idle_secs: u64,
    },
    UserReturned {
        absence_secs: u64,
        wake_type: WakeType,
    },
    FocusSessionSuspended {
        remaining_secs: u64,
        phase_name: String,
    },
    CronCatchUpReady {
        immediate_count: usize,
        deferred_count: usize,
        expired_count: usize,
    },
    WakePanelReady {
        greeting: String,
        away_secs: u64,
    },
}

impl LifecycleEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::SystemWillSleep => "SystemWillSleep",
            Self::SystemDidWake { .. } => "SystemDidWake",
            Self::UserBecameIdle { .. } => "UserBecameIdle",
            Self::UserReturned { .. } => "UserReturned",
            Self::FocusSessionSuspended { .. } => "FocusSessionSuspended",
            Self::CronCatchUpReady { .. } => "CronCatchUpReady",
            Self::WakePanelReady { .. } => "WakePanelReady",
        }
    }
}

/// Community events. Carried by `DomainEvent::Community`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CommunityEvent {
    CommunityDiscovered {
        community_id: String,
        name: String,
        member_count: u32,
    },
    CommunityUpdated {
        community_id: String,
        name: String,
        reason: String,
    },
    CommunityWeakened {
        community_id: String,
        name: String,
        stability: f64,
    },
    CoActivationStrengthened {
        fact_id_a: String,
        fact_id_b: String,
        strength: f64,
    },
}

impl CommunityEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::CommunityDiscovered { .. } => "CommunityDiscovered",
            Self::CommunityUpdated { .. } => "CommunityUpdated",
            Self::CommunityWeakened { .. } => "CommunityWeakened",
            Self::CoActivationStrengthened { .. } => "CoActivationStrengthened",
        }
    }
}

/// Coding-memory events. Carried by `DomainEvent::CodingMemory`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CodingMemoryEvent {
    PatternApplied {
        pattern_id: String,
        session_id: String,
        repo: Option<String>,
        source: String,
    },
    PatternOutcome {
        pattern_id: String,
        outcome: String,
        evidence: String,
        measured_at: String,
    },
    FixAttemptFailed {
        problem_hash: String,
        repo: Option<String>,
        attempt_count: u32,
    },
    MemoryRetrieved {
        memory_ids: Vec<String>,
        query: String,
        session_id: String,
        turn_id: Option<String>,
    },
    AssistantMsgCompleted {
        session_id: String,
        turn_id: Option<String>,
        cited_memory_ids: Vec<String>,
    },
    RetrievalSkillApplied {
        skill: String,
        before_score: f32,
        after_score: f32,
        budget_used: String,
        session_id: String,
    },
}

impl CodingMemoryEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::PatternApplied { .. } => "PatternApplied",
            Self::PatternOutcome { .. } => "PatternOutcome",
            Self::FixAttemptFailed { .. } => "FixAttemptFailed",
            Self::MemoryRetrieved { .. } => "MemoryRetrieved",
            Self::AssistantMsgCompleted { .. } => "AssistantMsgCompleted",
            Self::RetrievalSkillApplied { .. } => "RetrievalSkillApplied",
        }
    }
}

/// Learning events. Carried by `DomainEvent::Learning`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LearningEvent {
    BehavioralPatternDetected {
        pattern_type: String,
        pattern_key: String,
        sample_count: i32,
        detail: String,
    },
    ContradictionDetected {
        existing_subject: String,
        existing_predicate: String,
        existing_object: String,
        new_object: String,
        confidence: f64,
    },
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
        subject: String,
        domain: String,
        reinforcement_count: i64,
    },
    KnowledgeAtomExtracted {
        atom_id: String,
        note_id: String,
        text: String,
    },
    FlashcardScheduled {
        flashcard_id: String,
        atom_id: String,
        due_at: String,
    },
    AtomRetentionDecayed {
        atom_id: String,
        retention: f64,
    },
    AtomSemanticFactLinked {
        atom_id: String,
        fact_id: String,
        similarity: f64,
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
}

impl LearningEvent {
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::BehavioralPatternDetected { .. } => "BehavioralPatternDetected",
            Self::ContradictionDetected { .. } => "ContradictionDetected",
            Self::KnowledgeAtomCreated { .. } => "KnowledgeAtomCreated",
            Self::KnowledgeAtomAccepted { .. } => "KnowledgeAtomAccepted",
            Self::KnowledgeAtomArchived { .. } => "KnowledgeAtomArchived",
            Self::AtomFlashcardReviewed { .. } => "AtomFlashcardReviewed",
            Self::AtomReinforced { .. } => "AtomReinforced",
            Self::KnowledgeAtomExtracted { .. } => "KnowledgeAtomExtracted",
            Self::FlashcardScheduled { .. } => "FlashcardScheduled",
            Self::AtomRetentionDecayed { .. } => "AtomRetentionDecayed",
            Self::AtomSemanticFactLinked { .. } => "AtomSemanticFactLinked",
            Self::AtomInteracted { .. } => "AtomInteracted",
            Self::RetentionMilestoneReached { .. } => "RetentionMilestoneReached",
            Self::TranslationCompleted { .. } => "TranslationCompleted",
            Self::NoteStudied { .. } => "NoteStudied",
            Self::PracticeUnitCompleted { .. } => "PracticeUnitCompleted",
            Self::PracticeSessionCompleted { .. } => "PracticeSessionCompleted",
            Self::FlashcardSessionCompleted { .. } => "FlashcardSessionCompleted",
            Self::KnowledgeTransferDetected { .. } => "KnowledgeTransferDetected",
        }
    }
}

/// Events emitted by feature crates for cross-domain communication.
///
/// The cognitive layer subscribes to all events to extract facts,
/// detect patterns, and drive proactive coaching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomainEvent {
    // -- Productivity --
    Productivity(ProductivityEvent),

    // -- Tasks --
    Task(TaskEvent),

    // -- Notes --
    Note(NoteEvent),

    // -- Chat --
    ChatTurnCompleted {
        session_key: String,
        /// The user's message content for cognitive extraction.
        /// `None` for legacy events or when content is unavailable.
        #[serde(default)]
        user_message: Option<String>,
    },

    // -- Tool execution --
    ToolExecution(ToolExecutionEvent),

    // -- Cross-domain --
    CrossDomain(CrossDomainEvent),

    // -- Coaching feedback --
    Coaching(CoachingEvent),

    // -- Learning --
    Learning(LearningEvent),
    LanguageLearning(LanguageLearningEvent),

    // -- Productivity interventions --
    InterventionTriggered {
        intervention_type: String,
        urgency: String,
        message: String,
        suggested_action: String,
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

    // -- Lifecycle events --
    Lifecycle(LifecycleEvent),

    /// Emitted when a cross-domain connection dot is ready for UI display.
    CrossDomainDotReady {
        source_kind: String,
        source_id: String,
        source_title: String,
        target_kind: String,
        target_id: String,
        target_title: String,
        confidence: f64,
        tooltip: String,
        detail_route: Option<String>,
    },

    // -- Community lifecycle --
    Community(CommunityEvent),

    // -- Notifications --
    Notification(NotificationEvent),

    // -- Scheduler alarms --
    Alarm(AlarmEvent),

    PluginEvent {
        plugin_id: String,
        kind: String,
        payload: serde_json::Value,
    },

    // -- Coding memory --
    CodingMemory(CodingMemoryEvent),

    /// Launcher item was executed (opened, run, activated).
    LauncherItemExecuted {
        item_id: String,
        kind: String,
        query: Option<String>,
    },

    /// SQLite `PRAGMA data_version` advanced unexpectedly — i.e. some
    /// connection outside our process pool wrote, and we never saw the
    /// matching domain event. Listeners should perform a broad invalidate.
    DataVersionBumped {
        previous: u32,
        current: u32,
    },

    /// Escape-hatch for events that don't have a dedicated variant.
    /// Used by `fan_out_event` to publish `AgentEvent` values to the
    /// cognitive-ingest bus without creating a circular crate dependency.
    Generic {
        kind: String,
        payload: serde_json::Value,
    },

    Todo(TodoEvent),
    BashJob(BashJobEvent),
}

impl DomainEvent {
    /// Return the enum variant name without payload (e.g. `"NoteContentChanged"`).
    ///
    /// Unlike `format!("{:?}", self)`, this never allocates a copy of large
    /// inner fields like note content.
    pub fn variant_name(&self) -> &'static str {
        // serde tag serialization would work but allocates; a manual match is zero-cost.
        match self {
            Self::Productivity(e) => e.variant_name(),
            Self::Task(e) => e.variant_name(),
            Self::Note(e) => e.variant_name(),
            Self::ChatTurnCompleted { .. } => "ChatTurnCompleted",
            Self::ToolExecution(e) => e.variant_name(),
            Self::CrossDomain(e) => e.variant_name(),
            Self::Coaching(e) => e.variant_name(),
            Self::Learning(e) => e.variant_name(),
            Self::LanguageLearning(e) => e.variant_name(),
            Self::InterventionTriggered { .. } => "InterventionTriggered",
            Self::SkillRouted { .. } => "SkillRouted",
            Self::Lifecycle(e) => e.variant_name(),
            Self::CrossDomainDotReady { .. } => "CrossDomainDotReady",
            Self::Community(e) => e.variant_name(),
            Self::Notification(e) => e.variant_name(),
            Self::Alarm(e) => e.variant_name(),
            Self::PluginEvent { .. } => "PluginEvent",
            Self::CodingMemory(e) => e.variant_name(),
            Self::LauncherItemExecuted { .. } => Self::KIND_LAUNCHER_ITEM_EXECUTED,
            Self::DataVersionBumped { .. } => Self::KIND_DATA_VERSION_BUMPED,
            Self::Generic { .. } => "Generic",
            Self::Todo(_) => "Todo",
            Self::BashJob(inner) => match inner {
                BashJobEvent::Started { .. } => "BashJob.Started",
                BashJobEvent::Completed { .. } => "BashJob.Completed",
                BashJobEvent::Failed { .. } => "BashJob.Failed",
                BashJobEvent::Cancelled { .. } => "BashJob.Cancelled",
                BashJobEvent::Lost { .. } => "BashJob.Lost",
                BashJobEvent::AttachStarted { .. } => "BashJob.AttachStarted",
                BashJobEvent::AttachEnded { .. } => "BashJob.AttachEnded",
            },
        }
    }

    /// `event_type` value for [`DomainEvent::UserCorrectedAI`].
    pub const KIND_USER_CORRECTED_AI: &'static str = "UserCorrectedAI";
    /// `event_type` value for [`DomainEvent::ChatTurnCompleted`].
    pub const KIND_CHAT_TURN_COMPLETED: &'static str = "ChatTurnCompleted";
    /// `event_type` value for [`DomainEvent::TaskCreated`].
    pub const KIND_TASK_CREATED: &'static str = "TaskCreated";
    /// `event_type` value for [`DomainEvent::TaskCompleted`].
    pub const KIND_TASK_COMPLETED: &'static str = "TaskCompleted";
    /// `event_type` value for [`DomainEvent::UserStatedFact`].
    pub const KIND_USER_STATED_FACT: &'static str = "UserStatedFact";
    /// `event_type` value for [`DomainEvent::CoachingFeedback`].
    pub const KIND_COACHING_FEEDBACK: &'static str = "CoachingFeedback";
    /// `event_type` value for [`DomainEvent::CoachingStrategyApplied`].
    pub const KIND_COACHING_STRATEGY_APPLIED: &'static str = "CoachingStrategyApplied";
    /// `event_type` value for [`DomainEvent::CoachingPatternDetected`].
    pub const KIND_COACHING_PATTERN_DETECTED: &'static str = "CoachingPatternDetected";
    /// `event_type` value for [`DomainEvent::BehavioralPatternDetected`].
    pub const KIND_BEHAVIORAL_PATTERN_DETECTED: &'static str = "BehavioralPatternDetected";
    /// `event_type` value for [`DomainEvent::KnowledgeAtomCreated`].
    pub const KIND_KNOWLEDGE_ATOM_CREATED: &'static str = "KnowledgeAtomCreated";
    /// `event_type` value for [`DomainEvent::KnowledgeAtomAccepted`].
    pub const KIND_KNOWLEDGE_ATOM_ACCEPTED: &'static str = "KnowledgeAtomAccepted";
    /// `event_type` value for [`DomainEvent::KnowledgeAtomArchived`].
    pub const KIND_KNOWLEDGE_ATOM_ARCHIVED: &'static str = "KnowledgeAtomArchived";
    /// `event_type` value for [`DomainEvent::AtomFlashcardReviewed`].
    pub const KIND_ATOM_FLASHCARD_REVIEWED: &'static str = "AtomFlashcardReviewed";
    /// `event_type` value for [`DomainEvent::AtomReinforced`].
    pub const KIND_ATOM_REINFORCED: &'static str = "AtomReinforced";
    /// `event_type` value for [`DomainEvent::KnowledgeAtomExtracted`].
    pub const KIND_KNOWLEDGE_ATOM_EXTRACTED: &'static str = "KnowledgeAtomExtracted";
    /// `event_type` value for [`DomainEvent::FlashcardScheduled`].
    pub const KIND_FLASHCARD_SCHEDULED: &'static str = "FlashcardScheduled";
    /// `event_type` value for [`DomainEvent::AtomRetentionDecayed`].
    pub const KIND_ATOM_RETENTION_DECAYED: &'static str = "AtomRetentionDecayed";
    /// `event_type` value for [`DomainEvent::AtomSemanticFactLinked`].
    pub const KIND_ATOM_SEMANTIC_FACT_LINKED: &'static str = "AtomSemanticFactLinked";
    /// `event_type` value for [`DomainEvent::AtomInteracted`].
    pub const KIND_ATOM_INTERACTED: &'static str = "AtomInteracted";
    /// `event_type` value for [`DomainEvent::RetentionMilestoneReached`].
    pub const KIND_RETENTION_MILESTONE_REACHED: &'static str = "RetentionMilestoneReached";
    /// `event_type` value for [`DomainEvent::TranslationCompleted`].
    pub const KIND_TRANSLATION_COMPLETED: &'static str = "TranslationCompleted";
    /// `event_type` value for [`DomainEvent::NoteCreated`].
    pub const KIND_NOTE_CREATED: &'static str = "NoteCreated";
    /// `event_type` value for [`DomainEvent::NoteUpdated`].
    pub const KIND_NOTE_UPDATED: &'static str = "NoteUpdated";
    /// `event_type` value for [`DomainEvent::DistractionDetected`].
    pub const KIND_DISTRACTION_DETECTED: &'static str = "DistractionDetected";
    /// `event_type` value for [`DomainEvent::ActivitySessionCompleted`].
    pub const KIND_ACTIVITY_SESSION_COMPLETED: &'static str = "ActivitySessionCompleted";
    /// `event_type` value for [`DomainEvent::FocusSessionStarted`].
    pub const KIND_FOCUS_SESSION_STARTED: &'static str = "FocusSessionStarted";
    /// `event_type` value for [`DomainEvent::ProductivityScoreComputed`].
    pub const KIND_PRODUCTIVITY_SCORE_COMPUTED: &'static str = "ProductivityScoreComputed";
    /// `event_type` value for [`DomainEvent::TaskDeferred`].
    pub const KIND_TASK_DEFERRED: &'static str = "TaskDeferred";
    /// `event_type` value for [`DomainEvent::NoteStudied"].
    pub const KIND_NOTE_STUDIED: &'static str = "NoteStudied";
    /// `event_type` value for [`DomainEvent::PracticeUnitCompleted`].
    pub const KIND_PRACTICE_UNIT_COMPLETED: &'static str = "PracticeUnitCompleted";
    /// `event_type` value for [`DomainEvent::PracticeSessionCompleted`].
    pub const KIND_PRACTICE_SESSION_COMPLETED: &'static str = "PracticeSessionCompleted";
    /// `event_type` value for [`DomainEvent::KnowledgeTransferDetected`].
    pub const KIND_KNOWLEDGE_TRANSFER_DETECTED: &'static str = "KnowledgeTransferDetected";
    /// `event_type` value for [`DomainEvent::CoachingLearningDigest`].
    pub const KIND_COACHING_LEARNING_DIGEST: &'static str = "CoachingLearningDigest";
    /// `event_type` value for [`DomainEvent::FlashcardSessionCompleted`].
    pub const KIND_FLASHCARD_SESSION_COMPLETED: &'static str = "FlashcardSessionCompleted";
    /// `event_type` value for [`DomainEvent::PronunciationScored`].
    pub const KIND_PRONUNCIATION_SCORED: &'static str = "PronunciationScored";
    /// `event_type` value for [`DomainEvent::ExamAttempted`].
    pub const KIND_EXAM_ATTEMPTED: &'static str = "ExamAttempted";
    /// `event_type` value for [`DomainEvent::PhoneticMasteryGained`].
    pub const KIND_PHONETIC_MASTERY_GAINED: &'static str = "PhoneticMasteryGained";
    /// `event_type` value for [`DomainEvent::LanguagePracticeSessionCompleted`].
    pub const KIND_LANGUAGE_PRACTICE_SESSION_COMPLETED: &'static str =
        "LanguagePracticeSessionCompleted";
    /// `event_type` value for [`DomainEvent::InterventionTriggered`].
    pub const KIND_INTERVENTION_TRIGGERED: &'static str = "InterventionTriggered";
    /// `event_type` value for [`DomainEvent::ContradictionDetected`].
    pub const KIND_CONTRADICTION_DETECTED: &'static str = "ContradictionDetected";
    /// `event_type` value for [`DomainEvent::SkillRouted`].
    pub const KIND_SKILL_ROUTED: &'static str = "SkillRouted";
    /// `event_type` value for [`DomainEvent::CrossDomainDotReady`].
    pub const KIND_CROSS_DOMAIN_DOT_READY: &'static str = "CrossDomainDotReady";
    /// `event_type` value for [`DomainEvent::CommunityDiscovered`].
    pub const KIND_COMMUNITY_DISCOVERED: &'static str = "CommunityDiscovered";
    /// `event_type` value for [`DomainEvent::CommunityUpdated`].
    pub const KIND_COMMUNITY_UPDATED: &'static str = "CommunityUpdated";
    /// `event_type` value for [`DomainEvent::CommunityWeakened`].
    pub const KIND_COMMUNITY_WEAKENED: &'static str = "CommunityWeakened";
    /// `event_type` value for [`DomainEvent::CoActivationStrengthened`].
    pub const KIND_CO_ACTIVATION_STRENGTHENED: &'static str = "CoActivationStrengthened";
    /// `event_type` value for [`DomainEvent::SystemWillSleep`].
    pub const KIND_SYSTEM_WILL_SLEEP: &'static str = "SystemWillSleep";
    /// `event_type` value for [`DomainEvent::SystemDidWake`].
    pub const KIND_SYSTEM_DID_WAKE: &'static str = "SystemDidWake";
    /// `event_type` value for [`DomainEvent::UserBecameIdle`].
    pub const KIND_USER_BECAME_IDLE: &'static str = "UserBecameIdle";
    /// `event_type` value for [`DomainEvent::UserReturned`].
    pub const KIND_USER_RETURNED: &'static str = "UserReturned";
    /// `event_type` value for [`DomainEvent::FocusSessionEnded`].
    pub const KIND_FOCUS_SESSION_ENDED: &'static str = "FocusSessionEnded";
    /// `event_type` value for [`DomainEvent::TaskFocusExpired`].
    pub const KIND_TASK_FOCUS_EXPIRED: &'static str = "TaskFocusExpired";
    /// `event_type` value for [`DomainEvent::ProductivitySessionEnded`].
    pub const KIND_PRODUCTIVITY_SESSION_ENDED: &'static str = "ProductivitySessionEnded";
    /// `event_type` value for [`DomainEvent::FocusSessionSuspended`].
    pub const KIND_FOCUS_SESSION_SUSPENDED: &'static str = "FocusSessionSuspended";
    /// `event_type` value for [`DomainEvent::CronCatchUpReady`].
    pub const KIND_CRON_CATCH_UP_READY: &'static str = "CronCatchUpReady";
    /// `event_type` value for [`DomainEvent::WakePanelReady`].
    pub const KIND_WAKE_PANEL_READY: &'static str = "WakePanelReady";
    /// `event_type` value for [`DomainEvent::HeldNotificationReleased`].
    pub const KIND_HELD_NOTIFICATION_RELEASED: &'static str = "HeldNotificationReleased";
    /// `event_type` value for [`DomainEvent::NotificationDeliveryFailed`].
    pub const KIND_NOTIFICATION_DELIVERY_FAILED: &'static str = "NotificationDeliveryFailed";
    /// `event_type` value for [`DomainEvent::TrayNotificationRequested`].
    pub const KIND_TRAY_NOTIFICATION_REQUESTED: &'static str = "TrayNotificationRequested";
    /// `event_type` value for [`DomainEvent::AlarmFired`].
    pub const KIND_ALARM_FIRED: &'static str = "AlarmFired";
    /// `event_type` value for [`DomainEvent::AlarmSnoozed`].
    pub const KIND_ALARM_SNOOZED: &'static str = "AlarmSnoozed";
    /// `event_type` value for [`DomainEvent::AlarmCancelled`].
    pub const KIND_ALARM_CANCELLED: &'static str = "AlarmCancelled";
    /// `event_type` value for [`DomainEvent::MissedAlarms`].
    pub const KIND_MISSED_ALARMS: &'static str = "MissedAlarms";
    /// `event_type` value for [`DomainEvent::PluginEvent`].
    pub const KIND_PLUGIN_EVENT: &'static str = "PluginEvent";
    pub const KIND_PATTERN_APPLIED: &'static str = "PatternApplied";
    pub const KIND_PATTERN_OUTCOME: &'static str = "PatternOutcome";
    pub const KIND_FIX_ATTEMPT_FAILED: &'static str = "FixAttemptFailed";
    pub const KIND_MEMORY_RETRIEVED: &'static str = "MemoryRetrieved";
    pub const KIND_ASSISTANT_MSG_COMPLETED: &'static str = "AssistantMsgCompleted";
    pub const KIND_RETRIEVAL_SKILL_APPLIED: &'static str = "RetrievalSkillApplied";
    /// `event_type` value for [`DomainEvent::LauncherItemExecuted`].
    pub const KIND_LAUNCHER_ITEM_EXECUTED: &'static str = "LauncherItemExecuted";
    /// `event_type` value for [`DomainEvent::DataVersionBumped`].
    pub const KIND_DATA_VERSION_BUMPED: &'static str = "DataVersionBumped";

    pub const KIND_BASH_JOB_STARTED: &str = "BashJob.Started";
    pub const KIND_BASH_JOB_COMPLETED: &str = "BashJob.Completed";
    pub const KIND_BASH_JOB_FAILED: &str = "BashJob.Failed";
    pub const KIND_BASH_JOB_CANCELLED: &str = "BashJob.Cancelled";
    pub const KIND_BASH_JOB_LOST: &str = "BashJob.Lost";

    /// Map this event to its domain category.
    ///
    /// Used by the cognitive pipeline, debug dashboard, and SSE streams.
    /// `UserStatedFact` returns `EventDomain::Custom(..)` carrying the
    /// user-supplied tag — every other variant resolves to a fixed enum
    /// variant known at compile time.
    pub fn domain(&self) -> crate::EventDomain {
        use crate::EventDomain as D;
        match self {
            Self::Task(_) => D::Work,

            Self::Productivity(_) => D::Energy,

            Self::CrossDomain(CrossDomainEvent::UserStatedFact { domain, .. }) => {
                D::Custom(domain.clone())
            }
            Self::CrossDomain(_) => D::Learning,
            Self::Coaching(_) => D::Coaching,
            Self::ChatTurnCompleted { .. } | Self::ToolExecution(_) => D::General,

            Self::Note(_) => D::Notes,

            Self::Learning(_) => D::Learning,

            Self::LanguageLearning(_) => D::LanguageLearning,

            Self::InterventionTriggered { .. } => D::Productivity,
            Self::PluginEvent { .. } => D::Plugin,
            Self::SkillRouted { .. } => D::Agent,
            Self::CrossDomainDotReady { .. } => D::Fabric,
            Self::Community(_) => D::Community,

            Self::Lifecycle(_) => D::Lifecycle,

            Self::Notification(_) => D::Notifications,

            Self::Alarm(_) => D::Scheduler,

            Self::CodingMemory(_) => D::Agent,

            Self::LauncherItemExecuted { .. } => D::Launcher,

            Self::DataVersionBumped { .. } => D::General,

            Self::Generic { .. } => D::General,

            Self::Todo(_) => D::Agent,
            Self::BashJob(_) => D::Agent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    pub fn publish_todo(&self, event: TodoEvent) {
        self.publish(DomainEvent::Todo(event));
    }

    pub fn publish_bash_job(&self, event: BashJobEvent) {
        self.publish(DomainEvent::BashJob(event));
    }

    pub fn publish_notification(&self, event: NotificationEvent) {
        self.publish(DomainEvent::Notification(event));
    }

    pub fn publish_alarm(&self, event: AlarmEvent) {
        self.publish(DomainEvent::Alarm(event));
    }

    pub fn publish_task(&self, event: TaskEvent) {
        self.publish(DomainEvent::Task(event));
    }
    pub fn publish_note(&self, event: NoteEvent) {
        self.publish(DomainEvent::Note(event));
    }
    pub fn publish_tool_execution(&self, event: ToolExecutionEvent) {
        self.publish(DomainEvent::ToolExecution(event));
    }
    pub fn publish_coaching(&self, event: CoachingEvent) {
        self.publish(DomainEvent::Coaching(event));
    }
    pub fn publish_cross_domain(&self, event: CrossDomainEvent) {
        self.publish(DomainEvent::CrossDomain(event));
    }
    pub fn publish_productivity(&self, event: ProductivityEvent) {
        self.publish(DomainEvent::Productivity(event));
    }
    pub fn publish_language_learning(&self, event: LanguageLearningEvent) {
        self.publish(DomainEvent::LanguageLearning(event));
    }
    pub fn publish_lifecycle(&self, event: LifecycleEvent) {
        self.publish(DomainEvent::Lifecycle(event));
    }
    pub fn publish_community(&self, event: CommunityEvent) {
        self.publish(DomainEvent::Community(event));
    }
    pub fn publish_coding_memory(&self, event: CodingMemoryEvent) {
        self.publish(DomainEvent::CodingMemory(event));
    }
    pub fn publish_learning(&self, event: LearningEvent) {
        self.publish(DomainEvent::Learning(event));
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

        bus.publish(DomainEvent::Productivity(
            ProductivityEvent::ProductivityScoreComputed {
                date: "2026-03-06".into(),
                score: 74.0,
            },
        ));

        let event = rx.recv().await.unwrap();
        assert!(
            matches!(event, DomainEvent::Productivity(ProductivityEvent::ProductivityScoreComputed { score, .. }) if score == 74.0)
        );
    }

    #[tokio::test]
    async fn test_domain_event_bus_multiple_subscribers() {
        let bus = DomainEventBus::new(16);
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(DomainEvent::Task(TaskEvent::TaskCompleted {
            task_id: "t1".into(),
            actual_duration_mins: Some(30),
            estimated_duration_mins: Some(45),
            deviation_pct: None,
        }));

        assert!(rx1.recv().await.is_ok());
        assert!(rx2.recv().await.is_ok());
    }

    #[test]
    fn test_behavioral_pattern_detected_serialization() {
        let event = DomainEvent::Learning(LearningEvent::BehavioralPatternDetected {
            pattern_type: "day_of_week".into(),
            pattern_key: "monday_task".into(),
            sample_count: 15,
            detail: "User uses task agent frequently on Mondays (15 interactions)".into(),
        });
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            DomainEvent::Learning(LearningEvent::BehavioralPatternDetected {
                sample_count: 15,
                ..
            })
        ));
    }

    #[test]
    fn test_domain_event_serialization() {
        let event = DomainEvent::CrossDomain(CrossDomainEvent::UserStatedFact {
            fact: "I prefer morning work".into(),
            domain: "productivity".into(),
        });
        let json = serde_json::to_string(&event).unwrap();
        let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            deserialized,
            DomainEvent::CrossDomain(CrossDomainEvent::UserStatedFact { .. })
        ));
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
        let event = DomainEvent::CrossDomain(CrossDomainEvent::UserCorrectedAI {
            original: "test".into(),
            correction: "fixed".into(),
            kind: CorrectionKind::Reaction,
            strength: 1.0,
            session_key: "desktop:main".into(),
            active_skill: Some("general".into()),
        });
        let json = serde_json::to_string(&event).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            DomainEvent::CrossDomain(CrossDomainEvent::UserCorrectedAI {
                kind,
                strength,
                session_key,
                active_skill,
                ..
            }) => {
                assert_eq!(kind, CorrectionKind::Reaction);
                assert!((strength - 1.0).abs() < f64::EPSILON);
                assert_eq!(session_key, "desktop:main");
                assert_eq!(active_skill, Some("general".to_string()));
            }
            _ => panic!("Expected UserCorrectedAI"),
        }
    }

    #[test]
    fn test_note_editing_finished_event() {
        let bus = DomainEventBus::new(32);
        let mut rx = bus.subscribe();
        bus.publish(DomainEvent::Note(NoteEvent::NoteEditingFinished {
            note_id: "note-1".to_string(),
        }));
        let event = rx.try_recv().unwrap();
        assert!(
            matches!(event, DomainEvent::Note(NoteEvent::NoteEditingFinished { note_id, .. }) if note_id == "note-1")
        );
    }

    #[test]
    fn alarm_fired_round_trips_json() {
        let event = DomainEvent::Alarm(AlarmEvent::AlarmFired {
            fire_id: "fire_abc".into(),
            kind: "task_alarm".into(),
            ref_id: Some("task_1".into()),
            payload_json: "{\"msg\":\"hi\"}".into(),
            fired_at_ms: 1_800_000_000_000,
        });
        let json = serde_json::to_string(&event).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            DomainEvent::Alarm(AlarmEvent::AlarmFired { fire_id, .. }) => {
                assert_eq!(fire_id, "fire_abc")
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn missed_alarms_round_trips() {
        let event = DomainEvent::Alarm(AlarmEvent::MissedAlarms {
            fire_ids: vec!["a".into(), "b".into(), "c".into()],
            oldest_fire_at_ms: 1_000,
            newest_fire_at_ms: 2_000,
        });
        let json = serde_json::to_string(&event).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            DomainEvent::Alarm(AlarmEvent::MissedAlarms {
                fire_ids,
                oldest_fire_at_ms,
                newest_fire_at_ms,
            }) => {
                assert_eq!(fire_ids.len(), 3);
                assert_eq!(oldest_fire_at_ms, 1_000);
                assert_eq!(newest_fire_at_ms, 2_000);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn held_notification_released_event_round_trips() {
        use super::{DomainEvent, NotificationEvent};
        let e = DomainEvent::Notification(NotificationEvent::HeldNotificationReleased {
            held_id: "h1".into(),
            alarm_id: "fire_1".into(),
            channels: vec!["telegram".into()],
        });
        let s = serde_json::to_string(&e).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            parsed,
            DomainEvent::Notification(NotificationEvent::HeldNotificationReleased { .. })
        ));
    }

    #[test]
    fn notification_delivery_failed_event_round_trips() {
        use super::{DomainEvent, NotificationEvent};
        let e = DomainEvent::Notification(NotificationEvent::NotificationDeliveryFailed {
            alarm_id: "fire_1".into(),
            channel: "discord".into(),
            error: "500".into(),
            attempts: 3,
        });
        let s = serde_json::to_string(&e).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            parsed,
            DomainEvent::Notification(NotificationEvent::NotificationDeliveryFailed { .. })
        ));
    }

    #[test]
    fn tray_notification_requested_event_round_trips() {
        use super::{DomainEvent, NotificationEvent};
        let e = DomainEvent::Notification(NotificationEvent::TrayNotificationRequested {
            title: "ping".into(),
            body: "hello".into(),
            alarm_id: Some("fire_1".into()),
        });
        let s = serde_json::to_string(&e).unwrap();
        assert!(matches!(
            serde_json::from_str::<DomainEvent>(&s).unwrap(),
            DomainEvent::Notification(NotificationEvent::TrayNotificationRequested { .. })
        ));
    }

    #[test]
    fn pattern_applied_roundtrips() {
        let e = DomainEvent::CodingMemory(CodingMemoryEvent::PatternApplied {
            pattern_id: "fp-1".into(),
            session_id: "s-1".into(),
            repo: Some("github.com/klynt/bot".into()),
            source: "recall_injection".into(),
        });
        let json = serde_json::to_string(&e).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed,
            DomainEvent::CodingMemory(CodingMemoryEvent::PatternApplied { .. })
        ));
    }

    #[test]
    fn retrieval_skill_applied_roundtrips() {
        let e = DomainEvent::CodingMemory(CodingMemoryEvent::RetrievalSkillApplied {
            skill: "query_rewriter".into(),
            before_score: 0.1,
            after_score: 0.7,
            budget_used: "deep_think".into(),
            session_id: "s-1".into(),
        });
        let json = serde_json::to_string(&e).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed,
            DomainEvent::CodingMemory(CodingMemoryEvent::RetrievalSkillApplied { .. })
        ));
    }
    #[test]
    fn state_changed_roundtrip() {
        let e = TodoEvent::StateChanged {
            thread_id: "t1".into(),
            agent_id: "root".into(),
            agent_profile: "root".into(),
            item_id: "i1".into(),
            from: TodoStatus::Pending,
            to: TodoStatus::InProgress,
            concurrency: ConcurrencyClass::Sequential,
            reason: None,
            timestamp: jiff::Timestamp::from_second(1_780_000_000).unwrap(),
        };
        let s = serde_json::to_string(&e).unwrap();
        let back: TodoEvent = serde_json::from_str(&s).unwrap();
        assert_eq!(e, back);
    }

    #[tokio::test]
    async fn publish_todo_round_trip() {
        let bus = DomainEventBus::new(64);
        let mut rx = bus.subscribe();
        let evt = TodoEvent::StateChanged {
            thread_id: "t1".into(),
            agent_id: "root".into(),
            agent_profile: "root".into(),
            item_id: "i1".into(),
            from: TodoStatus::Pending,
            to: TodoStatus::Done,
            concurrency: ConcurrencyClass::Safe,
            reason: None,
            timestamp: jiff::Timestamp::from_second(1_780_000_000).unwrap(),
        };
        bus.publish_todo(evt.clone());
        let received = rx.recv().await.unwrap();
        match received {
            DomainEvent::Todo(e) => assert_eq!(e, evt),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn bash_job_started_variant_name() {
        let evt = DomainEvent::BashJob(BashJobEvent::Started {
            job_id: "bash-aB3kF7c2qR".into(),
            thread_id: "session-1".into(),
            agent_id: "root".into(),
            command: "cargo test".into(),
            description: "run tests".into(),
            started_at: jiff::Timestamp::now(),
        });
        assert_eq!(evt.variant_name(), "BashJob.Started");
        assert_eq!(evt.domain(), crate::EventDomain::Agent);
    }

    #[allow(dead_code)]
    fn autotuner_decision_roundtrip() {
        let event = DomainEvent::CrossDomain(CrossDomainEvent::AutotunerDecision {
            trial_id: "abc-123".into(),
            verdict: "promoted".into(),
            improvement_pct: 12.5,
            affected_params: vec!["heuristic_confidence_threshold".into()],
        });
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("promoted"));
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            DomainEvent::CrossDomain(CrossDomainEvent::AutotunerDecision { verdict, .. }) => {
                assert_eq!(verdict, "promoted");
            }
            _ => panic!("Expected AutotunerDecision"),
        }
    }
}

#[cfg(test)]
mod bash_job_event_accessor_tests {
    use super::*;
    use jiff::Timestamp;

    #[test]
    fn accessors_return_inner_fields() {
        let started = BashJobEvent::Started {
            job_id: "bash-x".into(),
            thread_id: "t1".into(),
            agent_id: "a1".into(),
            command: "c".into(),
            description: "d".into(),
            started_at: Timestamp::now(),
        };
        assert_eq!(started.job_id(), "bash-x");
        assert_eq!(started.thread_id(), "t1");
        assert_eq!(started.agent_id(), "a1");

        let lost = BashJobEvent::Lost {
            job_id: "bash-y".into(),
            thread_id: "t2".into(),
            agent_id: "a2".into(),
        };
        assert_eq!(lost.job_id(), "bash-y");
        assert_eq!(lost.thread_id(), "t2");
        assert_eq!(lost.agent_id(), "a2");
    }
}

#[cfg(test)]
mod string_stability;
