//! Agent loop construction: tool registration, handler wiring, pipeline assembly.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

use bus::MessageBus;
use common::Result;
use config::Config;
use providers::DynProvider;
use session::SessionManager;
use tools::{
    calendar_tool::{CalendarHandler, CalendarTool},
    cron_tool::CronTool,
    enrichment::EnrichmentHandler,
    filesystem::register_fs_tools,
    goal_tool::{GoalHandler, GoalTool},
    learning_tool::{LearningHandler, LearningTool},
    message::MessageTool,
    plan_tool::PlanCompletionHandler,
    registry::ToolRegistry,
    shell::ExecTool,
    spawn::SpawnTool,
    web::{WebFetchTool, WebSearchTool},
    EmbeddingHandler, FinanceHandler,
};

use super::super::confidence::ConfidenceEvaluator;
use super::super::{CalendarSyncAdapter, ContextBuilder, CronHandlerAdapter, SubagentManager};
use super::{AgentLoop, LastActiveChannel};

impl AgentLoop {
    /// Create a new agent loop with optional cron service and shared instances
    #[allow(clippy::too_many_arguments)]
    pub async fn new_with_cron(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
        cron_service: Option<Arc<scheduling::CronService>>,
        todo_repo: storage::TodoRepo,
        embedding_repo: Option<storage::EmbeddingRepo>,
        goal_repo: Option<storage::GoalRepo>,
        plan_repo: Option<storage::PlanRepo>,
        notification_handle: Option<LastActiveChannel>,
        outcome_repo: storage::OutcomeRepo,
        learning_state_repo: storage::LearningStateRepo,
        memory_note_repo: storage::MemoryNoteRepo,
        strategy_repo: Option<storage::StrategyRepo>,
        calendar_sync_repo: storage::CalendarSyncRepo,
        event_cache_repo: storage::CalendarEventCacheRepo,
        conv_embedding_repo: Option<storage::ConvEmbeddingRepo>,
        finance_repos: Option<storage::Repos>,
    ) -> Result<Self> {
        let workspace = config.workspace_path();

        // Create context builder (SQL-backed memory)
        let mut context_builder = ContextBuilder::new(
            workspace.clone(),
            config.timezone.clone(),
            Some(todo_repo.clone()),
            goal_repo.clone(),
            memory_note_repo,
        )
        .await;
        context_builder.init().await.map_err(|e| {
            common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                "Failed to initialize context: {}",
                e
            )))
        })?;

        // Filter skills to those enabled by pack configuration
        if !config.packs.enabled_skills.is_empty() {
            context_builder.filter_skills(&config.packs.enabled_skills);
        }

        // Wrap early so both the learning subscriber and Ok(Self{}) share the same Arc.
        let context_builder = Arc::new(RwLock::new(context_builder));

        // Create session manager
        let sessions_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".klyntbot")
            .join("sessions");
        let session_manager = SessionManager::new(sessions_dir).await;

        // Create subagent manager
        let brave_api_key = (!config.tools.web.brave_api_key.is_empty())
            .then(|| config.tools.web.brave_api_key.expose().clone());

        let subagent_manager = Arc::new(
            SubagentManager::builder(Arc::clone(&provider), workspace.clone())
                .inbound_sender(bus.inbound_sender())
                .model(config.agents.defaults.model.clone())
                .brave_api_key(brave_api_key.clone())
                .web_max_results(config.tools.web.max_results)
                .exec_timeout(config.tools.exec.timeout)
                .restrict_to_workspace(config.tools.restrict_to_workspace)
                .build(),
        );

        // Create tool registry
        let mut tool_registry = ToolRegistry::new();

        // Register filesystem tools
        let allowed_dir = if config.tools.restrict_to_workspace {
            Some(workspace.clone())
        } else {
            None
        };

        register_fs_tools(&mut tool_registry, allowed_dir);

        // Register shell tool
        tool_registry.register(ExecTool::new(
            config.tools.exec.timeout,
            Some(workspace.clone()),
            config.tools.restrict_to_workspace,
        ));

        // Register web tools (reuse brave_api_key from above)
        tool_registry.register(WebSearchTool::new(
            brave_api_key,
            config.tools.web.max_results,
        ));
        tool_registry.register(WebFetchTool::new());

        // Register message tool
        tool_registry.register(MessageTool::new(bus.outbound_sender()));

        // Register ask_user tool
        tool_registry.register(tools::ask_user::AskUserTool);

        // Register spawn tool with subagent manager as handler
        tool_registry.register(SpawnTool::with_handler(
            Arc::clone(&subagent_manager) as Arc<dyn tools::spawn::SpawnHandler>
        ));

        // Register cron tool (with service if provided)
        if let Some(cron_svc) = cron_service {
            let adapter = Arc::new(CronHandlerAdapter::new(cron_svc));
            tool_registry.register(CronTool::with_handler(adapter));
        }

        // Clone todo_repo before moving into TodoTool (it's still needed by calendar, reminders, etc.)
        let todo_repo_for_memory = todo_repo.clone();
        let todo_repo_shared = todo_repo.clone();

        // Register todo tool (SQL repo)
        let mut todo_tool = tools::todo::TodoTool::new(
            todo_repo,
            config.todo.focus.max_slots,
            config.todo.focus.deadline_hours,
            config.timezone.clone(),
        );

        // Create NotificationDispatcher early so it can be shared with calendar adapter
        let notification_dispatcher = if !config.todo.notifications.targets.is_empty() {
            Some(Arc::new(super::super::NotificationDispatcher::new(
                bus.outbound_sender(),
                config.todo.notifications.clone(),
            )))
        } else {
            None
        };

        // Register calendar tool (if any provider is enabled)
        let calendar_adapter = if config.calendar.is_any_enabled() {
            let adapter = Arc::new(
                CalendarSyncAdapter::new(
                    todo_repo_shared.clone(),
                    calendar_sync_repo,
                    event_cache_repo,
                    &config.calendar,
                    config.timezone.clone(),
                    notification_dispatcher.clone(),
                    config.calendar.bidirectional_sync,
                )
                .await?,
            );

            // Inject calendar handler into TodoTool for immediate sync
            todo_tool =
                todo_tool.with_calendar_handler(Arc::clone(&adapter) as Arc<dyn CalendarHandler>);

            tool_registry.register(CalendarTool::new(
                Arc::clone(&adapter) as Arc<dyn CalendarHandler>
            ));
            Some(adapter)
        } else {
            None
        };

        // Register enrichment engine (if enabled)
        if config.todo.enrichment.enabled {
            let enrichment_engine = Arc::new(super::super::enrichment::EnrichmentEngine::new(
                config.todo.enrichment.clone(),
            ));

            todo_tool = todo_tool.with_enrichment_handler(
                Arc::clone(&enrichment_engine) as Arc<dyn EnrichmentHandler>
            );
        }

        // Create outcome store + recorder early so TodoTool can report enrichment feedback.
        // The full LearningService is wired later using the same store.
        let outcome_store = if config.learning.enabled {
            Some(Arc::new(RwLock::new(crate::learning::OutcomeStore::new(
                outcome_repo,
            ))))
        } else {
            None
        };
        let outcome_recorder = outcome_store
            .as_ref()
            .map(|store| Arc::new(crate::learning::OutcomeRecorder::new(Arc::clone(store))));

        // Wire enrichment feedback into learning system
        if let Some(ref recorder) = outcome_recorder {
            todo_tool = todo_tool.with_feedback_handler(
                Arc::clone(recorder) as Arc<dyn tools::EnrichmentFeedbackHandler>
            );
        }

        // Create shared embedding engine (used by both todo and conversation embedding)
        let embedding_engine = Arc::new(tools::EmbeddingEngine::new());

        // Register todo embedding (if enabled) and capture for MemoryTool
        let todo_embedding_handler: Option<Arc<dyn EmbeddingHandler>>;
        let todo_embedding_repo: Option<storage::EmbeddingRepo>;

        if config.todo.search.enabled {
            if let Some(emb_repo) = embedding_repo {
                let embedding_handler = Arc::new(tools::EmbeddingEngineImpl::new(
                    Arc::clone(&embedding_engine),
                    emb_repo.clone(),
                ));

                // Clone for MemoryTool before moving into TodoTool
                todo_embedding_repo = Some(emb_repo.clone());

                todo_tool = todo_tool
                    .with_embedding_handler(
                        Arc::clone(&embedding_handler) as Arc<dyn EmbeddingHandler>
                    )
                    .with_embedding_repo(emb_repo)
                    .with_search_config(
                        config.todo.search.semantic_threshold,
                        config.todo.search.rrf_k,
                    );

                // Capture for MemoryTool unified search
                todo_embedding_handler =
                    Some(Arc::clone(&embedding_handler) as Arc<dyn EmbeddingHandler>);
            } else {
                todo_embedding_handler = None;
                todo_embedding_repo = None;
            }
        } else {
            todo_embedding_handler = None;
            todo_embedding_repo = None;
        }

        tool_registry.register(todo_tool);

        // Register goal tool (if goal_repo is provided)
        if let Some(ref gr) = goal_repo {
            let goal_handler = Arc::new(super::super::GoalHandlerImpl::new(gr.clone()));
            tool_registry.register(GoalTool::new(Some(goal_handler as Arc<dyn GoalHandler>)));
        }

        // Register plan tool and keep plan_repo reference for run_plan_execution()
        let stored_plan_repo = if let Some(ref pr) = plan_repo {
            let plan_handler = Arc::new(super::super::PlanHandlerImpl::new(pr.clone()));
            tool_registry.register(tools::plan_tool::PlanTool::new(Some(
                plan_handler as Arc<dyn tools::plan_tool::PlanHandler>,
            )));
            Some(pr.clone())
        } else {
            None
        };

        // Wire plan completion handler (updates linked goal when plan finishes)
        let plan_completion_handler: Option<Arc<dyn PlanCompletionHandler>> =
            goal_repo.as_ref().map(|gr| {
                Arc::new(
                    super::super::plan_completion_handler::PlanCompletionHandlerImpl::new(
                        gr.clone(),
                    ),
                ) as Arc<dyn PlanCompletionHandler>
            });

        // Register conversation embedding handler (Phase 4.1)
        let conversation_embedding_handler = if config.conversation.embedding.enabled {
            if let Some(repo) = conv_embedding_repo {
                let conv_store = tools::ConversationEmbeddingStore::new(repo);
                let handler = Arc::new(
                    super::super::conversation_embedding_handler::ConversationEmbeddingHandlerImpl::new(
                        Arc::clone(&embedding_engine),
                        conv_store,
                    ),
                );
                Some(handler as Arc<dyn tools::ConversationEmbeddingHandler>)
            } else {
                None
            }
        } else {
            None
        };

        // Register MemoryTool (Phase 4.1) - if conversation search is enabled
        if config.conversation.search.enabled {
            if let Some(ref handler) = conversation_embedding_handler {
                let mut memory_tool = tools::MemoryTool::new()
                    .with_conversation_handler(Arc::clone(handler))
                    .with_todo_repo(todo_repo_for_memory)
                    .with_threshold(config.conversation.search.semantic_threshold)
                    .with_rrf_k(config.todo.search.rrf_k);

                // Inject todo embedding dependencies if available
                if let Some(ref h) = todo_embedding_handler {
                    memory_tool = memory_tool.with_todo_embedding_handler(Arc::clone(h));
                }
                if let Some(repo) = todo_embedding_repo {
                    memory_tool = memory_tool.with_embedding_repo(repo);
                }

                tool_registry.register(memory_tool);
            }
        }

        // ── Finance Tool Registration ──────────────────────────────────────────
        if config.finance.enabled {
            if let Some(fin_repos) = finance_repos {
                let price_service =
                    tools::PriceService::new(config.finance.price_refresh.cache_ttl_minutes);

                let finance_handler_impl =
                    Arc::new(crate::finance_adapter::FinanceHandlerImpl::new(
                        fin_repos.clone(),
                        price_service.clone(),
                        config.finance.clone(),
                    ));

                let finance_tool = tools::FinanceTool::new(
                    fin_repos.finance_accounts,
                    fin_repos.finance_transactions,
                    fin_repos.finance_budgets,
                    fin_repos.finance_investments,
                    fin_repos.finance_goals,
                    fin_repos.finance_liabilities,
                    price_service,
                    config.finance.default_currency.clone(),
                )
                .with_finance_handler(Arc::clone(&finance_handler_impl) as Arc<dyn FinanceHandler>);

                tool_registry.register(finance_tool);
            }
        }

        // Confidence evaluator (with per-tool overrides if configured)
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

        // Create ReminderEngine using the shared notification dispatcher and calendar handler
        let reminder_engine = if let Some(ref dispatcher) = notification_dispatcher {
            let calendar_handler_opt = calendar_adapter
                .as_ref()
                .map(|adapter| Arc::clone(adapter) as Arc<dyn CalendarHandler>);
            let mut engine = super::super::ReminderEngine::new(
                todo_repo_shared.clone(),
                calendar_handler_opt,
                Arc::clone(dispatcher),
                std::time::Duration::from_secs(300), // Check every 5 minutes
            );
            engine.start();
            Some(Arc::new(RwLock::new(engine)))
        } else {
            None
        };

        // Start RecurringTaskSpawner (checks every 60s for due recurring templates)
        let mut recurring_spawner = super::super::RecurringTaskSpawner::new(
            todo_repo_shared.clone(),
            config.timezone.clone(),
            std::time::Duration::from_secs(60),
        );
        recurring_spawner.start();
        let recurring_task_spawner = Some(Arc::new(RwLock::new(recurring_spawner)));

        // Initialize learning background service (reuses outcome_store + recorder created earlier)
        let learning_service = if let Some(ref store) = outcome_store {
            let adaptive = Arc::new(RwLock::new(
                crate::learning::adaptive::AdaptiveThresholds::new(
                    learning_state_repo.clone(),
                    config.confidence.threshold,
                    config.learning.min_threshold,
                    config.learning.max_threshold,
                    config.learning.min_outcomes_for_adaptation,
                )
                .await,
            ));

            // Register LearningTool so the LLM can query learning insights
            let learning_handler = Arc::new(super::super::LearningHandlerImpl::new(
                Arc::clone(store),
                Arc::clone(&adaptive),
            ));
            tool_registry.register(LearningTool::new(Some(
                learning_handler as Arc<dyn LearningHandler>,
            )));

            // Create event bus: LearningService publishes, AgentLoop subscribes.
            let event_bus = Arc::new(bus::LearningEventBus::new(16));

            // Spawn subscriber task — updates ContextBuilder threshold on ThresholdChanged.
            // Task self-terminates when the event bus sender drops (i.e., after LearningService stops).
            let cb_for_subscriber = Arc::clone(&context_builder);
            let mut event_rx = event_bus.subscribe();
            tokio::spawn(async move {
                while let Ok(event) = event_rx.recv().await {
                    if let bus::LearningEvent::ThresholdChanged { new_threshold, .. } = event {
                        let mut cb = cb_for_subscriber.write().await;
                        cb.set_confidence_threshold(new_threshold);
                        info!(
                            "ContextBuilder threshold updated by LearningService: {:.3}",
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

        // Take ownership of the inbound receiver
        let inbound_rx = bus
            .take_inbound_rx()
            .expect("Inbound receiver already taken");

        // Build the adaptive orchestrator pipeline.
        // All dependencies (provider, tool_registry, config) are already available.
        let tool_registry = Arc::new(RwLock::new(tool_registry));
        let execution_core = Arc::new(crate::execution::ExecutionCore::new(
            provider.clone(),
            Arc::clone(&tool_registry),
        ));
        // Share execution_core with plan execution (clone Arc before passing to dispatch)
        let plan_execution_core = if stored_plan_repo.is_some() {
            Some(Arc::clone(&execution_core))
        } else {
            None
        };
        let engine_dispatch = Arc::new(crate::execution::EngineDispatch::new(execution_core));
        let mut orchestrator =
            crate::orchestrator::Orchestrator::new(provider.clone(), &config.agents.defaults.model);
        if let Some(repo) = strategy_repo {
            orchestrator = orchestrator.with_strategy_repo(repo);
        }
        let orchestrator = Arc::new(orchestrator);
        let data_dir = config::config_dir()
            .unwrap_or_else(|_| config.workspace_path())
            .join("data");
        let cost_tracker = Arc::new(crate::output::CostTracker::new(data_dir));

        let pipeline_config = crate::pipeline::PipelineConfig {
            execution_model: config.agents.defaults.model.clone(),
            system_prompt: String::new(), // populated per-request from ContextBuilder
            context_window: provider.context_window(),
            max_response_tokens: config.agents.defaults.max_tokens as usize,
            channel: "unknown".to_string(), // overridden per-request
            provider_name: provider.name().to_string(),
        };

        let pipeline = Arc::new(crate::pipeline::AgentPipeline::new(
            orchestrator,
            engine_dispatch,
            cost_tracker,
            pipeline_config,
        ));

        info!("Adaptive orchestrator pipeline initialized");

        Ok(Self {
            bus,
            inbound_rx: Some(inbound_rx),
            provider,
            config,
            context_builder,
            session_manager: Arc::new(RwLock::new(session_manager)),
            tool_registry,
            confidence_evaluator,
            running: Arc::new(AtomicBool::new(false)),
            last_active_channel: notification_handle,
            reminder_engine,
            recurring_task_spawner,
            _notification_dispatcher: notification_dispatcher,
            _calendar_adapter: calendar_adapter,
            conversation_embedding_handler,
            plan_execution_core,
            plan_repo: stored_plan_repo,
            plan_executing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            outcome_recorder,
            learning_service,
            plan_completion_handler,
            pipeline,
        })
    }

    /// Create a new agent loop (without cron service)
    #[allow(clippy::too_many_arguments)] // Delegates to new_with_cron
    pub async fn new(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
        todo_repo: storage::TodoRepo,
        embedding_repo: Option<storage::EmbeddingRepo>,
        goal_repo: Option<storage::GoalRepo>,
        plan_repo: Option<storage::PlanRepo>,
        outcome_repo: storage::OutcomeRepo,
        learning_state_repo: storage::LearningStateRepo,
        memory_note_repo: storage::MemoryNoteRepo,
        strategy_repo: Option<storage::StrategyRepo>,
        calendar_sync_repo: storage::CalendarSyncRepo,
        event_cache_repo: storage::CalendarEventCacheRepo,
        conv_embedding_repo: Option<storage::ConvEmbeddingRepo>,
        finance_repos: Option<storage::Repos>,
    ) -> Result<Self> {
        Self::new_with_cron(
            bus,
            provider,
            config,
            None,
            todo_repo,
            embedding_repo,
            goal_repo,
            plan_repo,
            None,
            outcome_repo,
            learning_state_repo,
            memory_note_repo,
            strategy_repo,
            calendar_sync_repo,
            event_cache_repo,
            conv_embedding_repo,
            finance_repos,
        )
        .await
    }
}
