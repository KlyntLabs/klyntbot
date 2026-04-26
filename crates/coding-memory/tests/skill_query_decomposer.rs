use coding_memory::retrieval_skills::{
    BudgetTier, EscalationContext, QueryDecomposer, RetrievalSkill,
};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn compound_query_yields_multiple_subs() {
    let calls = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let calls_c = calls.clone();
    let retrieve = Arc::new(move |q: String| {
        let calls = calls_c.clone();
        Box::pin(async move {
            calls.lock().unwrap().push(q);
            common::Result::Ok((vec![0.8f32], vec![Uuid::new_v4()]))
        })
            as std::pin::Pin<
                Box<dyn std::future::Future<Output = common::Result<(Vec<f32>, Vec<Uuid>)>> + Send>,
            >
    }) as coding_memory::retrieval_skills::query_decomposer::RetrieveFn;

    let skill = QueryDecomposer::new(retrieve);
    let ctx = EscalationContext {
        query: "fix the parser bug and improve error messages".into(),
        coverage_score: 0.05,
        budget_tier: BudgetTier::DeepThink,
        repo: None,
    };
    let _ = skill.apply(&ctx).await.unwrap();
    let count = calls.lock().unwrap().len();
    assert!((2..=4).contains(&count), "got {count}");
}
