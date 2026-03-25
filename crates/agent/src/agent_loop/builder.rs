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
    AreaSource, BootstrapSource, IdentitySource, PageContextSource, PersonaContextSource,
    ProductivityContextSource, ProjectContextSource, SessionContextSource, TodoSource,
};
use super::super::{CronHandlerAdapter, SubagentManager};
use super::{AgentLoop, LastActiveChannel};

/// Builder for constructing an [`AgentLoop`] with all its dependencies.
///
/// Required fields (`bus`, `provider`, `config`) are constructor params.
/// Optional: `pool` (enables feature-tasks, finance), `cron_service`, `notification_handle`.
///
/// # Example
/// ```ignore
/// let agent = AgentLoop::builder(bus, provider, config)
///     .with_pool(pool)
///     .build()
///     .await?;
/// ```
/// Adapter bridging `providers::DynProvider` → `context_engine::DecomposerLlm` trait.
struct DecomposerLlmAdapter {
    provider: DynProvider,
    params: providers::ChatParams,
}

#[async_trait::async_trait]
impl context_engine::DecomposerLlm for DecomposerLlmAdapter {
    async fn generate(&self, prompt: &str) -> std::result::Result<String, String> {
        let messages = vec![providers::Message::User {
            content: providers::UserContent::Text(prompt.to_string()),
        }];
        let response = self
            .provider
            .chat(&messages, None, &self.params)
            .await
            .map_err(|e| e.to_string())?;
        response.content.ok_or_else(|| "empty response".to_string())
    }
}

pub struct AgentLoopBuilder {
    bus: Arc<MessageBus>,
    provider: DynProvider,
    config: Config,
    pool: Option<sqlx::SqlitePool>,
    vector_store: Option<storage::VectorStore>,
    cron_service: Option<Arc<scheduling::CronService>>,
    notification_handle: Option<LastActiveChannel>,
    notification_sender: Option<Arc<dyn common::NotificationSender>>,
    domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    cognitive_provider: Option<DynProvider>,
    pipeline_tx: Option<tokio::sync::broadcast::Sender<cognitive::PipelineEvent>>,
    user_situation: Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
    activity_svc: Option<Arc<activity_log::ActivityIngestionService>>,
    autotuner: Option<Arc<crate::autotuner::AutoTunerOrchestrator>>,
    active_view: Option<Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>>,
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
            autotuner: None,
            active_view: None,
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

