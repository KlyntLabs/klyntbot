use coding_memory::recall::index_builder::IndexBuilder;
use cognitive::retrieval::ScoredFact;

#[test]
fn fact_to_index_entry_includes_token_estimate() {
    let fact = cognitive::SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        subject: "auth_module".into(),
        predicate: "uses".into(),
        object: "JWT with HS256".into(),
        recorded_at: jiff::Timestamp::now().to_string(),
        confidence: 0.9,
        scope_repo_id: Some("repo:demo".into()),
        memory_type: "repo_context".into(),
        ..Default::default()
    };
    let scored = ScoredFact { score: 0.8, fact, similarity: Some(0.8) };
    let builder = IndexBuilder::new();
    let entry = builder.from_scored_fact(&scored);
    assert_eq!(entry.kind, "repo_context");
    assert!(entry.token_cost > 0);
    assert!(entry.confidence > 0.0);
}
