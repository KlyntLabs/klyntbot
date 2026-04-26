use coding_memory::distiller::fact_builder::build_prepared;
use coding_memory::distiller::record_observation::{Observation, ObservationScope};
use coding_memory::distiller::PreparedFact;
use coding_memory::facts::CodingKind;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use jiff::Timestamp;
use uuid::Uuid;

fn prov() -> ProvenanceMetadata {
    ProvenanceMetadata {
        source_events: vec![Uuid::new_v4()],
        session_id: "s".into(),
        turn_id: Some("t".into()),
        distilled_at: Timestamp::now(),
        distiller_model: "claude-haiku-4-5".into(),
        source_kind: ProvenanceKind::DistillerLlm,
    }
}

#[test]
fn repo_context_becomes_prepared_fact() {
    let o = Observation {
        kind: CodingKind::RepoContext,
        subject: "repo:github.com/klynt/bot".into(),
        predicate: "framework".into(),
        object: "tauri".into(),
        confidence: 0.9,
        scope: ObservationScope::Repo,
        reasoning: "Cargo.toml lists tauri 2".into(),
        outcome: None,
    };
    let built = build_prepared(&o, Some("github.com/klynt/bot"), &prov()).unwrap();
    let PreparedFact {
        fact,
        scope_repo_id,
        ..
    } = match built {
        coding_memory::distiller::fact_builder::Prepared::Fact(f) => f,
        _ => panic!("expected Fact"),
    };
    assert_eq!(fact.domain, "work");
    assert_eq!(fact.subject, "repo:github.com/klynt/bot");
    assert_eq!(fact.predicate, "framework");
    assert_eq!(fact.memory_type, "fact");
    assert_eq!(scope_repo_id.as_deref(), Some("github.com/klynt/bot"));
}

#[test]
fn style_preference_becomes_prepared_fact_with_preferences_domain() {
    let o = Observation {
        kind: CodingKind::StylePreference,
        subject: "user".into(),
        predicate: "prefers".into(),
        object: "tabs".into(),
        confidence: 0.7,
        scope: ObservationScope::Global,
        reasoning: "observed 3x".into(),
        outcome: None,
    };
    let built = build_prepared(&o, None, &prov()).unwrap();
    let fact = match built {
        coding_memory::distiller::fact_builder::Prepared::Fact(PreparedFact { fact, .. }) => fact,
        _ => panic!(),
    };
    assert_eq!(fact.domain, "preferences");
    assert_eq!(fact.subject, "user");
}

#[test]
fn fix_attempt_becomes_prepared_episode_with_kind() {
    let o = Observation {
        kind: CodingKind::FixAttempt,
        subject: "bug:parser-null-pointer".into(),
        predicate: "fixed".into(),
        object: "added guard in parse_expr".into(),
        confidence: 0.8,
        scope: ObservationScope::Repo,
        reasoning: "tests now pass".into(),
        outcome: None,
    };
    let built = build_prepared(&o, Some("github.com/klynt/bot"), &prov()).unwrap();
    let ep = match built {
        coding_memory::distiller::fact_builder::Prepared::Episode(e) => e,
        _ => panic!("expected Episode"),
    };
    assert_eq!(ep.kind, "fix_attempt");
    assert!(ep.episode.content.contains("added guard"));
}

#[test]
fn workflow_pattern_becomes_prepared_fact_with_pattern_memory_type() {
    let o = Observation {
        kind: CodingKind::WorkflowPattern,
        subject: "workflow:test-before-commit".into(),
        predicate: "applies_when".into(),
        object: "touching code paths with existing tests".into(),
        confidence: 0.6,
        scope: ObservationScope::Repo,
        reasoning: "observed 4x".into(),
        outcome: None,
    };
    let built = build_prepared(&o, Some("x"), &prov()).unwrap();
    let fact = match built {
        coding_memory::distiller::fact_builder::Prepared::Fact(PreparedFact { fact, .. }) => fact,
        _ => panic!(),
    };
    assert_eq!(fact.domain, "procedural");
    assert_eq!(fact.memory_type, "pattern");
}