    pub fn with_notification_sender(mut self, sender: Arc<dyn common::NotificationSender>) -> Self {
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

    pub fn with_autotuner(
        mut self,
        orchestrator: Arc<crate::autotuner::AutoTunerOrchestrator>,
    ) -> Self {
        self.autotuner = Some(orchestrator);
        self
    }

    pub fn with_active_view(
        mut self,
        view: Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>,
    ) -> Self {
        self.active_view = Some(view);
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

        // ── Skill discovery ─────────────────────────────────────────────────
        let mut discovery_sources = vec![skill_system::discovery::SkillSource::BuiltIn(
            skill_system::discovery::BUILTIN_SKILLS
                .iter()
                .map(|(n, c)| (n.to_string(), c.to_string()))
                .collect(),
        )];
        // Scan user skills directory if data_dir is set
        let data_dir_path = config.data_dir_path();
        let user_skills_dir = data_dir_path.join("skills");
        if user_skills_dir.exists() {
            discovery_sources.push(skill_system::discovery::SkillSource::Directory(
                user_skills_dir,
                skill_system::types::SkillScope::User,
            ));
        }
        // Scan project-level skills if project_root is set
        if let Some(ref project_root) = config.project_root {
            let project_skills = std::path::PathBuf::from(project_root)
                .join(".agents")
                .join("skills");
            if project_skills.exists() {
                discovery_sources.push(skill_system::discovery::SkillSource::Directory(
                    project_skills,
                    skill_system::types::SkillScope::Project,
                ));
            }
        }

        let mut skill_catalog =
            skill_system::types::SkillCatalog::discover(&discovery_sources).await?;
        let skill_router = skill_system::router::SkillRouter::new(&skill_catalog);

        // Build reference files map for SkillContextSource
        let reference_files = Arc::new(skill_system::discovery::builtin_reference_map());

        // Shared active profile — written by AgentRuntime, read by SkillContextSource
        let active_profile: Arc<
            tokio::sync::RwLock<Option<Arc<skill_system::types::SkillPackage>>>,
        > = Arc::new(tokio::sync::RwLock::new(None));

        // Shared activated skills — written per-message, read by SkillContextSource
        let activated_skills: Arc<
            tokio::sync::RwLock<Vec<Arc<skill_system::types::SkillPackage>>>,
        > = Arc::new(tokio::sync::RwLock::new(vec![]));

        // ── Shared embedding engine ──
        let embedding_engine = Arc::new(tools::EmbeddingEngine::new());

        // Precompute skill description embeddings for semantic matching
        {
            let engine_clone = Arc::clone(&embedding_engine);
            let embed_fn: skill_system::types::EmbedFn =
                Arc::new(move |text: &str| engine_clone.embed(text));
            skill_catalog.precompute_embeddings(&embed_fn).await;
        }

        let skill_catalog = Arc::new(tokio::sync::RwLock::new(skill_catalog));
        let skill_router = Arc::new(tokio::sync::RwLock::new(skill_router));

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
            Box::new(SessionContextSource::new(repos.clone())),
            Box::new(AreaSource::new(repos.areas.clone())),
            Box::new(TodoSource::new(repos.tasks.clone())),
            Box::new(skill_system::context::SkillContextSource::new(
                Arc::clone(&active_profile),
                Arc::clone(&activated_skills),
                Arc::clone(&reference_files),
            )),
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
                            crate::adapters::cognitive_embedder::SemanticFactEmbedderImpl::new(
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
                    relevance_weight_temporal: config.cognitive.relevance_weight_temporal,
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
                    let accum_repo = cognitive::AccumulatedObservationRepo::new(pool.clone());
                    let failed_obs_repo = cognitive::FailedObservationRepo::new(pool.clone());
                    let cancel = CancellationToken::new();
                    let bg_service = cognitive::background::BackgroundConsolidationService::start(
                        cognitive::background::BackgroundServiceConfig {
                            event_rx,
                            extraction,
                            consolidation,
                            repo: fact_repo,
                            episodic_repo: Some(episodic_repo),
                            embedder: cognitive_embedder_local,
                            cancel: cancel.clone(),
                            pipeline_tx: self.pipeline_tx.take(),
                            accum_repo: Some(accum_repo),
                            failed_obs_repo: Some(failed_obs_repo),
                            promote_threshold: config.cognitive.accumulate_promote_threshold,
                            min_days: config.cognitive.accumulate_min_days,
                            domain_bus: self.domain_event_bus.clone(),
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
                let text_embedder =
                    Arc::new(crate::adapters::cognitive_embedder::TextEmbedderImpl::new(
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

        let summary_provider = Arc::new(crate::adapters::llm_summary::LlmSummaryProvider::new(
            provider.clone(),
            config.agents.defaults.model.clone(),
        ));
        let context_engine = context_engine::ContextEngine::new()
            .with_sources(sources)
            .with_token_counter(context_engine::token_counter_for_model(
                &config.agents.defaults.model,
            ))
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
            if let Some(ref cron_svc) = self.cron_service {
                let adapter: Arc<dyn tools::cron_tool::CronHandler> =
                    Arc::new(CronHandlerAdapter::new(Arc::clone(cron_svc)));
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
                let text_embedder =
                    Arc::new(crate::adapters::cognitive_embedder::TextEmbedderImpl::new(
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

        // ── Wire memory retrieval + InsightForge ─────────────────────
        let mut memory_service_for_shadow: Option<Arc<cognitive::UnifiedMemoryService>> = None;
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
            // Wire live champion memory params into retrieval
            if let Some(ref orchestrator) = self.autotuner {
                if let Some(sink) = orchestrator.memory_param_sink() {
                    retriever = retriever.with_champion_overrides(sink);
                }
            }
            let memory_service = Arc::new(retriever);
            memory_service_for_shadow = Some(Arc::clone(&memory_service));
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
                if let Some(ref cp) = self.cognitive_provider {
                    let llm_adapter = Arc::new(DecomposerLlmAdapter {
                        provider: cp.clone(),
                        params: providers::cognitive_chat_params(&config, 256),
                    });
                    let llm_decomposer = Arc::new(context_engine::LlmDecomposer::new(llm_adapter));
                    Arc::new(context_engine::FallbackDecomposer::new(llm_decomposer, 3))
                } else {
                    Arc::new(context_engine::HeuristicDecomposer)
                };
            let mut forge =
                context_engine::InsightForge::new(forge_config, decomposer, Arc::clone(&retriever));

            // Register domain searchers
            let note_repo_for_searcher =
                feature_notes::repo::NoteRepo::new(storage_pool.inner().clone());
            forge.add_searcher(Arc::new(crate::domain_searchers::NoteSearcher::new(
                note_repo_for_searcher,
            )));
            forge.add_searcher(Arc::new(crate::domain_searchers::TaskSearcher::new(
                repos.clone(),
            )));
            let entity_repo = cognitive::repos::EntityRepo::new(storage_pool.inner().clone());
            forge.add_searcher(Arc::new(crate::domain_searchers::GraphSearcher::new(
                entity_repo.clone(),
            )));
            forge.add_searcher(Arc::new(crate::domain_searchers::FinanceSearcher::new(
                repos.clone(),
            )));

            // BookRAG integration
            if config.cognitive.book_index.enabled {
                let book_entity_repo =
                    cognitive::repos::EntityRepo::new(storage_pool.inner().clone());
                let tree_repo = Arc::new(cognitive::repos::SqliteBookTreeRepo::new(
                    storage_pool.inner().clone(),
                ));
                let gt_link_repo = Arc::new(cognitive::repos::SqliteGTLinkRepo::new(
                    storage_pool.inner().clone(),
                ));
                let book_index = crate::adapters::book_index_wiring::build_book_index(
                    tree_repo.clone(),
                    book_entity_repo,
                    gt_link_repo.clone(),
                    embedding_engine.clone(),
                );
                let bookrag_searcher = crate::adapters::book_index_wiring::build_bookrag_searcher(
                    book_index.clone(),
                    provider.clone(),
                    &config.cognitive.book_index.retrieval,
                );
                forge.add_searcher(bookrag_searcher);

                // Build entity extractor for GT-Link population
                let entity_extractor = crate::adapters::book_index_wiring::build_entity_extractor(
                    cognitive::repos::EntityRepo::new(storage_pool.inner().clone()),
                    gt_link_repo.clone(),
                    provider.clone(),
                    &config,
                );

                // Start BookIndex updater (listens for NoteContentChanged/NoteDeleted/TaskHierarchyChanged)
                if let Some(ref domain_bus) = self.domain_event_bus {
                    let updater_rx = domain_bus.subscribe();
                    let task_repo_for_updater =
                        storage::TaskRepo::new(storage_pool.inner().clone());
                    let project_repo_for_updater =
                        storage::ProjectRepo::new(storage_pool.inner().clone());
                    let _updater = crate::adapters::book_index_updater::BookIndexUpdater::start(
                        updater_rx,
                        tree_repo.clone(),
                        book_index.clone(),
                        CancellationToken::new(),
                        Some(entity_extractor.clone()),
                        Some(task_repo_for_updater),
                        Some(project_repo_for_updater),
                    );
                    info!("BookIndex updater started");
                }

                // Build skill trees at startup (non-blocking)
                let tree_repo_for_skills = tree_repo.clone();
                tokio::spawn(async move {
                    match crate::adapters::book_index_skill_builder::build_all_skill_trees(
                        tree_repo_for_skills.as_ref(),
                    )
                    .await
                    {
                        Ok(checksum) => {
                            tracing::info!("BookIndex: skill trees built (checksum: {checksum})")
                        }
                        Err(e) => tracing::warn!("BookIndex: skill tree build failed: {e}"),
                    }
                });

                // Backfill existing notes (non-blocking)
                let note_repo_for_backfill =
                    feature_notes::repo::NoteRepo::new(storage_pool.inner().clone());
                let tree_repo_for_backfill = tree_repo.clone();
                let book_index_for_backfill = book_index.clone();
                let extractor_for_backfill = entity_extractor.clone();
                tokio::spawn(async move {
                    match crate::adapters::book_index_backfill::backfill_existing_notes(
                        &note_repo_for_backfill,
                        tree_repo_for_backfill.as_ref(),
                        &book_index_for_backfill,
                        Some(&extractor_for_backfill),
                    )
                    .await
                    {
                        Ok(0) => {}
                        Ok(n) => tracing::info!("BookIndex: backfilled {n} existing notes"),
                        Err(e) => tracing::warn!("BookIndex backfill failed: {e}"),
                    }
                });
            }

            // One-time entity backfill from pre-existing SPO facts (non-blocking)
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

            context_engine
                .with_memory_retriever(retriever)
                .with_insight_forge(forge)
        } else {
            context_engine
        };

        // Phase 2: Wire LLM provider + config model for query rewriting fallback
        let rewriter_provider = self.cognitive_provider.clone();
        let rewriter_model = config.agents.rewriter_model.clone();
        let mut query_rewriter = crate::adapters::query_rewriter::ContextualQueryRewriter::new(
            rewriter_provider,
            rewriter_model,
            800, // 800ms hard cap per spec
        );

        // Phase 3: Wire autotuner champion overrides for rewrite params
        if let Some(ref orchestrator) = self.autotuner {
            if let Some(sink) = orchestrator.memory_param_sink() {
                query_rewriter = query_rewriter.with_champion_overrides(sink);
            }
        }

        let query_rewriter: Arc<dyn context_engine::QueryRewriter> = Arc::new(query_rewriter);
        let context_engine = context_engine.with_query_rewriter(query_rewriter);

        let context_engine = Arc::new(context_engine);

        // Outputs for MemoryTool — populated inside the pool block if embedding is enabled
        let todo_embedding_handler: Option<Arc<dyn tools::EmbeddingHandler>>;

        // ── Feature-tasks tool (requires real pool) ───────────────────────
        if self.pool.is_some() {
            let pool_ref = storage_pool.inner();
            let task_repo = storage::TaskRepo::new(pool_ref.clone());
            let task_repo_for_handlers = task_repo.clone();
            let area_repo = storage::AreaRepo::new(pool_ref.clone());
            let mut task_tool = feature_tasks::TaskTool::new(
                task_repo,
                config.todo.focus.max_slots,
                config.todo.focus.deadline_hours,
                config.timezone.clone(),
            )
            .with_area_repo(area_repo);

            // Set enrichment threshold from config
            task_tool =
                task_tool.with_enrichment_threshold(config.todo.enrichment.auto_apply_threshold);

            // Enrichment engine — directly implements feature_tasks::EnrichmentHandler
            if config.todo.enrichment.enabled {
                let mut enrichment_engine =
                    super::super::enrichment::EnrichmentEngine::new(config.todo.enrichment.clone());
                if config.todo.enrichment.use_llm {
                    enrichment_engine = enrichment_engine
                        .with_provider(provider.clone(), config.agents.defaults.model.clone());
                }
                let enrichment_engine = Arc::new(enrichment_engine);
                task_tool = task_tool.with_enrichment_handler(
                    enrichment_engine as Arc<dyn feature_tasks::EnrichmentHandler>,
                );
            }

            // Task embedding (semantic search)
            if let (true, Some(vs)) = (config.todo.search.enabled, self.vector_store.clone()) {
                let task_embed_impl =
                    Arc::new(crate::adapters::task_embedding::TaskEmbeddingAdapter::new(
                        Arc::clone(&embedding_engine),
                        vs.clone(),
                    ));

                let memory_embed_impl = Arc::new(tools::EmbeddingEngineImpl::new(
                    Arc::clone(&embedding_engine),
                    vs.clone(),
                ));

                task_tool = task_tool
                    .with_embedding_handler(
                        Arc::clone(&task_embed_impl) as Arc<dyn feature_tasks::EmbeddingHandler>
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
                Arc::new(crate::adapters::progress::ProgressHandlerImpl::new(
                    repos.key_results.clone(),
                    repos.objectives.clone(),
                    repos.tasks.clone(),
                ));
            task_tool = task_tool.with_progress_handler(Arc::clone(&progress_handler));

            // Wire DomainEventBus for task lifecycle events
            if let Some(ref domain_bus) = self.domain_event_bus {
                task_tool = task_tool.with_domain_bus(Arc::clone(domain_bus));
            }

            // ── Phase 2: Agentic Intelligence handlers ────────────────────
            let decomp_handler = Arc::new(crate::handlers::LlmDecompositionHandler::new(
                provider.clone(),
                config.agents.defaults.model.clone(),
                task_repo_for_handlers.clone(),
                self.domain_event_bus.clone(),
            ));
            task_tool = task_tool.with_decomposition_handler(
                decomp_handler as Arc<dyn feature_tasks::DecompositionHandler>,
            );

            let exec_handler = Arc::new(crate::handlers::LlmTaskExecutionHandler::new(
                task_repo_for_handlers.clone(),
                self.domain_event_bus.clone(),
            ));
            task_tool = task_tool.with_execution_handler(
                exec_handler as Arc<dyn feature_tasks::TaskExecutionHandler>,
            );

            let plan_handler = Arc::new(crate::handlers::LlmDayPlanningHandler::new(
                provider.clone(),
                config.agents.defaults.model.clone(),
                self.domain_event_bus.clone(),
            ));
            task_tool = task_tool
                .with_planning_handler(plan_handler as Arc<dyn feature_tasks::DayPlanningHandler>);

            // ── Phase 3: Proactive Ecosystem handlers ───────────────────────
            let proactive_handler = Arc::new(crate::handlers::LlmProactiveHandler::new(
                provider.clone(),
                config.agents.defaults.model.clone(),
                task_repo_for_handlers.clone(),
                feature_tasks::TasksConfig::default(),
            ));
            task_tool = task_tool.with_proactive_handler(
                proactive_handler as Arc<dyn feature_tasks::ProactiveHandler>,
            );

            let suggestion_applier = Arc::new(crate::handlers::TaskSuggestionApplier::new(
                task_repo_for_handlers.clone(),
            ));
            task_tool = task_tool.with_suggestion_applier(
                suggestion_applier as Arc<dyn feature_tasks::SuggestionApplier>,
            );

            let forecast_handler = Arc::new(crate::handlers::LlmForecastHandler::new(
                provider.clone(),
                config.agents.defaults.model.clone(),
                task_repo_for_handlers,
            ));
            task_tool = task_tool
                .with_forecast_handler(forecast_handler as Arc<dyn feature_tasks::ForecastHandler>);

            tool_registry.register(task_tool);

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
            repos.tasks.clone(),
        ));
        // ── Annotate tool ──────────────────────────────────────────────────
        tool_registry.register(tools::AnnotateTool::new(cognitive::AnnotationRepo::new(
            storage_pool.inner().clone(),
        )));

        // ── Conversation recall handler ──────────────────────────────────
        let conversation_recall_handler: Option<Arc<dyn tools::ConversationRecallHandler>> =
            recall_service.as_ref().map(|service| {
                Arc::new(
                    crate::adapters::conversation_recall::ConversationRecallHandlerImpl::new(
                        Arc::clone(service),
                    ),
                ) as Arc<dyn tools::ConversationRecallHandler>
            });

        // ── Memory tool ───────────────────────────────────────────────────
        if config.conversation.search.enabled {
            if let Some(ref handler) = conversation_recall_handler {
                let mut memory_tool = tools::MemoryTool::new()
                    .with_conversation_handler(Arc::clone(handler))
                    .with_todo_repo(repos.tasks.clone())
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
                let cache_ttl = config.finance.price_refresh.cache_ttl_minutes;
                let finance_storage = storage::FinanceStorage::from_pool(pool);
                let rate_cache = feature_finance::rate_cache::RateCache::new(
                    finance_storage.exchange_rates.clone(),
                    i64::from(cache_ttl),
                );
                let mut price_service =
                    feature_finance::PriceService::with_rate_cache(cache_ttl, rate_cache.clone());

                // Apply config exchange rate overrides
                if let Some(ref overrides) = config.finance.exchange_rates {
                    price_service.set_exchange_rate_overrides(overrides.clone());
                }

                let finance_handler_impl =
                    Arc::new(crate::adapters::finance::FinanceHandlerImpl::new(
                        repos.clone(),
                        price_service.clone(),
                        config.finance.clone(),
                    ));

                let mut finance_tool = feature_finance::FinanceTool::new(
                    finance_storage,
                    price_service,
                    config.finance.default_currency.clone(),
                )
                .with_rate_cache(rate_cache)
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
            let prod_handler =
                Arc::new(crate::adapters::productivity::ProductivityHandlerImpl::new(
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
            let manager = mcp::McpManager::connect_all(
                &config.mcp,
                None,
                mcp::McpClientOptions::default(),
            )
            .await;
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
                repos.tasks.clone(),
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
            repos.tasks.clone(),
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
            let svc_arc = Arc::new(RwLock::new(service));

            // Register cron handler so the learning analysis shows in the Automations page.
            // The handler triggers the existing background loop rather than running inline.
            if let Some(ref cron_svc) = self.cron_service {
                let svc_for_cron = Arc::clone(&svc_arc);
                cron_svc.register_handler(
                    "__klyntbot_learning_analysis",
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let svc = Arc::clone(&svc_for_cron);
                        // Trigger the analysis via the existing Notify mechanism
                        if let Ok(guard) = svc.try_read() {
                            guard.trigger_analysis();
                        }
                        Ok(Some("Learning analysis triggered".to_string()))
                    }),
                );
            }

            Some(svc_arc)
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

        // ── MCP health check (auto-reconnect downed servers) ─────────────
        let mcp_manager_arc = Arc::new(tokio::sync::Mutex::new(mcp_manager));
        let mcp_health_check_token = {
            let mgr_guard = mcp_manager_arc.lock().await;
            if mgr_guard.is_some() {
                drop(mgr_guard);
                Some(CancellationToken::new())
            } else {
                None
            }
        };

        // ── Agent Runtime ─────────────────────────────────────────────────
        let tool_registry = Arc::new(RwLock::new(tool_registry));

        // Start health check now that tool_registry is wrapped in Arc<RwLock<>>
        if let Some(ref token) = mcp_health_check_token {
            let _health_handle = mcp::McpManager::start_health_check(
                Arc::clone(&mcp_manager_arc),
                Arc::clone(&tool_registry),
                token.clone(),
            );
        }

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

        let mut analyzer = crate::intent_pipeline::analysis::IntentAnalyzer::new(
            provider.clone(),
            &config.agents.defaults.model,
            &config.orchestrator,
        )
        .with_strategy_repo(repos.strategies.clone())
        .with_shadow_mode();

        // Wire live autotuner reference into IntentAnalyzer for per-message threshold reads
        if let Some(ref orchestrator) = self.autotuner {
            analyzer = analyzer.with_autotuner(Arc::clone(orchestrator));
        }

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
            scenario_max_graph_depth: config.scenario.max_graph_depth,
            pipeline_timeout_secs: config.agents.defaults.pipeline_timeout_secs,
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
            Arc::clone(&skill_catalog),
            Arc::clone(&skill_router),
            analyzer,
            Arc::clone(&context_engine),
            router,
            cost_tracker,
            runtime_config,
            Arc::clone(&active_profile),
        )
        .with_strategy_repo(repos.strategies.clone())
        .with_activated_skills(Arc::clone(&activated_skills))
        .with_embedding_engine(Arc::clone(&embedding_engine));

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

        // Inject user situation for RetrievalContext
        if let Some(ref sit) = self.user_situation {
            runtime = runtime.with_user_situation(Arc::clone(sit));
        }

        // Wire task repo for active task context in query rewriting
        runtime = runtime.with_task_repo(repos.tasks.clone());

        // Inject active view for RetrievalContext
        if let Some(ref view) = self.active_view {
            runtime = runtime.with_active_view(Arc::clone(view));
        }

        // Inject autotuner shadow hook
        if let Some(ref orchestrator) = self.autotuner {
            // Build the concrete AutoTunerHook for shadow classification
            if let Some(ref pool) = self.pool {
                let trial_repo = storage::TrialRepo::new(pool.clone());
                let mut hook = crate::autotuner::hooks::AutoTunerHookImpl::new(
                    Arc::clone(orchestrator),
                    trial_repo,
                    provider.clone(),
                    &config.agents.defaults.model,
                    &config.orchestrator,
                );

                // Wire Phase 2 shadow retriever if memory service is available
                if let Some(ref mem_svc) = memory_service_for_shadow {
                    let config_defaults = [
                        config.cognitive.relevance_weight_semantic,
                        config.cognitive.relevance_weight_retrievability,
                        config.cognitive.relevance_weight_importance,
                        config.cognitive.relevance_weight_frequency,
                        config.cognitive.relevance_weight_situation,
                        config.cognitive.relevance_weight_temporal,
                    ];
                    let shadow_retriever = Arc::new(
                        crate::autotuner::shadow_retriever::AgentShadowRetriever::new(
                            Arc::clone(mem_svc),
                            config_defaults,
                        ),
                    );
                    hook = hook.with_shadow_retriever(
                        shadow_retriever as Arc<dyn autotuner::ShadowRetriever>,
                    );
                }

                runtime = runtime.with_autotuner_hook(Arc::new(hook));
            }
        }

        // Inject squad execution dependencies (requires pool for SquadRepo)
        if let Some(ref pool) = self.pool {
            let squad_repo = cognitive::SquadRepo::new(pool.clone());
            let squad_provider = self
                .cognitive_provider
                .as_ref()
                .cloned()
                .unwrap_or_else(|| provider.clone());
            let squad_params = providers::cognitive_chat_params(&config, 4096);
            let blackboard_repo = Some(cognitive::BlackboardRepo::new(pool.clone()));
            runtime =
                runtime.with_squad_deps(squad_repo, squad_provider, squad_params, blackboard_repo);
        }

        // Inject domain event bus for SkillRouted event emission
        if let Some(ref domain_bus) = self.domain_event_bus {
            runtime = runtime.with_domain_bus(Arc::clone(domain_bus));
        }

        let runtime = Arc::new(runtime);

        // Two-phase init: set the self-reference for delegation after Arc wrapping
        runtime.set_delegation_self_ref(Arc::clone(&runtime) as Arc<dyn tools::DelegationHandler>);

        info!("Agent runtime initialized");

        // Session cleanup and memory maintenance are handled by CronService
        // (registered in app-core/init/cron.rs as __klyntbot_session_cleanup
        // and __klyntbot_memory_maintenance).

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
            _session_cleanup_token: None,
            _memory_maintenance_token: None,
            mcp_manager: Arc::clone(&mcp_manager_arc),
            _mcp_health_check_token: mcp_health_check_token,
            domain_event_bus: self.domain_event_bus,
            trial_repo: if self.autotuner.is_some() {
                self.pool
                    .as_ref()
                    .map(|p| storage::TrialRepo::new(p.clone()))
            } else {
                None
            },
            cognitive_bg_service: tokio::sync::Mutex::new(cognitive_bg_service),
            _inference_loop_token,
            activity_svc: self.activity_svc,
            skill_catalog,
            skill_router,
            embedding_engine,
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
