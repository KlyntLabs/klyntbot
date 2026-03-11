//! Agent loop construction: tool registration, handler wiring, pipeline assembly.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use bus::MessageBus;
use common::Result;
use config::Config;
use context_engine::ContextSource;
use providers::DynProvider;
use session::SessionManager;
use tools::{
    area_tool::AreaTool,
    browser::BrowserTool,
    cron_tool::CronTool,
    filesystem::register_fs_tools,
    learning_tool::{LearningHandler, LearningTool},
    message::MessageTool,
    okr_tool::OkrTool,
    project_tool::ProjectTool,
    registry::ToolRegistry,
    spawn::SpawnTool,
    web::{WebFetchTool, WebSearchTool},
};
use tools_core::FeaturePackage;

use super::super::confidence::ConfidenceEvaluator;
use super::super::context_sources::{
    AgentContextSource, AreaSource, BootstrapSource, IdentitySource, PageContextSource,
    PersonaContextSource, ProductivityContextSource, ProjectContextSource, TodoSource,
};
use super::super::{CronHandlerAdapter, SubagentManager};
use super::{AgentLoop, LastActiveChannel};

/// Builder for constructing an [`AgentLoop`] with all its dependencies.
///
/// Required fields (`bus`, `provider`, `config`) are constructor params.
/// Optional: `pool` (enables feature-todo, finance), `cron_service`, `notification_handle`.
///
/// # Example
/// ```ignore
/// let agent = AgentLoop::builder(bus, provider, config)
///     .with_pool(pool)
///     .build()
///     .await?;
/// ```
pub struct AgentLoopBuilder {
    bus: Arc<MessageBus>,
    provider: DynProvider,
    config: Config,
    pool: Option<sqlx::SqlitePool>,
    vector_store: Option<storage::VectorStore>,
    cron_service: Option<Arc<scheduling::CronService>>,
    notification_handle: Option<LastActiveChannel>,
    notification_sender: Option<Arc<dyn common::utils::notify::NotificationSender>>,
    domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    cognitive_provider: Option<DynProvider>,
    pipeline_tx: Option<tokio::sync::broadcast::Sender<cognitive::PipelineEvent>>,
    user_situation: Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
    activity_svc: Option<Arc<activity_log::ActivityIngestionService>>,
}

impl AgentLoopBuilder {
    pub fn new(bus: Arc<MessageBus>, provider: DynProvider, config: Config) -> Self {
        Self {
            bus,
            provider,
            config,
            pool: None,
            vector_store: None,
            cron_service: None,
            notification_handle: None,
            notification_sender: None,
            domain_event_bus: None,
            cognitive_provider: None,
            pipeline_tx: None,
            user_situation: None,
            activity_svc: None,
        }
    }

