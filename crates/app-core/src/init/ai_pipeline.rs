use ai_core::{
    AiEventMeta, AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer, SignalRouter,
};
use bus::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use std::sync::Arc;

/// Translator: DomainEvent -> Option<AiSignal>. Returns None when the event
/// has no pipeline registration (e.g. transient infra events). Domain is
/// set by the feature's `#[derive(AiEvent)]` enum-level `#[ai(domain = ...)]`.
pub fn translate(event: &DomainEvent) -> Option<AiSignal> {
    if let Some(e) = try_into_task_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Tasks;
        return Some(sig);
    }
    if let Some(e) = try_into_finance_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Finance;
        return Some(sig);
    }
    if let Some(e) = try_into_coaching_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Coaching;
        return Some(sig);
    }
    if let Some(e) = try_into_productivity_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Productivity;
        return Some(sig);
    }
    if let Some(e) = try_into_note_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Notes;
        return Some(sig);
    }
    if let Some(e) = try_into_learning_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::Learning;
        return Some(sig);
    }
    if let Some(e) = try_into_community_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::General;
        return Some(sig);
    }
    if let Some(e) = try_into_co_activation_event(event) {
        let mut sig = e.to_signal();
        sig.domain = RecallDomain::General;
        return Some(sig);
    }
    translate_system_event(event)
}

fn try_into_coaching_event(e: &DomainEvent) -> Option<feature_coaching::events::CoachingEvent> {
    use feature_coaching::events::CoachingEvent;
    match e {
        DomainEvent::CoachingStrategyApplied {
            strategy_id,
            rule_text,
            accepted,
        } => Some(CoachingEvent::StrategyApplied {
            strategy_id: strategy_id.clone(),
            rule_text: rule_text.clone(),
            accepted: *accepted,
        }),
        _ => None,
    }
}

fn try_into_productivity_event(
    e: &DomainEvent,
) -> Option<feature_productivity::events::ProductivityEvent> {
    use feature_productivity::events::ProductivityEvent;
    match e {
        DomainEvent::ProductivitySessionEnded {
            session_id,
            quality,
            duration_mins,
        } => Some(ProductivityEvent::SessionEnded {
            session_id: session_id.clone(),
            quality: *quality,
            duration_mins: *duration_mins,
        }),
        DomainEvent::FocusSessionStarted {
            session_type,
            target_mins,
        } => Some(ProductivityEvent::FocusSessionStarted {
            session_type: session_type.clone(),
            target_mins: *target_mins,
        }),
        DomainEvent::FocusSessionEnded {
            duration_secs,
            quality,
            interruptions,
        } => Some(ProductivityEvent::FocusSessionEnded {
            duration_secs: *duration_secs,
            quality: *quality,
            interruptions: *interruptions,
        }),
        DomainEvent::DistractionDetected {
            app,
            duration_secs,
            context,
        } => Some(ProductivityEvent::DistractionDetected {
            app: app.clone(),
            duration_secs: *duration_secs,
            context: context.clone(),
        }),
        DomainEvent::ActivitySessionCompleted {
            date,
            total_active_secs,
            productive_secs,
            distracting_secs,
        } => Some(ProductivityEvent::ActivitySessionCompleted {
            date: date.clone(),
            total_active_secs: *total_active_secs,
            productive_secs: *productive_secs,
            distracting_secs: *distracting_secs,
        }),
        DomainEvent::ProductivityScoreComputed { date, score } => {
            Some(ProductivityEvent::ProductivityScoreComputed {
                date: date.clone(),
                score: *score,
            })
        }
        _ => None,
    }
}

