use coding_memory::retrieval_skills::raw_event_escalator::{
    EventLookupFn, ProvenanceIdsFn, RawEventEscalator,
};
use coding_memory::retrieval_skills::{BudgetTier, EscalationContext, RetrievalSkill};
use std::sync::Arc;

#[tokio::test]
async fn surfaces_event_payload_text() {
    let provenance: ProvenanceIdsFn = Arc::new(|| vec!["evt1".into(), "evt2".into()]);
    let lookup: EventLookupFn = Arc::new(|ids| {
        Box::pin(async move {
            common::Result::Ok(
                ids.into_iter()
                    .map(|id| serde_json::json!({"event_id": id, "kind": "FileEdit"}))
                    .collect(),
            )
        })
    });
    let skill = RawEventEscalator::new(provenance, lookup);
    let out = skill
        .apply(&EscalationContext {
            query: "x".into(),
            coverage_score: 0.0,
            budget_tier: BudgetTier::Ultra,
            repo: None,
        })
        .await
        .unwrap();
    assert!(out.added_context.contains("FileEdit"));
}
