//! Agent loop: the core processing engine.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use bus::{InboundMessage, MessageBus, OutboundMessage};
use common::Result;
use config::Config;
use providers::{DynProvider, Message};
use session::SessionManager;
use tokio::sync::mpsc;
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
    EmbeddingHandler, RoutingContext,
};

use super::confidence::ConfidenceEvaluator;
use super::{AgentEvent, CalendarSyncAdapter, ContextBuilder, CronHandlerAdapter, SubagentManager};

/// A request to execute an approved plan via the execution queue.
///
/// Sent through the plan queue mpsc channel to the dedicated worker task,
/// which calls `run_plan_execution()` for each request in order.
pub struct PlanExecutionRequest {
    /// The ID of the plan to execute (must be in Approved state).
    pub plan_id: uuid::Uuid,
    /// Routing context for channel/session routing during execution.
    pub routing_ctx: tools::RoutingContext,
}

/// Default session history limit (number of messages)
const DEFAULT_HISTORY_LIMIT: usize = 50;

/// Handle for consuming streaming agent output.
pub struct StreamingHandle {
    /// Agent events (content chunks, tool status).
    pub event_rx: mpsc::Receiver<AgentEvent>,
    /// Interaction requests from ask_user tool (with oneshot response channels).
    pub interaction_rx: mpsc::Receiver<tools::InteractionBundle>,
    /// Token to cancel processing.
    pub cancel_token: CancellationToken,
    /// Background task handle.
    pub handle: JoinHandle<Result<String>>,
}

/// Type alias for last active channel tracking (channel + chat ID)
type LastActiveChannel = Arc<RwLock<Option<(common::ChannelName, common::ChatId)>>>;

/// Agent loop - the core processing engine
pub struct AgentLoop {
    pub(crate) bus: Arc<MessageBus>,
    inbound_rx: Option<mpsc::Receiver<InboundMessage>>,
    pub(crate) provider: DynProvider,
    pub(crate) config: Config,
    pub(crate) context_builder: Arc<RwLock<ContextBuilder>>,
    pub(crate) session_manager: Arc<RwLock<SessionManager>>,
    pub(crate) tool_registry: Arc<RwLock<ToolRegistry>>,
    pub(crate) confidence_evaluator: Option<ConfidenceEvaluator>,
    running: Arc<AtomicBool>,
    last_active_channel: Option<LastActiveChannel>,
    reminder_engine: Option<Arc<RwLock<super::ReminderEngine>>>,
    recurring_task_spawner: Option<Arc<RwLock<super::RecurringTaskSpawner>>>,
    /// Held for lifetime; shared with CalendarSyncAdapter
    _notification_dispatcher: Option<Arc<super::NotificationDispatcher>>,
    /// Calendar sync adapter (shared with ReminderEngine)
    _calendar_adapter: Option<Arc<CalendarSyncAdapter>>,
    /// Conversation embedding handler for semantic memory (Phase 4.1)
    pub(crate) conversation_embedding_handler: Option<Arc<dyn tools::ConversationEmbeddingHandler>>,
    /// Execution core for multi-cycle plan step execution
    pub(crate) plan_execution_core: Option<Arc<crate::execution::ExecutionCore>>,
    /// Plan store for direct plan state management during execution
    pub(crate) plan_store: Option<Arc<RwLock<plan::PlanStore>>>,
    /// Tracks if a plan is currently executing
    pub(crate) plan_executing: Arc<std::sync::atomic::AtomicBool>,
    /// Outcome recorder for the learning system (None if learning disabled)
    pub(crate) outcome_recorder: Option<Arc<crate::learning::OutcomeRecorder>>,
    /// Background learning service for adaptive threshold updates (None if learning disabled)
    learning_service: Option<Arc<RwLock<crate::learning::LearningService>>>,
    /// Handler called after plan execution finishes (updates linked goal metrics)
    pub(crate) plan_completion_handler: Option<Arc<dyn PlanCompletionHandler>>,
    /// Adaptive orchestrator pipeline: classify → assemble → dispatch → validate → record.
    pub(crate) pipeline: Arc<crate::pipeline::AgentPipeline>,
}

