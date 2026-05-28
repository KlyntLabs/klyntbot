//! Frozen-string anchor for the DomainEvent nesting migration.
//! These strings feed DB `event_type` queries and MUST NOT change.
use super::*;

#[test]
fn kind_constants_are_frozen() {
    assert_eq!(DomainEvent::KIND_USER_CORRECTED_AI, "UserCorrectedAI");
    assert_eq!(DomainEvent::KIND_CHAT_TURN_COMPLETED, "ChatTurnCompleted");
    assert_eq!(DomainEvent::KIND_TASK_CREATED, "TaskCreated");
    assert_eq!(DomainEvent::KIND_TASK_COMPLETED, "TaskCompleted");
    assert_eq!(DomainEvent::KIND_USER_STATED_FACT, "UserStatedFact");
    assert_eq!(DomainEvent::KIND_COACHING_FEEDBACK, "CoachingFeedback");
    assert_eq!(
        DomainEvent::KIND_COACHING_STRATEGY_APPLIED,
        "CoachingStrategyApplied"
    );
    assert_eq!(
        DomainEvent::KIND_COACHING_PATTERN_DETECTED,
        "CoachingPatternDetected"
    );
    assert_eq!(
        DomainEvent::KIND_BEHAVIORAL_PATTERN_DETECTED,
        "BehavioralPatternDetected"
    );
    assert_eq!(
        DomainEvent::KIND_KNOWLEDGE_ATOM_CREATED,
        "KnowledgeAtomCreated"
    );
    assert_eq!(
        DomainEvent::KIND_KNOWLEDGE_ATOM_ACCEPTED,
        "KnowledgeAtomAccepted"
    );
    assert_eq!(
        DomainEvent::KIND_KNOWLEDGE_ATOM_ARCHIVED,
        "KnowledgeAtomArchived"
    );
    assert_eq!(
        DomainEvent::KIND_ATOM_FLASHCARD_REVIEWED,
        "AtomFlashcardReviewed"
    );
    assert_eq!(DomainEvent::KIND_ATOM_REINFORCED, "AtomReinforced");
    assert_eq!(
        DomainEvent::KIND_KNOWLEDGE_ATOM_EXTRACTED,
        "KnowledgeAtomExtracted"
    );
    assert_eq!(DomainEvent::KIND_FLASHCARD_SCHEDULED, "FlashcardScheduled");
    assert_eq!(
        DomainEvent::KIND_ATOM_RETENTION_DECAYED,
        "AtomRetentionDecayed"
    );
    assert_eq!(
        DomainEvent::KIND_ATOM_SEMANTIC_FACT_LINKED,
        "AtomSemanticFactLinked"
    );
    assert_eq!(DomainEvent::KIND_ATOM_INTERACTED, "AtomInteracted");
    assert_eq!(
        DomainEvent::KIND_RETENTION_MILESTONE_REACHED,
        "RetentionMilestoneReached"
    );
    assert_eq!(
        DomainEvent::KIND_TRANSLATION_COMPLETED,
        "TranslationCompleted"
    );
    assert_eq!(DomainEvent::KIND_NOTE_CREATED, "NoteCreated");
    assert_eq!(DomainEvent::KIND_NOTE_UPDATED, "NoteUpdated");
    assert_eq!(
        DomainEvent::KIND_DISTRACTION_DETECTED,
        "DistractionDetected"
    );
    assert_eq!(
        DomainEvent::KIND_ACTIVITY_SESSION_COMPLETED,
        "ActivitySessionCompleted"
    );
    assert_eq!(
        DomainEvent::KIND_FOCUS_SESSION_STARTED,
        "FocusSessionStarted"
    );
    assert_eq!(
        DomainEvent::KIND_PRODUCTIVITY_SCORE_COMPUTED,
        "ProductivityScoreComputed"
    );
    assert_eq!(DomainEvent::KIND_TASK_DEFERRED, "TaskDeferred");
    assert_eq!(DomainEvent::KIND_NOTE_STUDIED, "NoteStudied");
    assert_eq!(
        DomainEvent::KIND_PRACTICE_UNIT_COMPLETED,
        "PracticeUnitCompleted"
    );
    assert_eq!(
        DomainEvent::KIND_PRACTICE_SESSION_COMPLETED,
        "PracticeSessionCompleted"
    );
    assert_eq!(
        DomainEvent::KIND_KNOWLEDGE_TRANSFER_DETECTED,
        "KnowledgeTransferDetected"
    );
    assert_eq!(
        DomainEvent::KIND_COACHING_LEARNING_DIGEST,
        "CoachingLearningDigest"
    );
    assert_eq!(
        DomainEvent::KIND_FLASHCARD_SESSION_COMPLETED,
        "FlashcardSessionCompleted"
    );
    assert_eq!(
        DomainEvent::KIND_PRONUNCIATION_SCORED,
        "PronunciationScored"
    );
    assert_eq!(DomainEvent::KIND_EXAM_ATTEMPTED, "ExamAttempted");
    assert_eq!(
        DomainEvent::KIND_PHONETIC_MASTERY_GAINED,
        "PhoneticMasteryGained"
    );
    assert_eq!(
        DomainEvent::KIND_LANGUAGE_PRACTICE_SESSION_COMPLETED,
        "LanguagePracticeSessionCompleted"
    );
    assert_eq!(
        DomainEvent::KIND_INTERVENTION_TRIGGERED,
        "InterventionTriggered"
    );
    assert_eq!(
        DomainEvent::KIND_CONTRADICTION_DETECTED,
        "ContradictionDetected"
    );
    assert_eq!(DomainEvent::KIND_SKILL_ROUTED, "SkillRouted");
    assert_eq!(
        DomainEvent::KIND_CROSS_DOMAIN_DOT_READY,
        "CrossDomainDotReady"
    );
    assert_eq!(
        DomainEvent::KIND_COMMUNITY_DISCOVERED,
        "CommunityDiscovered"
    );
    assert_eq!(DomainEvent::KIND_COMMUNITY_UPDATED, "CommunityUpdated");
    assert_eq!(DomainEvent::KIND_COMMUNITY_WEAKENED, "CommunityWeakened");
    assert_eq!(
        DomainEvent::KIND_CO_ACTIVATION_STRENGTHENED,
        "CoActivationStrengthened"
    );
    assert_eq!(DomainEvent::KIND_SYSTEM_WILL_SLEEP, "SystemWillSleep");
    assert_eq!(DomainEvent::KIND_SYSTEM_DID_WAKE, "SystemDidWake");
    assert_eq!(DomainEvent::KIND_USER_BECAME_IDLE, "UserBecameIdle");
    assert_eq!(DomainEvent::KIND_USER_RETURNED, "UserReturned");
    assert_eq!(DomainEvent::KIND_FOCUS_SESSION_ENDED, "FocusSessionEnded");
    assert_eq!(DomainEvent::KIND_TASK_FOCUS_EXPIRED, "TaskFocusExpired");
    assert_eq!(
        DomainEvent::KIND_PRODUCTIVITY_SESSION_ENDED,
        "ProductivitySessionEnded"
    );
    assert_eq!(
        DomainEvent::KIND_FOCUS_SESSION_SUSPENDED,
        "FocusSessionSuspended"
    );
    assert_eq!(DomainEvent::KIND_CRON_CATCH_UP_READY, "CronCatchUpReady");
    assert_eq!(DomainEvent::KIND_WAKE_PANEL_READY, "WakePanelReady");
    assert_eq!(
        DomainEvent::KIND_HELD_NOTIFICATION_RELEASED,
        "HeldNotificationReleased"
    );
    assert_eq!(
        DomainEvent::KIND_NOTIFICATION_DELIVERY_FAILED,
        "NotificationDeliveryFailed"
    );
    assert_eq!(
        DomainEvent::KIND_TRAY_NOTIFICATION_REQUESTED,
        "TrayNotificationRequested"
    );
    assert_eq!(DomainEvent::KIND_ALARM_FIRED, "AlarmFired");
    assert_eq!(DomainEvent::KIND_ALARM_SNOOZED, "AlarmSnoozed");
    assert_eq!(DomainEvent::KIND_ALARM_CANCELLED, "AlarmCancelled");
    assert_eq!(DomainEvent::KIND_MISSED_ALARMS, "MissedAlarms");
    assert_eq!(DomainEvent::KIND_PLUGIN_EVENT, "PluginEvent");
    assert_eq!(DomainEvent::KIND_PATTERN_APPLIED, "PatternApplied");
    assert_eq!(DomainEvent::KIND_PATTERN_OUTCOME, "PatternOutcome");
    assert_eq!(DomainEvent::KIND_FIX_ATTEMPT_FAILED, "FixAttemptFailed");
    assert_eq!(DomainEvent::KIND_MEMORY_RETRIEVED, "MemoryRetrieved");
    assert_eq!(
        DomainEvent::KIND_ASSISTANT_MSG_COMPLETED,
        "AssistantMsgCompleted"
    );
    assert_eq!(
        DomainEvent::KIND_RETRIEVAL_SKILL_APPLIED,
        "RetrievalSkillApplied"
    );
    assert_eq!(
        DomainEvent::KIND_LAUNCHER_ITEM_EXECUTED,
        "LauncherItemExecuted"
    );
    assert_eq!(DomainEvent::KIND_DATA_VERSION_BUMPED, "DataVersionBumped");
    assert_eq!(DomainEvent::KIND_BASH_JOB_STARTED, "BashJob.Started");
    assert_eq!(DomainEvent::KIND_BASH_JOB_COMPLETED, "BashJob.Completed");
    assert_eq!(DomainEvent::KIND_BASH_JOB_FAILED, "BashJob.Failed");
    assert_eq!(DomainEvent::KIND_BASH_JOB_CANCELLED, "BashJob.Cancelled");
    assert_eq!(DomainEvent::KIND_BASH_JOB_LOST, "BashJob.Lost");
}

