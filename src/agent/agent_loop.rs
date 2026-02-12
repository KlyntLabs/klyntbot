//! Agent loop: the core processing engine.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::bus::{InboundMessage, MessageBus, OutboundMessage};
use crate::config::Config;
use crate::error::Result;
use crate::providers::{DynProvider, Message, tool_calls_to_messages};
use crate::session::SessionManager;
use crate::tools::{
    cron_tool::CronTool,
    filesystem::register_fs_tools,
    message::MessageTool,
    registry::ToolRegistry,
    shell::ExecTool,
    spawn::SpawnTool,
    web::{WebFetchTool, WebSearchTool},
    ToolContext,
};
use futures_util::StreamExt;
use std::collections::HashMap;

use super::{ContextBuilder, SubagentManager};

/// Maximum number of tool-calling iterations before returning final response
const MAX_TOOL_ITERATIONS: usize = 20;

/// Default session history limit (number of messages)
const DEFAULT_HISTORY_LIMIT: usize = 50;

/// Agent loop - the core processing engine
pub struct AgentLoop {
    bus: Arc<MessageBus>,
    provider: DynProvider,
    config: Config,
    context_builder: Arc<RwLock<ContextBuilder>>,
    session_manager: Arc<RwLock<SessionManager>>,
    tool_registry: Arc<RwLock<ToolRegistry>>,
    #[allow(dead_code)] // Will be used when full agent loop implementation is completed
    subagent_manager: Arc<SubagentManager>,
    running: Arc<RwLock<bool>>,
}

