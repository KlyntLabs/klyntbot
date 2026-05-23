//! Cognitive-memory system builder for the agent loop.
//!
//! Assembles the [`UnifiedMemoryService`], [`InsightForge`], tree-builder
//! subscribers, and entity backfills when a real database pool is available.

use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use bus::DomainEventBus;
use config::Config;
use providers::DynProvider;

use crate::agent_loop::builder::DecomposerLlmAdapter;

/// Result of the cognitive-memory build phase.
pub(crate) struct CognitiveBuildResult {
    pub context_engine: context_engine::ContextEngine,
    pub tree_builder_token: Option<CancellationToken>,
    pub memory_service_for_shadow: Option<Arc<cognitive::UnifiedMemoryService>>,
    pub memory_retriever_for_prf: Option<Arc<dyn context_engine::MemoryRetriever>>,
    pub predictive_cache: Option<Arc<cognitive::services::predictive_cache::PredictiveCache>>,
}

/// Build the cognitive memory system: UnifiedMemoryService, InsightForge,
/// tree builders, and entity backfills.
pub(crate) async fn build_cognitive_system(
    config: &Config,
    context_engine: context_engine::ContextEngine,
    fact_repo: cognitive::SemanticFactRepo,
    cognitive_embedder: Option<Arc<dyn cognitive::SemanticFactEmbedder>>,
    cognitive_retrieval_config: Option<cognitive::CognitiveRetrievalConfig>,
    recall_service: &Option<Arc<cognitive::ConversationRecallService>>,
    pool: &Option<sqlx::SqlitePool>,
    vector_store: &Option<storage::VectorStore>,
    domain_event_bus: &Option<Arc<DomainEventBus>>,
    context_update_queue: &Option<Arc<bus::ContextUpdateQueue>>,
    embedding_engine: &Arc<tools::EmbeddingEngine>,
    skill_store: &Arc<tokio::sync::RwLock<skill_system::SkillStore>>,
    user_situation: &Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
    autotuner: &Option<Arc<crate::autotuner::AutoTunerOrchestrator>>,
    cognitive_provider: &Option<DynProvider>,
    storage_pool: &storage::StoragePool,
    repos: &storage::Repos,
) -> CognitiveBuildResult {
    let mut retriever = cognitive::UnifiedMemoryService::new(fact_repo)
        .with_recall_opt(recall_service.clone())
        .with_embedder_opt(cognitive_embedder);
    if let Some(ref p) = pool {
        retriever =
            retriever.with_episodic_repo(cognitive::EpisodicMemoryRepo::new(p.clone()));
    }
    if let Some(cfg) = cognitive_retrieval_config {
        retriever = retriever.with_config(cfg);
    }
    if let Some(ref sit) = user_situation {
        retriever = retriever.with_situation(Arc::clone(sit));
    }
    // Wire live champion memory params into retrieval
    if let Some(ref orchestrator) = autotuner {
        if let Some(sink) = orchestrator.memory_param_sink() {
            retriever = retriever.with_champion_overrides(sink);
        }
    }
    // Wire co-activation tracking for intelligent scoring
    if let Some(ref p) = pool {
        retriever = retriever
            .with_co_activation_repo(cognitive::CoActivationRepo::new(p.clone()));
    }
    // Wire entity graph for graph-aware retrieval boost
    if let Some(ref p) = pool {
        retriever = retriever.with_entity_repo(cognitive::EntityRepo::new(p.clone()));
    }
    // KCA Track 6: PPR cache
    if let Some(ref p) = pool {
        let ppr_cache = Arc::new(cognitive::services::ppr_retrieval::CachedPprGraph::new(
            cognitive::EntityRepo::new(p.clone()),
            std::time::Duration::from_secs(300),
        ));
        retriever = retriever.with_ppr_cache(ppr_cache);
    }
    // KCA Track 7: predictive cache
    let predictive_cache_inner =
        Arc::new(cognitive::services::predictive_cache::PredictiveCache::new(
            100,
            std::time::Duration::from_secs(
                config.cognitive.predictive_cache.ttl_seconds as u64,
            ),
        ));
    let predictive_cache = Some(predictive_cache_inner.clone());
    retriever = retriever.with_predictive_cache(predictive_cache_inner);
    // KCA Track 13: temporal pruner
    let temporal_pruner: Option<
        Arc<dyn cognitive::services::temporal_pruner::TemporalPrunerHandler>,
    > = cognitive_provider.as_ref().map(|p| {
        let model = config
            .cognitive
            .temporal_prune_model
            .clone()
            .unwrap_or_else(|| {
                config
                    .cognitive
                    .model
                    .clone()
                    .unwrap_or_else(|| config.agents.defaults.model.clone())
            });
        Arc::new(
            crate::adapters::cognitive_handlers::LlmTemporalPrunerHandler::new(
                p.clone(),
                providers::ChatParams::new(&model)
                    .with_max_tokens(512)
                    .with_temperature(0.0)
                    .with_response_format(providers::ResponseFormat::JsonObject),
            ),
        )
            as Arc<dyn cognitive::services::temporal_pruner::TemporalPrunerHandler>
    });
    if let Some(ref pruner) = temporal_pruner {
        retriever = retriever.with_temporal_pruner(pruner.clone());
    }
    let memory_service = Arc::new(retriever);
    let memory_service_for_shadow = Some(Arc::clone(&memory_service));
    let retriever: Arc<dyn context_engine::MemoryRetriever> =
        memory_service as Arc<dyn context_engine::MemoryRetriever>;

    // Create InsightForge with the same retriever
    let forge_config = context_engine::InsightForgeConfig {
        enabled: config.cognitive.insight_forge_enabled,
        max_sub_queries: config.cognitive.insight_forge_max_sub_queries,
        per_source_limit: config.cognitive.insight_forge_per_source_limit,
        total_limit: config.cognitive.insight_forge_total_limit,
        per_source_timeout_ms: config.cognitive.insight_forge_per_source_timeout_ms,
        ..context_engine::InsightForgeConfig::default()
    };
    // Use FallbackDecomposer if a cognitive provider is available
    let decomposer: Arc<dyn context_engine::QueryDecomposer> =
        if let Some(ref cp) = cognitive_provider {
            let llm_adapter = Arc::new(DecomposerLlmAdapter {
                provider: cp.clone(),
                params: providers::cognitive_chat_params(config, 256),
            });
            let llm_decomposer = Arc::new(context_engine::LlmDecomposer::new(llm_adapter));
            Arc::new(context_engine::FallbackDecomposer::new(llm_decomposer, 3))
        } else {
            Arc::new(context_engine::HeuristicDecomposer)
        };
    let mut forge =
        context_engine::InsightForge::new(forge_config, decomposer, Arc::clone(&retriever));

    // Register domain searchers
    forge.add_searcher(Arc::new(crate::domain_searchers::TaskSearcher::new(
        repos.clone(),
    )));

    let mut tree_builder_token = None;

    // NoteTreeNavigator with optional community search (Phase 2)
    if config.cognitive.book_index.enabled {
        let tree_repo: Arc<dyn context_engine::book_index::BookTreeRepo> = Arc::new(
            cognitive::repos::SqliteBookTreeRepo::new(storage_pool.inner().clone()),
        );

        if let Some(ref vs) = vector_store {
            let text_embedder: Arc<dyn cognitive::TextEmbedder> =
                Arc::new(crate::adapters::cognitive_embedder::TextEmbedderImpl::new(
                    Arc::clone(embedding_engine),
                ));
            let tree_node_search = Arc::new(
                crate::adapters::tree_node_search::TreeNodeSearchAdapter::new(
                    Arc::new(vs.clone()),
                    text_embedder.clone(),
                ),
            );

            // Community search adapter (Phase 2)
            let community_repo =
                cognitive::repos::CommunityRepo::new(storage_pool.inner().clone());
            let community_adapter = Arc::new(
                crate::adapters::community_search::CommunitySearchAdapter::new(
                    Arc::new(vs.clone()),
                    community_repo,
                ),
            );

            let note_tree_navigator =
                context_engine::insight_forge::note_tree_navigator::NoteTreeNavigator::new(
                    tree_repo.clone(),
                    tree_node_search,
                    None,
                )
                .with_community_search(community_adapter.clone(), community_adapter);
            forge.add_searcher(Arc::new(note_tree_navigator));

            // Parent cancellation token for all tree builder subscriber tasks.
            // Child tokens are derived below so cancelling this one stops all 8 tasks.
            let tree_builder_parent_token = CancellationToken::new();
            tree_builder_token = Some(tree_builder_parent_token.clone());

            // NoteTreeBuilder subscriber (event-driven tree rebuild + LanceDB embed)
            if let Some(ref domain_bus) = domain_event_bus {
                let note_tree_note_repo =
                    feature_notes::repo::NoteRepo::new(storage_pool.inner().clone());
                let note_tree_builder =
                    Arc::new(crate::adapters::note_tree_builder::NoteTreeBuilder::new(
                        tree_repo.clone(),
                        Arc::new(vs.clone()),
                        text_embedder.clone(),
                        context_update_queue.clone(),
                        domain_event_bus.clone(),
                        note_tree_note_repo,
                    ));
                let tree_builder_rx = domain_bus.subscribe();
                let tree_builder_shutdown = tree_builder_parent_token.child_token();
                let _tree_builder_handle = tokio::spawn({
                    let builder = Arc::clone(&note_tree_builder);
                    let shutdown = tree_builder_shutdown.clone();
                    async move {
                        builder.run(tree_builder_rx, shutdown).await;
                    }
                });
                info!("NoteTreeBuilder subscriber started");

                // TaskTreeBuilder subscriber (event-driven task tree rebuild + LanceDB embed)
                let task_tree_builder =
                    Arc::new(crate::adapters::task_tree_builder::TaskTreeBuilder::new(
                        tree_repo.clone(),
                        Arc::new(vs.clone()),
                        text_embedder.clone(),
                        context_update_queue.clone(),
                        domain_event_bus.clone(),
                        storage_pool.inner().clone(),
                    ));
                let task_tree_rx = domain_bus.subscribe();
                let task_tree_shutdown = tree_builder_parent_token.child_token();
                let _task_tree_handle = tokio::spawn({
                    let builder = Arc::clone(&task_tree_builder);
                    let shutdown = task_tree_shutdown.clone();
                    async move {
                        builder.run(task_tree_rx, shutdown).await;
                    }
                });
                info!("TaskTreeBuilder subscriber started");

                // EntityTreeLinker subscriber (links entities ↔ tree nodes)
                let entity_linker =
                    Arc::new(crate::adapters::entity_tree_linker::EntityTreeLinker::new(
                        storage_pool.inner().clone(),
                    ));
                let linker_rx = domain_bus.subscribe();
                let linker_shutdown = tree_builder_parent_token.child_token();
                let _linker_handle = tokio::spawn({
                    let linker = Arc::clone(&entity_linker);
                    let shutdown = linker_shutdown.clone();
                    async move {
                        linker.run(linker_rx, shutdown).await;
                    }
                });
                info!("EntityTreeLinker subscriber started");

                // CommunityBuilder subscriber (Louvain detection)
                let community_repo_for_builder =
                    cognitive::repos::CommunityRepo::new(storage_pool.inner().clone());
                let community_builder =
                    Arc::new(crate::adapters::community_builder::CommunityBuilder::new(
                        community_repo_for_builder,
                        Arc::new(vs.clone()),
                        text_embedder.clone(),
                        tree_repo.clone(),
                        context_update_queue.clone(),
                    ));
                let comm_builder_rx = domain_bus.subscribe();
                let comm_builder_shutdown = tree_builder_parent_token.child_token();
                let _comm_builder_handle = tokio::spawn({
                    let builder = Arc::clone(&community_builder);
                    let shutdown = comm_builder_shutdown.clone();
                    async move {
                        builder.run(comm_builder_rx, shutdown).await;
                    }
                });
                info!("CommunityBuilder subscriber started");

                // ProductivityTreeBuilder subscriber
                let productivity_tree_builder =
                    Arc::new(crate::adapters::productivity_tree_builder::ProductivityTreeBuilder::new(
                        tree_repo.clone(),
                        Arc::new(vs.clone()),
                        text_embedder.clone(),
                        context_update_queue.clone(),
                        domain_event_bus.clone(),
                    ));
                let productivity_tree_rx = domain_bus.subscribe();
                let productivity_tree_shutdown = tree_builder_parent_token.child_token();
                let _productivity_tree_handle = tokio::spawn({
                    let builder = Arc::clone(&productivity_tree_builder);
                    let shutdown = productivity_tree_shutdown.clone();
                    async move {
                        builder.run(productivity_tree_rx, shutdown).await;
                    }
                });
                info!("ProductivityTreeBuilder subscriber started");

                // OkrTreeBuilder subscriber
                let okr_tree_builder =
                    Arc::new(crate::adapters::okr_tree_builder::OkrTreeBuilder::new(
                        tree_repo.clone(),
                        Arc::new(vs.clone()),
                        text_embedder.clone(),
                        context_update_queue.clone(),
                        domain_event_bus.clone(),
                    ));
                let okr_tree_rx = domain_bus.subscribe();
                let okr_tree_shutdown = tree_builder_parent_token.child_token();
                let _okr_tree_handle = tokio::spawn({
                    let builder = Arc::clone(&okr_tree_builder);
                    let shutdown = okr_tree_shutdown.clone();
                    async move {
                        builder.run(okr_tree_rx, shutdown).await;
                    }
                });
                info!("OkrTreeBuilder subscriber started");

                // LearningTreeBuilder subscriber
                let learning_tree_builder = Arc::new(
                    crate::adapters::learning_tree_builder::LearningTreeBuilder::new(
                        tree_repo.clone(),
                        Arc::new(vs.clone()),
                        text_embedder.clone(),
                        context_update_queue.clone(),
                        domain_event_bus.clone(),
                    ),
                );
                let learning_tree_rx = domain_bus.subscribe();
                let learning_tree_shutdown = tree_builder_parent_token.child_token();
                let _learning_tree_handle = tokio::spawn({
                    let builder = Arc::clone(&learning_tree_builder);
                    let shutdown = learning_tree_shutdown.clone();
                    async move {
                        builder.run(learning_tree_rx, shutdown).await;
                    }
                });
                info!("LearningTreeBuilder subscriber started");

                // Background tree node backfill
                if std::env::var("KLYNTBOT_BACKFILL").is_ok_and(|v| v == "1" || v == "true") {
                    let backfill_builder = Arc::clone(&note_tree_builder);
                    let backfill_task_tree_builder = Arc::clone(&task_tree_builder);
                    let backfill_learning_tree_builder = Arc::clone(&learning_tree_builder);
                    let backfill_productivity_tree_builder =
                        Arc::clone(&productivity_tree_builder);
                    let backfill_note_repo =
                        feature_notes::repo::NoteRepo::new(storage_pool.inner().clone());
                    let backfill_linker = Arc::clone(&entity_linker);
                    let backfill_community_builder = Arc::clone(&community_builder);
                    let backfill_entity_embedder =
                        crate::adapters::entity_embedder::EntityEmbedder::new(
                            Arc::new(vs.clone()),
                            text_embedder.clone(),
                        );
                    let backfill_pool = storage_pool.inner().clone();
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                        if let Err(e) =
                            super::backfill::backfill_tree_nodes(&backfill_note_repo, &backfill_builder)
                                .await
                        {
                            warn!("Note tree backfill error: {e}");
                        }
                        if let Err(e) =
                            super::backfill::backfill_task_trees(&backfill_task_tree_builder).await
                        {
                            warn!("Task tree backfill error: {e}");
                        }
                        match backfill_learning_tree_builder
                            .backfill_from_atoms(&backfill_pool)
                            .await
                        {
                            Ok(0) => {}
                            Ok(n) => info!("Learning tree backfill: {n} atoms indexed"),
                            Err(e) => warn!("Learning tree backfill error: {e}"),
                        }
                        match backfill_productivity_tree_builder
                            .backfill_from_summaries(&backfill_pool)
                            .await
                        {
                            Ok(0) => {}
                            Ok(n) => {
                                info!("Productivity tree backfill: {n} summaries indexed")
                            }
                            Err(e) => warn!("Productivity tree backfill error: {e}"),
                        }
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        super::backfill::backfill_entity_links(&backfill_linker).await;
                        if let Err(e) = backfill_community_builder.rebuild_communities().await {
                            warn!("Community backfill error: {e}");
                        }
                        match backfill_entity_embedder.backfill_all(&backfill_pool).await {
                            Ok(0) => {}
                            Ok(n) => {
                                info!("Entity embedding backfill: {n} entities embedded")
                            }
                            Err(e) => warn!("Entity embedding backfill error: {e}"),
                        }
                    });
                } else {
                    info!(
                        "Tree node backfill skipped. Set KLYNTBOT_BACKFILL=1 to re-index all data on startup."
                    );
                }
            }

            // Build skill trees at startup (non-blocking)
            let tree_repo_for_skills = tree_repo.clone();
            let skill_store_for_trees = Arc::clone(skill_store);
            tokio::spawn(async move {
                let store = skill_store_for_trees.read().await;
                match crate::adapters::book_index_skill_builder::build_all_skill_trees(
                    tree_repo_for_skills.as_ref(),
                    &store,
                )
                .await
                {
                    Ok(checksum) => {
                        tracing::info!("BookIndex: skill trees built (checksum: {checksum})")
                    }
                    Err(e) => tracing::warn!("BookIndex: skill tree build failed: {e}"),
                }
            });
        }
    }

    // One-time entity backfill from pre-existing SPO facts (non-blocking)
    let entity_repo = cognitive::repos::EntityRepo::new(storage_pool.inner().clone());
    tokio::spawn(async move {
        match entity_repo.backfill_from_facts().await {
            Ok(0) => {}
            Ok(n) => tracing::info!("Backfilled {n} entities from SPO facts"),
            Err(e) => tracing::debug!("Entity backfill error (non-fatal): {e}"),
        }
    });

    // One-time entity backfill from note_entity_mentions (non-blocking)
    let entity_repo2 = cognitive::repos::EntityRepo::new(storage_pool.inner().clone());
    tokio::spawn(async move {
        match entity_repo2.backfill_from_note_mentions().await {
            Ok(0) => {}
            Ok(n) => tracing::info!("Backfilled {n} entities from note mentions"),
            Err(e) => tracing::debug!("Note mention backfill error (non-fatal): {e}"),
        }
    });

    // Capture retriever clone for PRF pipeline stage
    let memory_retriever_for_prf = Some(Arc::clone(&retriever));

    // Wire cognitive memory scorer for tiered compression
    let context_engine = if config.cognitive.history_compression.use_cognitive_scoring {
        let scorer = Arc::new(
            crate::adapters::memory_scorer_impl::CognitiveMemoryScorer::new(Arc::clone(
                &retriever,
            )),
        );
        context_engine.with_memory_scorer(scorer)
    } else {
        context_engine
    };

    let context_engine = context_engine
        .with_memory_retriever(retriever)
        .with_insight_forge(forge);

    CognitiveBuildResult {
        context_engine,
        tree_builder_token,
        memory_service_for_shadow,
        memory_retriever_for_prf,
        predictive_cache,
    }
}
