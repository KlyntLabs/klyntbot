//! Context-source assembly for the agent loop.
//!
//! Builds the ordered list of [`ContextSource`] implementations, optionally
//! starting the cognitive background-consolidation and session-memory services
//! when a real database pool is available.

use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

use config::Config;
use context_engine::ContextSource;
use providers::DynProvider;

/// Result of the context-source build phase.
pub(crate) struct ContextSourcesResult {
    pub sources: Vec<Box<dyn ContextSource>>,
    pub cognitive_fact_repo: Option<cognitive::SemanticFactRepo>,
    pub cognitive_embedder: Option<Arc<dyn cognitive::SemanticFactEmbedder>>,
    pub cognitive_retrieval_config: Option<cognitive::CognitiveRetrievalConfig>,
    pub cognitive_bg_service: Option<cognitive::background::BackgroundConsolidationService>,
    pub session_memory_service: Option<cognitive::SessionMemoryService>,
    pub prod_repos: Option<feature_productivity::repos::ProductivityRepos>,
    pub confidence_bits: Arc<std::sync::atomic::AtomicU32>,
    pub inference_loop_token: Option<CancellationToken>,
}

/// Assemble all context sources and optional background services.
pub(crate) async fn build_context_sources(
    config: &Config,
    pool: &Option<sqlx::SqlitePool>,
    vector_store: &Option<storage::VectorStore>,
    domain_event_bus: &Option<Arc<bus::DomainEventBus>>,
    cognitive_provider: &Option<DynProvider>,
    pipeline_tx: &mut Option<tokio::sync::broadcast::Sender<cognitive::PipelineEvent>>,
    context_update_queue: &Option<Arc<bus::ContextUpdateQueue>>,
    data_dir_path: &std::path::Path,
    workspace: &std::path::Path,
    repos: &storage::Repos,
    storage_pool: &storage::StoragePool,
    embedding_engine: &Arc<tools::EmbeddingEngine>,
    skill_store: &Arc<tokio::sync::RwLock<skill_system::SkillStore>>,
    mut pre_registered_sources: Vec<Box<dyn ContextSource>>,
    shared_fact_repo: Option<cognitive::SemanticFactRepo>,
    shared_embedder: Option<Arc<dyn cognitive::SemanticFactEmbedder>>,
) -> common::Result<ContextSourcesResult> {
    let confidence_bits = Arc::new(std::sync::atomic::AtomicU32::new(
        config.confidence.threshold.to_bits(),
    ));

    // Soul context source (KLYNTBOT.md)
    let soul_source = skill_system::SoulContextSource::load(data_dir_path)?;
    // Skill listing source (frontmatter listing of all skills)
    let skill_listing_source =
        skill_system::SkillListingSource::new(Arc::clone(skill_store));

    let mut sources: Vec<Box<dyn ContextSource>> = vec![
        Box::new(soul_source),
        Box::new(skill_listing_source),
        Box::new(crate::context_sources::IdentitySource::new(
            workspace.to_path_buf(),
            config.timezone.clone(),
        )),
        Box::new(crate::context_sources::BootstrapSource::new(
            workspace.to_path_buf(),
        )),
        Box::new(crate::context_sources::SessionContextSource::new(
            repos.clone(),
        )),
        Box::new(crate::context_sources::SessionMemoryContextSource::new(
            storage::SessionMemoryRepo::new(storage_pool.inner().clone()),
        )),
        Box::new(crate::context_sources::AreaSource::new(repos.areas.clone())),
        Box::new(crate::context_sources::PageContextSource::new(repos.clone())),
    ];

    // Cognitive context source (optional — requires real pool).
    // These are hoisted so UnifiedMemoryService can use them outside the block.
    let mut cognitive_fact_repo: Option<cognitive::SemanticFactRepo> = None;
    let mut cognitive_embedder: Option<Arc<dyn cognitive::SemanticFactEmbedder>> = None;
    let mut cognitive_retrieval_config: Option<cognitive::CognitiveRetrievalConfig> = None;

    let cognitive_bg_service: Option<
        cognitive::background::BackgroundConsolidationService,
    > = if let Some(ref pool) = pool {
        // Use shared repos from app-core plugins when available (eliminates duplication).
        let fact_repo = shared_fact_repo.unwrap_or_else(|| cognitive::SemanticFactRepo::new(pool.clone()));
        let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());

        // Use shared embedder from app-core plugins when available.
        let cognitive_embedder_local: Option<Arc<dyn cognitive::SemanticFactEmbedder>> =
            shared_embedder.or_else(|| {
                vector_store.as_ref().map(|vs| {
                    Arc::new(
                        crate::adapters::cognitive_embedder::SemanticFactEmbedderImpl::new(
                            Arc::clone(embedding_engine),
                            vs.clone(),
                        ),
                    ) as Arc<dyn cognitive::SemanticFactEmbedder>
                })
            });

        // Build retrieval config from app config
        let retrieval_config = cognitive::CognitiveRetrievalConfig {
            dynamic_facts_enabled: config.cognitive.dynamic_facts_enabled,
            static_fact_limit: config.cognitive.static_fact_limit,
            dynamic_fact_limit: config.cognitive.dynamic_fact_limit,
            vector_top_k: config.cognitive.vector_top_k,
            min_similarity: config.cognitive.min_similarity,
            max_stability: config.cognitive.max_stability,
            relevance_weight_semantic: config.cognitive.relevance_weight_semantic,
            relevance_weight_retrievability: config.cognitive.relevance_weight_retrievability,
            relevance_weight_importance: config.cognitive.relevance_weight_importance,
            relevance_weight_frequency: config.cognitive.relevance_weight_frequency,
            relevance_weight_situation: config.cognitive.relevance_weight_situation,
            relevance_weight_temporal: config.cognitive.relevance_weight_temporal,
            relevance_weight_hierarchy: 0.10,
            relevance_weight_path_coherence: 0.05,
            relevance_weight_community: 0.15,
            relevance_weight_cross_note: 0.10,
            relevance_weight_recall_support: config.cognitive.relevance_weight_recall_support,
            relevance_weight_graph_path_boost: config.cognitive.relevance_weight_graph_path_boost,
        };

        // Hoist for UnifiedMemoryService wiring below
        cognitive_fact_repo = Some(fact_repo.clone());
        cognitive_embedder = cognitive_embedder_local.clone();
        cognitive_retrieval_config = Some(retrieval_config);

        let recall_registry = ai_core::RecallProviderRegistry::new()
            .with(feature_tasks::TasksFeature::default());
        let cog_source =
            cognitive::CognitiveContextSource::new(fact_repo.clone(), rule_repo)
                .with_static_fact_limit(config.cognitive.static_fact_limit)
                .with_confidence_threshold(Arc::clone(&confidence_bits))
                .with_recall_registry(recall_registry);
        sources.push(Box::new(cog_source));

        // Project context source — injects project instructions, role, and memories.
        sources.push(Box::new(crate::context_sources::ProjectContextSource::new(
            repos.clone(),
            fact_repo.clone(),
        )));

        // Annotation context source — injects critical annotations into prompt.
        let annotation_repo = cognitive::AnnotationRepo::new(pool.clone());
        sources.push(Box::new(
            crate::context_sources::AnnotationContextSource::new(annotation_repo.clone()),
        ));

        // Start background consolidation service if we have a DomainEventBus
        if let Some(ref domain_bus) = domain_event_bus {
            let event_rx = domain_bus.subscribe();
            let (extraction, consolidation): (
                Arc<dyn cognitive::ExtractionHandler>,
                Arc<dyn cognitive::ConsolidationHandler>,
            ) = if let Some(ref cp) = cognitive_provider {
                let params = providers::cognitive_chat_params(config, 1024);
                (
                    Arc::new(
                        crate::adapters::cognitive_handlers::LlmExtractionHandler::new(
                            cp.clone(),
                            params.clone(),
                        ),
                    ),
                    Arc::new(
                        crate::adapters::cognitive_handlers::LlmConsolidationHandler::new(
                            cp.clone(),
                            params,
                        ),
                    ),
                )
            } else {
                (
                    Arc::new(
                        crate::adapters::cognitive_handlers::HeuristicExtractionHandler,
                    ),
                    Arc::new(
                        crate::adapters::cognitive_handlers::HeuristicConsolidationHandler,
                    ),
                )
            };
            let episodic_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
            let failed_obs_repo = cognitive::FailedObservationRepo::new(pool.clone());
            let cancel = CancellationToken::new();
            let (signal_tx, signal_rx) = cognitive::pipeline::signal_queue(256);
            let bg_service = cognitive::background::BackgroundConsolidationService::start(
                cognitive::background::BackgroundServiceConfig {
                    event_rx,
                    extraction,
                    consolidation,
                    repo: fact_repo,
                    episodic_repo: Some(episodic_repo),
                    embedder: cognitive_embedder_local,
                    cancel: cancel.clone(),
                    pipeline_tx: pipeline_tx.take(),
                    failed_obs_repo: Some(failed_obs_repo),
                    domain_bus: domain_event_bus.clone(),
                    context_update_queue: context_update_queue.clone(),
                    session_repo: Some(storage::SessionRepo::new(pool.clone())),
                    rule_repo: Some(cognitive::repos::ProceduralRuleRepo::new(
                        pool.clone(),
                    )),
                    signal_tx: Some(signal_tx),
                    signal_rx: Some(signal_rx),
                    session_memory_repo: Some(storage::SessionMemoryRepo::new(
                        pool.clone(),
                    )),
                    intelligence_mode: config.cognitive.intelligence_mode,
                    density_repo: Some(cognitive::ConversationDensityRepo::new(pool.clone())),
                    pending_repo: Some(cognitive::repos::PendingMemoryRepo::new(pool.clone())),
                    deep_handler: cognitive_provider.as_ref().map(|cp| {
                        let params = providers::cognitive_chat_params(config, 4096);
                        Arc::new(
                            crate::adapters::cognitive_handlers::LlmDeepConsolidationHandler::new(
                                cp.clone(),
                                params,
                            ),
                        )
                            as Arc<dyn cognitive::pipeline::DeepConsolidationHandler>
                    }),
                    graph_link_handler: cognitive_provider.as_ref().map(|cp| {
                        let model = config.cognitive.graph_linker_model.clone()
                            .unwrap_or_else(|| config.cognitive.model.clone().unwrap_or_else(|| "default".into()));
                        let params = providers::ChatParams::new(&model)
                            .with_max_tokens(2048)
                            .with_temperature(0.1)
                            .with_response_format(providers::ResponseFormat::JsonObject);
                        Arc::new(
                            crate::adapters::cognitive_handlers::LlmGraphLinkHandler::new(
                                cp.clone(),
                                params,
                            ),
                        )
                            as Arc<dyn cognitive::services::graph_linker::GraphLinkHandler>
                    }),
                    critic_handler: cognitive_provider.as_ref().map(|cp| {
                        let model = config.cognitive.critic_model.clone()
                            .unwrap_or_else(|| config.cognitive.model.clone().unwrap_or_else(|| config.agents.defaults.model.clone()));
                        let params = providers::ChatParams::new(model)
                            .with_max_tokens(1024)
                            .with_temperature(0.0)
                            .with_response_format(providers::ResponseFormat::JsonObject);
                        Arc::new(
                            crate::adapters::cognitive_handlers::LlmExtractionCriticHandler::new(
                                cp.clone(),
                                params,
                            ),
                        )
                            as Arc<dyn cognitive::services::extraction_critic::ExtractionCriticHandler>
                    }),
                    critic_log_repo: Some(cognitive::repos::ExtractionCriticLogRepo::new(
                        storage::StoragePool::from_existing(pool.clone()),
                    )),
                },
            );
            info!("Cognitive background consolidation service started");
            Some(bg_service)
        } else {
            None
        }
    } else {
        None
    };

    // ── Session memory service (per-session scratchpad maintenance) ─────
    let session_memory_service: Option<cognitive::SessionMemoryService> =
        if let (Some(ref pool), Some(ref domain_bus)) = (pool, domain_event_bus) {
            let cancel = CancellationToken::new();
            let svc = cognitive::SessionMemoryService::start(cognitive::SessionMemoryConfig {
                event_rx: domain_bus.subscribe(),
                session_repo: storage::SessionRepo::new(pool.clone()),
                memory_repo: storage::SessionMemoryRepo::new(pool.clone()),
                provider: cognitive_provider.clone(),
                cancel,
            });
            info!("Session memory service started");
            Some(svc)
        } else {
            None
        };

    // Productivity repos (optional — requires real pool + enabled). Stored for
    // reuse by the tool registration block below. NOTE: the ProductivityContextSource
    // itself is registered by ProductivityPlugin (app-core), which arrives via
    // pre_registered_sources — do not push it here or it registers twice.
    let prod_repos = if config.productivity.enabled {
        pool
            .as_ref()
            .map(|pool| feature_productivity::repos::ProductivityRepos::new(pool.clone()))
    } else {
        None
    };

    // Work context source (optional — requires real pool + enabled config).
    let mut inference_loop_token = None;
    if config.work_context.enabled {
        if pool.is_some() {
            sources.push(Box::new(activity_log::WorkContextSource::new(
                storage_pool.clone(),
            )));

            // Start inference engine + background loop
            let text_embedder =
                Arc::new(crate::adapters::cognitive_embedder::TextEmbedderImpl::new(
                    Arc::clone(embedding_engine),
                ));
            let inference_config =
                activity_log::inference::ContextInferenceConfig::from_work_context_config(
                    &config.work_context,
                );
            let engine = Arc::new(activity_log::inference::ContextInferenceEngine::new(
                storage_pool.clone(),
                text_embedder,
                vector_store.clone(),
                inference_config,
            ));

            let token = CancellationToken::new();
            let dormancy_days = config.work_context.max_dormancy_days as i64;
            let _handle = activity_log::inference_loop::ContextInferenceLoop::start(
                Arc::clone(&engine),
                storage_pool.clone(),
                config.work_context.inference_interval_mins,
                dormancy_days,
                token.clone(),
            );
            info!("Work context inference loop started");
            inference_loop_token = Some(token);
        }
    }

    // Append plugin-registered sources
    sources.append(&mut pre_registered_sources);

    // Sort by priority (descending) — ensures correct ordering in prompt
    sources.sort_by_key(|s| std::cmp::Reverse(s.priority()));

    Ok(ContextSourcesResult {
        sources,
        cognitive_fact_repo,
        cognitive_embedder,
        cognitive_retrieval_config,
        cognitive_bg_service,
        session_memory_service,
        prod_repos,
        confidence_bits,
        inference_loop_token,
    })
}
