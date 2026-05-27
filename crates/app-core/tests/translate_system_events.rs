use ai_core::{AiEventMeta, RecallDomain};
use app_core::init::ai_pipeline::{translate_bash_job, translate_system_event};
use bus::DomainEvent;

#[test]
fn chat_turn_completed_translates_to_general_signal() {
    let e = DomainEvent::ChatTurnCompleted {
        session_key: "s1".into(),
        user_message: Some("hi".into()),
    };
    let sig = translate_system_event(&e).expect("should translate");
    assert_eq!(sig.domain, RecallDomain::General);
    assert_eq!(sig.event_kind, "ChatTurnCompleted");
}

#[test]
fn session_ended_translates() {
    let e = DomainEvent::SessionEnded {
        session_id: "s1".into(),
        session_type: "focus".into(),
        duration_secs: 3600,
        quality_score: Some(0.8),
        category_purity: 0.9,
    };
    let sig = translate_system_event(&e).expect("should translate");
    assert_eq!(sig.event_kind, "SessionEnded");
}

#[test]
fn coaching_pattern_translates() {
    let e = DomainEvent::CoachingPatternDetected {
        pattern_name: "afternoon_energy_drop".into(),
        confidence: 0.8,
        description: "desc".into(),
        domain: "productivity".into(),
        signal_count: 3,
        rule_text: "Schedule demanding tasks in the morning".into(),
    };
    let coaching_event =
        feature_coaching::events::try_from_domain_event(&e).expect("should parse coaching event");
    let sig = coaching_event.to_signal();
    assert_eq!(sig.event_kind, "PatternDetected");
    assert_eq!(
        sig.content,
        "Coaching pattern detected: afternoon_energy_drop (severity 0.8)"
    );
}

#[test]
fn atom_reinforced_translates() {
    let e = DomainEvent::AtomReinforced {
        atom_id: "a1".into(),
        referencing_note_id: "n1".into(),
        new_salience: 0.8,
        subject: "rust errors".into(),
        domain: "learning".into(),
        reinforcement_count: 3,
    };
    let learning_event =
        feature_learning::try_from_domain_event(&e).expect("should parse learning event");
    let sig = learning_event.to_signal();
    assert_eq!(sig.event_kind, "AtomReinforced");
}

#[test]
fn bash_job_failed_translates() {
    let e = DomainEvent::BashJob(bus::BashJobEvent::Failed {
        job_id: "bash-x".into(),
        thread_id: "t".into(),
        agent_id: "a".into(),
        exit_code: Some(1),
        failure_kind: "TestFailure".into(),
        failure_detail: "...".into(),
    });
    let sig = translate_bash_job(&e).expect("should translate");
    assert_eq!(sig.event_kind, "BashJob.Failed");
}
