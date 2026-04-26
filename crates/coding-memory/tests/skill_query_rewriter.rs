use coding_memory::retrieval_skills::{
    BudgetTier, EscalationContext, QueryRewriter, RetrievalSkill,
};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn produces_three_rewrites_and_unions_ids() {
    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls_c = calls.clone();
    let retrieve = Arc::new(move |_q: String| {
        let calls = calls_c.clone();
        let ids = vec![id1, id2];
        Box::pin(async move {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            common::Result::Ok((vec![0.95f32, 0.7f32], ids))
        })
            as std::pin::Pin<
                Box<dyn std::future::Future<Output = common::Result<(Vec<f32>, Vec<Uuid>)>> + Send>,
            >
    }) as coding_memory::retrieval_skills::query_rewriter::RetrieveFn;

    let skill = QueryRewriter::new(retrieve);
    let ctx = EscalationContext {
        query: "fix the null pointer bug in parser".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::DeepThink,
        repo: None,
    };
    let out = skill.apply(&ctx).await.unwrap();
    assert!(calls.load(std::sync::atomic::Ordering::SeqCst) >= 3);
    assert!(out.added_ids.len() <= 2);
}
