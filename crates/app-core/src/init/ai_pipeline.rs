use ai_core::{
    AiEventMeta, AiMetrics, AiSignal, RecallDomain, SalienceVerdict, SignalConsumer, SignalRouter,
};
use bus::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use std::sync::Arc;

/// Translator: DomainEvent -> Option<AiSignal>. Returns None when the event
/// has no pipeline registration (e.g. transient infra events). Each feature
/// crate owns its own `try_from_domain_event` translator; this function is a
/// pure dispatch shim. Domain is set by the feature's `#[derive(AiEvent)]`
/// enum-level `#[ai(domain = ...)]`.
pub fn translate(event: &DomainEvent) -> Option<AiSignal> {
    fn with_domain<E: AiEventMeta>(ev: E, domain: RecallDomain) -> AiSignal {
        let mut sig = ev.to_signal();
        sig.domain = domain;
        sig
    }

    if let Some(e) = feature_tasks::events::try_from_domain_event(event) {
        return Some(with_domain(e, RecallDomain::Tasks));
    }
    if let Some(e) = feature_finance::events::try_from_domain_event(event) {
        return Some(with_domain(e, RecallDomain::Finance));
    }
    if let Some(e) = feature_coaching::events::try_from_domain_event(event) {
        return Some(with_domain(e, RecallDomain::Coaching));
    }
    if let Some(e) = feature_productivity::events::try_from_domain_event(event) {
        return Some(with_domain(e, RecallDomain::Productivity));
    }
    if let Some(e) = feature_notes::events::try_from_domain_event(event) {
        return Some(with_domain(e, RecallDomain::Notes));
    }
    if let Some(e) = feature_learning::try_from_domain_event(event) {
        return Some(with_domain(e, RecallDomain::Learning));
    }
    if let Some(e) = feature_language_learning::try_from_domain_event(event) {
        return Some(with_domain(e, RecallDomain::LanguageLearning));
    }
    if let Some(e) =
        cognitive::services::community_intelligence::events::try_from_domain_event(event)
    {
        return Some(with_domain(e, RecallDomain::General));
    }
    if let Some(e) =
        cognitive::services::community_intelligence::co_activation_events::try_from_domain_event(
            event,
        )
    {
        return Some(with_domain(e, RecallDomain::General));
    }
    translate_system_event(event)
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
            // Route chat turns through the LLM-based extraction pipeline
            // rather than the keyword-heuristic accumulator. The LLM
            // extractor produces structured S-P-O facts (with identity
            // binding) — the heuristic path can only catch overt
            // "I work at X" patterns and stores everything as subject=user.
            salience: SalienceVerdict::Extract,
            importance: 0.6,
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
    feature_language_learning::LanguageLearningFeature::register(&mut reg);
    feature_coaching::CoachingFeature::register(&mut reg);
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
    reg.register_all(feature_language_learning::LanguageLearningEvent::FEATURE_METRICS);
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
    fn registry_seeded_with_all_v3_features() {
        let reg = build_feature_registry();
        for d in [
            RecallDomain::Tasks,
            RecallDomain::Finance,
            RecallDomain::Productivity,
            RecallDomain::Notes,
            RecallDomain::Learning,
            RecallDomain::LanguageLearning,
            RecallDomain::Coaching,
        ] {
            assert!(reg.by_domain(&d).is_some(), "missing {:?}", d);
        }
    }
}
