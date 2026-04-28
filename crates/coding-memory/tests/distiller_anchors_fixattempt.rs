use coding_memory::distiller::fact_builder::{build_prepared, Prepared};
use coding_memory::distiller::record_observation::{Observation, ObservationScope};
use coding_memory::facts::CodingKind;
use coding_memory::scope::{ProvenanceKind, ProvenanceMetadata};
use coding_memory::TreeSitterExtractor;
use jiff::Timestamp;
use uuid::Uuid;

#[test]
fn fixattempt_episode_carries_anchored_symbols_when_files_known() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("buggy.rs");
    std::fs::write(&file, "fn buggy() { panic!(); }\n").unwrap();

    let prov = ProvenanceMetadata {
        source_events: vec![Uuid::new_v4()],
        session_id: "s1".into(),
        turn_id: Some("t1".into()),
        distilled_at: Timestamp::now(),
        distiller_model: "test".into(),
        source_kind: ProvenanceKind::DistillerLlm,
    };
    let obs = Observation {
        kind: CodingKind::FixAttempt,
        subject: "panic in buggy".into(),
        predicate: "fixed_by".into(),
        object: "guard against None".into(),
        confidence: 0.8,
        scope: ObservationScope::Repo,
        reasoning: "added a guard".into(),
        outcome: Some(coding_memory::facts::FixOutcome::Success),
        files: vec![file.clone()],
    };
    let extractor = TreeSitterExtractor::new();
    let prepared = build_prepared(&obs, Some("repo:test"), &prov, Some(&extractor)).unwrap();

    let Prepared::Episode(ep) = prepared else {
        panic!("expected Prepared::Episode");
    };
    let metadata: serde_json::Value =
        serde_json::from_str(ep.metadata_json.unwrap().to_string().as_str()).unwrap();
    let anchors = metadata["anchoredSymbols"]
        .as_array()
        .expect("anchors array");
    assert_eq!(anchors.len(), 1);
    assert_eq!(anchors[0]["symbol"], "buggy");
}
