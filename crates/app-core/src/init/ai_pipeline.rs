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
        DomainEvent::AtomReinforced {
            atom_id,
            subject,
            domain,
            reinforcement_count,
            ..
        } => Some(AiSignal {
            event_kind: "AtomReinforced",
            importance: (0.5 + *reinforcement_count as f64 * 0.15).min(0.95),
            content: subject.clone(),
            metrics: AiMetrics {
                category: Some(domain.clone()),
                ..AiMetrics::default()
            },
            entity: Some(ai_core::EntityRef {
                entity_type: "knowledge_atom",
                id: atom_id.clone(),
                name: subject.clone(),
            }),
            ..base
        }),
        DomainEvent::DistractionDetected { app, .. } => Some(AiSignal {
            event_kind: "DistractionDetected",
            content: app.clone(),
            metrics: AiMetrics {
                app: Some(app.clone()),
                ..AiMetrics::default()
            },
            coaching_signal: true,
            ..base
        }),
        DomainEvent::FocusSessionStarted { .. } => Some(AiSignal {
            event_kind: "FocusSessionStarted",
            coaching_signal: true,
            ..base
        }),
        DomainEvent::FocusSessionEnded { quality, .. } => Some(AiSignal {
            event_kind: "FocusSessionEnded",
            importance: *quality,
            metrics: AiMetrics {
                amount: Some(*quality),
                ..AiMetrics::default()
            },
            coaching_signal: true,
            ..base
        }),
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
        _ => None,
    }
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
