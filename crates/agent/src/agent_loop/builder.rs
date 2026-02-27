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
    browser::BrowserTool,
    calendar_tool::{CalendarHandler, CalendarTool},
    cron_tool::CronTool,
    filesystem::register_fs_tools,
    goal_tool::{GoalHandler, GoalTool},
    learning_tool::{LearningHandler, LearningTool},
    message::MessageTool,
    plan_tool::PlanCompletionHandler,
    registry::ToolRegistry,
    spawn::SpawnTool,
    web::{WebFetchTool, WebSearchTool},
};
use tools_core::FeaturePackage;

use super::super::confidence::ConfidenceEvaluator;
use super::super::context_sources::{
    BootstrapSource, ConfidenceSource, GoalSource, IdentitySource, MemorySource,
    SkillContentSource, SkillSummarySource, TodoSource,
};
use super::super::{CalendarSyncAdapter, CronHandlerAdapter, SkillManager, SubagentManager};
use super::{AgentLoop, LastActiveChannel};

/// Builder for constructing an [`AgentLoop`] with all its dependencies.
///
/// Required fields: `bus`, `provider`, `config`.
/// Optional: `pool` (enables feature-todo, finance), `cron_service`, `notification_handle`.
///
/// # Example
/// ```ignore
/// let agent = AgentLoop::builder()
///     .with_bus(bus)
///     .with_provider(provider)
///     .with_config(config)
///     .with_pool(pool)
///     .build()
///     .await?;
/// ```
pub struct AgentLoopBuilder {
    bus: Option<Arc<MessageBus>>,
    provider: Option<DynProvider>,
    config: Option<Config>,
    pool: Option<sqlx::SqlitePool>,
    vector_store: Option<storage::VectorStore>,
    cron_service: Option<Arc<scheduling::CronService>>,
    notification_handle: Option<LastActiveChannel>,
}

impl Default for AgentLoopBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentLoopBuilder {
    pub fn new() -> Self {
        Self {
            bus: None,
            provider: None,
            config: None,
            pool: None,
            vector_store: None,
            cron_service: None,
            notification_handle: None,
        }
    }

    pub fn with_bus(mut self, bus: Arc<MessageBus>) -> Self {
        self.bus = Some(bus);
        self
    }