    pub fn with_pool(mut self, pool: sqlx::SqlitePool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn with_vector_store(mut self, store: storage::VectorStore) -> Self {
        self.vector_store = Some(store);
        self
    }

    pub fn with_cron_service(mut self, service: Arc<scheduling::CronService>) -> Self {
        self.cron_service = Some(service);
        self
    }

    pub fn with_notification_handle(mut self, handle: LastActiveChannel) -> Self {
        self.notification_handle = Some(handle);
        self
    }

    pub fn with_notification_sender(
        mut self,
        sender: Arc<dyn common::utils::notify::NotificationSender>,
    ) -> Self {
        self.notification_sender = Some(sender);
        self
    }

    pub fn with_domain_bus(mut self, bus: Arc<bus::DomainEventBus>) -> Self {
        self.domain_event_bus = Some(bus);
        self
    }

    pub fn with_cognitive_provider(mut self, provider: Option<DynProvider>) -> Self {
        self.cognitive_provider = provider;
        self
    }

    pub fn with_pipeline_tx(
        mut self,
        tx: tokio::sync::broadcast::Sender<cognitive::PipelineEvent>,
    ) -> Self {
        self.pipeline_tx = Some(tx);
        self
    }

    pub fn with_user_situation(
        mut self,
        situation: Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>,
    ) -> Self {
        self.user_situation = Some(situation);
        self
    }

    pub fn with_activity_service(
        mut self,
        svc: Arc<activity_log::ActivityIngestionService>,
    ) -> Self {
        self.activity_svc = Some(svc);
        self
    }

    /// Consume the builder and construct an [`AgentLoop`].
    pub async fn build(mut self) -> Result<AgentLoop> {
        let bus = self.bus;
        let provider = self.provider;
        let config = self.config;

        let workspace = config.workspace_path();

        // Ensure workspace directory exists so fs tools work correctly
        tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
            common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                "Failed to create workspace directory {}: {}",
                workspace.display(),
                e
            )))
        })?;

        // ── Create repos from pool (real or in-memory fallback) ──────────
        // Storage-dependent features (todo, finance, sessions) are disabled via
        // the `if self.pool.is_some()` guards below when no real pool is provided.
        let storage_pool = if let Some(pool) = self.pool.clone() {
            storage::StoragePool::from_existing(pool)
        } else {
            storage::StoragePool::connect_in_memory()
                .await
                .map_err(|e| {
                    common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                        "Failed to create in-memory SQLite fallback: {}",
                        e
                    )))
                })?
        };
        let repos = storage::Repos::from_pool(&storage_pool);

        // ── Agent profiles ─────────────────────────────────────────────────
        let mut agent_manager = crate::agent_profile::AgentManager::new();
        agent_manager.load_builtin_agents()?;
        if let Some(data_dir) = &config.data_dir {
            agent_manager
                .load_workspace_agents(std::path::Path::new(data_dir))
                .await
                .ok(); // non-fatal
        }
        let agent_manager = Arc::new(agent_manager);

        // Shared active profile — written by AgentRuntime, read by AgentContextSource
        let active_profile: Arc<
            tokio::sync::RwLock<Option<Arc<crate::agent_profile::AgentProfile>>>,
        > = Arc::new(tokio::sync::RwLock::new(None));

        // ── Shared embedding engine ──
        let embedding_engine = Arc::new(tools::EmbeddingEngine::new());

        // ── Context sources ───────────────────────────────────────────────
        let confidence_bits = Arc::new(std::sync::atomic::AtomicU32::new(
            config.confidence.threshold.to_bits(),
        ));

        // Load persona manager for PersonaContextSource
        let personas_dir = workspace.join("personas");
        let persona_manager = Arc::new(tokio::sync::RwLock::new(
            crate::persona::PersonaManager::load(&personas_dir).await,
        ));

        let mut sources: Vec<Box<dyn ContextSource>> = vec![
            Box::new(IdentitySource::new(
                workspace.clone(),
                config.timezone.clone(),
            )),
            Box::new(BootstrapSource::new(workspace.clone())),
            Box::new(AreaSource::new(repos.areas.clone())),
            Box::new(TodoSource::new(repos.actions.clone())),
            Box::new(AgentContextSource::new(Arc::clone(&active_profile))),
            Box::new(PersonaContextSource::new(
                Arc::clone(&persona_manager),
                repos.session_context.clone(),
            )),
            Box::new(PageContextSource::new(repos.clone())),
        ];

        // Cognitive context source (optional — requires real pool).
        // These are hoisted so UnifiedMemoryService can use them outside the block.
        let mut cognitive_fact_repo: Option<cognitive::SemanticFactRepo> = None;
        let mut cognitive_embedder: Option<Arc<dyn cognitive::SemanticFactEmbedder>> = None;
        let mut cognitive_retrieval_config: Option<cognitive::CognitiveRetrievalConfig> = None;

        let cognitive_bg_service: Option<cognitive::background::BackgroundConsolidationService> =
            if let Some(ref pool) = self.pool {
                // Run cognitive feature migrations (idempotent)
                storage::StoragePool::run_feature_migrations(
                    pool,
                    &cognitive::cognitive_migrations(),
                )
                .await
                .map_err(|e| {
                    common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                        "Failed to run cognitive migrations: {}",
                        e
                    )))
                })?;

                let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
                let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());

                // Create SemanticFactEmbedder if embedding engine + vector store available
                let cognitive_embedder_local: Option<Arc<dyn cognitive::SemanticFactEmbedder>> =
                    if let Some(ref vs) = self.vector_store {
                        Some(Arc::new(
                            crate::cognitive_embedder::SemanticFactEmbedderImpl::new(
                                Arc::clone(&embedding_engine),
                                vs.clone(),
                            ),
                        ))
                    } else {
                        None
                    };

                // Build retrieval config from app config
                let retrieval_config = cognitive::CognitiveRetrievalConfig {
                    dynamic_facts_enabled: config.cognitive.dynamic_facts_enabled,
                    static_fact_limit: config.cognitive.static_fact_limit,
                    dynamic_fact_limit: config.cognitive.dynamic_fact_limit,
                    vector_top_k: config.cognitive.vector_top_k,
                    min_similarity: config.cognitive.min_similarity,
                    max_stability: config.cognitive.max_stability,
                    relevance_weight_semantic: config.cognitive.relevance_weight_semantic,
                    relevance_weight_retrievability: config
                        .cognitive
                        .relevance_weight_retrievability,
                    relevance_weight_importance: config.cognitive.relevance_weight_importance,
                    relevance_weight_frequency: config.cognitive.relevance_weight_frequency,
                    relevance_weight_situation: config.cognitive.relevance_weight_situation,
                };

                // Hoist for UnifiedMemoryService wiring below
                cognitive_fact_repo = Some(fact_repo.clone());
                cognitive_embedder = cognitive_embedder_local.clone();
                cognitive_retrieval_config = Some(retrieval_config);

                let cog_source =
                    cognitive::CognitiveContextSource::new(fact_repo.clone(), rule_repo)
                        .with_static_fact_limit(config.cognitive.static_fact_limit)
                        .with_confidence_threshold(Arc::clone(&confidence_bits));
                sources.push(Box::new(cog_source));

                // Project context source — injects project instructions, role, and memories.
                sources.push(Box::new(ProjectContextSource::new(
                    repos.clone(),
                    fact_repo.clone(),
                )));

                // Annotation context source — injects critical annotations into prompt.
                let annotation_repo = cognitive::AnnotationRepo::new(pool.clone());
                sources.push(Box::new(
                    crate::context_sources::AnnotationContextSource::new(annotation_repo.clone()),
                ));

                // Start background consolidation service if we have a DomainEventBus
                if let Some(ref domain_bus) = self.domain_event_bus {
                    let event_rx = domain_bus.subscribe();
                    let (extraction, consolidation): (
                        Arc<dyn cognitive::ExtractionHandler>,
                        Arc<dyn cognitive::ConsolidationHandler>,
                    ) = if let Some(ref cp) = self.cognitive_provider {
                        let params = providers::cognitive_chat_params(&config, 1024);
                        (
                            Arc::new(crate::cognitive_handlers::LlmExtractionHandler::new(
                                cp.clone(),
                                params.clone(),
                            )),
                            Arc::new(crate::cognitive_handlers::LlmConsolidationHandler::new(
                                cp.clone(),
                                params,
                            )),
                        )
                    } else {
                        (
                            Arc::new(crate::cognitive_handlers::HeuristicExtractionHandler),
                            Arc::new(crate::cognitive_handlers::HeuristicConsolidationHandler),
                        )
                    };
                    let episodic_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                    let accum_repo = cognitive::AccumulatedObservationRepo::new(pool.clone());
                    let failed_obs_repo = cognitive::FailedObservationRepo::new(pool.clone());
                    let cancel = CancellationToken::new();
                    let bg_service = cognitive::background::BackgroundConsolidationService::start(
                        event_rx,
                        extraction,
                        consolidation,
                        fact_repo,
                        Some(episodic_repo),
                        cognitive_embedder_local,
                        cancel.clone(),
                        self.pipeline_tx.take(),
                        Some(accum_repo),
                        Some(failed_obs_repo),
                        config.cognitive.accumulate_promote_threshold,
                        config.cognitive.accumulate_min_days,
                    );
                    info!("Cognitive background consolidation service started");
                    Some(bg_service)
                } else {
                    None
                }
            } else {
                None
            };

        // Productivity context source (optional — requires real pool + enabled).
        // prod_repos is stored for reuse by the tool registration block below.
        let prod_repos = if config.productivity.enabled {
            self.pool
                .as_ref()
                .map(|pool| feature_productivity::repos::ProductivityRepos::new(pool.clone()))
        } else {
            None
        };
        if let Some(ref repos) = prod_repos {
            sources.push(Box::new(ProductivityContextSource::new(repos.clone())));
        }

        // Work context source (optional — requires real pool + enabled config).
        let _inference_loop_token: Option<CancellationToken> = if config.work_context.enabled {
            if let Some(ref _pool) = self.pool {
                sources.push(Box::new(activity_log::WorkContextSource::new(
                    storage_pool.clone(),
                )));

                // Start inference engine + background loop
                let text_embedder = Arc::new(crate::cognitive_embedder::TextEmbedderImpl::new(
                    Arc::clone(&embedding_engine),
                ));
                let inference_config =
                    activity_log::inference::ContextInferenceConfig::from_work_context_config(
                        &config.work_context,
                    );
                let engine = Arc::new(activity_log::inference::ContextInferenceEngine::new(
                    storage_pool.clone(),
                    text_embedder,
                    self.vector_store.clone(),
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

                Some(token)
            } else {
                None
            }
        } else {
            None
        };

        // Sort by priority (descending) — ensures correct ordering in prompt
        sources.sort_by_key(|s| std::cmp::Reverse(s.priority()));

        let summary_provider = Arc::new(crate::llm_summary_provider::LlmSummaryProvider::new(
            provider.clone(),
            config.agents.defaults.model.clone(),
        ));
        let context_engine = context_engine::ContextEngine::new()
            .with_sources(sources)
            .with_token_counter(context_engine::best_token_counter())
            .with_summary_provider(summary_provider);

        // ── Session manager (SQL-backed) ──────────────────────────────────
        let session_manager = SessionManager::from_repo(
            storage::SessionRepo::new(storage_pool.inner().clone()),
            config.conversation.session.max_cache_size,
        )
        .await;

        // ── Subagent manager ──────────────────────────────────────────────
        let brave_api_key = (!config.tools.web.brave_api_key.is_empty())
            .then(|| config.tools.web.brave_api_key.expose().clone());

        let subagent_manager = Arc::new(
            SubagentManager::builder(Arc::clone(&provider), workspace.clone())
                .inbound_sender(bus.inbound_sender())
                .model(config.agents.defaults.model.clone())
                .brave_api_key(brave_api_key.clone())
                .web_max_results(config.tools.web.max_results)
                .restrict_to_workspace(config.tools.restrict_to_workspace)
                .max_concurrent_subagents(config.agents.defaults.max_concurrent_subagents)
                .agent_task_repo(repos.agent_tasks.clone())
                .build(),
        );

        // ── Tool registry ─────────────────────────────────────────────────
        let mut tool_registry = ToolRegistry::new();

        // Filesystem tools
        let allowed_dir = if config.tools.restrict_to_workspace {
            Some(workspace.clone())
        } else {
            None
        };
        register_fs_tools(&mut tool_registry, allowed_dir.clone());

        // Search tools
        tool_registry.register(tools::grep::GrepTool::new(allowed_dir.clone()));
        tool_registry.register(tools::glob_tool::GlobTool::new(allowed_dir.clone()));

        // Web tools
        tool_registry.register(WebSearchTool::new(
            brave_api_key,
            config.tools.web.max_results,
        ));
        tool_registry.register(WebFetchTool::new());

        if config.tools.browser.enabled {
            match BrowserTool::new(config.tools.browser.trust_level.clone()) {
                Ok(tool) => {
                    tool_registry.register(tool);
                    info!("Browser tool registered");
                }
                Err(e) => {
                    warn!("Browser tool unavailable: {}", e);
                }
            }
        }

        // Message tool
        tool_registry.register(MessageTool::new(bus.outbound_sender()));

        // Ask-user tool
        tool_registry.register(tools::ask_user::AskUserTool);

        // Spawn tool
        tool_registry.register(SpawnTool::with_handler(
            Arc::clone(&subagent_manager) as Arc<dyn tools::spawn::SpawnHandler>
        ));

        // Cron tool (optional)
        let cron_handler: Option<Arc<dyn tools::cron_tool::CronHandler>> =
            if let Some(cron_svc) = self.cron_service {
                let adapter: Arc<dyn tools::cron_tool::CronHandler> =
                    Arc::new(CronHandlerAdapter::new(cron_svc));
                tool_registry.register(CronTool::with_handler(Arc::clone(&adapter)));
                Some(adapter)
            } else {
                None
            };

        // Shared references — SqlitePool is Clone+Send+Sync via Arc internally

        // ── Notification dispatcher ───────────────────────────────────────
        let notification_dispatcher = if !config.todo.notifications.targets.is_empty() {
            Some(Arc::new(match &self.notification_sender {
                Some(sender) => super::super::NotificationDispatcher::with_sender(
                    bus.outbound_sender(),
                    config.todo.notifications.clone(),
                    Arc::clone(sender),
                ),
                None => super::super::NotificationDispatcher::new(
                    bus.outbound_sender(),
                    config.todo.notifications.clone(),
                ),
            }))
        } else {
            None
        };

        // ── Learning: outcome store ────────────────────────────
        let outcome_store = if config.learning.enabled {
            Some(Arc::new(RwLock::new(crate::learning::OutcomeStore::new(
                repos.outcomes.clone(),
            ))))
        } else {
            None
        };

        // ── Create ConversationRecallService (shared by retriever + handler) ──
        let recall_service: Option<Arc<cognitive::ConversationRecallService>> =
            if let (true, Some(ref vs)) = (
                config.conversation.embedding.enabled,
                self.vector_store.clone(),
            ) {
                let text_embedder = Arc::new(crate::cognitive_embedder::TextEmbedderImpl::new(
                    Arc::clone(&embedding_engine),
                ));
                Some(Arc::new(cognitive::ConversationRecallService::new(
                    vs.clone(),
                    text_embedder,
                    cognitive::RecallConfig {
                        decay_half_life_days: config.conversation.memory.decay_half_life_days
                            as f64,
                        default_threshold: config.conversation.search.semantic_threshold as f32,
                        ..cognitive::RecallConfig::default()
                    },
                )))
            } else {
                None
            };

        // ── Wire automatic memory retrieval (UnifiedMemoryService) ───
        let context_engine = if let Some(fact_repo) = cognitive_fact_repo {
            let mut retriever = cognitive::UnifiedMemoryService::new(fact_repo)
                .with_recall_opt(recall_service.clone())
                .with_embedder_opt(cognitive_embedder);
            if let Some(cfg) = cognitive_retrieval_config {
                retriever = retriever.with_config(cfg);
            }
            if let Some(ref sit) = self.user_situation {
                retriever = retriever.with_situation(Arc::clone(sit));
            }
            context_engine.with_memory_retriever(Arc::new(retriever))
        } else {
            context_engine
        };
        let context_engine = Arc::new(context_engine);

        // Outputs for MemoryTool — populated inside the pool block if embedding is enabled
        let todo_embedding_handler: Option<Arc<dyn tools::EmbeddingHandler>>;

        // ── Feature-todo tool (requires real pool) ────────────────────────
        if self.pool.is_some() {
            let pool_ref = storage_pool.inner();
            let feature_todo_repo = feature_todo::ActionRepo::new(pool_ref.clone());
            let mut todo_tool = feature_todo::TaskTool::new(
                feature_todo_repo,
                config.todo.focus.max_slots,
                config.todo.focus.deadline_hours,
                config.timezone.clone(),
            );

            // Set enrichment threshold from config
            todo_tool =
                todo_tool.with_enrichment_threshold(config.todo.enrichment.auto_apply_threshold);

            // Enrichment engine
            if config.todo.enrichment.enabled {
                let mut enrichment_engine =
                    super::super::enrichment::EnrichmentEngine::new(config.todo.enrichment.clone());
                if config.todo.enrichment.use_llm {
                    enrichment_engine = enrichment_engine
                        .with_provider(provider.clone(), config.agents.defaults.model.clone());
                }
                let enrichment_engine = Arc::new(enrichment_engine);
                todo_tool =
                    todo_tool
                        .with_enrichment_handler(Arc::clone(&enrichment_engine)
                            as Arc<dyn feature_todo::EnrichmentHandler>);
            }

            // Todo embedding (semantic search)
            if let (true, Some(vs)) = (config.todo.search.enabled, self.vector_store.clone()) {
                let todo_embed_impl = Arc::new(
                    crate::todo_embedding_handler::TodoEmbeddingHandlerImpl::new(
                        Arc::clone(&embedding_engine),
                        vs.clone(),
                    ),
                );

                let memory_embed_impl = Arc::new(tools::EmbeddingEngineImpl::new(
                    Arc::clone(&embedding_engine),
                    vs.clone(),
                ));

                todo_tool = todo_tool
                    .with_embedding_handler(
                        Arc::clone(&todo_embed_impl) as Arc<dyn feature_todo::EmbeddingHandler>
                    )
                    .with_embedding_store(vs)
                    .with_search_config(
                        config.todo.search.semantic_threshold,
                        config.todo.search.rrf_k,
                    );

                todo_embedding_handler =
                    Some(Arc::clone(&memory_embed_impl) as Arc<dyn tools::EmbeddingHandler>);
            } else {
                todo_embedding_handler = None;
            }

            // Inject progress handler for KR→Objective cascade
            let progress_handler: Arc<dyn tools_core::ProgressHandler> =
                Arc::new(super::super::progress_handler::ProgressHandlerImpl::new(
                    repos.key_results.clone(),
                    repos.objectives.clone(),
                    repos.actions.clone(),
                ));
            todo_tool = todo_tool.with_progress_handler(Arc::clone(&progress_handler));

            // Wire DomainEventBus for task lifecycle events
            if let Some(ref domain_bus) = self.domain_event_bus {
                todo_tool = todo_tool.with_domain_bus(Arc::clone(domain_bus));
            }

            tool_registry.register(todo_tool);

            // ── OKR tool (needs same progress handler) ────────────────────
            tool_registry.register(
                OkrTool::new(repos.objectives.clone(), repos.key_results.clone())
                    .with_progress_handler(Arc::clone(&progress_handler)),
            );
        } else {
            // No pool available (e.g., test environment)
            todo_embedding_handler = None;

            // OKR tool without progress handler
            tool_registry.register(OkrTool::new(
                repos.objectives.clone(),
                repos.key_results.clone(),
            ));
        }

        // ── Area tool ───────────────────────────────────────────────────
        tool_registry.register(AreaTool::new(repos.areas.clone()));

        // ── Project tool ────────────────────────────────────────────────
        tool_registry.register(ProjectTool::new(
            repos.projects.clone(),
            repos.actions.clone(),
        ));
        // ── Annotate tool ──────────────────────────────────────────────────
        tool_registry.register(tools::AnnotateTool::new(cognitive::AnnotationRepo::new(
            storage_pool.inner().clone(),
        )));

        // ── Conversation recall handler ──────────────────────────────────
        let conversation_recall_handler: Option<Arc<dyn tools::ConversationRecallHandler>> =
            recall_service.as_ref().map(|service| {
                Arc::new(
                    super::super::conversation_recall_handler::ConversationRecallHandlerImpl::new(
                        Arc::clone(service),
                    ),
                ) as Arc<dyn tools::ConversationRecallHandler>
            });

        // ── Memory tool ───────────────────────────────────────────────────
        if config.conversation.search.enabled {
            if let Some(ref handler) = conversation_recall_handler {
                let mut memory_tool = tools::MemoryTool::new()
                    .with_conversation_handler(Arc::clone(handler))
                    .with_todo_repo(repos.actions.clone())
                    .with_threshold(config.conversation.search.semantic_threshold)
                    .with_rrf_k(config.todo.search.rrf_k);

                if let (Some(ref h), Some(ref vs)) = (&todo_embedding_handler, &self.vector_store) {
                    memory_tool = memory_tool
                        .with_todo_embedding_handler(Arc::clone(h))
                        .with_embedding_store(vs.clone());
                }

                tool_registry.register(memory_tool);
            }
        }

        // ── Finance tool (requires real pool) ─────────────────────────────
        if let Some(pool) = &self.pool {
            if config.finance.enabled {
                let price_service = feature_finance::PriceService::new(
                    config.finance.price_refresh.cache_ttl_minutes,
                );

                let finance_handler_impl =
                    Arc::new(crate::finance_adapter::FinanceHandlerImpl::new(
                        repos.clone(),
                        price_service.clone(),
                        config.finance.clone(),
                    ));

                let finance_storage = storage::FinanceStorage::from_pool(pool);
                let mut finance_tool = feature_finance::FinanceTool::new(
                    finance_storage,
                    price_service,
                    config.finance.default_currency.clone(),
                )
                .with_finance_handler(
                    Arc::clone(&finance_handler_impl) as Arc<dyn feature_finance::FinanceHandler>
                );

                // Wire DomainEventBus for transaction and budget events
                if let Some(ref domain_bus) = self.domain_event_bus {
                    finance_tool = finance_tool.with_domain_bus(Arc::clone(domain_bus));
                }

                tool_registry.register(finance_tool);
            }
        }

        // ── Notes tool (requires real pool) ──────────────────────────────────
        // Notes migrations are already run by AppCore::init() — skip here to avoid
        // redundant SQL queries on boot (~50ms savings).
        if let Some(pool) = &self.pool {
            let note_repo = feature_notes::repo::NoteRepo::new(pool.clone());
            tool_registry.register(feature_notes::tool::NotesTool::new(note_repo));
            info!("Notes tool registered");
        }

        // ── Work context tool (requires real pool + enabled) ─────────────
        if config.work_context.enabled && self.pool.is_some() {
            tool_registry.register(activity_log::WorkContextTool::new(storage_pool.clone()));
        }

        // ── Productivity tool (reuses prod_repos from context source block) ──
        if let Some(prod_repos) = prod_repos {
            if let Some(pool) = &self.pool {
                // Run feature migrations (idempotent — skips already-applied)
                storage::StoragePool::run_feature_migrations(
                    pool,
                    &feature_productivity::ProductivityFeature::migrations_static(),
                )
                .await
                .map_err(|e| {
                    common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                        "Failed to run productivity migrations: {}",
                        e
                    )))
                })?;
            }

            let focus_mgr = Arc::new(feature_productivity::FocusManager::new(
                prod_repos.clone(),
                config.productivity.focus.clone(),
            ));
            let prod_handler = Arc::new(crate::productivity_handler::ProductivityHandlerImpl::new(
                provider.clone(),
                config.agents.defaults.model.clone(),
            ));
            let mut daily_agg = feature_productivity::DailyAggregator::new(prod_repos.clone())
                .with_handler(prod_handler);
            if let Some(ref domain_bus) = self.domain_event_bus {
                daily_agg = daily_agg.with_domain_bus(Arc::clone(domain_bus));
            }
            let aggregator = Arc::new(daily_agg);
            let productivity_tool =
                feature_productivity::ProductivityTool::new(prod_repos, focus_mgr, aggregator);
            tool_registry.register(productivity_tool);
        }

        // ── Plugin tools (WASM) ───────────────────────────────────────────
        if config.plugins.enabled {
            let plugins_dir = config.data_dir_path().join("plugins");
            match plugin_runtime::PluginManager::load_all(
                &plugins_dir,
                storage_pool.inner().clone(),
                &config.plugins,
                Some(bus.outbound_sender()),
            ) {
                Ok(plugin_manager) => {
                    let loaded_count = plugin_manager.packages().len();
                    for package in plugin_manager.into_packages() {
                        // Register plugin cron jobs
                        if let Some(ref handler) = cron_handler {
                            for cron_job in &package.manifest().cron_jobs {
                                let params = tools::cron_tool::AddCronJobParams {
                                    name: format!("plugin:{}:{}", package.name(), cron_job.tool),
                                    schedule: tools::cron_tool::CronSchedule::Cron {
                                        expr: cron_job.schedule.clone(),
                                        tz: None,
                                    },
                                    message: format!("Run plugin tool: {}", cron_job.tool),
                                    enabled: true,
                                    channel: None,
                                    to: None,
                                    internal: true,
                                    origin: "plugin".to_string(),
                                };
                                match handler.add_job(params).await {
                                    Ok(job) => {
                                        info!(
                                            plugin = %package.name(),
                                            job_id = %job.id,
                                            tool = %cron_job.tool,
                                            "registered plugin cron job"
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            plugin = %package.name(),
                                            tool = %cron_job.tool,
                                            error = %e,
                                            "failed to register plugin cron job"
                                        );
                                    }
                                }
                            }
                        }

                        // Register plugin tools
                        for tool in package.tools() {
                            tool_registry.register_dyn(tool);
                        }
                    }
                    if loaded_count > 0 {
                        info!(count = loaded_count, "WASM plugin tools registered");
                    }
                }
                Err(e) => {
                    warn!(error = %e, "failed to load WASM plugins");
                }
            }
        }

        // ── MCP tools (Model Context Protocol) ──────────────────────────
        let mcp_manager = if config.mcp.has_active_servers() {
            // connect_all logs startup progress internally via tracing
            let manager = mcp::McpManager::connect_all(&config.mcp, None).await;
            let mcp_tools = manager.tools();
            let tool_count = mcp_tools.len();
            for tool in mcp_tools {
                tool_registry.register_dyn(tool as tools_core::DynTool);
            }
            if tool_count > 0 {
                info!(
                    servers = manager.connected_count(),
                    tools = tool_count,
                    "MCP tools registered"
                );
            }
            Some(manager)
        } else {
            None
        };

        // ── Confidence evaluator ──────────────────────────────────────────
        let confidence_evaluator = if config.confidence.enabled {
            if config.confidence.tool_overrides.is_empty() {
                Some(ConfidenceEvaluator::new(config.confidence.threshold))
            } else {
                let mut tool_map =
                    crate::learning::ToolConfidenceMap::new(config.confidence.threshold);
                for (tool_name, threshold) in &config.confidence.tool_overrides {
                    tool_map.set_threshold(tool_name, *threshold);
                }
                Some(ConfidenceEvaluator::new_with_map(
                    config.confidence.threshold,
                    tool_map,
                ))
            }
        } else {
            None
        };

        // ── Reminder engine ───────────────────────────────────────────────
        let reminder_engine = if let Some(ref dispatcher) = notification_dispatcher {
            let mut engine = super::super::ReminderEngine::new(
                repos.actions.clone(),
                Arc::clone(dispatcher),
                std::time::Duration::from_secs(300),
            );
            engine.start();
            Some(Arc::new(RwLock::new(engine)))
        } else {
            None
        };

        // ── Recurring task spawner ────────────────────────────────────────
        let mut recurring_spawner = super::super::RecurringTaskSpawner::new(
            repos.actions.clone(),
            config.timezone.clone(),
            std::time::Duration::from_secs(60),
        );
        recurring_spawner.start();
        let recurring_task_spawner = Some(Arc::new(RwLock::new(recurring_spawner)));

        // ── Learning service ──────────────────────────────────────────────
        let learning_service = if let Some(ref store) = outcome_store {
            let adaptive = Arc::new(RwLock::new(
                crate::learning::adaptive::AdaptiveThresholds::new(
                    repos.learning_state.clone(),
                    config.confidence.threshold,
                    config.learning.min_threshold,
                    config.learning.max_threshold,
                    config.learning.min_outcomes_for_adaptation,
                )
                .await,
            ));

            // Register LearningTool
            let learning_handler = Arc::new(super::super::LearningHandlerImpl::new(
                repos.strategies.clone(),
                Arc::clone(&adaptive),
            ));
            tool_registry.register(LearningTool::new(Some(
                learning_handler as Arc<dyn LearningHandler>,
            )));

            // Event bus: subscriber updates cognitive confidence threshold
            let event_bus = Arc::new(bus::LearningEventBus::new(16));

            let threshold_for_subscriber = Arc::clone(&confidence_bits);
            let mut event_rx = event_bus.subscribe();
            tokio::spawn(async move {
                while let Ok(event) = event_rx.recv().await {
                    if let bus::LearningEvent::ThresholdChanged { new_threshold, .. } = event {
                        threshold_for_subscriber.store(new_threshold.to_bits(), Ordering::Relaxed);
                        info!(
                            "Confidence threshold updated by LearningService: {:.3}",
                            new_threshold
                        );
                    }
                }
            });

            let threshold_handle = confidence_evaluator.as_ref().map(|e| e.threshold_handle());
            let mut service = crate::learning::LearningService::new(
                Arc::clone(store),
                adaptive,
                threshold_handle,
                Duration::from_secs(config.learning.analysis_interval_secs),
            )
            .with_event_bus(event_bus);
            if let Some(ref domain_bus) = self.domain_event_bus {
                service = service.with_pattern_analyzer(crate::learning::PatternAnalyzer::new(
                    repos.interaction_log.clone(),
                    Arc::clone(domain_bus),
                ));
            }
            service.start();
            Some(Arc::new(RwLock::new(service)))
        } else {
            None
        };

        // ── Bus receiver ──────────────────────────────────────────────────
        let inbound_rx = bus
            .take_inbound_rx()
            .expect("Inbound receiver already taken");

        // ── Outcome recorder (for per-tool learning) ──────────────────────
        let outcome_recorder = outcome_store.as_ref().map(|store| {
            Arc::new(crate::learning::recorder::OutcomeRecorder::new(Arc::clone(
                store,
            )))
        });

        // ── Agent Runtime ─────────────────────────────────────────────────
        let tool_registry = Arc::new(RwLock::new(tool_registry));
        let mut execution_core =
            crate::execution::ExecutionCore::new(provider.clone(), Arc::clone(&tool_registry));
        if let Some(ref recorder) = outcome_recorder {
            execution_core = execution_core.with_outcome_recorder(Arc::clone(recorder));
        }
        if let Some(ref domain_bus) = self.domain_event_bus {
            execution_core = execution_core.with_domain_bus(Arc::clone(domain_bus));
        }
        let execution_core = Arc::new(execution_core);

        let direct_engine =
            crate::intent_pipeline::engines::direct::DirectEngine::new(Arc::clone(&execution_core));
        let reactive_engine = crate::intent_pipeline::engines::reactive::ReactiveEngine::new(
            Arc::clone(&execution_core),
            config.agents.defaults.max_tool_iterations,
        );

        let router =
            crate::intent_pipeline::router::ExecutionRouter::new(direct_engine, reactive_engine);

        let analyzer = crate::intent_pipeline::analysis::IntentAnalyzer::new(
            provider.clone(),
            &config.agents.defaults.model,
            &config.orchestrator,
        )
        .with_strategy_repo(repos.strategies.clone());

        let cost_tracker = Arc::new(
            crate::output::CostTracker::from_repo(storage::UsageRepo::new(
                storage_pool.inner().clone(),
            ))
            .with_monthly_budget(config.agents.monthly_budget_usd),
        );

        let runtime_config = crate::intent_pipeline::types::PipelineConfig {
            execution_model: config.agents.defaults.model.clone(),
            system_prompt: String::new(),
            context_window: provider.context_window(),
            max_response_tokens: config.agents.defaults.max_tokens as usize,
            channel: "unknown".to_string(),
            provider_name: provider.name().to_string(),
        };

        // ── Interaction recorder ──────────────────────────────────────────
        let interaction_recorder = if config.learning.enabled {
            Some(crate::learning::InteractionRecorder::new(
                repos.interaction_log.clone(),
            ))
        } else {
            None
        };

        let mut runtime = crate::agent_runtime::AgentRuntime::new(
            Arc::clone(&agent_manager),
            analyzer,
            Arc::clone(&context_engine),
            router,
            cost_tracker,
            runtime_config,
            Arc::clone(&active_profile),
        )
        .with_strategy_repo(repos.strategies.clone());

        if let Some(evaluator) = confidence_evaluator {
            runtime = runtime.with_confidence_evaluator(Arc::new(evaluator));
        }

        if let Some(recorder) = interaction_recorder {
            runtime = runtime.with_interaction_recorder(recorder);
        }

        // Inject procedural rule repo for transparency (L5 cognitive rules)
        let rule_repo = cognitive::ProceduralRuleRepo::new(storage_pool.inner().clone());
        runtime = runtime.with_procedural_rule_repo(rule_repo);

        // Inject tool registry for delegation support
        runtime = runtime.with_tool_registry(Arc::clone(&tool_registry));

        let runtime = Arc::new(runtime);

        // Two-phase init: set the self-reference for delegation after Arc wrapping
        runtime.set_delegation_self_ref(Arc::clone(&runtime) as Arc<dyn tools::DelegationHandler>);

        info!("Agent runtime initialized");

        // ── Session cleanup service ───────────────────────────────────────
        let session_cleanup_token = if self.pool.is_some() {
            let token = CancellationToken::new();
            let cleanup_service = crate::session_cleanup_service::SessionCleanupService::new(
                storage::SessionRepo::new(storage_pool.inner().clone()),
                config.conversation.session.ttl_days,
                config.conversation.session.cleanup_interval_hours,
                token.clone(),
            );
            cleanup_service.spawn();
            Some(token)
        } else {
            None
        };

        // ── Memory maintenance service ────────────────────────────────────
        let memory_maintenance_token =
            if let (Some(_), Some(vs)) = (&self.pool, self.vector_store.clone()) {
                let token = CancellationToken::new();
                let maintenance_service =
                    crate::memory_maintenance_service::MemoryMaintenanceService::new(
                        vs,
                        config.conversation.memory.max_age_days,
                        config.conversation.memory.maintenance_interval_hours,
                        token.clone(),
                    );
                maintenance_service.spawn();
                Some(token)
            } else {
                None
            };

        // ── Assemble AgentLoop ────────────────────────────────────────────
        let history_limit = config.conversation.session.history_limit;
        Ok(AgentLoop {
            bus,
            inbound_rx: Some(inbound_rx),
            config,
            context_engine,
            session_manager,
            tool_registry,
            running: Arc::new(AtomicBool::new(false)),
            last_active_channel: self.notification_handle,
            reminder_engine,
            recurring_task_spawner,
            _notification_dispatcher: notification_dispatcher,
            conversation_recall_handler,
            learning_service,
            runtime,
            strategy_repo: Some(repos.strategies.clone()),
            history_limit,
            _session_cleanup_token: session_cleanup_token,
            _memory_maintenance_token: memory_maintenance_token,
            mcp_manager: tokio::sync::Mutex::new(mcp_manager),
            _domain_event_bus: self.domain_event_bus,
            cognitive_bg_service: tokio::sync::Mutex::new(cognitive_bg_service),
            _inference_loop_token,
            activity_svc: self.activity_svc,
        })
    }
}

impl AgentLoop {
    /// Create a builder for constructing an `AgentLoop`.
    pub fn builder(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
    ) -> AgentLoopBuilder {
        AgentLoopBuilder::new(bus, provider, config)
    }
}
