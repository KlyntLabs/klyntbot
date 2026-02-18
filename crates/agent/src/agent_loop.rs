//! Agent loop: the core processing engine.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use bus::{InboundMessage, MessageBus, OutboundMessage};
use common::Result;
use config::Config;
use futures_util::future::join_all;
use futures_util::StreamExt;
use providers::{tool_calls_to_messages, ChatParams, DynProvider, Message};
use session::SessionManager;
use std::collections::HashMap;
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

use super::confidence::{self, ConfidenceEvaluator, DecisionAction, DecisionLogger};
use super::{AgentEvent, CalendarSyncAdapter, ContextBuilder, CronHandlerAdapter, SubagentManager};

/// Maximum number of tool-calling iterations before returning final response
const MAX_TOOL_ITERATIONS: usize = 20;

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

/// Send an event if the channel is available. No-op if `None`.
macro_rules! emit {
    ($tx:expr, $event:expr) => {
        if let Some(tx) = &$tx {
            let _ = tx.send($event).await;
        }
    };
}

/// Optional event channel for interactive streaming (CLI mode).
struct StreamingChannels {
    event_tx: Option<mpsc::Sender<AgentEvent>>,
}

impl StreamingChannels {
    /// No streaming — all channels disabled.
    fn none() -> Self {
        Self { event_tx: None }
    }
}

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
    bus: Arc<MessageBus>,
    inbound_rx: Option<mpsc::Receiver<InboundMessage>>,
    provider: DynProvider,
    config: Config,
    context_builder: Arc<RwLock<ContextBuilder>>,
    session_manager: Arc<RwLock<SessionManager>>,
    tool_registry: Arc<RwLock<ToolRegistry>>,
    #[allow(dead_code)] // Will be used when full agent loop implementation is completed
    subagent_manager: Arc<SubagentManager>,
    confidence_evaluator: Option<ConfidenceEvaluator>,
    decision_logger: DecisionLogger,
    running: Arc<AtomicBool>,
    last_active_channel: Option<LastActiveChannel>,
    reminder_engine: Option<Arc<RwLock<super::ReminderEngine>>>,
    recurring_task_spawner: Option<Arc<RwLock<super::RecurringTaskSpawner>>>,
    #[allow(dead_code)] // Held for lifetime; shared with CalendarSyncAdapter
    notification_dispatcher: Option<Arc<super::NotificationDispatcher>>,
    /// Calendar sync adapter (shared with ReminderEngine)
    #[allow(dead_code)] // Held for lifetime; shared with ReminderEngine
    calendar_adapter: Option<Arc<CalendarSyncAdapter>>,
    /// Conversation embedding handler for semantic memory (Phase 4.1)
    conversation_embedding_handler: Option<Arc<dyn tools::ConversationEmbeddingHandler>>,
    /// Plan executor for structured multi-step execution (Phase 2)
    plan_executor: Option<super::PlanExecutor>,
    /// Plan store for direct plan state management during execution
    plan_store: Option<Arc<RwLock<plan::PlanStore>>>,
    /// Tracks if a plan is currently executing (affects iteration limit)
    plan_executing: Arc<std::sync::atomic::AtomicBool>,
    /// Outcome recorder for the learning system (None if learning disabled)
    outcome_recorder: Option<Arc<crate::learning::OutcomeRecorder>>,
    /// Background learning service for adaptive threshold updates (None if learning disabled)
    learning_service: Option<Arc<RwLock<crate::learning::LearningService>>>,
    /// Handler called after plan execution finishes (updates linked goal metrics)
    plan_completion_handler: Option<Arc<dyn PlanCompletionHandler>>,
    /// Adaptive orchestrator pipeline (v2). When Some, `process_message_v2` routes
    /// through classify → assemble → dispatch → validate → record instead of the
    /// legacy `run_agent_loop` iteration loop.
    pipeline: Option<Arc<crate::pipeline::AgentPipeline>>,
}