#[test]
fn variant_name_and_domain_are_frozen_for_samples() {
    // Notification group representative
    let n = DomainEvent::Notification(NotificationEvent::TrayNotificationRequested {
        title: "t".into(),
        body: "b".into(),
        alarm_id: None,
    });
    assert_eq!(n.variant_name(), "TrayNotificationRequested");
    assert_eq!(n.domain(), crate::EventDomain::Notifications);

    // Alarm group representative
    let a = DomainEvent::Alarm(AlarmEvent::AlarmFired {
        fire_id: "f".into(),
        kind: "cron_job".into(),
        ref_id: None,
        payload_json: "{}".into(),
        fired_at_ms: 0,
    });
    assert_eq!(a.variant_name(), "AlarmFired");
    assert_eq!(a.domain(), crate::EventDomain::Scheduler);

    // Task group representative
    let t = DomainEvent::Task(TaskEvent::TaskCreated {
        task_id: "t1".into(),
        project: None,
        estimate_mins: None,
        task_type: "test".into(),
    });
    assert_eq!(t.variant_name(), "TaskCreated");
    assert_eq!(t.domain(), crate::EventDomain::Work);

    // Note group representative
    let note = DomainEvent::Note(NoteEvent::NoteCreated {
        note_id: "n1".into(),
        title: "title".into(),
    });
    assert_eq!(note.variant_name(), "NoteCreated");
    assert_eq!(note.domain(), crate::EventDomain::Notes);

    // Productivity group representative
    let p = DomainEvent::Productivity(ProductivityEvent::DistractionDetected {
        app: "app".into(),
        duration_secs: None,
        context: "ctx".into(),
    });
    assert_eq!(p.variant_name(), "DistractionDetected");
    assert_eq!(p.domain(), crate::EventDomain::Energy);

    // Learning group representative
    let l = DomainEvent::Learning(LearningEvent::AtomReinforced {
        atom_id: "a1".into(),
        referencing_note_id: "n1".into(),
        new_salience: 0.5,
        subject: "subj".into(),
        domain: "dom".into(),
        reinforcement_count: 1,
    });
    assert_eq!(l.variant_name(), "AtomReinforced");
    assert_eq!(l.domain(), crate::EventDomain::Learning);

    // Singleton representative
    let c = DomainEvent::ChatTurnCompleted {
        session_key: "sk".into(),
        user_message: None,
    };
    assert_eq!(c.variant_name(), "ChatTurnCompleted");
    assert_eq!(c.domain(), crate::EventDomain::General);

    // BashJob nested representative
    let b = DomainEvent::BashJob(BashJobEvent::Started {
        job_id: "j1".into(),
        thread_id: "t1".into(),
        agent_id: "a1".into(),
        command: "cmd".into(),
        description: "desc".into(),
        started_at: jiff::Timestamp::now(),
    });
    assert_eq!(b.variant_name(), "BashJob.Started");
    assert_eq!(b.domain(), crate::EventDomain::Agent);
}