fn try_into_note_event(e: &DomainEvent) -> Option<feature_notes::events::NoteEvent> {
    use feature_notes::events::NoteEvent;
    match e {
        DomainEvent::NoteCreated { note_id, title } => Some(NoteEvent::Created {
            note_id: note_id.clone(),
            title: title.clone(),
        }),
        DomainEvent::NoteUpdated { note_id, title } => Some(NoteEvent::Updated {
            note_id: note_id.clone(),
            title: title.clone(),
        }),
        DomainEvent::NoteContentChanged { note_id } => Some(NoteEvent::ContentChanged {
            note_id: note_id.clone(),
        }),
        DomainEvent::NoteEditingFinished { note_id } => Some(NoteEvent::EditingFinished {
            note_id: note_id.clone(),
        }),
        DomainEvent::NoteDeleted { note_id } => Some(NoteEvent::Deleted {
            note_id: note_id.clone(),
        }),
        DomainEvent::NoteStudied {
            note_id,
            duration_secs,
            atoms_reviewed,
            mode,
        } => Some(NoteEvent::Studied {
            note_id: note_id.clone(),
            duration_secs: *duration_secs,
            atoms_reviewed: *atoms_reviewed,
            mode: mode.clone(),
        }),
        DomainEvent::PracticeUnitCompleted {
            session_id,
            note_id,
            unit_index,
            grade,
            scores,
            confidence_rating,
            edited,
        } => Some(NoteEvent::PracticeUnitCompleted {
            session_id: session_id.clone(),
            note_id: note_id.clone(),
            unit_index: *unit_index,
            grade: grade.clone(),
            scores: scores.clone(),
            confidence_rating: *confidence_rating,
            edited: *edited,
        }),
        DomainEvent::PracticeSessionCompleted {
            session_id,
            note_id,
            units_completed,
            average_score,
            source_lang,
            target_lang,
            weak_unit_count,
        } => Some(NoteEvent::PracticeSessionCompleted {
            session_id: session_id.clone(),
            note_id: note_id.clone(),
            units_completed: *units_completed,
            average_score: *average_score,
            source_lang: source_lang.clone(),
            target_lang: target_lang.clone(),
            weak_unit_count: *weak_unit_count,
        }),
        DomainEvent::TranslationCompleted {
            note_id,
            source_lang,
            target_lang,
            word_count,
            is_selection,
        } => Some(NoteEvent::TranslationCompleted {
            note_id: note_id.clone(),
            source_lang: source_lang.clone(),
            target_lang: target_lang.clone(),
            word_count: *word_count,
            is_selection: *is_selection,
        }),
        _ => None,
    }
}

fn try_into_learning_event(e: &DomainEvent) -> Option<feature_learning::LearningEvent> {
    use feature_learning::LearningEvent;
    match e {
        DomainEvent::KnowledgeAtomExtracted {
            atom_id,
            note_id,
            text,
        } => Some(LearningEvent::AtomExtracted {
            atom_id: atom_id.clone(),
            note_id: note_id.clone(),
            text: text.clone(),
        }),
        DomainEvent::KnowledgeAtomCreated { atom_id, .. } => {
            Some(LearningEvent::AtomCreated {
                atom_id: atom_id.clone(),
            })
        }
        DomainEvent::KnowledgeAtomAccepted { atom_id, .. } => {
            Some(LearningEvent::AtomAccepted {
                atom_id: atom_id.clone(),
            })
        }
        DomainEvent::KnowledgeAtomArchived { atom_id, .. } => {
            Some(LearningEvent::AtomArchived {
                atom_id: atom_id.clone(),
            })
        }
        DomainEvent::AtomFlashcardReviewed {
            atom_id,
            card_id,
            quality,
            recall_speed_ms,
            new_retention_pct,
            source_note_id,
        } => Some(LearningEvent::FlashcardReviewed {
            atom_id: atom_id.clone(),
            card_id: card_id.clone(),
            quality: *quality,
            recall_speed_ms: *recall_speed_ms,
            new_retention_pct: *new_retention_pct,
            source_note_id: source_note_id.clone(),
        }),
        DomainEvent::AtomReinforced { atom_id, .. } => {
            Some(LearningEvent::AtomReinforced {
                atom_id: atom_id.clone(),
            })
        }
        DomainEvent::FlashcardScheduled {
            flashcard_id,
            atom_id,
            due_at,
        } => Some(LearningEvent::FlashcardScheduled {
            card_id: flashcard_id.clone(),
            atom_id: atom_id.clone(),
            due_at: due_at.clone(),
        }),
        DomainEvent::AtomRetentionDecayed { atom_id, retention } => {
            Some(LearningEvent::RetentionDecayed {
                atom_id: atom_id.clone(),
                retention: *retention,
            })
        }
        DomainEvent::AtomSemanticFactLinked {
            atom_id,
            fact_id,
            similarity,
        } => Some(LearningEvent::SemanticFactLinked {
            atom_id: atom_id.clone(),
            fact_id: fact_id.clone(),
            similarity: *similarity,
        }),
        _ => None,
    }
}

