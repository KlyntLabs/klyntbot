//! Query-enhancement pipeline builder.
//!
//! Assembles signal-enrichment, PRF, multi-query, and reranking stages
//! when `config.cognitive.query_enhancement.enabled` is true.

use std::sync::Arc;

use config::Config;
use providers::DynProvider;

/// Build query- and ranking-pipeline stages and attach them to the context engine.
pub(crate) fn build_query_enhancement(
    config: &Config,
    context_engine: context_engine::ContextEngine,
    cognitive_provider: &Option<DynProvider>,
    autotuner: &Option<Arc<crate::autotuner::AutoTunerOrchestrator>>,
    memory_retriever_for_prf: &Option<Arc<dyn context_engine::MemoryRetriever>>,
    latest_enhancement_trace: &Arc<context_engine::enhancement::LatestEnhancementTrace>,
) -> context_engine::ContextEngine {
    let qe = &config.cognitive.query_enhancement;
    if !qe.enabled {
        return context_engine;
    }

    let rewriter_provider = cognitive_provider.clone();
    let rewriter_model = config.agents.rewriter_model.clone();

    // Stage 1: Signal Enrichment — wraps ContextualQueryRewriter for
    // heuristic signal-based query enrichment. Autotuner champion
    // overrides are wired onto this rewriter so A/B trials apply.
    let memory_param_sink = autotuner
        .as_ref()
        .and_then(|orch| orch.memory_param_sink());

    let mut signal_rewriter = crate::adapters::query_rewriter::ContextualQueryRewriter::new(
        rewriter_provider.clone(),
        rewriter_model.clone(),
        800, // 800ms hard cap per spec
    );
    if let Some(ref sink) = memory_param_sink {
        signal_rewriter = signal_rewriter.with_champion_overrides(Arc::clone(sink));
    }
    let signal_stage =
        crate::adapters::signal_enrichment::SignalEnrichmentStage::new(signal_rewriter);

    let mut query_stages: Vec<Arc<dyn context_engine::enhancement::QueryStage>> =
        vec![Arc::new(signal_stage)];

    // Stage 2: PRF (needs memory retriever — only if available)
    if qe.prf.enabled {
        if let Some(ref retriever) = memory_retriever_for_prf {
            let prf_config = context_engine::enhancement::prf::PrfConfig {
                initial_fetch_limit: qe.prf.initial_fetch_limit,
                min_score_threshold: qe.prf.min_score_threshold,
                max_expansion_terms: qe.prf.max_expansion_terms,
            };
            let mut prf_stage = context_engine::enhancement::prf::PrfStage::new(
                Arc::clone(retriever),
                prf_config,
            );
            if let Some(ref sink) = memory_param_sink {
                prf_stage = prf_stage.with_champion_overrides(Arc::clone(sink));
            }
            query_stages.push(Arc::new(prf_stage));
        } else {
            tracing::debug!(
                "PRF stage enabled but no memory retriever available — skipping"
            );
        }
    }

    // Stage 3: Multi-Query (LLM, Deep+ only — budget gates at runtime)
    if qe.multi_query.enabled {
        let mq_model = qe
            .multi_query
            .model
            .clone()
            .or_else(|| rewriter_model.clone());
        let mut multi_query = crate::adapters::multi_query::MultiQueryStage::new(
            rewriter_provider.clone(),
            mq_model,
            qe.multi_query.max_variants,
        );
        if let Some(ref sink) = memory_param_sink {
            multi_query = multi_query.with_champion_overrides(Arc::clone(sink));
        }
        query_stages.push(Arc::new(multi_query));
    }

    let query_pipeline = Arc::new(
        context_engine::enhancement::QueryPipeline::new(query_stages)
            .with_latest_trace_store(Arc::clone(latest_enhancement_trace)),
    );

    // Ranking stages
    let mut ranking_stages: Vec<Arc<dyn context_engine::enhancement::RankingStage>> = vec![];

    if qe.reranking.enabled {
        let mut heuristic = context_engine::enhancement::heuristic_rerank::HeuristicRerankStage::new(
            context_engine::enhancement::heuristic_rerank::HeuristicRerankConfig::default(),
        );
        if let Some(ref sink) = memory_param_sink {
            heuristic = heuristic.with_champion_overrides(Arc::clone(sink));
        }
        ranking_stages.push(Arc::new(heuristic));

        let llm_model = qe
            .reranking
            .llm_rerank_model
            .clone()
            .or_else(|| rewriter_model.clone());
        let llm_rerank = crate::adapters::llm_rerank::LlmRerankStage::new(
            rewriter_provider.clone(),
            llm_model,
            qe.reranking.llm_rerank_top_n,
        );
        ranking_stages.push(Arc::new(llm_rerank));
    }

    let ranking_pipeline = Arc::new(context_engine::enhancement::RankingPipeline::new(
        ranking_stages,
    ));

    tracing::info!(
        prf = qe.prf.enabled && memory_retriever_for_prf.is_some(),
        multi_query = qe.multi_query.enabled,
        reranking = qe.reranking.enabled,
        "Query enhancement pipeline wired"
    );

    context_engine
        .with_query_pipeline(query_pipeline)
        .with_ranking_pipeline(ranking_pipeline)
}
