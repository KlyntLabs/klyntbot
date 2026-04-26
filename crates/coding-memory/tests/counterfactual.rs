use coding_memory::counterfactual::derive_dead_end;
use coding_memory::facts::{FixAttempt, FixOutcome};
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use jiff::Timestamp;
use uuid::Uuid;

#[test]
fn failure_attempt_emits_counterfactual() {
    let attempt = FixAttempt {
        problem_hash: "h".into(),
        problem: "null pointer".into(),
        files: vec![],
        approach: "add guard".into(),
        outcome: FixOutcome::Failure,
        insight: None,
        duration_ms: 100,
        test_before: None,
        test_after: None,
        anchored_symbols: vec![],
        provenance: ProvenanceMetadata {
            source_events: vec![Uuid::new_v4()],
            session_id: "s".into(),
            turn_id: None,
            distilled_at: Timestamp::now(),
            distiller_model: "m".into(),
            source_kind: ProvenanceKind::DistillerLlm,
        },
        sensitivity: coding_memory::scope::Sensitivity::default(),
    };
    let fact = derive_dead_end(&attempt, &attempt.provenance).unwrap();
    assert_eq!(fact.memory_type, "counterfactual");
    assert!(fact.subject.starts_with("problem:"));
}

#[test]
fn success_attempt_emits_no_counterfactual() {
    let attempt = FixAttempt {
        problem_hash: "h".into(),
        problem: "null pointer".into(),
        files: vec![],
        approach: "add guard".into(),
        outcome: FixOutcome::Success,
        insight: None,
        duration_ms: 100,
        test_before: None,
        test_after: None,
        anchored_symbols: vec![],
        provenance: ProvenanceMetadata {
            source_events: vec![Uuid::new_v4()],
            session_id: "s".into(),
            turn_id: None,
            distilled_at: Timestamp::now(),
            distiller_model: "m".into(),
            source_kind: ProvenanceKind::DistillerLlm,
        },
        sensitivity: coding_memory::scope::Sensitivity::default(),
    };
    assert!(derive_dead_end(&attempt, &attempt.provenance).is_none());
}
