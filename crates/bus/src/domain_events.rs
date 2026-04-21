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
    VoiceCapture {
        session_id: String,
        language: String,
        overall_confidence: f32,
        duration_secs: f32,
        engine: String,
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

    /// Emitted when a task's due date is set or changed. Consumed by feature_tasks::focus_alarms.
    TaskDueDateChanged {
        task_id: String,
        /// None means the due date was cleared.
        due_date: Option<String>,
    },

    /// Emitted when a task is focused/unfocused with a deadline. Consumed by feature_tasks::focus_alarms.
    TaskFocusChanged {
        task_id: String,
        /// None means unfocused.
        focus_deadline: Option<String>,
    },

    /// Emitted when a recurring template's next_instance_date changes.
    RecurringTemplateAdvanced {
        template_id: String,
        next_instance_date: Option<String>,
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
    },
    NoteEditingFinished {
        note_id: String,
    },
    NoteDeleted {
        note_id: String,
    },

    // -- Task hierarchy (BookIndex) --
    TaskHierarchyChanged {
        project_id: String,
    },

    /// Emitted after tree nodes have been rebuilt and embedded for a source.
    TreeNodesRebuilt {
        source_type: String,
        source_id: String,
    },

    // -- Chat --
    ChatTurnCompleted {
        session_key: String,
        /// The user's message content for cognitive extraction.
        /// `None` for legacy events or when content is unavailable.
        #[serde(default)]
        user_message: Option<String>,
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
    CoachingPatternDetected {
        pattern_name: String,
        confidence: f64,
        description: String,
        domain: String,
        signal_count: i32,
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
        subject: String,
        domain: String,
        reinforcement_count: i64,
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

    // -- Lifecycle events --
    /// macOS is about to sleep.
    SystemWillSleep,
    /// macOS woke from sleep.
    SystemDidWake {
        away_secs: u64,
        wake_type: WakeType,
    },
    /// User became idle (no input for threshold duration).
    UserBecameIdle {
        idle_secs: u64,
    },
    /// User returned after being idle or after system sleep.
    UserReturned {
        absence_secs: u64,
        wake_type: WakeType,
    },
    // -- Wake orchestrator ready signals --
    /// Focus timer was suspended due to sleep/idle.
    FocusSessionSuspended {
        remaining_secs: u64,
        phase_name: String,
    },
    /// Cron service classified missed jobs for catch-up.
    CronCatchUpReady {
        immediate_count: usize,
        deferred_count: usize,
        expired_count: usize,
    },
    /// Wake panel assembled and ready for UI display.
    WakePanelReady {
        greeting: String,
        away_secs: u64,
    },

    // -- Knowledge Fabric communities --
    /// Emitted when a new community is detected in the note graph.
    CommunityDiscovered {
        community_id: String,
        name: String,
        member_count: u32,
    },
    /// Emitted when an existing community's membership or properties change.
    CommunityUpdated {
        community_id: String,
        member_count: u32,
        stability: f64,
    },
    /// Emitted when a community's cohesion weakens below a threshold.
    CommunityWeakened {
        community_id: String,
        stability: f64,
    },

    // -- Squad debates --
    /// Emitted when a squad debate (multi-persona deliberation) completes.
    SquadDebateCompleted {
        squad_id: String,
        session_key: String,
        rounds_completed: u8,
        consensus_score: f64,
        persona_accuracies: Vec<(String, f64)>,
        was_partial: bool,
        token_cost: u64,
        average_consensus_score: f64,
        top_performer_persona_id: Option<String>,
    },
    /// Emitted when a squad interaction pattern is detected or updated.
    SquadInteractionPattern {
        squad_id: String,
        mode: String,
        persona_id: Option<String>,
        domain_hint: Option<String>,
    },

    // ── Brain ambient signals ──────────────────────────────────
    /// Emitted when a memory fact is promoted to a wider scope (e.g. session → long-term).
    MemoryPromoted {
        fact_id: String,
        summary: String,
        from_scope: String,
        to_scope: String,
    },
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
    /// Emitted when an incoming message is deferred rather than processed immediately.
    MessageDeferred {
        channel: String,
        sender: String,
        preview: String,
    },

    // -- Notifications --
    /// Emitted when a held notification is released (e.g. quiet hours ended).
    HeldNotificationReleased {
        held_id: String,
        alarm_id: String,
        channels: Vec<String>,
    },

    /// Emitted after all retry attempts for a notification delivery have been exhausted.
    NotificationDeliveryFailed {
        alarm_id: String,
        channel: String,
        error: String,
        attempts: u32,
    },

    /// Emitted when a tray (system/menu-bar) notification should be shown.
    TrayNotificationRequested {
        title: String,
        body: String,
        alarm_id: Option<String>,
    },

    // -- Scheduler alarms --
    /// A scheduled fire has matured. `kind` identifies which subsystem owns the fire;
    /// `ref_id` is that subsystem's identifier. Conventions:
    /// - `kind = "task_alarm"` → `ref_id` is the task id
    /// - `kind = "cron_job"` → `ref_id` is the cron_jobs.id
    /// - `kind = "standalone_alarm"` → `ref_id` is the alarm id
    /// - `kind = "held_release"` → `ref_id` is the held_notifications.id
    ///
    /// `payload_json` is the raw `scheduled_fires.payload` string; subscribers
    /// that need structured data should parse it. This keeps the bus event
    /// decoupled from the storage schema.
    AlarmFired {
        fire_id: String,
        kind: String,
        ref_id: Option<String>,
        payload_json: String,
        fired_at_ms: i64,
    },
    /// Emitted when a fired alarm is snoozed to a later time.
    AlarmSnoozed {
        fire_id: String,
        new_fire_at_ms: i64,
    },
    /// Emitted when a scheduled alarm is cancelled.
    AlarmCancelled {
        fire_id: String,
        reason: String,
    },
    /// Emitted when alarms were missed (e.g. while app was offline).
    MissedAlarms {
        fire_ids: Vec<String>,
        oldest_fire_at_ms: i64,
        newest_fire_at_ms: i64,
    },
}

impl DomainEvent {
    /// Return the enum variant name without payload (e.g. `"NoteContentChanged"`).
    ///
    /// Unlike `format!("{:?}", self)`, this never allocates a copy of large
    /// inner fields like note content.
    pub fn variant_name(&self) -> &'static str {
        // serde tag serialization would work but allocates; a manual match is zero-cost.
        match self {
            Self::ActivitySessionCompleted { .. } => "ActivitySessionCompleted",
            Self::FocusSessionStarted { .. } => "FocusSessionStarted",
            Self::FocusSessionEnded { .. } => "FocusSessionEnded",
            Self::DistractionDetected { .. } => "DistractionDetected",
            Self::ProductivityScoreComputed { .. } => "ProductivityScoreComputed",
            Self::SessionCreated { .. } => "SessionCreated",
            Self::SessionEnded { .. } => "SessionEnded",
            Self::QualityScored { .. } => "QualityScored",
            Self::PredictiveAlert { .. } => "PredictiveAlert",
            Self::NarrativeGenerated { .. } => "NarrativeGenerated",
            Self::RuleEvolved { .. } => "RuleEvolved",
            Self::VoiceJournalProcessed { .. } => "VoiceJournalProcessed",
            Self::VoiceCapture { .. } => "VoiceCapture",
            Self::TaskCreated { .. } => "TaskCreated",
            Self::TaskCompleted { .. } => "TaskCompleted",
            Self::TaskDeferred { .. } => "TaskDeferred",
            Self::TaskDecomposed { .. } => "TaskDecomposed",
            Self::TaskExecutionStarted { .. } => "TaskExecutionStarted",
            Self::TaskBlocked { .. } => "TaskBlocked",
            Self::TaskUnblocked { .. } => "TaskUnblocked",
            Self::TaskStatusChanged { .. } => "TaskStatusChanged",
            Self::TaskPriorityChanged { .. } => "TaskPriorityChanged",
            Self::TaskFieldUpdated { .. } => "TaskFieldUpdated",
            Self::TaskDueDateChanged { .. } => "TaskDueDateChanged",
            Self::TaskFocusChanged { .. } => "TaskFocusChanged",
            Self::RecurringTemplateAdvanced { .. } => "RecurringTemplateAdvanced",
            Self::TaskFocusStarted { .. } => "TaskFocusStarted",
            Self::TaskFocusEnded { .. } => "TaskFocusEnded",
            Self::EstimationRecorded { .. } => "EstimationRecorded",
            Self::GoalProgress { .. } => "GoalProgress",
            Self::TransactionRecorded { .. } => "TransactionRecorded",
            Self::BudgetAlert { .. } => "BudgetAlert",
            Self::NoteCreated { .. } => "NoteCreated",
            Self::NoteUpdated { .. } => "NoteUpdated",
            Self::NoteContentChanged { .. } => "NoteContentChanged",
            Self::NoteEditingFinished { .. } => "NoteEditingFinished",
            Self::NoteDeleted { .. } => "NoteDeleted",
            Self::TaskHierarchyChanged { .. } => "TaskHierarchyChanged",
            Self::TreeNodesRebuilt { .. } => "TreeNodesRebuilt",
            Self::ChatTurnCompleted { .. } => "ChatTurnCompleted",
            Self::ToolCallExecuted { .. } => "ToolCallExecuted",
            Self::UserStatedFact { .. } => "UserStatedFact",
            Self::UserCorrectedAI { .. } => "UserCorrectedAI",
            Self::AutotunerDecision { .. } => "AutotunerDecision",
            Self::CoachingFeedback { .. } => "CoachingFeedback",
            Self::CoachingPatternDetected { .. } => "CoachingPatternDetected",
            Self::BehavioralPatternDetected { .. } => "BehavioralPatternDetected",
            Self::KnowledgeAtomCreated { .. } => "KnowledgeAtomCreated",
            Self::KnowledgeAtomAccepted { .. } => "KnowledgeAtomAccepted",
            Self::KnowledgeAtomArchived { .. } => "KnowledgeAtomArchived",
            Self::AtomFlashcardReviewed { .. } => "AtomFlashcardReviewed",
            Self::AtomReinforced { .. } => "AtomReinforced",
            Self::AtomInteracted { .. } => "AtomInteracted",
            Self::RetentionMilestoneReached { .. } => "RetentionMilestoneReached",
            Self::TranslationCompleted { .. } => "TranslationCompleted",
            Self::NoteStudied { .. } => "NoteStudied",
            Self::PracticeUnitCompleted { .. } => "PracticeUnitCompleted",
            Self::PracticeSessionCompleted { .. } => "PracticeSessionCompleted",
            Self::KnowledgeTransferDetected { .. } => "KnowledgeTransferDetected",
            Self::CoachingLearningDigest { .. } => "CoachingLearningDigest",
            Self::FlashcardSessionCompleted { .. } => "FlashcardSessionCompleted",
            Self::InterventionTriggered { .. } => "InterventionTriggered",
            Self::MemoryPendingConfirmation { .. } => "MemoryPendingConfirmation",
            Self::ContradictionDetected { .. } => "ContradictionDetected",
            Self::SkillRouted { .. } => "SkillRouted",
            Self::TrialActivated { .. } => "TrialActivated",
            Self::MirrorTrialKilled { .. } => "MirrorTrialKilled",
            Self::MirrorSnippetCreated { .. } => "MirrorSnippetCreated",
            Self::CommunityDiscovered { .. } => "CommunityDiscovered",
            Self::CommunityUpdated { .. } => "CommunityUpdated",
            Self::CommunityWeakened { .. } => "CommunityWeakened",
            Self::SquadDebateCompleted { .. } => "SquadDebateCompleted",
            Self::SquadInteractionPattern { .. } => "SquadInteractionPattern",
            Self::MemoryPromoted { .. } => "MemoryPromoted",
            Self::CrossDomainDotReady { .. } => "CrossDomainDotReady",
            Self::MessageDeferred { .. } => "MessageDeferred",
            Self::SystemWillSleep => "SystemWillSleep",
            Self::SystemDidWake { .. } => "SystemDidWake",
            Self::UserBecameIdle { .. } => "UserBecameIdle",
            Self::UserReturned { .. } => "UserReturned",
            Self::FocusSessionSuspended { .. } => "FocusSessionSuspended",
            Self::CronCatchUpReady { .. } => "CronCatchUpReady",
            Self::WakePanelReady { .. } => "WakePanelReady",
            Self::HeldNotificationReleased { .. } => "HeldNotificationReleased",
            Self::NotificationDeliveryFailed { .. } => "NotificationDeliveryFailed",
            Self::TrayNotificationRequested { .. } => "TrayNotificationRequested",
            Self::AlarmFired { .. } => "AlarmFired",
            Self::AlarmSnoozed { .. } => "AlarmSnoozed",
            Self::AlarmCancelled { .. } => "AlarmCancelled",
            Self::MissedAlarms { .. } => "MissedAlarms",
        }
    }

    /// Map this event to its domain category string.
    ///
    /// Used by the cognitive pipeline, debug dashboard, and SSE streams.
    pub fn domain(&self) -> &str {
        match self {
            Self::TaskCreated { .. }
            | Self::TaskCompleted { .. }
            | Self::TaskDeferred { .. }
            | Self::GoalProgress { .. }
            | Self::TaskDecomposed { .. }
            | Self::TaskExecutionStarted { .. }
            | Self::TaskBlocked { .. }
            | Self::TaskUnblocked { .. }
            | Self::TaskFocusStarted { .. }
            | Self::TaskFocusEnded { .. }
            | Self::EstimationRecorded { .. }
            | Self::TaskStatusChanged { .. }
            | Self::TaskPriorityChanged { .. }
            | Self::TaskFieldUpdated { .. }
            | Self::TaskDueDateChanged { .. }
            | Self::TaskFocusChanged { .. }
            | Self::RecurringTemplateAdvanced { .. }
            | Self::TaskHierarchyChanged { .. }
            | Self::TreeNodesRebuilt { .. } => "work",

            Self::ActivitySessionCompleted { .. }
            | Self::FocusSessionStarted { .. }
            | Self::FocusSessionEnded { .. }
            | Self::DistractionDetected { .. }
            | Self::ProductivityScoreComputed { .. }
            | Self::SessionCreated { .. }
            | Self::SessionEnded { .. }
            | Self::QualityScored { .. }
            | Self::PredictiveAlert { .. }
            | Self::NarrativeGenerated { .. }
            | Self::RuleEvolved { .. }
            | Self::VoiceJournalProcessed { .. }
            | Self::VoiceCapture { .. } => "energy",

            Self::TransactionRecorded { .. } | Self::BudgetAlert { .. } => "finance",

            Self::UserStatedFact { domain, .. } => domain.as_str(),
            Self::UserCorrectedAI { .. } => "learning",
            Self::CoachingFeedback { .. } | Self::CoachingPatternDetected { .. } => "coaching",
            Self::ChatTurnCompleted { .. } | Self::ToolCallExecuted { .. } => "general",

            Self::NoteCreated { .. }
            | Self::NoteUpdated { .. }
            | Self::NoteContentChanged { .. }
            | Self::NoteDeleted { .. }
            | Self::NoteEditingFinished { .. } => "notes",

            Self::BehavioralPatternDetected { .. }
            | Self::ContradictionDetected { .. }
            | Self::AutotunerDecision { .. }
            | Self::KnowledgeAtomCreated { .. }
            | Self::KnowledgeAtomAccepted { .. }
            | Self::KnowledgeAtomArchived { .. }
            | Self::AtomFlashcardReviewed { .. }
            | Self::AtomReinforced { .. }
            | Self::AtomInteracted { .. }
            | Self::RetentionMilestoneReached { .. }
            | Self::TranslationCompleted { .. }
            | Self::NoteStudied { .. }
            | Self::PracticeUnitCompleted { .. }
            | Self::PracticeSessionCompleted { .. }
            | Self::KnowledgeTransferDetected { .. }
            | Self::CoachingLearningDigest { .. }
            | Self::FlashcardSessionCompleted { .. } => "learning",

            Self::InterventionTriggered { .. } => "productivity",
            Self::MemoryPendingConfirmation { .. } => "memory",
            Self::SkillRouted { .. } => "agent",
            Self::TrialActivated { .. } => "autotuner",
            Self::MirrorTrialKilled { .. } | Self::MirrorSnippetCreated { .. } => "mirror",

            Self::CommunityDiscovered { .. }
            | Self::CommunityUpdated { .. }
            | Self::CommunityWeakened { .. } => "fabric",

            Self::SquadDebateCompleted { .. } | Self::SquadInteractionPattern { .. } => "agent",

            Self::MemoryPromoted { .. } => "memory",
            Self::CrossDomainDotReady { .. } => "fabric",
            Self::MessageDeferred { .. } => "general",

            Self::SystemWillSleep
            | Self::SystemDidWake { .. }
            | Self::UserBecameIdle { .. }
            | Self::UserReturned { .. }
            | Self::FocusSessionSuspended { .. }
            | Self::CronCatchUpReady { .. }
            | Self::WakePanelReady { .. } => "lifecycle",

            Self::HeldNotificationReleased { .. }
            | Self::NotificationDeliveryFailed { .. }
            | Self::TrayNotificationRequested { .. } => "notifications",

            Self::AlarmFired { .. }
            | Self::AlarmSnoozed { .. }
            | Self::AlarmCancelled { .. }
            | Self::MissedAlarms { .. } => "scheduler",
        }
    }
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
    fn test_note_editing_finished_event() {
        let bus = DomainEventBus::new(32);
        let mut rx = bus.subscribe();
        bus.publish(DomainEvent::NoteEditingFinished {
            note_id: "note-1".to_string(),
        });
        let event = rx.try_recv().unwrap();
        assert!(
            matches!(event, DomainEvent::NoteEditingFinished { note_id, .. } if note_id == "note-1")
        );
    }

    #[test]
    fn alarm_fired_round_trips_json() {
        let event = DomainEvent::AlarmFired {
            fire_id: "fire_abc".into(),
            kind: "task_alarm".into(),
            ref_id: Some("task_1".into()),
            payload_json: "{\"msg\":\"hi\"}".into(),
            fired_at_ms: 1_800_000_000_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            DomainEvent::AlarmFired { fire_id, .. } => assert_eq!(fire_id, "fire_abc"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn missed_alarms_round_trips() {
        let event = DomainEvent::MissedAlarms {
            fire_ids: vec!["a".into(), "b".into(), "c".into()],
            oldest_fire_at_ms: 1_000,
            newest_fire_at_ms: 2_000,
        };
        let json = serde_json::to_string(&event).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
        match parsed {
            DomainEvent::MissedAlarms {
                fire_ids,
                oldest_fire_at_ms,
                newest_fire_at_ms,
            } => {
                assert_eq!(fire_ids.len(), 3);
                assert_eq!(oldest_fire_at_ms, 1_000);
                assert_eq!(newest_fire_at_ms, 2_000);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn held_notification_released_event_round_trips() {
        use super::DomainEvent;
        let e = DomainEvent::HeldNotificationReleased {
            held_id: "h1".into(),
            alarm_id: "fire_1".into(),
            channels: vec!["telegram".into()],
        };
        let s = serde_json::to_string(&e).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            parsed,
            DomainEvent::HeldNotificationReleased { .. }
        ));
    }

    #[test]
    fn notification_delivery_failed_event_round_trips() {
        use super::DomainEvent;
        let e = DomainEvent::NotificationDeliveryFailed {
            alarm_id: "fire_1".into(),
            channel: "discord".into(),
            error: "500".into(),
            attempts: 3,
        };
        let s = serde_json::to_string(&e).unwrap();
        let parsed: DomainEvent = serde_json::from_str(&s).unwrap();
        assert!(matches!(
            parsed,
            DomainEvent::NotificationDeliveryFailed { .. }
        ));
    }

    #[test]
    fn tray_notification_requested_event_round_trips() {
        use super::DomainEvent;
        let e = DomainEvent::TrayNotificationRequested {
            title: "ping".into(),
            body: "hello".into(),
            alarm_id: Some("fire_1".into()),
        };
        let s = serde_json::to_string(&e).unwrap();
        assert!(matches!(
            serde_json::from_str::<DomainEvent>(&s).unwrap(),
            DomainEvent::TrayNotificationRequested { .. }
        ));
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
