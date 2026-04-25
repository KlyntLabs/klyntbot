use coding_memory::retrieval_skills::evidence_focuser::{EvidenceFocuser, FetchTextsFn};
use coding_memory::retrieval_skills::{BudgetTier, EscalationContext, RetrievalSkill};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn focuses_to_top_five_by_lexical_cosine() {
    let ids: Vec<Uuid> = (0..20).map(|_| Uuid::new_v4()).collect();
    let texts: Vec<(Uuid, String)> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            let text = if i < 5 {
                "null pointer parser bug".into()
            } else {
                format!("unrelated text {i}")
            };
            (*id, text)
        })
        .collect();
    let fetch_texts: FetchTextsFn = Arc::new(move |_ids| {
        let texts = texts.clone();
        Box::pin(async move { common::Result::Ok(texts) })
    });
    let initial_ids = ids.clone();
    let initial_provider: Arc<dyn Fn() -> Vec<Uuid> + Send + Sync> =
        Arc::new(move || initial_ids.clone());

    let skill = EvidenceFocuser::new(initial_provider, fetch_texts);
    let ctx = EscalationContext {
        query: "null pointer parser bug".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::DeepThink,
        repo: None,
    };
    let out = skill.apply(&ctx).await.unwrap();
    assert_eq!(out.added_ids.len(), 5);
}
