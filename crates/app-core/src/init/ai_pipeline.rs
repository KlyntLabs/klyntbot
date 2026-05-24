use ai_core::{AiEventMeta, AiMetrics, AiSignal, RecallDomain, SalienceVerdict};
use bus::DomainEvent;
use jiff::Timestamp;

/// Top-level translator: DomainEvent -> Option<AiSignal>.
///
/// **Test helper only.** Production code uses `AiPipelinePlugin` which registers
/// feature translators via `PluginContext::add_event_translator()`, and
/// `FeatureHostResult::translate_event()` iterates those registered translators.
/// This hardcoded dispatch function is kept for integration tests that need
/// translation without booting the full plugin host.
pub fn translate(event: &DomainEvent) -> Option<AiSignal> {
    fn with_domain<E: AiEventMeta>(ev: E, domain: RecallDomain) -> AiSignal {
        let mut sig = ev.to_signal();
        sig.domain = domain;
        sig
    }

    if let Some(e) = feature_tasks::events::try_from_domain_event(event) {
        return Some(with_domain(e, RecallDomain::Tasks));
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
    if let Some(s) = translate_bash_job(event) {
        return Some(s);
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

/// Translator: BashJob events -> AiSignal.
pub fn translate_bash_job(event: &bus::DomainEvent) -> Option<AiSignal> {
    let bus::DomainEvent::BashJob(inner) = event else {
        return None;
    };

    let kind = match inner {
        bus::BashJobEvent::Started { .. } => "BashJob.Started",
        bus::BashJobEvent::Completed { .. } => "BashJob.Completed",
        bus::BashJobEvent::Failed { .. } => "BashJob.Failed",
        bus::BashJobEvent::Cancelled { .. } => "BashJob.Cancelled",
        bus::BashJobEvent::Lost { .. } => "BashJob.Lost",
        bus::BashJobEvent::AttachStarted { .. } => "BashJob.AttachStarted",
        bus::BashJobEvent::AttachEnded { .. } => "BashJob.AttachEnded",
    };

    Some(AiSignal {
        domain: RecallDomain::General,
        event_kind: kind,
        importance: importance_for_bash_event(inner),
        salience: SalienceVerdict::Accumulate,
        content: format!(
            "bash job {} ({}/{}/{})",
            inner.job_id(),
            kind,
            inner.thread_id(),
            inner.agent_id(),
        ),
        entity: None,
        timestamp: jiff::Timestamp::now(),
        raw_event: Some(event.clone()),
        metrics: AiMetrics::default(),
        coaching_signal: false,
        coaching_rule: None,
        metric_samples: vec![],
    })
}

fn importance_for_bash_event(e: &bus::BashJobEvent) -> f64 {
    match e {
        bus::BashJobEvent::Failed { .. } => 0.7,
        bus::BashJobEvent::Lost { .. } => 0.6,
        bus::BashJobEvent::Cancelled { .. } => 0.5,
        bus::BashJobEvent::Completed { .. } => 0.3,
        bus::BashJobEvent::Started { .. } => 0.2,
        bus::BashJobEvent::AttachStarted { .. } => 0.3,
        bus::BashJobEvent::AttachEnded { .. } => 0.3,
    }
}

/// Translator: System-level domain events -> AiSignal.
///
/// Handles events that are not owned by any specific feature crate
/// (ChatTurnCompleted, SessionEnded, SkillRouted, UserCorrectedAI,
/// AutotunerDecision, CoachingFeedback).
pub fn translate_system_event(event: &DomainEvent) -> Option<AiSignal> {
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
        // and ProductivityScoreComputed are now handled by feature translators.
        // AtomReinforced is now handled by the learning feature translator.
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
        DomainEvent::CoachingFeedback {
            intervention_id,
            response,
        } => Some(AiSignal {
            event_kind: "CoachingFeedback",
            importance: 0.5,
            content: intervention_id.clone(),
            coaching_signal: true,
            metrics: AiMetrics {
                category: Some(match response {
                    bus::FeedbackResponse::Helpful => "thumbs_up",
                    bus::FeedbackResponse::Dismissed => "thumbs_down",
                    bus::FeedbackResponse::StopSuggesting => "stop_suggesting",
                }.into()),
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
///
/// Deprecated: use `FeatureHostResult::build_feature_registry()` instead.
pub fn build_feature_registry() -> ai_core::AiFeatureRegistry {
    let mut reg = ai_core::AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);
    feature_productivity::ProductivityFeature::register(&mut reg);
    feature_notes::NotesFeature::register(&mut reg);
    feature_learning::LearningFeature::register(&mut reg);
    feature_language_learning::LanguageLearningFeature::register(&mut reg);
    feature_coaching::CoachingFeature::register(&mut reg);
    reg
}

#[cfg(test)]
mod bash_job_translate_tests {
    use super::*;
    use jiff::Timestamp;

    #[test]
    fn translates_failed() {
        let event = bus::DomainEvent::BashJob(bus::BashJobEvent::Failed {
            job_id: "bash-x".into(),
            thread_id: "t".into(),
            agent_id: "a".into(),
            exit_code: Some(1),
            failure_kind: "TestFailure".into(),
            failure_detail: "...".into(),
        });
        let signal = translate_bash_job(&event).unwrap();
        assert_eq!(signal.event_kind, "BashJob.Failed");
        assert!((signal.importance - 0.7).abs() < 1e-9);
    }

    #[test]
    fn translates_completed_lower_importance() {
        let event = bus::DomainEvent::BashJob(bus::BashJobEvent::Completed {
            job_id: "bash-x".into(),
            thread_id: "t".into(),
            agent_id: "a".into(),
            exit_code: 0,
            duration_ms: 1000,
        });
        let signal = translate_bash_job(&event).unwrap();
        assert!((signal.importance - 0.3).abs() < 1e-9);
    }

    #[test]
    fn returns_none_for_non_bash_event() {
        let event = bus::DomainEvent::SkillRouted {
            skill_name: "test".into(),
            confidence: 0.5,
            source: "keyword".into(),
            trigger_phrases: vec![],
            session_key: "s".into(),
        };
        assert!(translate_bash_job(&event).is_none());
    }
}

#[cfg(test)]
mod translate_system_event_tests {
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
        let sig = translate_system_event(&ev).expect("should translate");
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
        let sig = translate_system_event(&ev).expect("should translate");
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
        let sig = translate_system_event(&ev).expect("should translate");
        assert_eq!(sig.event_kind, "AutotunerDecision");
    }

    #[test]
    fn coaching_feedback_translates() {
        let ev = bus::DomainEvent::CoachingFeedback {
            intervention_id: "i1".into(),
            response: bus::FeedbackResponse::Helpful,
        };
        let sig = translate_system_event(&ev).expect("should translate");
        assert_eq!(sig.event_kind, "CoachingFeedback");
        assert!(sig.coaching_signal);
        assert_eq!(sig.metrics.category.as_deref(), Some("thumbs_up"));
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
