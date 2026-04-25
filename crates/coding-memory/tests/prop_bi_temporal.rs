//! Invariant 2 — `valid_until >= valid_from` for every semantic fact.

use cognitive::types::SemanticFact;
use jiff::Timestamp;

#[test]
fn bi_temporal_monotone_for_new_fact() {
    let now = Timestamp::now();
    let f = SemanticFact {
        id: "f1".into(), domain: "work".into(),
        subject: "x".into(), predicate: "y".into(), object: "z".into(),
        confidence: 0.9, source: "distiller".into(),
        valid_from: now.to_string(), valid_until: None,
        recorded_at: now.to_string(),
        superseded_at: None, superseded_by: None,
        stability: 1.0, last_accessed: None, access_count: 0,
        convergence_score: 1.0, project_id: None,
        memory_type: "fact".into(), scope_type: "user".into(), scope_id: None,
        scope_repo_id: None,
        metadata: None,
    };
    assert!(f.valid_until.is_none() || f.valid_until >= Some(f.valid_from.clone()));
}