impl AgentLoop {
    /// Create a new agent loop with optional cron service
    pub async fn new_with_cron(
        bus: Arc<MessageBus>,
        provider: DynProvider,
        config: Config,
        cron_service: Option<Arc<crate::cron::CronService>>,
    ) -> Result<Self> {
        let workspace = config.workspace_path();

        // Create context builder
        let mut context_builder = ContextBuilder::new(workspace.clone());
        context_builder.init().await.map_err(|e| {
            crate::error::KlyntbotError::Config(crate::error::ConfigError::Invalid(format!(
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
            .then(|| config.tools.web.brave_api_key.clone());

        let subagent_manager = Arc::new(SubagentManager::new(
            Arc::clone(&provider),
            workspace.clone(),
            bus.inbound_sender(),
            config.agents.defaults.model.clone(),
            brave_api_key.clone(),
            config.tools.web.max_results,
            config.tools.exec.timeout,
            config.tools.restrict_to_workspace,
        ));

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

        // Create shared tool context for routing info
        let tool_context = ToolContext::new();

        // Register message tool
        tool_registry.register(MessageTool::new(bus.outbound_sender(), tool_context.clone()));

        // Register spawn tool with subagent manager
        tool_registry.register(SpawnTool::with_manager(subagent_manager.clone(), tool_context.clone()));

        // Register cron tool (with service if provided)
        if let Some(cron_svc) = cron_service {
            tool_registry.register(CronTool::with_service(cron_svc, tool_context));
        } else {
            tool_registry.register(CronTool::new(tool_context));
        }

        Ok(Self {
            bus,
            provider,
            config,
            context_builder: Arc::new(RwLock::new(context_builder)),
            session_manager: Arc::new(RwLock::new(session_manager)),
            tool_registry: Arc::new(RwLock::new(tool_registry)),
            subagent_manager,
            running: Arc::new(RwLock::new(false)),
        })
    }

    /// Create a new agent loop (without cron service)
    pub async fn new(bus: Arc<MessageBus>, provider: DynProvider, config: Config) -> Result<Self> {
        Self::new_with_cron(bus, provider, config, None).await
    }

    /// Run the agent loop, processing messages from the bus
    pub async fn run(&self) -> Result<()> {
        {
            let mut running = self.running.write().await;
            *running = true;
        }

        info!("Agent loop started");

        while *self.running.read().await {
            // Wait for next message with timeout
            match tokio::time::timeout(Duration::from_secs(1), self.bus.consume_inbound()).await {
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
        let mut running = self.running.write().await;
        *running = false;
    }

    /// Process a single inbound message
    async fn process_message(&self, msg: InboundMessage) -> Result<()> {
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
        let session = session_manager.get_or_create(session_key.as_str())?;

        // Add user message to session
        session.add_message("user", &msg.content);

        // Get session history
        let history = session.get_history(DEFAULT_HISTORY_LIMIT);

        // Drop the write lock
        drop(session_manager);

        // Update tool contexts
        self.update_tool_contexts(msg.channel.as_str(), msg.chat_id.as_str()).await;

        // Build messages for LLM
        let context_builder = self.context_builder.read().await;
        let media = if msg.media.is_empty() {
            None
        } else {
            Some(msg.media.clone())
        };
        let messages = context_builder
            .build_messages(history, &msg.content, media, msg.channel.as_str(), msg.chat_id.as_str())
            .await;

        drop(context_builder);

        // Run agent loop
        let response_content = self.run_agent_loop(messages, false).await?;

        // Save assistant response to session
        self.save_to_session(session_key.as_str(), &response_content).await;

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
                if let Ok(session) = session_manager.get_or_create(msg.chat_id.as_str()) {
                    session.clear();
                    Some(session.clone())
                } else {
                    None
                }
            };

            if let Some(session) = session_to_save {
                if let Err(e) = session_manager.save(&session) {
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

        // Update tool contexts to the origin channel (not "system")
        self.update_tool_contexts(origin_channel, origin_chat_id)
            .await;

        // Get or create session and add system message as "user" role
        let mut session_manager = self.session_manager.write().await;
        let session = session_manager.get_or_create(&session_key)?;

        // Format system message with sender_id prefix
        let system_msg_content = format!("[System: {}] {}", msg.sender_id, msg.content);
        session.add_message("user", &system_msg_content);

        // Get session history
        let history = session.get_history(DEFAULT_HISTORY_LIMIT);

        // Drop the write lock before building messages
        drop(session_manager);

        // Build messages for LLM
        let context_builder = self.context_builder.read().await;
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
        let response_content = self.run_agent_loop(messages, false).await?;

        // Save assistant response to session
        self.save_to_session(session_key.as_str(), &response_content).await;

        // Publish response to origin channel
        let out_msg = OutboundMessage::new(
            origin_channel.to_string(),
            origin_chat_id.to_string(),
            response_content,
        );
        self.bus.publish_outbound(out_msg).await?;

        Ok(())
    }

    /// Update tool contexts with current channel/chat_id.
    ///
    /// Injects the current conversation context into tools that need routing info.
    /// This is called before processing each message to ensure tools like message,
    /// spawn, and cron know where to route their results.
    async fn update_tool_contexts(&self, channel: &str, chat_id: &str) {
        let registry = self.tool_registry.read().await;

        // Context-aware tools need to be updated
        if let Some(tool) = registry.get("message") {
            tool.set_context(channel, chat_id);
        }

        if let Some(tool) = registry.get("spawn") {
            tool.set_context(channel, chat_id);
        }

        if let Some(tool) = registry.get("cron") {
            tool.set_context(channel, chat_id);
        }
    }

    /// Save an assistant response to the session.
    async fn save_to_session(&self, session_key: &str, content: &str) {
        let mut session_manager = self.session_manager.write().await;
        if let Ok(session) = session_manager.get_or_create(session_key) {
            session.add_message("assistant", content);
            let session_clone = session.clone();
            if let Err(e) = session_manager.save(&session_clone) {
                warn!("Failed to save session: {}", e);
            }
        }
    }

    /// Run the agent iteration loop with the given messages.
    /// Returns the final response content.
    async fn run_agent_loop(
        &self,
        messages: Vec<Message>,
        use_streaming: bool,
    ) -> Result<String> {
        let mut current_messages = messages;
        let mut final_content = None;

        for iteration in 0..MAX_TOOL_ITERATIONS {
            debug!("Agent iteration {}/{}", iteration + 1, MAX_TOOL_ITERATIONS);

            let tool_registry = self.tool_registry.read().await;
            let tools = tool_registry.get_definitions();
            drop(tool_registry);

            // Delegate to streaming or non-streaming path
            let response = if use_streaming && self.provider.supports_streaming() {
                self.run_streaming_iteration(&mut current_messages, &tools).await?
            } else {
                self.run_standard_iteration(&mut current_messages, &tools).await?
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

        Ok(final_content.unwrap_or_else(|| {
            "I've finished processing. Is there anything else I can help with?".to_string()
        }))
    }

    /// Run a single standard (non-streaming) iteration.
    async fn run_standard_iteration(
        &self,
        messages: &mut Vec<Message>,
        tools: &[serde_json::Value],
    ) -> Result<IterationOutcome> {
        let response = self
            .provider
            .chat(messages, Some(tools), Some(&self.config.agents.defaults.model))
            .await?;

        // Check for tool calls
        if !response.tool_calls.is_empty() {
            debug!("LLM requested {} tool calls", response.tool_calls.len());

            let tool_call_messages = tool_calls_to_messages(&response.tool_calls);
            messages.push(Message::assistant_with_tools(tool_call_messages));

            // Execute tools
            for tool_call in response.tool_calls {
                debug!("Executing tool: {}", tool_call.name);

                let result = {
                    let registry = self.tool_registry.read().await;
                    registry
                        .execute(&tool_call.name, tool_call.arguments.clone())
                        .await
                };

                let result_str = match result {
                    Ok(r) => r,
                    Err(e) => format!("Error: {}", e),
                };

                messages.push(Message::tool(tool_call.id, tool_call.name, result_str));
            }

            return Ok(IterationOutcome::ToolCallsProcessed);
        }

        // No tool calls - check for final content
        if let Some(content) = response.content {
            Ok(IterationOutcome::FinalContent(content))
        } else {
            Ok(IterationOutcome::Empty)
        }
    }

    /// Run a single streaming iteration.
    ///
    /// Content is accumulated silently (no stdout output) so the caller
    /// can decide how to render it (e.g. markdown formatting).
    async fn run_streaming_iteration(
        &self,
        messages: &mut Vec<Message>,
        tools: &[serde_json::Value],
    ) -> Result<IterationOutcome> {
        let mut stream = self
            .provider
            .chat_stream(messages, Some(tools), Some(&self.config.agents.defaults.model))
            .await?;

        let mut accumulated_content = String::new();
        let mut accumulated_tool_calls: HashMap<usize, ToolCallAccumulator> = HashMap::new();

        // Process stream chunks
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;

            // Accumulate content (display is handled by the caller)
            if let Some(content) = chunk.content {
                accumulated_content.push_str(&content);
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

        // Build tool calls from accumulated data
        let tool_calls: Vec<crate::providers::ToolCall> = accumulated_tool_calls
            .into_values()
            .map(|acc| {
                let arguments: serde_json::Value = serde_json::from_str(&acc.arguments)
                    .unwrap_or_else(|_| serde_json::json!({"raw": acc.arguments}));

                crate::providers::ToolCall {
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

            for tool_call in tool_calls {
                debug!("Executing tool: {}", tool_call.name);

                let result = {
                    let registry = self.tool_registry.read().await;
                    registry
                        .execute(&tool_call.name, tool_call.arguments.clone())
                        .await
                };

                let result_str = match result {
                    Ok(r) => r,
                    Err(e) => format!("Error: {}", e),
                };

                messages.push(Message::tool(tool_call.id, tool_call.name, result_str));
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
        let session = session_manager.get_or_create(&session_key)?;
        session.add_message("user", &content);
        let history = session.get_history(DEFAULT_HISTORY_LIMIT);
        drop(session_manager);

        // Build messages for LLM
        let context_builder = self.context_builder.read().await;
        let messages = context_builder
            .build_messages(history, &content, None, "cli", &session_key)
            .await;
        drop(context_builder);

        // Run agent loop (with streaming enabled for CLI)
        let response_content = self.run_agent_loop(messages, true).await?;

        // Save to session
        self.save_to_session(&session_key, &response_content).await;

        Ok(response_content)
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