fn try_into_community_event(
    e: &DomainEvent,
) -> Option<cognitive::services::community_intelligence::events::CommunityEvent> {
    use cognitive::services::community_intelligence::events::CommunityEvent;
    match e {
        DomainEvent::CommunityDiscovered {
            community_id,
            name,
            member_count,
        } => Some(CommunityEvent::Discovered {
            community_id: community_id.clone(),
            name: name.clone(),
            member_count: *member_count,
        }),
        DomainEvent::CommunityUpdated {
            community_id,
            name,
            reason,
        } => Some(CommunityEvent::Updated {
            community_id: community_id.clone(),
            name: name.clone(),
            reason: reason.clone(),
        }),
        DomainEvent::CommunityWeakened {
            community_id,
            name,
            stability,
        } => Some(CommunityEvent::Weakened {
            community_id: community_id.clone(),
            name: name.clone(),
            stability: *stability,
        }),
        _ => None,
    }
}

fn try_into_co_activation_event(
    e: &DomainEvent,
) -> Option<cognitive::services::community_intelligence::co_activation_events::CoActivationEvent> {
    use cognitive::services::community_intelligence::co_activation_events::CoActivationEvent;
    match e {
        DomainEvent::CoActivationStrengthened {
            fact_id_a,
            fact_id_b,
            strength,
        } => Some(CoActivationEvent::Strengthened {
            fact_id_a: fact_id_a.clone(),
            fact_id_b: fact_id_b.clone(),
            strength: *strength,
        }),
        _ => None,
    }
}

fn translate_system_event(event: &DomainEvent) -> Option<AiSignal> {
    let now = Timestamp::now();
    let base = AiSignal {
        domain: RecallDomain::General,
        event_kind: "",
        importance: 0.3,
        salience: SalienceVerdict::Accumulate,
        content: String::new(),
        entity: None,
        timestamp: now,
        raw_event: None,
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: Vec::new(),
    };

    match event {
        DomainEvent::ChatTurnCompleted { user_message, .. } => Some(AiSignal {
            event_kind: "ChatTurnCompleted",
            content: user_message.clone().unwrap_or_default(),
            ..base
        }),
        DomainEvent::SessionEnded {
            session_id,
            quality_score,
            ..
        } => Some(AiSignal {
            event_kind: "SessionEnded",
            importance: quality_score.unwrap_or(0.5),
            content: session_id.clone(),
            metrics: AiMetrics {
                amount: *quality_score,
                ..AiMetrics::default()
            },
            ..base
        }),
        DomainEvent::CoachingPatternDetected {
            pattern_name,
            confidence,
            rule_text,
            domain,
            ..
        } => Some(AiSignal {
            event_kind: "CoachingPatternDetected",
            importance: *confidence,
            content: rule_text.clone(),
            metrics: AiMetrics {
                category: Some(pattern_name.clone()),
                ..AiMetrics::default()
            },
            domain: RecallDomain::from_str_or_general(domain.as_str()),
            ..base
        }),
        // DistractionDetected, FocusSessionStarted, FocusSessionEnded, ActivitySessionCompleted,
        // and ProductivityScoreComputed are now handled by try_into_productivity_event →
        // ProductivityEvent::From → AiSignal via the macro-generated to_signal().
        // AtomReinforced is now handled by try_into_learning_event.
        DomainEvent::SkillRouted {
            skill_name,
            confidence,
            ..
        } => Some(AiSignal {
            event_kind: "SkillRouted",
            importance: *confidence,
            content: skill_name.clone(),
            ..base
        }),
        DomainEvent::UserCorrectedAI {
            active_skill,
            strength,
            ..
        } => Some(AiSignal {
            event_kind: "UserCorrectedAI",
            importance: *strength,
            content: active_skill.clone().unwrap_or_default(),
            ..base
        }),
        DomainEvent::AutotunerDecision {
            verdict,
            improvement_pct,
            trial_id,
            ..
        } => Some(AiSignal {
            event_kind: "AutotunerDecision",
            importance: 0.6,
            content: format!("{}: {} ({:.1}%)", trial_id, verdict, improvement_pct),
            metrics: AiMetrics {
                category: Some(verdict.clone()),
                ..AiMetrics::default()
            },
            ..base
        }),
        _ => None,
    }
}