    pub fn with_provider(mut self, provider: DynProvider) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn with_config(mut self, config: Config) -> Self {
        self.config = Some(config);
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

    pub fn with_cron_service(mut self, service: Arc<scheduling::CronService>) -> Self {
        self.cron_service = Some(service);
        self
    }

    pub fn with_notification_handle(mut self, handle: LastActiveChannel) -> Self {
        self.notification_handle = Some(handle);
        self
    }

    /// Consume the builder and construct an [`AgentLoop`].
    ///
    /// Returns an error if any required field is missing or if initialization fails.
    pub async fn build(self) -> Result<AgentLoop> {
        // ── Validate required fields ──────────────────────────────────────
        let bus = self.bus.ok_or_else(|| {
            common::KlyntbotError::Config(common::ConfigError::MissingField("bus".into()))
        })?;
        let provider = self.provider.ok_or_else(|| {
            common::KlyntbotError::Config(common::ConfigError::MissingField("provider".into()))
        })?;
        let config = self.config.ok_or_else(|| {
            common::KlyntbotError::Config(common::ConfigError::MissingField("config".into()))
        })?;

        let workspace = config.workspace_path();

        // Ensure workspace directory exists so fs tools work correctly
        if !workspace.exists() {
            tokio::fs::create_dir_all(&workspace).await.map_err(|e| {
                common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                    "Failed to create workspace directory {}: {}",
                    workspace.display(),
                    e
                )))
            })?;
        }

        // ── Create repos from pool (real or in-memory fallback) ──────────
        // Storage-dependent features (todo, finance, sessions) are disabled via
        // the `if self.pool.is_some()` guards below when no real pool is provided.
        let storage_pool = if let Some(pool) = self.pool.clone() {
            storage::StoragePool::from_existing(pool)
        } else {
            storage::StoragePool::connect_in_memory()
                .await
                .unwrap_or_else(|e| panic!("Failed to create in-memory SQLite fallback: {e}"))
        };
        let repos = storage::Repos::from_pool(&storage_pool);

        // ── Skills (init + filter) ────────────────────────────────────────
        let mut skill_manager = SkillManager::new();
        skill_manager.load(workspace.clone()).await.map_err(|e| {
            common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                "Failed to initialize skills: {}",
                e
            )))
        })?;

        if !config.packs.enabled_skills.is_empty() {
            skill_manager.filter_by_skills(&config.packs.enabled_skills);
        }

        let skill_manager = Arc::new(skill_manager);

        // ── Context sources ───────────────────────────────────────────────
        let confidence_source = ConfidenceSource::new(config.confidence.threshold);
        let confidence_threshold_handle = confidence_source.threshold_handle();

        // ── Shared embedding engine (created early for MemoryStore + later use) ──
        let embedding_engine = Arc::new(tools::EmbeddingEngine::new());

        // ── Memory store (with optional embedding-based relevance filtering) ──
        let memory_store = if config.conversation.embedding.enabled && self.vector_store.is_some() {
            crate::memory::MemoryStore::with_embeddings(
                repos.memory_notes.clone(),
                self.vector_store.clone().unwrap(),
                Arc::clone(&embedding_engine),
                config.conversation.search.semantic_threshold,
            )
        } else {
            crate::memory::MemoryStore::new(repos.memory_notes.clone())
        };

        let mut sources: Vec<Box<dyn ContextSource>> = vec![
            Box::new(IdentitySource::new(
                workspace.clone(),
                config.timezone.clone(),
            )),
            Box::new(BootstrapSource::new(workspace.clone())),
            Box::new(MemorySource::new(memory_store)),
            Box::new(TodoSource::new(repos.todos.clone())),
            Box::new(GoalSource::new(repos.goals.clone())),
            Box::new(confidence_source),
            Box::new(SkillSummarySource::new(Arc::clone(&skill_manager))),
            Box::new(SkillContentSource::new(Arc::clone(&skill_manager))),
        ];

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
        let session_manager =
            SessionManager::from_repo(storage::SessionRepo::new(storage_pool.inner().clone()))
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
        register_fs_tools(&mut tool_registry, allowed_dir);

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

        // Clone repos for shared use
        let todo_repo_for_memory = repos.todos.clone();
        let todo_repo_shared = repos.todos.clone();

        // ── Notification dispatcher ───────────────────────────────────────
        let notification_dispatcher = if !config.todo.notifications.targets.is_empty() {
            Some(Arc::new(super::super::NotificationDispatcher::new(
                bus.outbound_sender(),
                config.todo.notifications.clone(),
            )))
        } else {
            None
        };

        // ── Calendar tool ─────────────────────────────────────────────────
        let calendar_adapter = if config.calendar.is_any_enabled() {
            let adapter = Arc::new(
                CalendarSyncAdapter::new(
                    todo_repo_shared.clone(),
                    repos.calendar_sync.clone(),
                    repos.calendar_event_cache.clone(),
                    &config.calendar,
                    config.timezone.clone(),
                    notification_dispatcher.clone(),
                    config.calendar.bidirectional_sync,
                )
                .await?,
            );

            tool_registry.register(CalendarTool::new(
                Arc::clone(&adapter) as Arc<dyn CalendarHandler>
            ));
            Some(adapter)
        } else {
            None
        };

        // ── Learning: outcome store ────────────────────────────
        let outcome_store = if config.learning.enabled {
            Some(Arc::new(RwLock::new(
                crate::learning::outcome_store::OutcomeStore::new(repos.outcomes.clone()),
            )))
        } else {
            None
        };

        // ── Wire automatic memory retrieval (cross-channel LanceDB ANN) ─
        let context_engine = if config.conversation.embedding.enabled && self.vector_store.is_some()
        {
            let conv_store_for_retriever =
                tools::ConversationEmbeddingStore::new(self.vector_store.clone().unwrap());
            // Compute per-day decay factor from configured half-life: factor = 0.5^(1/half_life)
            let decay_factor =
                0.5_f64.powf(1.0 / config.conversation.memory.decay_half_life_days as f64);
            let retriever = Arc::new(
                super::super::conversation_memory_retriever::ConversationMemoryRetriever::new(
                    Arc::clone(&embedding_engine),
                    conv_store_for_retriever,
                    config.conversation.search.semantic_threshold,
                    decay_factor,
                ),
            );
            context_engine.with_memory_retriever(retriever)
        } else {
            context_engine
        };
        let context_engine = Arc::new(context_engine);

        // Outputs for MemoryTool — populated inside the pool block if embedding is enabled
        let todo_embedding_handler: Option<Arc<dyn tools::EmbeddingHandler>>;

        // ── Feature-todo tool (requires real pool) ────────────────────────
        if self.pool.is_some() {
            let pool_ref = storage_pool.inner();
            let feature_todo_repo = feature_todo::TodoRepo::new(pool_ref.clone());
            let mut todo_tool = feature_todo::TodoTool::new(
                feature_todo_repo,
                config.todo.focus.max_slots,
                config.todo.focus.deadline_hours,
                config.timezone.clone(),
            );

            // Inject calendar handler
            if let Some(ref adapter) = calendar_adapter {
                let todo_cal_sync = Arc::new(
                    crate::todo_calendar_sync_adapter::TodoCalendarSyncAdapter::new(Arc::clone(
                        adapter,
                    )
                        as Arc<dyn CalendarHandler>),
                );
                todo_tool = todo_tool.with_calendar_handler(
                    todo_cal_sync as Arc<dyn feature_todo::CalendarSyncHandler>,
                );
            }

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
            if config.todo.search.enabled && self.vector_store.is_some() {
                let vs = self.vector_store.clone().unwrap();

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

            tool_registry.register(todo_tool);
        } else {
            // No pool available (e.g., test environment)
            todo_embedding_handler = None;
        }

        // ── Goal tool ─────────────────────────────────────────────────────
        {
            let goal_handler = Arc::new(
                super::super::GoalHandlerImpl::new(repos.goals.clone())
                    .with_plan_repo(repos.plans.clone())
                    .with_provider(provider.clone(), config.agents.defaults.model.clone()),
            );
            tool_registry.register(GoalTool::new(Some(goal_handler as Arc<dyn GoalHandler>)));
        }

        // ── Plan tool ─────────────────────────────────────────────────────
        let plan_handler = Arc::new(
            super::super::PlanHandlerImpl::new(repos.plans.clone())
                .with_provider(provider.clone(), config.agents.defaults.model.clone()),
        );
        tool_registry.register(tools::plan_tool::PlanTool::new(Some(
            plan_handler as Arc<dyn tools::plan_tool::PlanHandler>,
        )));
        let stored_plan_repo = Some(repos.plans.clone());

        // Plan completion handler (updates linked goal)
        let plan_completion_handler: Option<Arc<dyn PlanCompletionHandler>> = Some(Arc::new(
            super::super::plan_completion_handler::PlanCompletionHandlerImpl::new(
                repos.goals.clone(),
            ),
        )
            as Arc<dyn PlanCompletionHandler>);

        // ── Conversation embedding handler ────────────────────────────────
        let conversation_embedding_handler =
            if config.conversation.embedding.enabled && self.vector_store.is_some() {
                let conv_store =
                    tools::ConversationEmbeddingStore::new(self.vector_store.clone().unwrap());
                let handler = Arc::new(
                super::super::conversation_embedding_handler::ConversationEmbeddingHandlerImpl::new(
                    Arc::clone(&embedding_engine),
                    conv_store,
                ),
            );
                Some(handler as Arc<dyn tools::ConversationEmbeddingHandler>)
            } else {
                None
            };

        // ── Memory tool ───────────────────────────────────────────────────
        if config.conversation.search.enabled {
            if let Some(ref handler) = conversation_embedding_handler {
                let mut memory_tool = tools::MemoryTool::new()
                    .with_conversation_handler(Arc::clone(handler))
                    .with_todo_repo(todo_repo_for_memory)
                    .with_threshold(config.conversation.search.semantic_threshold)
                    .with_rrf_k(config.todo.search.rrf_k);

                if let Some(ref h) = todo_embedding_handler {
                    memory_tool = memory_tool.with_todo_embedding_handler(Arc::clone(h));
                }

                tool_registry.register(memory_tool);
            }
        }

        // ── Finance tool (requires real pool) ─────────────────────────────
        if config.finance.enabled && self.pool.is_some() {
            let price_service =
                feature_finance::PriceService::new(config.finance.price_refresh.cache_ttl_minutes);

            let finance_handler_impl = Arc::new(crate::finance_adapter::FinanceHandlerImpl::new(
                repos.clone(),
                price_service.clone(),
                config.finance.clone(),
            ));

            let finance_tool = feature_finance::FinanceTool::new(
                repos.finance_accounts.clone(),
                repos.finance_transactions.clone(),
                repos.finance_budgets.clone(),
                repos.finance_investments.clone(),
                repos.finance_goals.clone(),
                repos.finance_liabilities.clone(),
                price_service,
                config.finance.default_currency.clone(),
            )
            .with_finance_handler(
                Arc::clone(&finance_handler_impl) as Arc<dyn feature_finance::FinanceHandler>
            );

            tool_registry.register(finance_tool);
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
            let calendar_handler_opt = calendar_adapter
                .as_ref()
                .map(|adapter| Arc::clone(adapter) as Arc<dyn CalendarHandler>);
            let mut engine = super::super::ReminderEngine::new(
                todo_repo_shared.clone(),
                calendar_handler_opt,
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
            todo_repo_shared.clone(),
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

            // Event bus: subscriber updates ConfidenceSource threshold
            let event_bus = Arc::new(bus::LearningEventBus::new(16));

            let threshold_for_subscriber = confidence_threshold_handle.clone();
            let mut event_rx = event_bus.subscribe();
            tokio::spawn(async move {
                while let Ok(event) = event_rx.recv().await {
                    if let bus::LearningEvent::ThresholdChanged { new_threshold, .. } = event {
                        threshold_for_subscriber.store(new_threshold.to_bits(), Ordering::Relaxed);
                        info!(
                            "ConfidenceSource threshold updated by LearningService: {:.3}",
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
            service.start();
            Some(Arc::new(RwLock::new(service)))
        } else {
            None
        };

        // ── Bus receiver ──────────────────────────────────────────────────
        let inbound_rx = bus
            .take_inbound_rx()
            .expect("Inbound receiver already taken");

        // ── Pipeline ──────────────────────────────────────────────────────
        let tool_registry = Arc::new(RwLock::new(tool_registry));
        let execution_core = Arc::new(crate::execution::ExecutionCore::new(
            provider.clone(),
            Arc::clone(&tool_registry),
        ));

        let plan_execution_core = if stored_plan_repo.is_some() {
            Some(Arc::clone(&execution_core))
        } else {
            None
        };

        // Build IntentPipeline (replaces AgentPipeline + Orchestrator + EngineDispatch)
        let direct_engine = crate::intent_pipeline::engines::direct::DirectEngine::new(
            Arc::clone(&execution_core),
        );
        let reactive_engine = crate::intent_pipeline::engines::reactive::ReactiveEngine::new(
            Arc::clone(&execution_core),
            config.agents.defaults.max_tool_iterations,
        );

        let planned_engine = stored_plan_repo.as_ref().map(|repo| {
            crate::intent_pipeline::engines::planned::PlannedEngine::new(
                Arc::clone(&execution_core),
                repo.clone(),
                provider.clone(),
                config.agents.defaults.model.clone(),
                plan::conversions::str_to_visibility(&config.orchestrator.default_plan_visibility),
            )
        });

        let router = crate::intent_pipeline::router::ExecutionRouter::new(
            direct_engine,
            reactive_engine,
            planned_engine,
            config.orchestrator.max_escalations,
        );

        let analyzer = crate::intent_pipeline::analyzer::IntentAnalyzer::new(
            provider.clone(),
            &config.agents.defaults.model,
            &config.orchestrator,
        )
        .with_strategy_repo(repos.strategies.clone());

        let cost_tracker = Arc::new(crate::output::CostTracker::from_repo(
            storage::UsageRepo::new(storage_pool.inner().clone()),
        ));

        let pipeline_config = crate::intent_pipeline::pipeline::PipelineConfig {
            execution_model: config.agents.defaults.model.clone(),
            system_prompt: String::new(),
            context_window: provider.context_window(),
            max_response_tokens: config.agents.defaults.max_tokens as usize,
            channel: "unknown".to_string(),
            provider_name: provider.name().to_string(),
        };

        let pipeline = Arc::new(
            crate::intent_pipeline::IntentPipeline::new(
                analyzer,
                context_engine::ContextEngine::new(),
                router,
                cost_tracker,
                pipeline_config,
            )
            .with_strategy_repo(repos.strategies.clone()),
        );

        info!("Intent pipeline initialized");

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
        let memory_maintenance_token = if self.pool.is_some() && self.vector_store.is_some() {
            let token = CancellationToken::new();
            let maintenance_service =
                crate::memory_maintenance_service::MemoryMaintenanceService::new(
                    self.vector_store.clone().unwrap(),
                    config.conversation.memory.max_age_days,
                    config.conversation.memory.maintenance_interval_hours,
                    token.clone(),
                );
            maintenance_service.spawn();
            Some(token)
        } else {
            None
        };

        // ── Plan cleanup service ──────────────────────────────────────────
        let plan_cleanup_token = if self.pool.is_some() {
            let token = CancellationToken::new();
            let cleanup_service =
                crate::intent_pipeline::visibility::PlanCleanupService::new(
                    repos.plans.clone(),
                    token.clone(),
                );
            cleanup_service.spawn();
            Some(token)
        } else {
            None
        };

        // ── Assemble AgentLoop ────────────────────────────────────────────
        let history_limit = config.conversation.session.history_limit;
        Ok(AgentLoop {
            bus,
            inbound_rx: Some(inbound_rx),
            skill_manager: Arc::clone(&skill_manager),
            provider,
            config,
            context_engine,
            session_manager,
            tool_registry,
            confidence_evaluator,
            running: Arc::new(AtomicBool::new(false)),
            last_active_channel: self.notification_handle,
            reminder_engine,
            recurring_task_spawner,
            _notification_dispatcher: notification_dispatcher,
            _calendar_adapter: calendar_adapter,
            conversation_embedding_handler,
            plan_execution_core,
            plan_repo: stored_plan_repo,
            plan_executing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            learning_service,
            plan_completion_handler,
            pipeline,
            strategy_repo: Some(repos.strategies.clone()),
            history_limit,
            _session_cleanup_token: session_cleanup_token,
            _memory_maintenance_token: memory_maintenance_token,
            _plan_cleanup_token: plan_cleanup_token,
        })
    }
}

impl AgentLoop {
    /// Create a builder for constructing an `AgentLoop`.
    pub fn builder() -> AgentLoopBuilder {
        AgentLoopBuilder::new()
    }
}