impl AgentLoop {
    /// Create a new agent loop with optional cron service and shared instances
    #[allow(clippy::too_many_arguments)] // Architectural decision: follows existing goal_store pattern
    #[allow(deprecated)] // goal_store_path / plan_store_path pending future SQL migration
    pub async fn new_with_cron(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
        cron_service: Option<Arc<scheduling::CronService>>,
        todo_repo: storage::TodoRepo,
        embedding_repo: Option<storage::EmbeddingRepo>,
        goal_store: Option<Arc<RwLock<goal::GoalStore>>>,
        plan_store: Option<Arc<RwLock<plan::PlanStore>>>,
        notification_handle: Option<LastActiveChannel>,
        outcome_repo: storage::OutcomeRepo,
        learning_state_repo: storage::LearningStateRepo,
        memory_note_repo: storage::MemoryNoteRepo,
        strategy_repo: Option<storage::StrategyRepo>,
        calendar_sync_repo: storage::CalendarSyncRepo,
        event_cache_repo: storage::CalendarEventCacheRepo,
        conv_embedding_repo: Option<storage::ConvEmbeddingRepo>,
    ) -> Result<Self> {
        let workspace = config.workspace_path();

        // Create context builder (SQL-backed memory)
        let mut context_builder = ContextBuilder::new(
            workspace.clone(),
            config.timezone.clone(),
            Some(todo_repo.clone()),
            goal_store.as_ref().map(Arc::clone),
            memory_note_repo,
        )
        .await;
        context_builder.init().await.map_err(|e| {
            common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                "Failed to initialize context: {}",
                e
            )))
        })?;
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
            Some(Arc::new(super::NotificationDispatcher::new(
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
            let enrichment_engine = Arc::new(super::enrichment::EnrichmentEngine::new(
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

        // Register goal tool (if goal_store is provided)
        if let Some(ref gs) = goal_store {
            let goal_handler = Arc::new(super::GoalHandlerImpl::new(Arc::clone(gs)));
            tool_registry.register(GoalTool::new(Some(goal_handler as Arc<dyn GoalHandler>)));
        }

        // Register plan tool and keep plan_store reference for run_plan_execution()
        let stored_plan_store = if let Some(ps) = plan_store {
            let ps_clone = Arc::clone(&ps);
            let plan_handler = Arc::new(super::PlanHandlerImpl::new(ps));
            tool_registry.register(tools::plan_tool::PlanTool::new(Some(
                plan_handler as Arc<dyn tools::plan_tool::PlanHandler>,
            )));
            Some(ps_clone)
        } else {
            None
        };

        // Wire plan completion handler (updates linked goal when plan finishes)
        let plan_completion_handler: Option<Arc<dyn PlanCompletionHandler>> =
            goal_store.as_ref().map(|gs| {
                Arc::new(
                    super::plan_completion_handler::PlanCompletionHandlerImpl::new(Arc::clone(gs)),
                ) as Arc<dyn PlanCompletionHandler>
            });

        // Register conversation embedding handler (Phase 4.1)
        let conversation_embedding_handler = if config.conversation.embedding.enabled {
            if let Some(repo) = conv_embedding_repo {
                let conv_store = tools::ConversationEmbeddingStore::new(repo);
                let handler = Arc::new(
                    super::conversation_embedding_handler::ConversationEmbeddingHandlerImpl::new(
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
            let mut engine = super::ReminderEngine::new(
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
        let mut recurring_spawner = super::RecurringTaskSpawner::new(
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
            let learning_handler = Arc::new(super::LearningHandlerImpl::new(
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
        let plan_execution_core = if stored_plan_store.is_some() {
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
            plan_store: stored_plan_store,
            plan_executing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            outcome_recorder,
            learning_service,
            plan_completion_handler,
            pipeline,
        })
    }

    /// Create a new agent loop (without cron service)
    #[allow(clippy::too_many_arguments)] // Delegates to new_with_cron
    #[allow(deprecated)] // goal_store_path / plan_store_path pending future SQL migration
    pub async fn new(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
        todo_repo: storage::TodoRepo,
        embedding_repo: Option<storage::EmbeddingRepo>,
        outcome_repo: storage::OutcomeRepo,
        learning_state_repo: storage::LearningStateRepo,
        memory_note_repo: storage::MemoryNoteRepo,
        strategy_repo: Option<storage::StrategyRepo>,
        calendar_sync_repo: storage::CalendarSyncRepo,
        event_cache_repo: storage::CalendarEventCacheRepo,
        conv_embedding_repo: Option<storage::ConvEmbeddingRepo>,
    ) -> Result<Self> {
        let goal_path = config.goal_store_path();
        let goal_store = Some(Arc::new(RwLock::new(goal::GoalStore::new(goal_path))));
        let plan_path = config.plan_store_path();
        let plan_store = Some(Arc::new(RwLock::new(plan::PlanStore::new(plan_path))));
        Self::new_with_cron(
            bus,
            provider,
            config,
            None,
            todo_repo,
            embedding_repo,
            goal_store,
            plan_store,
            None,
            outcome_repo,
            learning_state_repo,
            memory_note_repo,
            strategy_repo,
            calendar_sync_repo,
            event_cache_repo,
            conv_embedding_repo,
        )
        .await
    }

    /// Check if a plan is currently executing.
    /// Returns true if plan mode is active, false for normal chat mode.
    /// This determines the iteration limit: 50 for plans, 20 for chat.
    pub fn is_plan_executing(&self) -> bool {
        self.plan_executing.load(Ordering::SeqCst)
    }

    /// Run the agent loop, processing messages from the bus
    pub async fn run(&mut self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        info!("Agent loop started");

        // Take ownership of the receiver
        let mut inbound_rx = self
            .inbound_rx
            .take()
            .expect("AgentLoop::run can only be called once");

        while self.running.load(Ordering::SeqCst) {
            // Wait for next message with timeout
            match tokio::time::timeout(Duration::from_secs(1), inbound_rx.recv()).await {
                Ok(Some(msg)) => {
                    if let Err(e) = self.process_message(msg).await {
                        error!("Error processing message: {}", e);
                    }
                }
                Ok(None) => {
                    // Bus closed
                    break;
                }
                Err(_) => {
                    // Timeout, continue loop
                    continue;
                }
            }
        }

        info!("Agent loop stopped");
        Ok(())
    }

    /// Get a handle to the shutdown flag.
    ///
    /// This allows stopping the agent loop without holding its Mutex lock,
    /// which is important because `run()` holds the lock for its entire
    /// lifetime.
    pub fn shutdown_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    /// Stop the agent loop
    pub async fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Gracefully shutdown all background tasks
    pub async fn shutdown(&self) -> Result<()> {
        // First stop the agent loop
        self.stop().await;

        // Stop the reminder engine
        if let Some(engine) = &self.reminder_engine {
            let mut engine_guard = engine.write().await;
            engine_guard.stop().await;
        }

        // Stop the recurring task spawner
        if let Some(spawner) = &self.recurring_task_spawner {
            let mut spawner_guard = spawner.write().await;
            spawner_guard.stop().await;
        }

        // Stop the learning background service
        if let Some(service) = &self.learning_service {
            let mut service_guard = service.write().await;
            service_guard.stop().await;
        }

        Ok(())
    }

    // run_plan_execution() is in plan_runner.rs

    /// Process a single inbound message
    #[tracing::instrument(skip(self, msg), fields(channel = %msg.channel, sender = %msg.sender_id))]
    async fn process_message(&self, msg: InboundMessage) -> Result<()> {
        // Validate message size
        if let Err(e) = msg.validate() {
            warn!("Message validation failed: {}", e);
            return Ok(()); // silently drop oversized messages
        }

        // Handle system messages (subagent results)
        if msg.channel.as_str() == "system" {
            return self.process_system_message(msg).await;
        }

        // Track last active channel for notifications
        if let Some(last_active) = &self.last_active_channel {
            *last_active.write().await = Some((msg.channel.clone(), msg.chat_id.clone()));
        }

        let preview = if msg.content.len() > 80 {
            format!("{}...", truncate_safe(&msg.content, 80))
        } else {
            msg.content.clone()
        };

        info!(
            "Processing message from {}:{}: {}",
            msg.channel, msg.sender_id, preview
        );

        // Get or create session
        let session_key = msg.session_key();
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(session_key.as_str()).await?;

        // Add user message to session
        session.add_message("user", &msg.content);

        // Async conversation embedding hook for user message
        {
            let msg_id = session
                .messages
                .last()
                .expect("Message should exist after add_message")
                .id
                .clone();
            self.spawn_embed_message(session_key.as_str(), "user", &msg.content, &msg_id);
        }

        // Get session history
        let history = session.get_history(DEFAULT_HISTORY_LIMIT).to_vec();

        // Drop the write lock
        drop(session_manager);

        // Run through pipeline
        let routing_ctx = RoutingContext::new(msg.channel.clone(), msg.chat_id.clone());
        let response_content = self
            .run_pipeline(&msg.content, history, &routing_ctx)
            .await?;

        // Save assistant response to session
        self.save_to_session(session_key.as_str(), &response_content)
            .await;

        // Send response
        let out_msg = OutboundMessage::new(msg.channel, msg.chat_id, response_content);
        self.bus.publish_outbound(out_msg).await?;

        Ok(())
    }

    /// Process system messages (e.g., subagent results)
    async fn process_system_message(&self, msg: InboundMessage) -> Result<()> {
        info!("Processing system message from {}", msg.sender_id);

        // Handle session reset messages
        if msg.sender_id == "telegram_reset" && msg.content == "__RESET_SESSION__" {
            let mut session_manager = self.session_manager.write().await;
            let session_to_save = {
                if let Ok(session) = session_manager.get_or_create(msg.chat_id.as_str()).await {
                    session.clear();
                    Some(session.clone())
                } else {
                    None
                }
            };

            if let Some(session) = session_to_save {
                if let Err(e) = session_manager.save(&session).await {
                    warn!("Failed to save cleared session: {}", e);
                }
            }
            return Ok(());
        }

        // The chat_id contains the original "channel:chat_id" to route back to
        let parts: Vec<&str> = msg.chat_id.as_str().split(':').collect();
        if parts.len() != 2 {
            warn!("Invalid system message chat_id format: {}", msg.chat_id);
            return Ok(());
        }

        let origin_channel = parts[0];
        let origin_chat_id = parts[1];

        // Session key for the original conversation
        let session_key = format!("{}:{}", origin_channel, origin_chat_id);

        // Get or create session and add system message as "user" role
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(&session_key).await?;

        // Format system message with sender_id prefix
        let system_msg_content = format!("[System: {}] {}", msg.sender_id, msg.content);
        session.add_message("user", &system_msg_content);

        // Get session history
        let history = session.get_history(DEFAULT_HISTORY_LIMIT).to_vec();

        // Drop the write lock before processing
        drop(session_manager);

        // Run through pipeline
        let routing_ctx = RoutingContext::new(origin_channel.into(), origin_chat_id.into());
        let response_content = self
            .run_pipeline(&system_msg_content, history, &routing_ctx)
            .await?;

        // Save assistant response to session
        self.save_to_session(session_key.as_str(), &response_content)
            .await;

        // Publish response to origin channel
        let out_msg = OutboundMessage::new(
            origin_channel.to_string(),
            origin_chat_id.to_string(),
            response_content,
        );
        self.bus.publish_outbound(out_msg).await?;

        Ok(())
    }

    /// Save an assistant response to the session.
    /// Check if a conversation message should be embedded based on config.
    fn should_embed_conversation(&self, session_key: &str, role: &str) -> bool {
        // Check if role is excluded
        if self
            .config
            .conversation
            .embedding
            .exclude_roles
            .contains(&role.to_string())
        {
            return false;
        }

        // Check if channel is excluded
        if let Some(channel) = session_key.split(':').next() {
            if self
                .config
                .conversation
                .embedding
                .exclude_channels
                .contains(&channel.to_string())
            {
                return false;
            }
        }

        true
    }

    /// Fire-and-forget embedding of a conversation message.
    ///
    /// Spawns a background task to embed the message if a handler is configured
    /// and the session/role is not excluded.
    fn spawn_embed_message(&self, session_key: &str, role: &str, content: &str, msg_id: &str) {
        if let Some(handler) = &self.conversation_embedding_handler {
            if self.should_embed_conversation(session_key, role) {
                let h = handler.clone();
                let sk = session_key.to_string();
                let r = role.to_string();
                let c = content.to_string();
                let id = msg_id.to_string();

                tokio::spawn(async move {
                    if let Err(e) = h.embed_message(&sk, &r, &c, &id).await {
                        warn!("Failed to embed {} message: {}", r, e);
                    }
                });
            }
        }
    }

    async fn save_to_session(&self, session_key: &str, content: &str) {
        let mut session_manager = self.session_manager.write().await;
        if let Ok(session) = session_manager.get_or_create(session_key).await {
            session.add_message("assistant", content);

            let msg_id = session
                .messages
                .last()
                .expect("Message should exist after add_message")
                .id
                .clone();
            self.spawn_embed_message(session_key, "assistant", content, &msg_id);

            let session_clone = session.clone();
            if let Err(e) = session_manager.save(&session_clone).await {
                warn!("Failed to save session: {}", e);
            }
        }
    }

    /// Convert session history to provider Messages.
    fn convert_history(history: &[session::SessionMessage]) -> Vec<Message> {
        history
            .iter()
            .map(|m| match m.role.as_str() {
                "system" => Message::system(&m.content),
                "user" => Message::user(&m.content),
                "assistant" => Message::assistant(&m.content),
                _ => Message::user(&m.content),
            })
            .collect()
    }

    /// Get tool definitions and names from the registry.
    async fn get_tool_info(&self) -> (Vec<serde_json::Value>, Vec<String>) {
        let tool_registry = self.tool_registry.read().await;
        let tool_defs = tool_registry.get_definitions();
        let tool_names = tool_registry.tool_names();
        (tool_defs, tool_names)
    }

    /// Run a message through the pipeline with the given routing context.
    async fn run_pipeline(
        &self,
        content: &str,
        history: Vec<session::SessionMessage>,
        routing_ctx: &RoutingContext,
    ) -> Result<String> {
        let mut context_builder = self.context_builder.write().await;
        let system_prompt = context_builder
            .build_system_prompt(routing_ctx.channel.as_str(), routing_ctx.chat_id.as_str())
            .await;
        drop(context_builder);

        let history_messages = Self::convert_history(&history);
        let (tool_defs, tool_names) = self.get_tool_info().await;
        let tool_name_refs: Vec<&str> = tool_names.iter().map(|s| s.as_str()).collect();

        let result = self
            .pipeline
            .process_message(
                content,
                history_messages,
                &tool_defs,
                &tool_name_refs,
                routing_ctx,
                Some(&system_prompt),
            )
            .await?;

        info!(
            "Pipeline: strategy={}, escalations={}",
            result.strategy_used, result.escalations
        );

        Ok(result.content)
    }

    /// Process a message directly (for CLI mode).
    ///
    /// Returns the agent's response text directly instead of publishing to the bus.
    pub async fn process_direct(&self, content: String, session_key: String) -> Result<String> {
        let preview = if content.len() > 80 {
            format!("{}...", truncate_safe(&content, 80))
        } else {
            content.clone()
        };
        debug!("Processing direct message: {}", preview);

        // Get or create session
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(&session_key).await?;
        session.add_message("user", &content);

        // Async conversation embedding hook for user message (CLI)
        {
            let msg_id = session
                .messages
                .last()
                .expect("Message should exist after add_message")
                .id
                .clone();
            self.spawn_embed_message(&session_key, "user", &content, &msg_id);
        }

        let history = session.get_history(DEFAULT_HISTORY_LIMIT).to_vec();
        drop(session_manager);

        // Run through pipeline
        let routing_ctx = RoutingContext::new("cli".into(), session_key.clone().into());
        let response_content = self.run_pipeline(&content, history, &routing_ctx).await?;

        // Save to session
        self.save_to_session(&session_key, &response_content).await;

        Ok(response_content)
    }

    /// Process a message with real-time event streaming and cancellation support.
    ///
    /// Returns:
    /// - `event_rx`: Receiver for agent events (content, tool status, prompts)
    /// - `user_tx`: Sender for user responses to interactive prompts
    /// - `cancel_token`: Token to cancel processing
    /// - `handle`: Join handle for the background task
    pub async fn process_direct_streaming(
        self: &Arc<Self>,
        content: String,
        session_key: String,
    ) -> Result<StreamingHandle> {
        let preview = if content.len() > 80 {
            format!("{}...", truncate_safe(&content, 80))
        } else {
            content.clone()
        };
        debug!("Processing streaming direct message: {}", preview);

        // Get or create session and build messages before spawning
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(&session_key).await?;
        session.add_message("user", &content);

        // Async conversation embedding hook for user message (CLI streaming)
        {
            let msg_id = session
                .messages
                .last()
                .expect("Message should exist after add_message")
                .id
                .clone();
            self.spawn_embed_message(&session_key, "user", &content, &msg_id);
        }

        let history = session.get_history(DEFAULT_HISTORY_LIMIT).to_vec();
        drop(session_manager);

        // Create event channel and interaction channel
        let (event_tx, event_rx) = mpsc::channel(64);
        let (interaction_tx, interaction_rx) = mpsc::channel(4);

        // Routing context with interaction channel for ask_user tool
        let routing_ctx = RoutingContext::with_interaction(
            "cli".into(),
            session_key.clone().into(),
            interaction_tx,
        );

        let cancel_token = CancellationToken::new();

        // Clone Arcs for the spawned task
        let agent = Arc::clone(self);
        let sk = session_key.clone();

        let handle = tokio::spawn(async move {
            let result = match agent.run_pipeline(&content, history, &routing_ctx).await {
                Ok(response) => {
                    // Emit the full response for the CLI renderer
                    let _ = event_tx
                        .send(AgentEvent::ContentChunk(response.clone()))
                        .await;
                    let _ = event_tx.send(AgentEvent::Done(response.clone())).await;
                    Ok(response)
                }
                Err(e) => {
                    let _ = event_tx.send(AgentEvent::Error(e.to_string())).await;
                    Err(e)
                }
            };

            // Save to session regardless of success/failure
            if let Ok(ref content) = result {
                agent.save_to_session(&sk, content).await;
            }

            result
        });

        Ok(StreamingHandle {
            event_rx,
            interaction_rx,
            cancel_token,
            handle,
        })
    }

    /// Get the model name from config (for display purposes).
    pub fn model_name(&self) -> &str {
        &self.config.agents.defaults.model
    }
}

/// Safely truncate a string to approximately `max_bytes` without splitting
/// multi-byte UTF-8 characters. Returns the original string if already short enough.
fn truncate_safe(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let end = s
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max_bytes)
        .last()
        .unwrap_or(0);
    &s[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use bus::{LearningEvent, LearningEventBus};

    /// AC-I2.3/2.4: AgentLoop subscriber task updates ContextBuilder threshold
    /// when LearningService publishes a ThresholdChanged event.
    #[tokio::test]
    async fn test_learning_subscriber_updates_context_threshold() {
        use crate::ContextBuilder;

        let workspace = std::path::PathBuf::from("/tmp/test-subscriber-i2-agent");
        let pool =
            storage::StoragePool::connect_lazy("postgres://localhost/klyntbot_test").unwrap();
        let memory_note_repo = storage::MemoryNoteRepo::new(pool.inner().clone());
        let ctx = Arc::new(RwLock::new(
            ContextBuilder::new(workspace, "UTC".to_string(), None, None, memory_note_repo).await,
        ));

        let event_bus = Arc::new(LearningEventBus::new(16));
        let ctx_clone = Arc::clone(&ctx);
        let mut rx = event_bus.subscribe();

        // Spawn subscriber (same pattern as AgentLoop::new_with_cron wires it)
        let handle = tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                if let LearningEvent::ThresholdChanged { new_threshold, .. } = event {
                    let mut cb = ctx_clone.write().await;
                    cb.set_confidence_threshold(new_threshold);
                }
            }
        });

        // Publish threshold change (what LearningService will do)
        event_bus
            .publish(LearningEvent::ThresholdChanged {
                old_threshold: 0.70,
                new_threshold: 0.82,
                reason: "test_subscriber".to_string(),
            })
            .await;

        tokio::time::sleep(Duration::from_millis(50)).await;

        let cb = ctx.read().await;
        assert!(
            (cb.confidence_threshold() - 0.82).abs() < f32::EPSILON,
            "Expected threshold 0.82, got {}",
            cb.confidence_threshold()
        );

        handle.abort();
    }
}