impl AgentLoop {
    /// Create a new agent loop with optional cron service and shared instances
    #[allow(clippy::too_many_arguments)] // Architectural decision: follows existing goal_store pattern
    pub async fn new_with_cron(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
        cron_service: Option<Arc<scheduling::CronService>>,
        todo_store: Arc<RwLock<tools::todo_store::TodoStore>>,
        goal_store: Option<Arc<RwLock<goal::GoalStore>>>,
        plan_store: Option<Arc<RwLock<plan::PlanStore>>>,
        notification_handle: Option<LastActiveChannel>,
    ) -> Result<Self> {
        let workspace = config.workspace_path();

        // Create context builder
        let mut context_builder = ContextBuilder::new(
            workspace.clone(),
            config.timezone.clone(),
            Some(Arc::clone(&todo_store)),
            goal_store.as_ref().map(Arc::clone),
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

        // Register todo tool
        let mut todo_tool = tools::todo::TodoTool::new(
            Arc::clone(&todo_store),
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
                    Arc::clone(&todo_store),
                    &config.calendar,
                    config.timezone.clone(),
                    notification_dispatcher.clone(),
                    config.calendar.bidirectional_sync(),
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
            let store_path = config.learning_outcomes_path();
            Some(Arc::new(RwLock::new(crate::learning::OutcomeStore::new(
                store_path,
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
        let todo_embedding_store: Option<Arc<RwLock<tools::EmbeddingStore>>>;

        if config.todo.search.enabled {
            let emb_store_path = config.embedding_store_path();
            let emb_store = Arc::new(RwLock::new(tools::EmbeddingStore::new(emb_store_path)));
            let embedding_handler = Arc::new(tools::EmbeddingEngineImpl::new(
                Arc::clone(&embedding_engine),
                Arc::clone(&emb_store),
            ));

            todo_tool = todo_tool
                .with_embedding_handler(Arc::clone(&embedding_handler) as Arc<dyn EmbeddingHandler>)
                .with_embedding_store(Arc::clone(&emb_store))
                .with_search_config(
                    config.todo.search.semantic_threshold,
                    config.todo.search.rrf_k,
                );

            // Capture for MemoryTool unified search
            todo_embedding_handler =
                Some(Arc::clone(&embedding_handler) as Arc<dyn EmbeddingHandler>);
            todo_embedding_store = Some(emb_store);
        } else {
            todo_embedding_handler = None;
            todo_embedding_store = None;
        }

        tool_registry.register(todo_tool);

        // Register goal tool (if goal_store is provided)
        if let Some(ref gs) = goal_store {
            let goal_handler = Arc::new(super::GoalHandlerImpl::new(Arc::clone(gs)));
            tool_registry.register(GoalTool::new(Some(goal_handler as Arc<dyn GoalHandler>)));
        }

        // Register plan tool and create plan executor (if plan_store is provided)
        let (plan_executor, stored_plan_store) = if let Some(ps) = plan_store {
            let ps_clone = Arc::clone(&ps);
            let plan_handler = Arc::new(super::PlanHandlerImpl::new(ps));
            tool_registry.register(tools::plan_tool::PlanTool::new(Some(
                plan_handler as Arc<dyn tools::plan_tool::PlanHandler>,
            )));
            // Create PlanExecutor and keep plan_store reference for run_plan_execution()
            (Some(super::PlanExecutor::new()), Some(ps_clone))
        } else {
            (None, None)
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
            let conv_store_path = dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".klyntbot")
                .join("data")
                .join("conversation_embeddings.jsonl");

            let conv_store = Arc::new(RwLock::new(tools::ConversationEmbeddingStore::new(
                conv_store_path,
            )));
            let handler = Arc::new(
                super::conversation_embedding_handler::ConversationEmbeddingHandlerImpl::new(
                    Arc::clone(&embedding_engine),
                    conv_store,
                ),
            );
            Some(handler as Arc<dyn tools::ConversationEmbeddingHandler>)
        } else {
            None
        };

        // Register MemoryTool (Phase 4.1) - if conversation search is enabled
        if config.conversation.search.enabled {
            if let Some(ref handler) = conversation_embedding_handler {
                let mut memory_tool = tools::MemoryTool::new()
                    .with_conversation_handler(Arc::clone(handler))
                    .with_todo_store(Arc::clone(&todo_store))
                    .with_threshold(config.conversation.search.semantic_threshold)
                    .with_rrf_k(config.todo.search.rrf_k);

                // Inject todo embedding dependencies if available
                if let Some(ref h) = todo_embedding_handler {
                    memory_tool = memory_tool.with_todo_embedding_handler(Arc::clone(h));
                }
                if let Some(ref s) = todo_embedding_store {
                    memory_tool = memory_tool.with_todo_embedding_store(Arc::clone(s));
                }

                tool_registry.register(memory_tool);
            }
        }

        // Confidence evaluator
        let confidence_evaluator = if config.confidence.enabled {
            Some(ConfidenceEvaluator::new(config.confidence.threshold))
        } else {
            None
        };

        let decision_logger = match &config.confidence.log_path {
            Some(path) => DecisionLogger::new(path.clone()),
            None => DecisionLogger::default_path(),
        };

        // Create ReminderEngine using the shared notification dispatcher and calendar handler
        let reminder_engine = if let Some(ref dispatcher) = notification_dispatcher {
            let calendar_handler_opt = calendar_adapter
                .as_ref()
                .map(|adapter| Arc::clone(adapter) as Arc<dyn CalendarHandler>);
            let mut engine = super::ReminderEngine::new(
                Arc::clone(&todo_store),
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
            Arc::clone(&todo_store),
            config.timezone.clone(),
            std::time::Duration::from_secs(60),
        );
        recurring_spawner.start();
        let recurring_task_spawner = Some(Arc::new(RwLock::new(recurring_spawner)));

        // Initialize learning background service (reuses outcome_store + recorder created earlier)
        let learning_service = if let Some(ref store) = outcome_store {
            let state_path = config.learning_state_path();
            let adaptive = Arc::new(RwLock::new(
                crate::learning::adaptive::AdaptiveThresholds::load(
                    state_path,
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
        let execution_core = Arc::new(
            crate::execution::ExecutionCore::new(provider.clone(), Arc::clone(&tool_registry)),
        );
        let engine_dispatch = Arc::new(crate::execution::EngineDispatch::new(execution_core));
        let orchestrator = Arc::new(crate::orchestrator::Orchestrator::new(
            provider.clone(),
            &config.agents.defaults.model,
        ));
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

        let pipeline = Some(Arc::new(crate::pipeline::AgentPipeline::new(
            orchestrator,
            engine_dispatch,
            cost_tracker,
            pipeline_config,
        )));

        info!("Adaptive orchestrator pipeline initialized");

        Ok(Self {
            bus,
            inbound_rx: Some(inbound_rx),
            provider,
            config,
            context_builder,
            session_manager: Arc::new(RwLock::new(session_manager)),
            tool_registry,
            subagent_manager,
            confidence_evaluator,
            decision_logger,
            running: Arc::new(AtomicBool::new(false)),
            last_active_channel: notification_handle,
            reminder_engine,
            recurring_task_spawner,
            notification_dispatcher,
            calendar_adapter,
            conversation_embedding_handler,
            plan_executor,
            plan_store: stored_plan_store,
            plan_executing: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            outcome_recorder,
            learning_service,
            plan_completion_handler,
            pipeline,
        })
    }

    /// Create a new agent loop (without cron service)
    pub async fn new(bus: Arc<MessageBus>, provider: DynProvider, config: Config) -> Result<Self> {
        let todo_path = config.todo_store_path();
        let todo_store = Arc::new(RwLock::new(tools::todo_store::TodoStore::new(todo_path)));
        let goal_path = config.goal_store_path();
        let goal_store = Some(Arc::new(RwLock::new(goal::GoalStore::new(goal_path))));
        let plan_path = config.plan_store_path();
        let plan_store = Some(Arc::new(RwLock::new(plan::PlanStore::new(plan_path))));
        Self::new_with_cron(
            bus, provider, config, None, todo_store, goal_store, plan_store, None,
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

    /// Execute an approved plan sequentially, step by step.
    ///
    /// State machine: Approved → Executing → (Completed | Failed)
    ///
    /// For each step:
    /// 1. Marks step as Executing
    /// 2. Calls plan_executor.execute_step() to run the step
    /// 3. Updates step status (Completed or Failed)
    /// 4. Advances current_step_index
    /// 5. Persists plan after each step
    ///
    /// On completion, sets plan.completed_at and transitions to Completed.
    /// If any step fails beyond max_attempts, transitions to Failed.
    pub async fn run_plan_execution(
        &self,
        plan_id: &uuid::Uuid,
        routing_ctx: &RoutingContext,
    ) -> Result<String> {
        use chrono::Utc;
        use plan::{PlanStatus, StepStatus};

        let (executor, store_arc) = match (&self.plan_executor, &self.plan_store) {
            (Some(e), Some(s)) => (e, Arc::clone(s)),
            _ => {
                return Err(common::KlyntbotError::Tool(
                    common::ToolError::ExecutionFailed(
                        "Plan execution not configured (missing plan_store or plan_executor)"
                            .into(),
                    ),
                ))
            }
        };

        // Load and validate, then transition to Executing in-place (zero-copy).
        let (plan_title, initial_step_idx) = {
            let mut store = store_arc.write().await;
            let plan = store.get(plan_id).await?.ok_or_else(|| {
                common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
            })?;

            // Validate: must be in Approved state
            PlanStatus::validate_transition(&plan.status, &PlanStatus::Executing)?;

            let title = plan.title.clone();
            let step_idx = plan.current_step_index;

            // Transition to Executing in-place — no Plan clone
            {
                let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                    common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
                })?;
                p.status = PlanStatus::Executing;
                p.updated_at = Utc::now();
            }
            store.persist_latest(plan_id).await?;

            (title, step_idx)
        };

        // Set executing flag (affects iteration limit in run_agent_loop)
        self.plan_executing.store(true, Ordering::SeqCst);

        // RAII guard: clears the flag on ALL exit paths — normal return, break, and ? propagation.
        // Without this guard, any ? operator inside the loop leaks the flag as true permanently.
        struct PlanExecutingGuard(Arc<std::sync::atomic::AtomicBool>);
        impl Drop for PlanExecutingGuard {
            fn drop(&mut self) {
                self.0.store(false, std::sync::atomic::Ordering::SeqCst);
            }
        }
        let _flag_guard = PlanExecutingGuard(Arc::clone(&self.plan_executing));

        let mut step_idx = initial_step_idx;
        // Tracks how many full plan regenerations have occurred (distinct from per-step retries)
        let mut backtrack_count: usize = 0;
        // step_count is re-read after regeneration
        let mut step_count = {
            let mut store = store_arc.write().await;
            store
                .get_mut(plan_id)
                .await?
                .map(|p| p.steps.len())
                .unwrap_or(0)
        };

        info!(
            "Starting plan execution: '{}' ({} steps)",
            plan_title, step_count
        );

        let result: Result<(String, bool)> = loop {
            if step_idx >= step_count {
                break Ok((
                    format!(
                        "Plan '{}' completed: all {} steps executed.",
                        plan_title, step_count
                    ),
                    true,
                ));
            }

            // Under one lock: build step context, clone just the PlanStep (not whole Plan),
            // mark step Executing in-place, then persist. Lock released before LLM call.
            let (plan_context, step_snapshot, step_description) = {
                let mut store = store_arc.write().await;
                let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                    common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
                })?;

                let ctx = executor.build_step_context(p, step_idx);
                let step_desc = p.steps[step_idx].description.clone();
                // Clone just the step for execute_step — far cheaper than cloning Plan
                let step_snap = p.steps[step_idx].clone();

                // Mark Executing in-place
                p.steps[step_idx].status = StepStatus::Executing;
                p.steps[step_idx].started_at = Some(Utc::now());
                p.steps[step_idx].attempt_count += 1;
                p.updated_at = Utc::now();
                // NLL releases the borrow of store through p here (last use of p)
                store.persist_latest(plan_id).await?;

                (ctx, step_snap, step_desc)
            };

            debug!(
                "Executing step {}/{}: {}",
                step_idx + 1,
                step_count,
                step_description
            );

            // Execute the step — no store lock held during the long-running LLM call
            let step_start = Instant::now();
            let step_result = executor
                .execute_step(
                    &step_snapshot,
                    &plan_context,
                    &self.provider,
                    &self.tool_registry,
                    routing_ctx,
                    self.confidence_evaluator.as_ref(),
                )
                .await;
            let step_duration_ms = step_start.elapsed().as_millis() as u64;

            // Record plan step outcome for the learning system (best-effort)
            if let Some(recorder) = &self.outcome_recorder {
                let (success, error_cat, step_confidence, recorded_tool_name) = match &step_result {
                    Ok(r) => (
                        r.success,
                        None,
                        r.confidence.as_ref(),
                        r.tool_name
                            .clone()
                            .unwrap_or_else(|| step_description.clone()),
                    ),
                    Err(_) => (
                        false,
                        Some("execution_error"),
                        None,
                        step_description.clone(),
                    ),
                };
                let session_key = format!("{}:{}", routing_ctx.channel, routing_ctx.chat_id);
                recorder
                    .record_tool_outcome(
                        &recorded_tool_name,
                        success,
                        error_cat,
                        step_duration_ms,
                        step_confidence,
                        crate::learning::ExecutionMode::PlanStep {
                            plan_id: plan_id.to_string(),
                            step_index: step_idx,
                        },
                        &session_key,
                    )
                    .await;
            }

            match step_result {
                Ok(result) if result.success => {
                    info!(
                        "Step {}/{} completed successfully",
                        step_idx + 1,
                        step_count
                    );

                    // Mark step Completed in-place — no Plan clone
                    let mut store = store_arc.write().await;
                    let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                        common::KlyntbotError::Plan(common::PlanError::NotFound(
                            plan_id.to_string(),
                        ))
                    })?;
                    p.steps[step_idx].status = StepStatus::Completed;
                    p.steps[step_idx].result = Some(result.output);
                    p.steps[step_idx].completed_at = Some(Utc::now());
                    step_idx += 1;
                    p.current_step_index = step_idx;
                    p.updated_at = Utc::now();
                    store.persist_latest(plan_id).await?;
                }
                Ok(result) => {
                    // Step failed — read attempt info from store, then update in-place
                    let (attempt, max_attempts) = {
                        let mut store = store_arc.write().await;
                        let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                            common::KlyntbotError::Plan(common::PlanError::NotFound(
                                plan_id.to_string(),
                            ))
                        })?;
                        (
                            p.steps[step_idx].attempt_count,
                            p.steps[step_idx].max_attempts,
                        )
                    };
                    let failure_reason = result.failure_reason.unwrap_or(result.output);

                    warn!(
                        "Step {}/{} failed (attempt {}/{}): {}",
                        step_idx + 1,
                        step_count,
                        attempt,
                        max_attempts,
                        failure_reason
                    );

                    // Record backtrack entry in-place
                    {
                        let mut store = store_arc.write().await;
                        let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                            common::KlyntbotError::Plan(common::PlanError::NotFound(
                                plan_id.to_string(),
                            ))
                        })?;
                        p.backtrack_history.push(plan::BacktrackEntry {
                            step_index: step_idx,
                            attempt,
                            failure_reason: failure_reason.clone(),
                            timestamp: Utc::now(),
                        });
                        p.updated_at = Utc::now();
                        store.persist_latest(plan_id).await?;
                    }

                    if attempt < max_attempts {
                        // Exponential backoff before retry: 2^(attempt-1) seconds
                        let backoff_secs = 2u64.pow((attempt as u32).saturating_sub(1));
                        debug!(
                            "Retrying step {} in {}s (attempt {}/{})",
                            step_idx + 1,
                            backoff_secs,
                            attempt + 1,
                            max_attempts
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                    } else {
                        // Max per-step attempts exhausted — check backtrack limit
                        backtrack_count += 1;
                        if backtrack_count >= super::plan_executor::MAX_BACKTRACK_ATTEMPTS {
                            // Backtrack limit reached — fail the plan in-place
                            let mut store = store_arc.write().await;
                            let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                                common::KlyntbotError::Plan(common::PlanError::NotFound(
                                    plan_id.to_string(),
                                ))
                            })?;
                            p.steps[step_idx].status = StepStatus::Failed;
                            p.updated_at = Utc::now();
                            store.persist_latest(plan_id).await?;
                            break Err(common::KlyntbotError::Plan(
                                common::PlanError::BacktrackLimitReached(
                                    super::plan_executor::MAX_BACKTRACK_ATTEMPTS,
                                ),
                            ));
                        }

                        warn!(
                            "Regenerating steps from index {} after {} attempts (backtrack {}/{})",
                            step_idx,
                            attempt,
                            backtrack_count,
                            super::plan_executor::MAX_BACKTRACK_ATTEMPTS
                        );

                        // Regeneration requires a Plan snapshot for the LLM call (one-time clone)
                        let plan_snapshot = {
                            let mut store = store_arc.write().await;
                            let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                                common::KlyntbotError::Plan(common::PlanError::NotFound(
                                    plan_id.to_string(),
                                ))
                            })?;
                            p.steps[step_idx].status = StepStatus::Failed;
                            p.clone() // One-time snapshot clone for the LLM regeneration call
                        };

                        let new_steps = executor
                            .regenerate_from(
                                &plan_snapshot,
                                step_idx,
                                &failure_reason,
                                &self.provider,
                            )
                            .await?;

                        // Replace steps from step_idx forward in-place
                        let mut store = store_arc.write().await;
                        let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                            common::KlyntbotError::Plan(common::PlanError::NotFound(
                                plan_id.to_string(),
                            ))
                        })?;
                        p.steps.truncate(step_idx);
                        p.steps.extend(new_steps);
                        step_count = p.steps.len();
                        p.updated_at = Utc::now();
                        store.persist_latest(plan_id).await?;

                        // step_idx stays the same — now points to first regenerated step
                        info!(
                            "Regenerated {} steps from index {}",
                            step_count - step_idx,
                            step_idx
                        );
                    }
                }
                Err(e) => {
                    // Hard error — mark step Failed in-place, let finalizer own plan status
                    let mut store = store_arc.write().await;
                    let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                        common::KlyntbotError::Plan(common::PlanError::NotFound(
                            plan_id.to_string(),
                        ))
                    })?;
                    p.steps[step_idx].status = StepStatus::Failed;
                    p.updated_at = Utc::now();
                    store.persist_latest(plan_id).await?;
                    break Err(e);
                }
            }

            // Guard against exceeding iteration limit.
            // Compare backtrack_count (plan regenerations) — NOT backtrack_history.len()
            // (which counts every per-step retry and inflates the check prematurely).
            let iter_limit = {
                let mut store = store_arc.write().await;
                let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                    common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
                })?;
                p.iteration_limit
            };
            if backtrack_count >= iter_limit {
                let mut store = store_arc.write().await;
                let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                    common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
                })?;
                p.steps[step_idx].status = StepStatus::Failed;
                p.updated_at = Utc::now();
                store.persist_latest(plan_id).await?;
                break Ok((
                    format!(
                        "Plan '{}' halted: iteration limit ({}) reached.",
                        plan_title, iter_limit
                    ),
                    false,
                ));
            }
        };

        // Clear executing flag
        self.plan_executing.store(false, Ordering::SeqCst);

        // Finalize plan status in-place — no Plan clone
        let (summary, plan_goal_id, plan_succeeded) = {
            let mut store = store_arc.write().await;
            let p = store.get_mut(plan_id).await?.ok_or_else(|| {
                common::KlyntbotError::Plan(common::PlanError::NotFound(plan_id.to_string()))
            })?;
            let goal_id = p.goal_id;
            let (msg, succeeded) = match &result {
                Ok((msg, true)) => {
                    p.status = PlanStatus::Completed;
                    p.completed_at = Some(Utc::now());
                    p.updated_at = Utc::now();
                    ((*msg).clone(), true)
                }
                Ok((msg, _)) => {
                    p.status = PlanStatus::Failed;
                    p.updated_at = Utc::now();
                    ((*msg).clone(), false)
                }
                Err(_) => {
                    p.status = PlanStatus::Failed;
                    p.updated_at = Utc::now();
                    (
                        format!("Plan '{}' failed with an error.", plan_title),
                        false,
                    )
                }
            };
            store.persist_latest(plan_id).await?;
            (msg, goal_id, succeeded)
        };

        // Notify the completion handler (best-effort; updates linked goal metrics)
        if let Some(handler) = &self.plan_completion_handler {
            if let Err(e) = handler
                .on_plan_completed(plan_id, plan_goal_id, plan_succeeded, &summary)
                .await
            {
                warn!("PlanCompletionHandler failed (non-fatal): {}", e);
            }
        }

        info!("{}", summary);
        result.map(|_| summary.clone())
    }

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
            format!("{}...", &msg.content[..80])
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

        // Phase 4.1: Async conversation embedding hook for user message
        if let Some(handler) = &self.conversation_embedding_handler {
            if self.should_embed_conversation(session_key.as_str(), "user") {
                let h = handler.clone();
                let sk = session_key.to_string();
                let role = "user".to_string();
                let content_str = msg.content.clone();
                let msg_id = session
                    .messages
                    .last()
                    .expect("Message should exist after add_message")
                    .id
                    .clone();

                tokio::spawn(async move {
                    if let Err(e) = h.embed_message(&sk, &role, &content_str, &msg_id).await {
                        warn!("Failed to embed user message: {}", e);
                    }
                });
            }
        }

        // Get session history
        let history = session.get_history(DEFAULT_HISTORY_LIMIT).to_vec();

        // Drop the write lock
        drop(session_manager);

        // Create routing context for tools
        let routing_ctx = RoutingContext::new(msg.channel.clone(), msg.chat_id.clone());

        let response_content = if let Some(pipeline) = &self.pipeline {
            // Pipeline path: build system prompt separately, convert history, delegate
            let mut context_builder = self.context_builder.write().await;
            let system_prompt = context_builder
                .build_system_prompt(msg.channel.as_str(), msg.chat_id.as_str())
                .await;
            drop(context_builder);

            let history_messages: Vec<Message> = history
                .iter()
                .map(|m| match m.role.as_str() {
                    "system" => Message::system(&m.content),
                    "user" => Message::user(&m.content),
                    "assistant" => Message::assistant(&m.content),
                    _ => Message::user(&m.content),
                })
                .collect();

            let tool_registry = self.tool_registry.read().await;
            let tool_defs = tool_registry.get_definitions();
            let tool_names: Vec<String> = tool_registry.tool_names();
            drop(tool_registry);
            let tool_name_refs: Vec<&str> = tool_names.iter().map(|s| s.as_str()).collect();

            let result = pipeline
                .process_message(
                    &msg.content,
                    history_messages,
                    &tool_defs,
                    &tool_name_refs,
                    &routing_ctx,
                    Some(&system_prompt),
                )
                .await?;

            info!(
                "Pipeline: strategy={}, escalations={}",
                result.strategy_used, result.escalations
            );
            result.content
        } else {
            // Legacy path
            let mut context_builder = self.context_builder.write().await;
            let media = if msg.media.is_empty() {
                None
            } else {
                Some(msg.media.clone())
            };
            let messages = context_builder
                .build_messages(
                    history,
                    &msg.content,
                    media,
                    msg.channel.as_str(),
                    msg.chat_id.as_str(),
                )
                .await;
            drop(context_builder);

            self.run_agent_loop(
                messages,
                false,
                &routing_ctx,
                StreamingChannels::none(),
                CancellationToken::new(),
            )
            .await?
        };

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

        // Create routing context for the origin channel (not "system")
        let routing_ctx = RoutingContext::new(origin_channel.into(), origin_chat_id.into());

        // Get or create session and add system message as "user" role
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(&session_key).await?;

        // Format system message with sender_id prefix
        let system_msg_content = format!("[System: {}] {}", msg.sender_id, msg.content);
        session.add_message("user", &system_msg_content);

        // Get session history
        let history = session.get_history(DEFAULT_HISTORY_LIMIT).to_vec();

        // Drop the write lock before building messages
        drop(session_manager);

        // Build messages for LLM
        let mut context_builder = self.context_builder.write().await;
        let messages = context_builder
            .build_messages(
                history,
                &system_msg_content,
                None,
                origin_channel,
                origin_chat_id,
            )
            .await;
        drop(context_builder);

        // Run agent loop
        let response_content = self
            .run_agent_loop(
                messages,
                false,
                &routing_ctx,
                StreamingChannels::none(),
                CancellationToken::new(),
            )
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

    async fn save_to_session(&self, session_key: &str, content: &str) {
        let mut session_manager = self.session_manager.write().await;
        if let Ok(session) = session_manager.get_or_create(session_key).await {
            session.add_message("assistant", content);

            // Phase 4.1: Async conversation embedding hook
            if let Some(handler) = &self.conversation_embedding_handler {
                if self.should_embed_conversation(session_key, "assistant") {
                    let h = handler.clone();
                    let sk = session_key.to_string();
                    let role = "assistant".to_string();
                    let content_str = content.to_string();
                    // Get the ID of the message we just added
                    let msg_id = session
                        .messages
                        .last()
                        .expect("Message should exist after add_message")
                        .id
                        .clone();

                    tokio::spawn(async move {
                        if let Err(e) = h.embed_message(&sk, &role, &content_str, &msg_id).await {
                            warn!("Failed to embed conversation: {}", e);
                        }
                    });
                }
            }

            let session_clone = session.clone();
            if let Err(e) = session_manager.save(&session_clone).await {
                warn!("Failed to save session: {}", e);
            }
        }
    }

    /// Run the agent iteration loop with the given messages.
    /// Returns the final response content.
    ///
    /// `channels` carries optional event/user-response/prompt-pending channels
    /// for interactive streaming mode. Pass `StreamingChannels::none()` when not
    /// streaming interactively.
    /// When `cancel_token` is cancelled, stops processing and returns partial content.
    #[tracing::instrument(skip(self, messages, routing_ctx, channels, cancel_token), fields(message_count = messages.len()))]
    async fn run_agent_loop(
        &self,
        messages: Vec<Message>,
        use_streaming: bool,
        routing_ctx: &RoutingContext,
        channels: StreamingChannels,
        cancel_token: CancellationToken,
    ) -> Result<String> {
        let mut current_messages = messages;
        let mut final_content = None;

        // Dynamic iteration limit: 50 for plan mode, 20 for chat mode
        let max_iterations = if self.is_plan_executing() {
            50
        } else {
            MAX_TOOL_ITERATIONS
        };

        for iteration in 0..max_iterations {
            // Check for cancellation before each iteration
            if cancel_token.is_cancelled() {
                debug!("Agent loop cancelled at iteration {}", iteration);
                break;
            }

            debug!("Agent iteration {}/{}", iteration + 1, max_iterations);
            emit!(
                channels.event_tx,
                AgentEvent::IterationStart {
                    iteration: iteration + 1,
                    max: max_iterations,
                }
            );

            let tool_registry = self.tool_registry.read().await;
            let tools = tool_registry.get_definitions();
            drop(tool_registry);

            // Delegate to streaming or non-streaming path
            let response = if use_streaming && self.provider.supports_streaming() {
                self.run_streaming_iteration(
                    &mut current_messages,
                    &tools,
                    routing_ctx,
                    &channels.event_tx,
                    &cancel_token,
                )
                .await
            } else {
                self.run_standard_iteration(
                    &mut current_messages,
                    &tools,
                    routing_ctx,
                    &channels.event_tx,
                )
                .await
            };

            // Emit Error event before propagating failures
            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    emit!(channels.event_tx, AgentEvent::Error(e.to_string()));
                    return Err(e);
                }
            };

            match response {
                IterationOutcome::ToolCallsProcessed => {
                    continue;
                }
                IterationOutcome::FinalContent(content) => {
                    final_content = Some(content);
                    break;
                }
                IterationOutcome::Empty => {
                    warn!("LLM returned no content and no tool calls");
                    break;
                }
            }
        }

        let result = final_content.unwrap_or_else(|| {
            "I've finished processing. Is there anything else I can help with?".to_string()
        });

        emit!(channels.event_tx, AgentEvent::Done(result.clone()));

        Ok(result)
    }

    /// Run a single standard (non-streaming) iteration.
    async fn run_standard_iteration(
        &self,
        messages: &mut Vec<Message>,
        tools: &[serde_json::Value],
        routing_ctx: &RoutingContext,
        event_tx: &Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<IterationOutcome> {
        let params = ChatParams::new(&self.config.agents.defaults.model);
        let response = self.provider.chat(messages, Some(tools), &params).await?;

        // Check for tool calls
        if !response.tool_calls.is_empty() {
            debug!("LLM requested {} tool calls", response.tool_calls.len());

            // Evaluate confidence if enabled; capture for outcome recording
            let mut last_confidence: Option<crate::confidence::ConfidenceAssessment> = None;
            if let Some(evaluator) = &self.confidence_evaluator {
                let content = response.content.as_deref().unwrap_or("");
                if let Some(assessment) = evaluator.parse_assessment(content) {
                    let action_str = match &assessment.action {
                        DecisionAction::Proceed => "proceed",
                        DecisionAction::Clarify { .. } => "clarify",
                        DecisionAction::Skip { .. } => "skip",
                    };
                    emit!(
                        event_tx,
                        AgentEvent::ConfidenceAssessed {
                            score: assessment.score,
                            action: action_str.to_string(),
                        }
                    );

                    // Log the assessment
                    let tool_names: Vec<String> = response
                        .tool_calls
                        .iter()
                        .map(|tc| tc.name.clone())
                        .collect();
                    let entry = confidence::types::DecisionLogEntry {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_key: routing_ctx.chat_id.to_string(),
                        iteration: 0,
                        tool_names,
                        user_message_preview: String::new(),
                        assessment: assessment.clone(),
                        outcome: None,
                        created_at: chrono::Utc::now(),
                    };
                    self.decision_logger.log(&entry).await;

                    // Act on confidence
                    match &assessment.action {
                        DecisionAction::Clarify { .. } => {
                            debug!(
                                score = assessment.score,
                                "Low confidence — nudging LLM to call ask_user"
                            );
                            let nudge = format!(
                                "Your confidence assessment was low (score: {:.2}). \
                                 Low dimensions: {}. Reasoning: \"{}\"\n\n\
                                 You MUST call the ask_user tool to clarify with the user \
                                 before proceeding. Ask contextually relevant questions \
                                 using appropriate types (single_select, yes_no, free_text). \
                                 Do NOT proceed with other tools until you have clarified.",
                                assessment.score,
                                low_dimensions_summary(
                                    &assessment,
                                    self.confidence_evaluator
                                        .as_ref()
                                        .map(|e| e.threshold())
                                        .expect(
                                            "confidence_evaluator must exist in Clarify branch"
                                        ),
                                ),
                                assessment.reasoning,
                            );
                            messages.push(Message::system(nudge));
                            return Ok(IterationOutcome::ToolCallsProcessed);
                        }
                        DecisionAction::Skip { reason } => {
                            debug!(reason = %reason, "Skipping tool calls");
                            return Ok(IterationOutcome::FinalContent(reason.clone()));
                        }
                        DecisionAction::Proceed => {
                            last_confidence = Some(assessment);
                        }
                    }
                }
            }

            let tool_call_messages = tool_calls_to_messages(&response.tool_calls);
            messages.push(Message::assistant_with_tools(tool_call_messages));

            // Execute tools in parallel
            let tool_futures: Vec<_> = response
                .tool_calls
                .iter()
                .map(|tc| {
                    let registry = self.tool_registry.clone();
                    let name = tc.name.clone();
                    let args = tc.arguments.clone();
                    let ctx = routing_ctx.clone();
                    let id = tc.id.clone();
                    let etx = event_tx.clone();
                    async move {
                        debug!("Executing tool: {}", name);
                        if let Some(tx) = &etx {
                            let _ = tx
                                .send(AgentEvent::ToolStart {
                                    name: name.clone(),
                                    args: args.clone(),
                                })
                                .await;
                        }
                        let start = Instant::now();
                        let reg = registry.read().await;
                        let result = reg.execute(&name, args, &ctx).await;
                        let duration_ms = start.elapsed().as_millis() as u64;
                        let success = result.is_ok();
                        if let Some(tx) = &etx {
                            let _ = tx
                                .send(AgentEvent::ToolEnd {
                                    name: name.clone(),
                                    success,
                                    duration_ms,
                                })
                                .await;
                        }
                        (id, name, result, duration_ms)
                    }
                })
                .collect();

            let results = join_all(tool_futures).await;
            let outcome_session_key = format!("{}:{}", routing_ctx.channel, routing_ctx.chat_id);
            for (id, name, result, duration_ms) in results {
                // Record outcome for learning system (best-effort, privacy-by-omission)
                if let Some(recorder) = &self.outcome_recorder {
                    let success = result.is_ok();
                    let error_cat = result
                        .as_ref()
                        .err()
                        .map(|e| crate::learning::recorder::categorize_error(&e.to_string()));
                    recorder
                        .record_tool_outcome(
                            &name,
                            success,
                            error_cat,
                            duration_ms,
                            last_confidence.as_ref(),
                            crate::learning::ExecutionMode::Chat,
                            &outcome_session_key,
                        )
                        .await;
                }
                let result_str = match result {
                    Ok(r) => r,
                    Err(e) => format!("Error: {}", e),
                };
                messages.push(Message::tool(id, name, result_str));
            }

            return Ok(IterationOutcome::ToolCallsProcessed);
        }

        // No tool calls - check for final content
        if let Some(content) = response.content {
            let clean = confidence::evaluator::strip_confidence_blocks(&content);
            // Emit ContentChunk so the CLI can display text even for non-streaming providers
            emit!(event_tx, AgentEvent::ContentChunk(clean.clone()));
            Ok(IterationOutcome::FinalContent(clean))
        } else {
            Ok(IterationOutcome::Empty)
        }
    }

    /// Run a single streaming iteration.
    ///
    /// Content chunks are emitted via `event_tx` for real-time display.
    /// Respects cancellation via `cancel_token`.
    async fn run_streaming_iteration(
        &self,
        messages: &mut Vec<Message>,
        tools: &[serde_json::Value],
        routing_ctx: &RoutingContext,
        event_tx: &Option<mpsc::Sender<AgentEvent>>,
        cancel_token: &CancellationToken,
    ) -> Result<IterationOutcome> {
        let params = ChatParams::new(&self.config.agents.defaults.model);
        let mut stream = self
            .provider
            .chat_stream(messages, Some(tools), &params)
            .await?;

        let mut accumulated_content = String::new();
        let mut accumulated_tool_calls: HashMap<usize, ToolCallAccumulator> =
            HashMap::with_capacity(4);

        // Process stream chunks with cancellation support
        loop {
            tokio::select! {
                chunk_opt = stream.next() => {
                    let Some(chunk_result) = chunk_opt else { break };
                    let chunk = chunk_result?;

                    // Accumulate and emit content chunks
                    if let Some(content) = chunk.content {
                        accumulated_content.push_str(&content);
                        emit!(event_tx, AgentEvent::ContentChunk(content));
                    }

                    // Accumulate tool calls
                    if let Some(delta) = chunk.tool_call_delta {
                        let accumulator = accumulated_tool_calls
                            .entry(delta.index)
                            .or_insert_with(ToolCallAccumulator::new);

                        if let Some(id) = delta.id {
                            accumulator.id = id;
                        }
                        if let Some(name) = delta.name {
                            accumulator.name = name;
                        }
                        if let Some(args) = delta.arguments {
                            accumulator.arguments.push_str(&args);
                        }
                    }

                    if chunk.is_final {
                        break;
                    }
                }
                _ = cancel_token.cancelled() => {
                    debug!("Streaming cancelled by user");
                    // Return whatever we have so far
                    if !accumulated_content.is_empty() {
                        return Ok(IterationOutcome::FinalContent(accumulated_content));
                    }
                    return Ok(IterationOutcome::Empty);
                }
            }
        }

        // Build tool calls from accumulated data
        let tool_calls: Vec<providers::ToolCall> = accumulated_tool_calls
            .into_values()
            .map(|acc| {
                let arguments: serde_json::Value = serde_json::from_str(&acc.arguments)
                    .unwrap_or_else(|_| serde_json::json!({"raw": acc.arguments}));

                providers::ToolCall {
                    id: acc.id,
                    name: acc.name,
                    arguments,
                }
            })
            .collect();

        // Handle tool calls or final content
        if !tool_calls.is_empty() {
            debug!("LLM requested {} tool calls", tool_calls.len());

            // Evaluate confidence if enabled; capture for outcome recording
            let mut last_confidence: Option<crate::confidence::ConfidenceAssessment> = None;
            if let Some(evaluator) = &self.confidence_evaluator {
                if let Some(assessment) = evaluator.parse_assessment(&accumulated_content) {
                    let action_str = match &assessment.action {
                        DecisionAction::Proceed => "proceed",
                        DecisionAction::Clarify { .. } => "clarify",
                        DecisionAction::Skip { .. } => "skip",
                    };
                    emit!(
                        event_tx,
                        AgentEvent::ConfidenceAssessed {
                            score: assessment.score,
                            action: action_str.to_string(),
                        }
                    );

                    let tool_names: Vec<String> =
                        tool_calls.iter().map(|tc| tc.name.clone()).collect();
                    let entry = confidence::types::DecisionLogEntry {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_key: routing_ctx.chat_id.to_string(),
                        iteration: 0,
                        tool_names,
                        user_message_preview: String::new(),
                        assessment: assessment.clone(),
                        outcome: None,
                        created_at: chrono::Utc::now(),
                    };
                    self.decision_logger.log(&entry).await;

                    match &assessment.action {
                        DecisionAction::Clarify { .. } => {
                            debug!(
                                score = assessment.score,
                                "Low confidence — nudging LLM to call ask_user"
                            );
                            let nudge = format!(
                                "Your confidence assessment was low (score: {:.2}). \
                                 Low dimensions: {}. Reasoning: \"{}\"\n\n\
                                 You MUST call the ask_user tool to clarify with the user \
                                 before proceeding. Ask contextually relevant questions \
                                 using appropriate types (single_select, yes_no, free_text). \
                                 Do NOT proceed with other tools until you have clarified.",
                                assessment.score,
                                low_dimensions_summary(
                                    &assessment,
                                    self.confidence_evaluator
                                        .as_ref()
                                        .map(|e| e.threshold())
                                        .expect(
                                            "confidence_evaluator must exist in Clarify branch"
                                        ),
                                ),
                                assessment.reasoning,
                            );
                            messages.push(Message::system(nudge));
                            return Ok(IterationOutcome::ToolCallsProcessed);
                        }
                        DecisionAction::Skip { reason } => {
                            return Ok(IterationOutcome::FinalContent(reason.clone()));
                        }
                        DecisionAction::Proceed => {
                            last_confidence = Some(assessment);
                        }
                    }
                }
            }

            let tool_call_messages = tool_calls_to_messages(&tool_calls);
            messages.push(Message::assistant_with_tools(tool_call_messages));

            // Execute tools in parallel
            let tool_futures: Vec<_> = tool_calls
                .iter()
                .map(|tc| {
                    let registry = self.tool_registry.clone();
                    let name = tc.name.clone();
                    let args = tc.arguments.clone();
                    let ctx = routing_ctx.clone();
                    let id = tc.id.clone();
                    let etx = event_tx.clone();
                    async move {
                        debug!("Executing tool: {}", name);
                        if let Some(tx) = &etx {
                            let _ = tx
                                .send(AgentEvent::ToolStart {
                                    name: name.clone(),
                                    args: args.clone(),
                                })
                                .await;
                        }
                        let start = Instant::now();
                        let reg = registry.read().await;
                        let result = reg.execute(&name, args, &ctx).await;
                        let duration_ms = start.elapsed().as_millis() as u64;
                        let success = result.is_ok();
                        if let Some(tx) = &etx {
                            let _ = tx
                                .send(AgentEvent::ToolEnd {
                                    name: name.clone(),
                                    success,
                                    duration_ms,
                                })
                                .await;
                        }
                        (id, name, result, duration_ms)
                    }
                })
                .collect();

            let results = join_all(tool_futures).await;
            let outcome_session_key = format!("{}:{}", routing_ctx.channel, routing_ctx.chat_id);
            for (id, name, result, duration_ms) in results {
                // Record outcome for learning system (best-effort, privacy-by-omission)
                if let Some(recorder) = &self.outcome_recorder {
                    let success = result.is_ok();
                    let error_cat = result
                        .as_ref()
                        .err()
                        .map(|e| crate::learning::recorder::categorize_error(&e.to_string()));
                    recorder
                        .record_tool_outcome(
                            &name,
                            success,
                            error_cat,
                            duration_ms,
                            last_confidence.as_ref(),
                            crate::learning::ExecutionMode::Chat,
                            &outcome_session_key,
                        )
                        .await;
                }
                let result_str = match result {
                    Ok(r) => r,
                    Err(e) => format!("Error: {}", e),
                };
                messages.push(Message::tool(id, name, result_str));
            }

            return Ok(IterationOutcome::ToolCallsProcessed);
        }

        if !accumulated_content.is_empty() {
            let clean = confidence::evaluator::strip_confidence_blocks(&accumulated_content);
            Ok(IterationOutcome::FinalContent(clean))
        } else {
            Ok(IterationOutcome::Empty)
        }
    }

    /// Set the adaptive orchestrator pipeline.
    ///
    /// When set, `process_message_v2()` becomes available, routing through the
    /// new classify → assemble → dispatch → validate → record pipeline instead
    /// of the legacy `run_agent_loop` iteration loop.
    ///
    /// TODO: Wire `process_message_v2` into the main `run()` message bus loop
    /// (currently only the legacy `process_message` path is used by `run()`).
    /// The v2 path is accessible programmatically via `process_message_v2()`
    /// or through CLI `chat` when pipeline is set.
    pub fn set_pipeline(&mut self, pipeline: Arc<crate::pipeline::AgentPipeline>) {
        self.pipeline = Some(pipeline);
    }

    /// Process a message through the Adaptive Orchestrator pipeline (v2).
    ///
    /// Requires `set_pipeline()` to have been called first. Falls back to
    /// the legacy `process_direct()` path if no pipeline is set.
    ///
    /// Pipeline flow: Orchestrator → ContextEngine → EngineDispatch →
    /// ResponseValidator → CostTracker
    pub async fn process_message_v2(&self, content: String, session_key: String) -> Result<String> {
        let pipeline = match &self.pipeline {
            Some(p) => Arc::clone(p),
            None => {
                debug!("No pipeline set, falling back to legacy process_direct");
                return self.process_direct(content, session_key).await;
            }
        };

        let preview = if content.len() > 80 {
            let end = content
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= 80)
                .last()
                .unwrap_or(0);
            format!("{}...", &content[..end])
        } else {
            content.clone()
        };
        debug!("Processing message (v2 pipeline): {}", preview);

        // Get or create session + add user message
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(&session_key).await?;
        session.add_message("user", &content);
        let history = session.get_history(DEFAULT_HISTORY_LIMIT).to_vec();
        drop(session_manager);

        // Convert session history to provider Messages
        let history_messages: Vec<Message> = history
            .iter()
            .map(|m| match m.role.as_str() {
                "system" => Message::system(&m.content),
                "user" => Message::user(&m.content),
                "assistant" => Message::assistant(&m.content),
                _ => Message::user(&m.content),
            })
            .collect();

        // Get tool definitions and names
        let tool_registry = self.tool_registry.read().await;
        let tool_defs = tool_registry.get_definitions();
        let tool_names: Vec<String> = tool_registry.tool_names();
        drop(tool_registry);

        let tool_name_refs: Vec<&str> = tool_names.iter().map(|s| s.as_str()).collect();

        // Create routing context
        let routing_ctx = RoutingContext::new("cli".into(), session_key.clone().into());

        // Run through pipeline (v2 uses config default system prompt)
        let result = pipeline
            .process_message(
                &content,
                history_messages,
                &tool_defs,
                &tool_name_refs,
                &routing_ctx,
                None,
            )
            .await?;

        info!(
            "Pipeline v2: strategy={}, escalations={}, valid={}",
            result.strategy_used, result.escalations, result.validation.is_valid
        );

        // Save to session
        self.save_to_session(&session_key, &result.content).await;

        Ok(result.content)
    }

    /// Process a message directly (for CLI mode).
    ///
    /// Returns the agent's response text directly instead of publishing to the bus.
    pub async fn process_direct(&self, content: String, session_key: String) -> Result<String> {
        let preview = if content.len() > 80 {
            let end = content
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|&i| i <= 80)
                .last()
                .unwrap_or(0);
            format!("{}...", &content[..end])
        } else {
            content.clone()
        };
        debug!("Processing direct message: {}", preview);

        // Get or create session
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(&session_key).await?;
        session.add_message("user", &content);

        // Phase 4.1: Async conversation embedding hook for user message (CLI)
        if let Some(handler) = &self.conversation_embedding_handler {
            if self.should_embed_conversation(&session_key, "user") {
                let h = handler.clone();
                let sk = session_key.clone();
                let role = "user".to_string();
                let content_str = content.clone();
                let msg_id = session
                    .messages
                    .last()
                    .expect("Message should exist after add_message")
                    .id
                    .clone();

                tokio::spawn(async move {
                    if let Err(e) = h.embed_message(&sk, &role, &content_str, &msg_id).await {
                        warn!("Failed to embed user message (CLI): {}", e);
                    }
                });
            }
        }

        let history = session.get_history(DEFAULT_HISTORY_LIMIT).to_vec();
        drop(session_manager);

        // Create routing context for CLI
        let routing_ctx = RoutingContext::new("cli".into(), session_key.clone().into());

        // Build messages for LLM
        let mut context_builder = self.context_builder.write().await;
        let messages = context_builder
            .build_messages(history, &content, None, "cli", &session_key)
            .await;
        drop(context_builder);

        // Run agent loop (with streaming enabled for CLI, no interactive prompts)
        let response_content = self
            .run_agent_loop(
                messages,
                true,
                &routing_ctx,
                StreamingChannels::none(),
                CancellationToken::new(),
            )
            .await?;

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
            format!("{}...", &content[..80])
        } else {
            content.clone()
        };
        debug!("Processing streaming direct message: {}", preview);

        // Get or create session and build messages before spawning
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(&session_key).await?;
        session.add_message("user", &content);

        // Phase 4.1: Async conversation embedding hook for user message (CLI streaming)
        if let Some(handler) = &self.conversation_embedding_handler {
            if self.should_embed_conversation(&session_key, "user") {
                let h = handler.clone();
                let sk = session_key.clone();
                let role = "user".to_string();
                let content_str = content.clone();
                let msg_id = session
                    .messages
                    .last()
                    .expect("Message should exist after add_message")
                    .id
                    .clone();

                tokio::spawn(async move {
                    if let Err(e) = h.embed_message(&sk, &role, &content_str, &msg_id).await {
                        warn!("Failed to embed user message (CLI streaming): {}", e);
                    }
                });
            }
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
        let cancel = cancel_token.clone();
        let sk = session_key.clone();

        let handle = tokio::spawn(async move {
            let result = if let Some(pipeline) = &agent.pipeline {
                // Pipeline path: non-streaming for now, but usage is tracked
                let mut context_builder = agent.context_builder.write().await;
                let system_prompt = context_builder
                    .build_system_prompt("cli", &sk)
                    .await;
                drop(context_builder);

                let history_messages: Vec<Message> = history
                    .iter()
                    .map(|m| match m.role.as_str() {
                        "system" => Message::system(&m.content),
                        "user" => Message::user(&m.content),
                        "assistant" => Message::assistant(&m.content),
                        _ => Message::user(&m.content),
                    })
                    .collect();

                let tool_registry = agent.tool_registry.read().await;
                let tool_defs = tool_registry.get_definitions();
                let tool_names: Vec<String> = tool_registry.tool_names();
                drop(tool_registry);
                let tool_name_refs: Vec<&str> =
                    tool_names.iter().map(|s| s.as_str()).collect();

                match pipeline
                    .process_message(
                        &content,
                        history_messages,
                        &tool_defs,
                        &tool_name_refs,
                        &routing_ctx,
                        Some(&system_prompt),
                    )
                    .await
                {
                    Ok(pipeline_result) => {
                        info!(
                            "Pipeline (streaming): strategy={}, escalations={}",
                            pipeline_result.strategy_used, pipeline_result.escalations
                        );

                        // Emit the full response for the CLI renderer
                        let _ = event_tx
                            .send(AgentEvent::ContentChunk(
                                pipeline_result.content.clone(),
                            ))
                            .await;
                        let _ = event_tx
                            .send(AgentEvent::Done(pipeline_result.content.clone()))
                            .await;

                        Ok(pipeline_result.content)
                    }
                    Err(e) => {
                        let _ = event_tx
                            .send(AgentEvent::Error(e.to_string()))
                            .await;
                        Err(e)
                    }
                }
            } else {
                // Legacy streaming path
                let mut context_builder = agent.context_builder.write().await;
                let messages = context_builder
                    .build_messages(history, &content, None, "cli", &sk)
                    .await;
                drop(context_builder);

                agent
                    .run_agent_loop(
                        messages,
                        true,
                        &routing_ctx,
                        StreamingChannels {
                            event_tx: Some(event_tx),
                        },
                        cancel,
                    )
                    .await
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

/// Summarize which confidence dimensions scored below threshold.
///
/// Used in the system nudge message when confidence is low, so the LLM
/// knows which aspects to clarify with the user.
fn low_dimensions_summary(
    assessment: &confidence::types::ConfidenceAssessment,
    threshold: f32,
) -> String {
    let mut low = Vec::new();
    if assessment.dimensions.intent_clarity < threshold {
        low.push(format!(
            "intent_clarity ({:.2})",
            assessment.dimensions.intent_clarity
        ));
    }
    if assessment.dimensions.tool_fit < threshold {
        low.push(format!("tool_fit ({:.2})", assessment.dimensions.tool_fit));
    }
    if assessment.dimensions.info_sufficiency < threshold {
        low.push(format!(
            "info_sufficiency ({:.2})",
            assessment.dimensions.info_sufficiency
        ));
    }
    if low.is_empty() {
        "overall score".to_string()
    } else {
        low.join(", ")
    }
}

/// Helper to accumulate tool call data across chunks
struct ToolCallAccumulator {
    id: String,
    name: String,
    arguments: String,
}

impl ToolCallAccumulator {
    fn new() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            arguments: String::new(),
        }
    }
}

/// Result of running a single agent iteration
enum IterationOutcome {
    /// Tool calls were processed; continue to next iteration
    ToolCallsProcessed,
    /// Final content was received; iteration complete
    FinalContent(String),
    /// No content and no tool calls; end iteration
    Empty,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bus::{LearningEvent, LearningEventBus};
    use chrono::Utc;
    use confidence::types::{
        AssessmentPhase, ConfidenceAssessment, ConfidenceDimensions, DecisionAction,
    };

    fn make_assessment(
        intent_clarity: f32,
        tool_fit: f32,
        info_sufficiency: f32,
    ) -> ConfidenceAssessment {
        ConfidenceAssessment {
            score: (intent_clarity + tool_fit + info_sufficiency) / 3.0,
            phase: AssessmentPhase::PreTool,
            reasoning: "test reasoning".to_string(),
            dimensions: ConfidenceDimensions {
                intent_clarity,
                tool_fit,
                info_sufficiency,
            },
            action: DecisionAction::default(),
            assessed_at: Utc::now(),
        }
    }

    /// AC-I2.3/2.4: AgentLoop subscriber task updates ContextBuilder threshold
    /// when LearningService publishes a ThresholdChanged event.
    #[tokio::test]
    async fn test_learning_subscriber_updates_context_threshold() {
        use crate::ContextBuilder;

        let workspace = std::path::PathBuf::from("/tmp/test-subscriber-i2-agent");
        let ctx = Arc::new(RwLock::new(
            ContextBuilder::new(workspace, "UTC".to_string(), None, None).await,
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

    #[test]
    fn test_low_dimensions_summary_all_low() {
        let assessment = make_assessment(0.3, 0.4, 0.2);
        let summary = low_dimensions_summary(&assessment, 0.7);
        assert!(summary.contains("intent_clarity"));
        assert!(summary.contains("tool_fit"));
        assert!(summary.contains("info_sufficiency"));
    }

    #[test]
    fn test_low_dimensions_summary_one_low() {
        let assessment = make_assessment(0.9, 0.8, 0.3);
        let summary = low_dimensions_summary(&assessment, 0.7);
        assert!(!summary.contains("intent_clarity"));
        assert!(!summary.contains("tool_fit"));
        assert!(summary.contains("info_sufficiency (0.30)"));
    }

    #[test]
    fn test_low_dimensions_summary_none_low() {
        let assessment = make_assessment(0.9, 0.8, 0.75);
        let summary = low_dimensions_summary(&assessment, 0.7);
        assert_eq!(summary, "overall score");
    }

    #[test]
    fn test_low_dimensions_summary_at_threshold() {
        // Exactly at threshold should NOT be listed as low
        let assessment = make_assessment(0.7, 0.7, 0.7);
        let summary = low_dimensions_summary(&assessment, 0.7);
        assert_eq!(summary, "overall score");
    }
}
