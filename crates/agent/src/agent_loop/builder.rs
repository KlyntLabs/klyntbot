//! Agent loop construction: tool registration, handler wiring, pipeline assembly.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;


use bus::MessageBus;
use common::Result;
use config::Config;

use providers::DynProvider;
use session::SessionManager;
use tools::{
    cron_tool::CronTool,
    okr_tool::OkrTool,
    registry::ToolRegistry,
    subagents::SubagentsTool,
};
use tools_core::FeaturePackage;


use super::super::{CronHandlerAdapter, SubagentManager};
use super::{AgentLoop, LastActiveChannel};

/// Builder for constructing an [`AgentLoop`] with all its dependencies.
///
/// Required fields (`bus`, `provider`, `config`) are constructor params.
/// Optional: `pool` (enables feature-tasks), `cron_executor`, `notification_handle`.
///
/// # Example
/// ```ignore
/// let agent = AgentLoop::builder(bus, provider, config)
///     .with_pool(pool)
///     .build()
///     .await?;
/// ```
/// Adapter bridging `providers::DynProvider` → `context_engine::DecomposerLlm` trait.
pub(crate) struct DecomposerLlmAdapter {
    pub(crate) provider: DynProvider,
    pub(crate) params: providers::ChatParams,
}

#[async_trait::async_trait]
impl context_engine::DecomposerLlm for DecomposerLlmAdapter {
    async fn generate(&self, prompt: &str) -> std::result::Result<String, String> {
        let messages = vec![providers::Message::User {
            content: providers::UserContent::Text(prompt.to_string()),
        }];
        let response = self
            .provider
            .chat(&messages, None, &self.params, &[])
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
    cron_executor: Option<(
        Arc<scheduling::temporal::cron_executor::CronExecutor>,
        storage::repos::cron::CronRepo,
    )>,
    notification_handle: Option<LastActiveChannel>,
    notification_sender: Option<Arc<dyn common::NotificationSender>>,
    domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    cognitive_provider: Option<DynProvider>,
    pipeline_tx: Option<tokio::sync::broadcast::Sender<cognitive::PipelineEvent>>,
    user_situation: Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
    activity_svc: Option<Arc<activity_log::ActivityIngestionService>>,
    autotuner: Option<Arc<crate::autotuner::AutoTunerOrchestrator>>,
    active_view: Option<Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>>,
    hot_config: Option<Arc<RwLock<config::HotConfig>>>,
    context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
    embedding_engine: Option<Arc<tools::EmbeddingEngine>>,
    approval_channel: Option<Arc<dyn approval::ApprovalChannel>>,
    approval_suggester: Option<Arc<dyn approval::ApprovalSuggester>>,
    injector_registry: Option<bus::InjectorRegistry>,
    job_supervisor: Option<tools_core::DynJobSupervisor>,
    pre_registered_tools: Vec<tools_core::DynTool>,
}

impl AgentLoopBuilder {
    pub fn new(bus: Arc<MessageBus>, provider: DynProvider, config: Config) -> Self {
        Self {
            bus,
            provider,
            config,
            pool: None,
            vector_store: None,
            cron_executor: None,
            notification_handle: None,
            notification_sender: None,
            domain_event_bus: None,
            cognitive_provider: None,
            pipeline_tx: None,
            user_situation: None,
            activity_svc: None,
            autotuner: None,
            active_view: None,
            hot_config: None,
            context_update_queue: None,
            embedding_engine: None,
            approval_channel: None,
            approval_suggester: None,
            injector_registry: None,
            job_supervisor: None,
            pre_registered_tools: vec![],
        }
    }

    pub fn with_approval_channel(mut self, channel: Arc<dyn approval::ApprovalChannel>) -> Self {
        self.approval_channel = Some(channel);
        self
    }
    pub fn with_approval_suggester(
        mut self,
        suggester: Arc<dyn approval::ApprovalSuggester>,
    ) -> Self {
        self.approval_suggester = Some(suggester);
        self
    }
    pub fn with_injector_registry(mut self, registry: bus::InjectorRegistry) -> Self {
        self.injector_registry = Some(registry);
        self
    }
    pub fn with_job_supervisor(mut self, supervisor: tools_core::DynJobSupervisor) -> Self {
        self.job_supervisor = Some(supervisor);
        self
    }
    pub fn with_pre_registered_tools(mut self, tools: Vec<tools_core::DynTool>) -> Self {
        self.pre_registered_tools = tools;
        self
    }
    pub fn with_embedding_engine(mut self, engine: Arc<tools::EmbeddingEngine>) -> Self {
        self.embedding_engine = Some(engine);
        self
    }

