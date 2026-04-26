use coding_memory::distiller::record_observation::{
    decode_observations, Observation, ObservationScope, RECORD_OBSERVATION_TOOL_NAME,
};
use coding_memory::facts::CodingKind;

#[test]
fn tool_name_is_record_observation() {
    assert_eq!(RECORD_OBSERVATION_TOOL_NAME, "record_observation");
}

#[test]
fn decodes_valid_fix_attempt() {
    let json = serde_json::json!({
        "kind": "fix_attempt",
        "subject": "repo:github.com/klynt/bot",
        "predicate": "fixed",
        "object": "null pointer in parser by adding guard",
        "confidence": 0.85,
        "scope": "repo",
        "reasoning": "tests passed after the edit"
    });
    let obs: Observation = decode_observations(&[json])
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(obs.kind, CodingKind::FixAttempt);
    assert_eq!(obs.confidence, 0.85);
    assert!(matches!(obs.scope, ObservationScope::Repo));
}

#[test]
fn rejects_invalid_kind() {
    let json = serde_json::json!({
        "kind": "problem_solution_pattern", // Reforge-only; Distiller cannot emit
        "subject": "x", "predicate": "y", "object": "z",
        "confidence": 0.5, "scope": "global", "reasoning": ""
    });
    assert!(decode_observations(&[json]).is_err());
}

#[test]
fn clamps_confidence_to_0_1() {
    let json = serde_json::json!({
        "kind": "style_preference",
        "subject": "user", "predicate": "prefers", "object": "tabs",
        "confidence": 1.7, "scope": "global", "reasoning": "observed 5x"
    });
    let obs = decode_observations(&[json])
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert!((obs.confidence - 1.0).abs() < f32::EPSILON);
}

#[test]
fn empty_input_yields_empty_output() {
    assert!(decode_observations(&[]).unwrap().is_empty());
}
