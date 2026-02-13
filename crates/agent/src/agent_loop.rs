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
    cron_tool::CronTool,
    filesystem::register_fs_tools,
    message::MessageTool,
    registry::ToolRegistry,
    shell::ExecTool,
    // spawn::SpawnTool, // TODO: Enable after refactoring to use SpawnHandler trait
    web::{WebFetchTool, WebSearchTool},
    RoutingContext,
};

use super::{AgentEvent, ContextBuilder, CronHandlerAdapter, SubagentManager};

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
    running: Arc<AtomicBool>,
}

impl AgentLoop {
    /// Create a new agent loop with optional cron service
    pub async fn new_with_cron(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
        cron_service: Option<Arc<scheduling::CronService>>,
    ) -> Result<Self> {
        let workspace = config.workspace_path();

        // Create context builder
        let mut context_builder = ContextBuilder::new(workspace.clone());
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
        let session_manager = SessionManager::new(sessions_dir);

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

        // Register spawn tool with subagent manager
        // TODO: Enable after refactoring spawn.rs to use SpawnHandler trait
        // tool_registry.register(SpawnTool::with_manager(subagent_manager.clone()));

        // Register cron tool (with service if provided)
        if let Some(cron_svc) = cron_service {
            let adapter = Arc::new(CronHandlerAdapter::new(cron_svc));
            tool_registry.register(CronTool::with_handler(adapter));
        }

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
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Create a new agent loop (without cron service)
    pub async fn new(bus: Arc<MessageBus>, provider: DynProvider, config: Config) -> Result<Self> {
        Self::new_with_cron(bus, provider, config, None).await
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

        // Run agent loop (no event channel for bus mode)
        let response_content = self
            .run_agent_loop(
                messages,
                false,
                &routing_ctx,
                None,
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
                None,
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
    /// When `event_tx` is provided, emits `AgentEvent` variants as processing happens.
    /// When `cancel_token` is cancelled, stops processing and returns partial content.
    #[tracing::instrument(skip(self, messages, routing_ctx, event_tx, cancel_token), fields(message_count = messages.len()))]
    async fn run_agent_loop(
        &self,
        messages: Vec<Message>,
        use_streaming: bool,
        routing_ctx: &RoutingContext,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
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
                event_tx,
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
                    &event_tx,
                    &cancel_token,
                )
                .await
            } else {
                self.run_standard_iteration(&mut current_messages, &tools, routing_ctx, &event_tx)
                    .await
            };

            // Emit Error event before propagating failures
            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    emit!(event_tx, AgentEvent::Error(e.to_string()));
                    return Err(e);
                }
            };

            match response {
                IterationOutcome::ToolCallsProcessed => continue,
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

        emit!(event_tx, AgentEvent::Done(result.clone()));

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
            // Emit ContentChunk so the CLI can display text even for non-streaming providers
            emit!(event_tx, AgentEvent::ContentChunk(content.clone()));
            Ok(IterationOutcome::FinalContent(content))
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
        let mut accumulated_tool_calls: HashMap<usize, ToolCallAccumulator> = HashMap::new();

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
            Ok(IterationOutcome::FinalContent(accumulated_content))
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

        // Run agent loop (with streaming enabled for CLI)
        let response_content = self
            .run_agent_loop(messages, true, &routing_ctx, None, CancellationToken::new())
            .await?;

        // Save to session
        self.save_to_session(&session_key, &response_content).await;

        Ok(response_content)
    }

    /// Process a message with real-time event streaming and cancellation support.
    ///
    /// Returns an event receiver for real-time updates, a cancellation token to
    /// stop processing, and a join handle for the background task.
    pub async fn process_direct_streaming(
        self: &Arc<Self>,
        content: String,
        session_key: String,
    ) -> Result<(
        mpsc::Receiver<AgentEvent>,
        CancellationToken,
        JoinHandle<Result<String>>,
    )> {
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

        let routing_ctx = RoutingContext::new("cli".into(), session_key.clone().into());

        let mut context_builder = self.context_builder.write().await;
        let messages = context_builder
            .build_messages(history, &content, None, "cli", &session_key)
            .await;
        drop(context_builder);

        // Create event channel and cancellation token
        let (event_tx, event_rx) = mpsc::channel(64);
        let cancel_token = CancellationToken::new();

        // Clone Arcs for the spawned task
        let agent = Arc::clone(self);
        let cancel = cancel_token.clone();
        let sk = session_key.clone();

        let handle = tokio::spawn(async move {
            let result = agent
                .run_agent_loop(messages, true, &routing_ctx, Some(event_tx), cancel)
                .await;

            // Save to session regardless of success/failure
            if let Ok(ref content) = result {
                agent.save_to_session(&sk, content).await;
            }

            result
        });

        Ok((event_rx, cancel_token, handle))
    }

    /// Get the model name from config (for display purposes).
    pub fn model_name(&self) -> &str {
        &self.config.agents.defaults.model
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
