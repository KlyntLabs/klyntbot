use coding_memory::retrieval_skills::causal_context_expander::{CausalContextExpander, EdgeLookupFn};
use coding_memory::retrieval_skills::{BudgetTier, EscalationContext, RetrievalSkill};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn no_edges_returns_failure_outcome() {
    let lookup: EdgeLookupFn = Arc::new(|_ids| Box::pin(async { common::Result::Ok(vec![]) }));
    let provider: Arc<dyn Fn() -> Vec<Uuid> + Send + Sync> = Arc::new(|| vec![Uuid::new_v4()]);
    let skill = CausalContextExpander::new(provider, lookup);
    let out = skill
        .apply(&EscalationContext {
            query: "x".into(),
            coverage_score: 0.0,
            budget_tier: BudgetTier::Ultra,
            repo: None,
        })
        .await
        .unwrap();
    assert!(!out.succeeded);
    assert!(out.added_ids.is_empty());
}
