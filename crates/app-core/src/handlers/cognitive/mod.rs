pub mod graph;
mod memory;
mod mutations;
mod operations;

pub(crate) use memory::fact_to_response;

/// Build a [`ReforgeHandler`] from a cognitive provider and config.
///
/// Returns an LLM-backed handler when a provider is available, a no-op fallback otherwise.
pub(crate) fn build_reforge_handler(
    cognitive_provider: &Option<providers::DynProvider>,
    config: &config::Config,
) -> Box<dyn cognitive::services::reforge::ReforgeHandler> {
    if let Some(ref cp) = cognitive_provider {
        let params = providers::cognitive_chat_params(config, 4096);
        Box::new(agent::adapters::reforge_handlers::LlmReforgeHandler::new(
            cp.clone(),
            params,
        ))
    } else {
        Box::new(agent::adapters::reforge_handlers::NoopReforgeHandler)
    }
}

/// Build a [`GraphEnrichmentHandler`] for Phase 6.5 graph consolidation.
///
/// Returns `None` when no cognitive provider is configured.
pub(crate) fn build_graph_enrichment_handler(
    cognitive_provider: &Option<providers::DynProvider>,
    config: &config::Config,
) -> Option<Box<dyn cognitive::services::reforge::GraphEnrichmentHandler>> {
    cognitive_provider.as_ref().map(|cp| {
        let params = providers::cognitive_chat_params(config, 4096);
        Box::new(
            agent::adapters::reforge_handlers::LlmGraphEnrichmentHandler::new(cp.clone(), params),
        ) as Box<dyn cognitive::services::reforge::GraphEnrichmentHandler>
    })
}

/// Build a [`CommunityIntelligenceHandler`] for Phase 6.5 community naming/merge/split.
///
/// Returns `None` when no cognitive provider is configured.
pub(crate) fn build_community_intelligence_handler(
    cognitive_provider: &Option<providers::DynProvider>,
    config: &config::Config,
) -> Option<Box<dyn cognitive::services::reforge::CommunityIntelligenceHandler>> {
    cognitive_provider.as_ref().map(|cp| {
        let params = providers::cognitive_chat_params(config, 4096);
        Box::new(
            agent::adapters::reforge_handlers::LlmGraphEnrichmentHandler::new(cp.clone(), params),
        ) as Box<dyn cognitive::services::reforge::CommunityIntelligenceHandler>
    })
}
