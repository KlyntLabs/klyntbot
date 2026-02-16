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
    EmbeddingHandler,
    filesystem::register_fs_tools,
    message::MessageTool,
    registry::ToolRegistry,
    shell::ExecTool,
    // spawn::SpawnTool, // TODO: Enable after refactoring to use SpawnHandler trait
    web::{WebFetchTool, WebSearchTool},
    RoutingContext,
};

use super::confidence::{self, ConfidenceEvaluator, DecisionAction, DecisionLogger};
use super::{AgentEvent, CalendarSyncAdapter, ContextBuilder, CronHandlerAdapter, SubagentManager};

/// Maximum number of tool-calling iterations before returning final response
const MAX_TOOL_ITERATIONS: usize = 20;

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
    #[allow(dead_code)] // TODO: Use for cleanup when agent loop stops
    reminder_engine: Option<Arc<RwLock<super::ReminderEngine>>>,
    #[allow(dead_code)] // Held for lifetime; stopped on agent shutdown
    recurring_task_spawner: Option<Arc<RwLock<super::RecurringTaskSpawner>>>,
    #[allow(dead_code)] // Held for lifetime; shared with CalendarSyncAdapter
    notification_dispatcher: Option<Arc<super::NotificationDispatcher>>,
}

impl AgentLoop {
    /// Create a new agent loop with optional cron service and shared instances
    pub async fn new_with_cron(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
        cron_service: Option<Arc<scheduling::CronService>>,
        todo_store: Arc<RwLock<tools::todo_store::TodoStore>>,
        notification_handle: Option<LastActiveChannel>,
    ) -> Result<Self> {
        let workspace = config.workspace_path();

        // Create context builder
        let mut context_builder = ContextBuilder::new(
            workspace.clone(),
            config.timezone.clone(),
            Some(Arc::clone(&todo_store)),
        )
        .await;
        context_builder.init().await.map_err(|e| {
            common::KlyntbotError::Config(common::ConfigError::Invalid(format!(
                "Failed to initialize context: {}",
                e
            )))
        })?;

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

        // Register spawn tool with subagent manager
        // TODO: Enable after refactoring spawn.rs to use SpawnHandler trait
        // tool_registry.register(SpawnTool::with_manager(subagent_manager.clone()));

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
        if config.calendar.is_any_enabled() {
            let calendar_adapter = Arc::new(
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
            todo_tool = todo_tool
                .with_calendar_handler(Arc::clone(&calendar_adapter) as Arc<dyn CalendarHandler>);

            tool_registry.register(CalendarTool::new(calendar_adapter));
        }

        // Register enrichment engine (if enabled)
        if config.todo.enrichment.enabled {
            let enrichment_engine = Arc::new(super::enrichment::EnrichmentEngine::new(
                config.todo.enrichment.clone(),
            ));

            todo_tool = todo_tool.with_enrichment_handler(
                Arc::clone(&enrichment_engine) as Arc<dyn EnrichmentHandler>
            );
        }

        // Register embedding engine for semantic search (Sprint 5)
        if config.todo.search.enabled {
            let emb_store_path = config.embedding_store_path();
            let emb_store = Arc::new(RwLock::new(tools::EmbeddingStore::new(emb_store_path)));
            let embedding_engine = Arc::new(tools::EmbeddingEngine::new());
            let embedding_handler = Arc::new(tools::EmbeddingEngineImpl::new(
                embedding_engine,
                Arc::clone(&emb_store),
            ));

            todo_tool = todo_tool
                .with_embedding_handler(Arc::clone(&embedding_handler) as Arc<dyn EmbeddingHandler>)
                .with_embedding_store(emb_store)
                .with_search_config(
                    config.todo.search.semantic_threshold,
                    config.todo.search.rrf_k,
                );
        }

        tool_registry.register(todo_tool);

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

        // Create ReminderEngine using the shared notification dispatcher
        let reminder_engine = if let Some(ref dispatcher) = notification_dispatcher {
            let mut engine = super::ReminderEngine::new(
                Arc::clone(&todo_store),
                None, // TODO: Add CalendarHandler when calendar sync is integrated
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

        // Take ownership of the inbound receiver
        let inbound_rx = bus
            .take_inbound_rx()
            .expect("Inbound receiver already taken");

        Ok(Self {
            bus,
            inbound_rx: Some(inbound_rx),
            provider,
            config,
            context_builder: Arc::new(RwLock::new(context_builder)),
            session_manager: Arc::new(RwLock::new(session_manager)),
            tool_registry: Arc::new(RwLock::new(tool_registry)),
            subagent_manager,
            confidence_evaluator,
            decision_logger,
            running: Arc::new(AtomicBool::new(false)),
            last_active_channel: notification_handle,
            reminder_engine,
            recurring_task_spawner,
            notification_dispatcher,
        })
    }

    /// Create a new agent loop (without cron service)
    pub async fn new(bus: Arc<MessageBus>, provider: DynProvider, config: Config) -> Result<Self> {
        let todo_path = config.todo_store_path();
        let todo_store = Arc::new(RwLock::new(tools::todo_store::TodoStore::new(todo_path)));
        Self::new_with_cron(bus, provider, config, None, todo_store, None).await
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

        // Get session history
        let history = session.get_history(DEFAULT_HISTORY_LIMIT).to_vec();

        // Drop the write lock
        drop(session_manager);

        // Create routing context for tools
        let routing_ctx = RoutingContext::new(msg.channel.clone(), msg.chat_id.clone());

        // Build messages for LLM
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

        // Run agent loop (no event/user/prompt channels for bus mode)
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
    async fn save_to_session(&self, session_key: &str, content: &str) {
        let mut session_manager = self.session_manager.write().await;
        if let Ok(session) = session_manager.get_or_create(session_key).await {
            session.add_message("assistant", content);
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

        for iteration in 0..MAX_TOOL_ITERATIONS {
            // Check for cancellation before each iteration
            if cancel_token.is_cancelled() {
                debug!("Agent loop cancelled at iteration {}", iteration);
                break;
            }

            debug!("Agent iteration {}/{}", iteration + 1, MAX_TOOL_ITERATIONS);
            emit!(
                channels.event_tx,
                AgentEvent::IterationStart {
                    iteration: iteration + 1,
                    max: MAX_TOOL_ITERATIONS,
                }
            );

            let mut tool_registry = self.tool_registry.write().await;
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

            // Evaluate confidence if enabled
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
                                        .unwrap_or(0.7),
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
                        DecisionAction::Proceed => {}
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
                        (id, name, result)
                    }
                })
                .collect();

            let results = join_all(tool_futures).await;
            for (id, name, result) in results {
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

            // Evaluate confidence if enabled
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
                                        .unwrap_or(0.7),
                                ),
                                assessment.reasoning,
                            );
                            messages.push(Message::system(nudge));
                            return Ok(IterationOutcome::ToolCallsProcessed);
                        }
                        DecisionAction::Skip { reason } => {
                            return Ok(IterationOutcome::FinalContent(reason.clone()));
                        }
                        DecisionAction::Proceed => {}
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
                        (id, name, result)
                    }
                })
                .collect();

            let results = join_all(tool_futures).await;
            for (id, name, result) in results {
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

    /// Process a message directly (for CLI mode).
    ///
    /// Returns the agent's response text directly instead of publishing to the bus.
    pub async fn process_direct(&self, content: String, session_key: String) -> Result<String> {
        let preview = if content.len() > 80 {
            format!("{}...", &content[..80])
        } else {
            content.clone()
        };
        debug!("Processing direct message: {}", preview);

        // Get or create session
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(&session_key).await?;
        session.add_message("user", &content);
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

        let mut context_builder = self.context_builder.write().await;
        let messages = context_builder
            .build_messages(history, &content, None, "cli", &session_key)
            .await;
        drop(context_builder);

        let cancel_token = CancellationToken::new();

        // Clone Arcs for the spawned task
        let agent = Arc::clone(self);
        let cancel = cancel_token.clone();
        let sk = session_key.clone();

        let handle = tokio::spawn(async move {
            let result = agent
                .run_agent_loop(
                    messages,
                    true,
                    &routing_ctx,
                    StreamingChannels {
                        event_tx: Some(event_tx),
                    },
                    cancel,
                )
                .await;

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