fn try_into_task_event(e: &DomainEvent) -> Option<feature_tasks::events::TaskEvent> {
    use feature_tasks::events::TaskEvent;
    match e {
        DomainEvent::TaskCreated {
            task_id,
            project,
            estimate_mins,
            ..
        } => Some(TaskEvent::Created {
            task_id: task_id.clone(),
            title: String::new(),   // title not in DomainEvent::TaskCreated
            area_id: String::new(), // area_id not in DomainEvent::TaskCreated
            project_id: project.clone(),
            priority: None,
            estimated_minutes: estimate_mins.map(|m| m as i32),
        }),
        DomainEvent::TaskCompleted {
            task_id,
            deviation_pct,
            ..
        } => Some(TaskEvent::Completed {
            task_id: task_id.clone(),
            title: String::new(), // title not in DomainEvent::TaskCompleted
            deviation_pct: *deviation_pct,
        }),
        DomainEvent::TaskFocusChanged {
            task_id,
            focus_deadline,
            ..
        } => Some(TaskEvent::FocusChanged {
            task_id: task_id.clone(),
            title: String::new(), // title not in DomainEvent::TaskFocusChanged
            focus_deadline: focus_deadline.as_ref().and_then(|d| d.parse().ok()),
        }),
        DomainEvent::TaskFocusExpired { task_id, title } => Some(TaskEvent::FocusExpired {
            task_id: task_id.clone(),
            title: title.clone(),
        }),
        DomainEvent::TaskDeferred { task_id, .. } => Some(TaskEvent::Deferred {
            task_id: task_id.clone(),
            title: String::new(),
            previous_due: None,
            new_due: None,
        }),
        DomainEvent::EstimationRecorded {
            task_id,
            estimated_mins,
            actual_mins,
            ..
        } => Some(TaskEvent::EstimationRecorded {
            task_id: task_id.clone(),
            estimated_minutes: Some(*estimated_mins as i32),
            actual_minutes: Some(*actual_mins as i32),
            deviation_pct: 0.0, // calculated field
        }),
        _ => None,
    }
}

fn try_into_finance_event(e: &DomainEvent) -> Option<feature_finance::events::FinanceEvent> {
    use feature_finance::events::FinanceEvent;
    match e {
        DomainEvent::TransactionRecorded {
            category,
            amount,
            is_over_budget,
        } => Some(FinanceEvent::TransactionRecorded {
            _tx_id: String::new(), // not in the old variant
            category: category.clone(),
            amount: *amount as i64,
            currency: String::new(), // not in the old variant
            _is_over_budget: *is_over_budget,
        }),
        DomainEvent::BudgetAlert {
            category,
            spent,
            limit,
        } => Some(FinanceEvent::BudgetAlert {
            category: category.clone(),
            spent: *spent as i64,
            limit: *limit as i64,
        }),
        DomainEvent::AccountCreated {
            account_id,
            name,
            currency,
        } => Some(FinanceEvent::AccountCreated {
            account_id: account_id.clone(),
            name: name.clone(),
            currency: currency.clone(),
        }),
        DomainEvent::BudgetCreated {
            budget_id,
            name,
            amount,
            currency,
        } => Some(FinanceEvent::BudgetCreated {
            _budget_id: budget_id.clone(),
            name: name.clone(),
            amount: *amount,
            currency: currency.clone(),
        }),
        DomainEvent::GoalCreated {
            goal_id,
            name,
            target_amount,
        } => Some(FinanceEvent::GoalCreated {
            goal_id: goal_id.clone(),
            name: name.clone(),
            target_amount: *target_amount,
        }),
        DomainEvent::GoalAchieved { goal_id, name } => Some(FinanceEvent::GoalAchieved {
            goal_id: goal_id.clone(),
            name: name.clone(),
        }),
        DomainEvent::FinanceGoalProgress {
            goal_id,
            name,
            current_amount,
            target_amount,
            delta,
        } => Some(FinanceEvent::GoalProgress {
            goal_id: goal_id.clone(),
            name: name.clone(),
            current_amount: *current_amount,
            target_amount: *target_amount,
            delta: *delta,
        }),
        _ => None,
    }
}

