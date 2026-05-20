pub mod graph;
mod memory;
mod mutations;
mod operations;

pub(crate) use memory::fact_to_response;

use async_trait::async_trait;

struct NoopReforgeHandler;

#[async_trait]
impl cognitive::services::reforge::ReforgeHandler for NoopReforgeHandler {
    async fn synthesize(
        &self,
        _input: &cognitive::services::reforge::types::SynthesizeInput,
    ) -> common::Result<cognitive::services::reforge::types::SynthesizeOutput> {
        Ok(cognitive::services::reforge::types::SynthesizeOutput {
            fact_updates: vec![],
            rule_updates: vec![],
            stale_facts: vec![],
            cross_session_patterns: vec![],
            extraction_quality_flag: None,
        })
    }

    async fn review(
        &self,
        _input: &cognitive::services::reforge::types::ReviewInput,
    ) -> common::Result<cognitive::services::reforge::types::ReviewOutput> {
        Ok(cognitive::services::reforge::types::ReviewOutput {
            skill_edits: vec![],
            routing_insights: vec![],
            context_priority_suggestions: vec![],
            trial_suggestions: vec![],
        })
    }

    async fn narrate(
        &self,
        _input: &cognitive::services::reforge::types::NarrateInput,
    ) -> common::Result<String> {
        Ok(String::new())
    }
}

/// Build a [`ReforgeHandler`] from a cognitive provider and config.
///
/// Returns an LLM-backed handler when a provider is available, a no-op fallback otherwise.
pub(crate) fn build_reforge_handler(
    _cognitive_provider: &Option<providers::DynProvider>,
    _config: &config::Config,
) -> Box<dyn cognitive::services::reforge::ReforgeHandler> {
    Box::new(NoopReforgeHandler)
}

/// Build a [`GraphEnrichmentHandler`] for Phase 6.5 graph consolidation.
///
/// Returns `None` when no cognitive provider is configured.
pub(crate) fn build_graph_enrichment_handler(
    _cognitive_provider: &Option<providers::DynProvider>,
    _config: &config::Config,
) -> Option<Box<dyn cognitive::services::reforge::GraphEnrichmentHandler>> {
    None
}

/// Build a [`CommunityIntelligenceHandler`] for Phase 6.5 community naming/merge/split.
///
/// Returns `None` when no cognitive provider is configured.
pub(crate) fn build_community_intelligence_handler(
    _cognitive_provider: &Option<providers::DynProvider>,
    _config: &config::Config,
) -> Option<Box<dyn cognitive::services::reforge::CommunityIntelligenceHandler>> {
    None
}

/// Build a [`MicroReforgeHandler`] for KCA Track 4.
///
/// Returns an LLM-backed handler when a provider is available, a no-op fallback otherwise.
pub(crate) fn build_micro_reforge_handler(
    cognitive_provider: &Option<providers::DynProvider>,
    config: &config::Config,
) -> std::sync::Arc<dyn cognitive::services::micro_reforge::MicroReforgeHandler> {
    if let Some(ref cp) = cognitive_provider {
        let model = config
            .cognitive
            .micro_reforge
            .model
            .clone()
            .unwrap_or_else(|| {
                config
                    .cognitive
                    .model
                    .clone()
                    .unwrap_or_else(|| config.agents.defaults.model.clone())
            });
        let params = providers::ChatParams::new(model)
            .with_temperature(0.2)
            .with_max_tokens(4096)
            .with_response_format(providers::ResponseFormat::JsonObject);
        std::sync::Arc::new(
            agent::adapters::cognitive_handlers::LlmMicroReforgeHandler::new(cp.clone(), params),
        )
    } else {
        std::sync::Arc::new(cognitive::services::micro_reforge::NoopMicroReforgeHandler)
    }
}

/// Build a [`HierarchicalSummarizer`] for KCA Track 8 episodic roll-ups.
///
/// Returns an LLM-backed handler when a provider is available, a no-op fallback otherwise.
pub(crate) fn build_hierarchical_summarizer(
    cognitive_provider: &Option<providers::DynProvider>,
    config: &config::Config,
) -> std::sync::Arc<dyn cognitive::services::hierarchical_compressor::HierarchicalSummarizer> {
    if let Some(ref cp) = cognitive_provider {
        let model = config
            .cognitive
            .hierarchical
            .model
            .clone()
            .unwrap_or_else(|| {
                config
                    .cognitive
                    .model
                    .clone()
                    .unwrap_or_else(|| config.agents.defaults.model.clone())
            });
        let params = providers::ChatParams::new(model)
            .with_temperature(0.3)
            .with_max_tokens(1024);
        std::sync::Arc::new(
            agent::adapters::cognitive_handlers::LlmHierarchicalSummarizer::new(cp.clone(), params),
        )
    } else {
        std::sync::Arc::new(
            cognitive::services::hierarchical_compressor::NoopHierarchicalSummarizer,
        )
    }
}