    pub fn with_pool(mut self, pool: sqlx::SqlitePool) -> Self {
        self.pool = Some(pool);
        self
    }

    pub fn with_vector_store(mut self, store: storage::VectorStore) -> Self {
        self.vector_store = Some(store);
        self
    }

    pub fn with_cron_executor(
        mut self,
        executor: Arc<scheduling::temporal::cron_executor::CronExecutor>,
        repo: storage::repos::cron::CronRepo,
    ) -> Self {
        self.cron_executor = Some((executor, repo));
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

    pub fn with_hot_config(mut self, hot_config: Arc<RwLock<config::HotConfig>>) -> Self {
        self.hot_config = Some(hot_config);
        self
    }

    pub fn with_context_update_queue(mut self, queue: Arc<bus::ContextUpdateQueue>) -> Self {
        self.context_update_queue = Some(queue);
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

        let hot_config = self.hot_config.unwrap_or_else(|| {
            Arc::new(tokio::sync::RwLock::new(config::HotConfig::from(&config)))
        });

        // ── Create repos from pool (real or in-memory fallback) ──────────
        // Storage-dependent features (todo, sessions) are disabled via
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

        // ── Skill store (flat, file-based) ─────────────────────────────────
        let data_dir_path = config.data_dir_path();
        let skills_dir = data_dir_path.join("skills");
        let skill_store = skill_system::SkillStore::load(&skills_dir)?;

        // Build skill reference index for the skill_reference tool
        let skill_reference_index = {
            let skill_bodies = skill_store.build_reference_index();
            // No separate reference files in the flat model — pass empty map
            let ref_files = std::collections::HashMap::new();
            Arc::new(tools::SkillReferenceIndex::new(skill_bodies, ref_files))
        };

        let skill_store = Arc::new(tokio::sync::RwLock::new(skill_store));

        // ── Shared embedding engine (reuse injected instance or create new) ──
        let embedding_engine = self.embedding_engine.unwrap_or_else(|| {
            Arc::new(
                tools::EmbeddingEngine::new()
                    .with_openai_model(config.cognitive.openai_embedding_model.clone()),
            )
        });

        // ── Context sources ───────────────────────────────────────────────
        let ctx_sources = super::builders::context_sources::build_context_sources(
            &config,
            &self.pool,
            &self.vector_store,
            &self.domain_event_bus,
            &self.cognitive_provider,
            &mut self.pipeline_tx,
            &self.context_update_queue,
            &data_dir_path,
            &workspace,
            &repos,
            &storage_pool,
            &embedding_engine,
            &skill_store,
        )
        .await?;
        let sources = ctx_sources.sources;
        let cognitive_fact_repo = ctx_sources.cognitive_fact_repo;
        let cognitive_embedder = ctx_sources.cognitive_embedder;
        let cognitive_retrieval_config = ctx_sources.cognitive_retrieval_config;
        let cognitive_bg_service = ctx_sources.cognitive_bg_service;
        let session_memory_service = ctx_sources.session_memory_service;
        let prod_repos = ctx_sources.prod_repos;
        let confidence_bits = ctx_sources.confidence_bits;
        let _inference_loop_token = ctx_sources.inference_loop_token;

        // Determine summarization model: config override or default
        let summary_model = config
            .cognitive
            .history_compression
            .model
            .clone()
            .unwrap_or_else(|| config.agents.defaults.model.clone());
        let summary_provider = Arc::new(crate::adapters::llm_summary::LlmSummaryProvider::new(
            provider.clone(),
            summary_model,
        ));
        let token_counter = context_engine::token_counter_for_model(&config.agents.defaults.model);
        let context_engine =
            context_engine::ContextEngine::new(config.cognitive.history_compression.clone())
                .with_sources(sources)
                .with_token_counter(Arc::clone(&token_counter))
                .with_summary_provider(summary_provider);

        // ── Session manager (SQL-backed) ──────────────────────────────────
        let session_manager = SessionManager::from_repo(
            storage::SessionRepo::new(storage_pool.inner().clone()),
            config.conversation.session.max_cache_size,
        )
        .await;

        // ── Subagent manager ──────────────────────────────────────────────
        let _brave_api_key = (!config.tools.web.brave_api_key.is_empty())
            .then(|| config.tools.web.brave_api_key.expose().clone());

        let subagent_manager = Arc::new(
            SubagentManager::builder(Arc::clone(&provider), workspace.clone())
                .inbound_sender(bus.inbound_sender())
                .model(config.agents.defaults.model.clone())
                .max_concurrent_subagents(config.agents.defaults.max_concurrent_subagents)
                .agent_task_repo(repos.agent_tasks.clone())
                .job_supervisor(self.job_supervisor.clone())
                .repos(repos.clone())
                .build(),
        );

        // ── Tool registry ─────────────────────────────────────────────────
        let mut tool_registry = ToolRegistry::new();

        // Pre-registered tools (from app-core plugins)
        for tool in self.pre_registered_tools {
            tool_registry.register_dyn(tool);
        }

        // Subagents tool
        tool_registry.register(SubagentsTool::with_handler(
            Arc::clone(&subagent_manager) as Arc<dyn tools::subagents::SubagentsHandler>
        ));

        // Cron tool (optional)
        if let Some((ref executor, ref repo)) = self.cron_executor {
            let adapter: Arc<dyn tools::cron_tool::CronHandler> =
                Arc::new(CronHandlerAdapter::new(Arc::clone(executor), repo.clone()));
            tool_registry.register(CronTool::with_handler(adapter));
        }

        // Shared references — SqlitePool is Clone+Send+Sync via Arc internally

        // Notification dispatcher removed (Phase 3): legacy agent::NotificationDispatcher
        // is replaced by notifications::NotificationDispatcher wired in app-core.

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
        // ── Cognitive memory system ───────────────────────────────────────
        let cog_result = if let Some(fact_repo) = cognitive_fact_repo {
            super::builders::cognitive::build_cognitive_system(
                &config,
                context_engine,
                fact_repo,
                cognitive_embedder,
                cognitive_retrieval_config,
                &recall_service,
                &self.pool,
                &self.vector_store,
                &self.domain_event_bus,
                &self.context_update_queue,
                &embedding_engine,
                &skill_store,
                &self.user_situation,
                &self.autotuner,
                &self.cognitive_provider,
                &storage_pool,
                &repos,
            )
            .await
        } else {
            super::builders::cognitive::CognitiveBuildResult {
                context_engine,
                tree_builder_token: None,
                memory_service_for_shadow: None,
                memory_retriever_for_prf: None,
                predictive_cache: None,
            }
        };
        let context_engine = cog_result.context_engine;
        let tree_builder_token = cog_result.tree_builder_token;
        let memory_service_for_shadow = cog_result.memory_service_for_shadow;
        let memory_retriever_for_prf = cog_result.memory_retriever_for_prf;
        let predictive_cache = cog_result.predictive_cache;

        // Shared store for the latest enhancement trace, written by the
        // QueryPipeline and read by the `memory` MCP tool.
        let latest_enhancement_trace =
            Arc::new(context_engine::enhancement::LatestEnhancementTrace::new());

        // Build the query enhancement pipeline.
        let context_engine = super::builders::query_enhancement::build_query_enhancement(
            &config,
            context_engine,
            &self.cognitive_provider,
            &self.autotuner,
            &memory_retriever_for_prf,
            &latest_enhancement_trace,
        );

        let context_engine = Arc::new(context_engine);

        // Outputs for MemoryTool — populated inside the pool block if embedding is enabled
        let todo_embedding_handler: Option<Arc<dyn tools::EmbeddingHandler>>;

        // ── Feature-tasks tool (via FeaturePackage) ───────────────────────
        if self.pool.is_some() {
            let pool_ref = storage_pool.inner();
            let task_repo = storage::TaskRepo::new(pool_ref.clone());
            let area_repo = storage::AreaRepo::new(pool_ref.clone());
            let mut task_tool = feature_tasks::TaskTool::new(
                task_repo,
                config.todo.focus.max_slots,
                config.todo.focus.deadline_hours,
                config.timezone.clone(),
            )
            .with_area_repo(area_repo);

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

            // Wire alarm writer (task_alarms repo + FireStore) so the `alarms`
            // param on TaskTool create/update materializes into scheduled_fires.
            // Spec §3 (rule model), §8.1 (TaskTool subfields).
            {
                let fire_store = Arc::new(scheduling::temporal::fire_store::FireStore::new(
                    repos.scheduled_fires.clone(),
                ));
                task_tool = task_tool.with_alarm_writer(repos.task_alarms.clone(), fire_store);
            }

            // Register via FeaturePackage
            let tasks_feature =
                feature_tasks::TasksFeature::new().with_task_tool(Arc::new(task_tool));
            for tool in tasks_feature.tools() {
                tool_registry.register_dyn(tool);
            }

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

        // ── Feature tools (memory, productivity, MCP, skill reference, recurring spawner) ──
        let tools_result = super::builders::tools::build_feature_tools(
            super::builders::tools::ToolsBuildInput {
                config: &config,
                tool_registry: &mut tool_registry,
                conversation_recall_handler: &conversation_recall_handler,
                todo_embedding_handler: &todo_embedding_handler,
                repos: &repos,
                prod_repos,
                vector_store: &self.vector_store,
                domain_event_bus: &self.domain_event_bus,
                latest_enhancement_trace: &latest_enhancement_trace,
                pool: &self.pool,
                provider: &provider,
                skill_reference_index: &skill_reference_index,
            },
        )
        .await?;
        let mcp_manager = tools_result.mcp_manager;
        let recurring_task_spawner = tools_result.recurring_task_spawner;

        // ── Learning service ──────────────────────────────────────────────
        let learning_result = super::builders::learning::build_learning_service(
            &config,
            &repos,
            &mut tool_registry,
            &outcome_store,
            &confidence_bits,
            &self.domain_event_bus,
            &self.cron_executor,
        )
        .await;
        let learning_service = learning_result.learning_service;

        // ── Bus receiver ──────────────────────────────────────────────────
        let inbound_rx = bus
            .take_inbound_rx()
            .expect("Inbound receiver already taken");

        // ── MCP health check (auto-reconnect downed servers) ─────────────
        let mcp_manager_arc = Arc::new(tokio::sync::RwLock::new(mcp_manager));
        let mcp_health_check_token = {
            let mgr_guard = mcp_manager_arc.read().await;
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

        // ── Agent Runtime ─────────────────────────────────────────────────
        let runtime = Arc::new(super::builders::runtime::build_runtime(
            super::builders::runtime::RuntimeBuildInput {
                config: config.clone(),
                provider: provider.clone(),
                context_engine: Arc::clone(&context_engine),
                tool_registry: Arc::clone(&tool_registry),
                token_counter: Arc::clone(&token_counter),
                outcome_recorder: outcome_store.as_ref().map(|store| {
                    Arc::new(crate::learning::recorder::OutcomeRecorder::new(Arc::clone(store)))
                }),
                domain_event_bus: self.domain_event_bus.clone(),
                pool: self.pool.clone(),
                approval_channel: self.approval_channel.clone(),
                approval_suggester: self.approval_suggester.clone(),
                hot_config: Arc::clone(&hot_config),
                autotuner: self.autotuner.clone(),
                predictive_cache: predictive_cache.clone(),
                cognitive_provider: self.cognitive_provider.clone(),
                user_situation: self.user_situation.clone(),
                task_repo: repos.tasks.clone(),
                interaction_log_repo: repos.interaction_log.clone(),
                active_view: self.active_view.clone(),
                memory_service_for_shadow: memory_service_for_shadow.clone(),
                context_update_queue: self.context_update_queue.clone(),
                injector_registry: self.injector_registry.clone(),
                storage_pool: storage_pool.clone(),
            },
        ));

        // Session cleanup and memory maintenance are handled by CronService
        // (registered in app-core/init/cron.rs as __klyntbot_session_cleanup
        // and __klyntbot_memory_maintenance).

        // ── Assemble AgentLoop ────────────────────────────────────────────
        let history_limit = config.conversation.session.history_limit;
        Ok(AgentLoop {
            bus,
            inbound_rx: Some(inbound_rx),
            config,
            session_manager,
            tool_registry,
            running: Arc::new(AtomicBool::new(false)),
            last_active_channel: self.notification_handle,
            recurring_task_spawner,
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
            _session_memory_service: session_memory_service,
            approval_grants_repo: self.pool.as_ref().map(|p| {
                approval::ApprovalGrantsRepo::new(storage::StoragePool::from_existing(p.clone()))
            }),
            _inference_loop_token,
            _tree_builder_token: tree_builder_token,
            activity_svc: self.activity_svc,
            skill_store,
            hot_config,
            subagent_manager: Some(subagent_manager),
            job_supervisor: self.job_supervisor.clone(),
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