/// Build the workspace `AiFeatureRegistry`. Every feature crate that derives
/// `AiFeature` must be listed here; new features are added in v3+ as a single
/// line per crate.
pub fn build_feature_registry() -> ai_core::AiFeatureRegistry {
    let mut reg = ai_core::AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);
    feature_finance::FinanceFeature::register(&mut reg);
    feature_productivity::ProductivityFeature::register(&mut reg);
    feature_notes::NotesFeature::register(&mut reg);
    feature_learning::LearningFeature::register(&mut reg);
    reg
}

/// Build a workspace-global MetricRegistry populated from every registered AiFeature's
/// event enum `FEATURE_METRICS` const. Called exactly once at startup.
pub fn build_metric_registry() -> ai_core::MetricRegistry {
    let mut reg = ai_core::MetricRegistry::new();
    reg.register_all(feature_tasks::TaskEvent::FEATURE_METRICS);
    reg.register_all(feature_finance::FinanceEvent::FEATURE_METRICS);
    reg.register_all(feature_coaching::events::CoachingEvent::FEATURE_METRICS);
    reg.register_all(feature_productivity::events::ProductivityEvent::FEATURE_METRICS);
    reg.register_all(feature_learning::LearningEvent::FEATURE_METRICS);
    reg.register_all(
        cognitive::services::community_intelligence::events::CommunityEvent::FEATURE_METRICS,
    );
    reg.register_all(cognitive::services::community_intelligence::co_activation_events::CoActivationEvent::FEATURE_METRICS);
    reg
}

pub fn start(bus: Arc<DomainEventBus>, consumers: Vec<Arc<dyn SignalConsumer>>) -> SignalRouter {
    SignalRouter::start(bus, consumers, translate)
}

#[cfg(test)]
mod translate_mirror_tests {
    use super::*;

    #[test]
    fn skill_routed_translates() {
        let ev = bus::DomainEvent::SkillRouted {
            skill_name: "general".into(),
            confidence: 0.85,
            source: "keyword".into(),
            trigger_phrases: vec!["hi".into()],
            session_key: "s".into(),
        };
        let sig = translate(&ev).expect("should translate");
        assert_eq!(sig.event_kind, "SkillRouted");
    }

    #[test]
    fn user_corrected_translates() {
        let ev = bus::DomainEvent::UserCorrectedAI {
            original: String::new(),
            correction: String::new(),
            kind: bus::CorrectionKind::Reaction,
            strength: 0.8,
            session_key: "s".into(),
            active_skill: Some("general".into()),
        };
        let sig = translate(&ev).expect("should translate");
        assert_eq!(sig.event_kind, "UserCorrectedAI");
    }

    #[test]
    fn autotuner_decision_translates_activated() {
        let ev = bus::DomainEvent::AutotunerDecision {
            trial_id: "t1".into(),
            verdict: "activated".into(),
            improvement_pct: 0.0,
            affected_params: vec!["x".into()],
        };
        let sig = translate(&ev).expect("should translate");
        assert_eq!(sig.event_kind, "AutotunerDecision");
    }
}

#[cfg(test)]
mod registry_build_test {
    use super::build_feature_registry;
    use ai_core::RecallDomain;

    #[test]
    fn registry_seeded_with_tasks_and_finance() {
        let reg = build_feature_registry();
        assert!(reg.by_domain(&RecallDomain::Tasks).is_some());
        assert!(reg.by_domain(&RecallDomain::Finance).is_some());
        assert!(reg.by_domain(&RecallDomain::Productivity).is_some());
        assert!(reg.by_domain(&RecallDomain::Notes).is_some());
        assert!(reg.by_domain(&RecallDomain::Learning).is_some());
    }
}
