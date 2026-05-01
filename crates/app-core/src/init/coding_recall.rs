use std::sync::Arc;

use bus::DomainEventBus;
use coding_memory::causal::CausalEdgeRepo;
use coding_memory::recall::CodingRecallService;
use storage::StoragePool;

/// Construct the [`CodingRecallService`] from storage pools and repos.
///
/// This is extracted from `init/mod.rs` so it can be built *before* the
/// `AgentLoop` is constructed and passed into `AgentLoopBuilder`.
pub async fn init_coding_recall(
    storage_pool: &StoragePool,
    domain_event_bus: &Arc<DomainEventBus>,
    causal_edges: &Arc<CausalEdgeRepo>,
) -> Option<Arc<CodingRecallService>> {
    let fact_repo = Arc::new(::cognitive::SemanticFactRepo::new(
        storage_pool.inner().clone(),
    ));
    let ep_repo = Arc::new(::cognitive::EpisodicMemoryRepo::new(
        storage_pool.inner().clone(),
    ));
    let ums = Arc::new(::cognitive::UnifiedMemoryService::new((*fact_repo).clone()));
    let telem = coding_memory::RecallInvocationRepo::new(storage_pool.clone());
    let budgeter = coding_memory::recall::budget::default_budgeter();

    let recall_weights = coding_memory::recall::load_recall_weights(storage_pool)
        .await
        .unwrap_or_else(|e| {
            tracing::warn!(error = %e, "load_recall_weights failed, using defaults");
            coding_memory::recall::default_weights()
        });

    let skills: Vec<std::sync::Arc<dyn coding_memory::RetrievalSkill>> = {
        use coding_memory::retrieval_skills::{QueryDecomposer, QueryRewriter};
        let ums_for_retrieve = ums.clone();
        let weights = recall_weights;
        let retrieve: coding_memory::retrieval_skills::query_rewriter::RetrieveFn =
            std::sync::Arc::new(move |q: String| {
                let ums = ums_for_retrieve.clone();
                let weights = weights;
                Box::pin(async move {
                    let scored = ums.retrieve_with_overrides(&q, 20, 0.0, weights).await?;
                    let mut sims = Vec::with_capacity(scored.len());
                    let mut ids = Vec::with_capacity(scored.len());
                    for s in scored {
                        sims.push(s.score as f32);
                        if let Ok(u) = s.fact.id.parse::<uuid::Uuid>() {
                            ids.push(u);
                        }
                    }
                    Ok((sims, ids))
                })
            });
        vec![
            std::sync::Arc::new(QueryRewriter::new(retrieve.clone())),
            std::sync::Arc::new(QueryDecomposer::new(retrieve)),
        ]
    };
    let registry = std::sync::Arc::new(coding_memory::RetrievalSkillRegistry::new(
        skills,
        domain_event_bus.clone(),
    ));

    Some(Arc::new(
        CodingRecallService::new(
            coding_memory::recall::CodingRecallServiceConfig {
                weights: recall_weights,
                ..coding_memory::recall::CodingRecallServiceConfig::default()
            },
            ums,
            fact_repo,
            ep_repo,
            telem,
            budgeter,
        )
        .with_skills(registry)
        .with_causal_repo(causal_edges.clone()),
    ))
}
