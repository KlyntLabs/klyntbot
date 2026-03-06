//! Agent loop: the core processing engine.

mod builder;

#[cfg(test)]
mod refactor_tests;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use bus::{InboundMessage, MessageBus, OutboundMessage};
use common::{utils::truncate_at_boundary, Result};
use config::Config;
use providers::Message;
use session::SessionManager;
use tokio::sync::mpsc;
use tools::RoutingContext;

use super::AgentEvent;

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
pub(crate) type LastActiveChannel = Arc<RwLock<Option<(common::ChannelName, common::ChatId)>>>;

/// Agent loop - the core processing engine
pub struct AgentLoop {
    pub(crate) bus: Arc<MessageBus>,
    pub(crate) inbound_rx: Option<mpsc::Receiver<InboundMessage>>,
    pub(crate) config: Config,
    pub(crate) context_engine: Arc<context_engine::ContextEngine>,
    pub(crate) session_manager: SessionManager,
    pub(crate) tool_registry: Arc<RwLock<tools::registry::ToolRegistry>>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) last_active_channel: Option<LastActiveChannel>,
    pub(crate) reminder_engine: Option<Arc<RwLock<super::ReminderEngine>>>,
    pub(crate) recurring_task_spawner: Option<Arc<RwLock<super::RecurringTaskSpawner>>>,
    /// Held for lifetime; shared with notification targets
    pub(crate) _notification_dispatcher: Option<Arc<super::NotificationDispatcher>>,
    /// Conversation embedding handler for semantic memory (Phase 4.1)
    pub(crate) conversation_embedding_handler: Option<Arc<dyn tools::ConversationEmbeddingHandler>>,
    /// Background learning service for adaptive threshold updates (None if learning disabled)
    pub(crate) learning_service: Option<Arc<RwLock<crate::learning::LearningService>>>,
    /// Agent runtime: agent-first pipeline replacing IntentPipeline.
    pub(crate) runtime: Arc<crate::agent_runtime::AgentRuntime>,
    /// Strategy repo for updating satisfaction scores from reactions.
    pub(crate) strategy_repo: Option<storage::StrategyRepo>,
    /// Maximum number of history messages to load per request.
    pub(crate) history_limit: usize,
    /// Cancellation token for the session cleanup background service.
    pub(crate) _session_cleanup_token: Option<CancellationToken>,
    /// Cancellation token for the memory maintenance background service.
    pub(crate) _memory_maintenance_token: Option<CancellationToken>,
    /// MCP manager for external server connections (kept alive for the agent's lifetime).
    /// Wrapped in Mutex so `shutdown(&self)` can take ownership for graceful disconnect.
    pub(crate) mcp_manager: tokio::sync::Mutex<Option<mcp::McpManager>>,
}

impl AgentLoop {
    /// Handle emoji reactions by mapping to satisfaction scores.
    /// Updates the most recent strategy_record for this chat. No response sent.
    async fn handle_reaction(&self, msg: &bus::InboundMessage) -> common::Result<()> {
        let score = match reaction_to_satisfaction(&msg.content) {
            Some(s) => s,
            None => {
                debug!("Ignoring unrecognized reaction emoji: {}", msg.content);
                return Ok(());
            }
        };

        if let Some(ref strategy_repo) = self.strategy_repo {
            let window_minutes = self.config.orchestrator.satisfaction_window_minutes as i64;
            let since = chrono::Utc::now() - chrono::Duration::minutes(window_minutes);
            match strategy_repo
                .set_satisfaction_for_chat(msg.chat_id.as_str(), since, score)
                .await
            {
                Ok(true) => {
                    info!(
                        "Updated satisfaction score {} for chat {}",
                        score, msg.chat_id
                    );
                }
                Ok(false) => {
                    debug!("No recent strategy record found for chat {}", msg.chat_id);
                }
                Err(e) => {
                    warn!("Failed to update satisfaction: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Extract the inbound receiver from the agent loop.
    ///
    /// Should be called once before wrapping in `Arc`. Returns `None` if already taken.
    /// Use `run_with_rx` to drive the agent loop after extracting the receiver.
    pub fn take_inbound_rx(&mut self) -> Option<mpsc::Receiver<InboundMessage>> {
        self.inbound_rx.take()
    }

    /// Run the agent loop with an externally-provided inbound receiver.
    ///
    /// Takes `&self` (not `&mut self`) so it can be called on `Arc<AgentLoop>` without
    /// any Mutex wrapper. Call `take_inbound_rx()` before wrapping in `Arc`, then pass
    /// the extracted receiver here.
    pub async fn run_with_rx(&self, mut inbound_rx: mpsc::Receiver<InboundMessage>) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        info!("Agent loop started");

        while self.running.load(Ordering::SeqCst) {
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

    /// Run the agent loop, processing messages from the bus.
    ///
    /// Backward-compatible method that calls `take_inbound_rx()` + `run_with_rx()`.
    pub async fn run(&mut self) -> Result<()> {
        let inbound_rx = self.inbound_rx.take().ok_or_else(|| {
            common::KlyntbotError::Bus("AgentLoop::run can only be called once".into())
        })?;
        self.run_with_rx(inbound_rx).await
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

        // Stop the session cleanup service
        if let Some(token) = &self._session_cleanup_token {
            token.cancel();
        }

        // Stop the memory maintenance service
        if let Some(token) = &self._memory_maintenance_token {
            token.cancel();
        }

        // Disconnect MCP servers (cleanly terminates child processes)
        if let Some(manager) = self.mcp_manager.lock().await.take() {
            manager.disconnect_all().await;
        }

        Ok(())
    }

    /// Reconnect a single MCP server and re-register its tools.
    ///
    /// Called after OAuth completes or settings changes to inject the new
    /// configuration into the MCP subprocess environment.
    pub async fn reconnect_mcp_server(&self, server_def: &config::McpServerDef) {
        let prefix = mcp::sanitize::server_prefix(&server_def.name);

        let mut manager_guard = self.mcp_manager.lock().await;
        let manager = match manager_guard.as_mut() {
            Some(m) => m,
            None => {
                // No MCP manager yet — create one using the agent's actual config
                *manager_guard = Some(mcp::McpManager::connect_all(&self.config.mcp, None).await);
                manager_guard.as_mut().unwrap()
            }
        };

        let new_tools = manager.reconnect_server(server_def).await;

        // Clean up old tools, then register new ones
        let mut registry = self.tool_registry.write().await;
        registry.unregister_by_prefix(&prefix);
        for tool in new_tools {
            registry.register_dyn(tool as tools_core::DynTool);
        }
        tracing::info!(
            server = %server_def.name,
            "MCP tools re-registered after reconnect"
        );
    }

    /// Disconnect a single MCP server and unregister all its tools.
    ///
    /// Called when a server is removed or disabled in settings.
    pub async fn disconnect_mcp_server(&self, server_name: &str) {
        let prefix = mcp::sanitize::server_prefix(server_name);

        // Unregister tools first
        {
            let mut registry = self.tool_registry.write().await;
            let removed = registry.unregister_by_prefix(&prefix);
            tracing::info!(server = %server_name, tools_removed = removed, "MCP tools unregistered");
        }

        // Disconnect server in MCP manager
        let mut manager_guard = self.mcp_manager.lock().await;
        if let Some(manager) = manager_guard.as_mut() {
            manager.disconnect_server(server_name).await;
        }
    }

    /// Process a single inbound message
    #[tracing::instrument(skip(self, msg), fields(channel = %msg.channel, sender = %msg.sender_id))]
    async fn process_message(&self, msg: InboundMessage) -> Result<()> {
        // Validate message size
        if let Err(e) = msg.validate() {
            warn!("Message validation failed: {}", e);
            let error_msg = OutboundMessage::new(
                msg.channel.clone(),
                msg.chat_id.clone(),
                format!(
                    "Message too large to process: {}. Please shorten and try again.",
                    e
                ),
            );
            if let Err(send_err) = self.bus.publish_outbound(error_msg).await {
                warn!("Failed to send validation error: {}", send_err);
            }
            return Ok(());
        }

        // Handle reaction messages — update satisfaction, no LLM call
        if msg.kind == bus::MessageKind::Reaction {
            return self.handle_reaction(&msg).await;
        }

        // Handle system messages (subagent results)
        if msg.channel.as_str() == common::SYSTEM_CHANNEL {
            return self.process_system_message(msg).await;
        }

        // Track last active channel for notifications
        if let Some(last_active) = &self.last_active_channel {
            *last_active.write().await = Some((msg.channel.clone(), msg.chat_id.clone()));
        }

        let preview = if msg.content.len() > 80 {
            format!("{}...", truncate_at_boundary(&msg.content, 80))
        } else {
            msg.content.clone()
        };

        info!(
            "Processing message from {}:{}: {}",
            msg.channel, msg.sender_id, preview
        );

        // Get or create session — returns per-session Arc<Mutex<Session>>
        let session_key = msg.session_key();
        let session_arc = self
            .session_manager
            .get_or_create(session_key.as_str())
            .await?;

        // Mutate session and collect data under the per-session lock
        let (history, embed_msg_id) = {
            let mut session = session_arc.lock().await;
            session.add_message("user", &msg.content);
            let msg_id = session.messages.last().map(|m| m.id.clone());
            let history = session.get_history(self.history_limit).to_vec();
            (history, msg_id)
            // per-session lock released here
        };

        // Async conversation embedding hook for user message
        if let Some(msg_id) = embed_msg_id {
            self.spawn_embed_message(session_key.as_str(), "user", &msg.content, &msg_id);
        }

        // Run through pipeline
        let routing_ctx = RoutingContext::new(msg.channel.clone(), msg.chat_id.clone());
        let response_content = self
            .run_pipeline(&msg.content, history, &routing_ctx, None, None)
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
        if msg.sender_id == common::TELEGRAM_RESET_SENDER {
            let key = msg.chat_id.as_str();
            if let Err(e) = self.session_manager.reset_session(key).await {
                warn!("Failed to reset session {}: {}", key, e);
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

        // Format system message with sender_id prefix
        let system_msg_content = format!("[System: {}] {}", msg.sender_id, msg.content);

        // Get or create session and mutate under the per-session lock
        let session_arc = self.session_manager.get_or_create(&session_key).await?;
        let history = {
            let mut session = session_arc.lock().await;
            session.add_message("user", &system_msg_content);
            session.get_history(self.history_limit).to_vec()
            // per-session lock released here
        };

        // Run through pipeline
        let routing_ctx = RoutingContext::new(origin_channel.into(), origin_chat_id.into());
        let response_content = self
            .run_pipeline(&system_msg_content, history, &routing_ctx, None, None)
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
            .iter()
            .any(|r| r == role)
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
                .iter()
                .any(|c| c == channel)
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
        if let Ok(session_arc) = self.session_manager.get_or_create(session_key).await {
            // Mutate under per-session lock, clone for async save
            let session_clone = {
                let mut session = session_arc.lock().await;
                session.add_message("assistant", content);

                if let Some(msg_id) = session.messages.last().map(|m| m.id.clone()) {
                    self.spawn_embed_message(session_key, "assistant", content, &msg_id);
                }

                session.clone()
                // per-session lock released here
            };

            if let Err(e) = self.session_manager.save(&session_clone).await {
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
    async fn get_tool_info(&self) -> (std::sync::Arc<Vec<serde_json::Value>>, Vec<String>) {
        let tool_registry = self.tool_registry.read().await;
        let tool_defs = tool_registry.get_definitions();
        let tool_names = tool_registry.tool_names();
        (tool_defs, tool_names)
    }

    /// Run a message through the agent runtime with the given routing context.
    async fn run_pipeline(
        &self,
        content: &str,
        history: Vec<session::SessionMessage>,
        routing_ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<AgentEvent>>,
        cancel_token: Option<CancellationToken>,
    ) -> Result<String> {
        let system_prompt = self
            .context_engine
            .build_system_prompt(
                routing_ctx.channel.as_str(),
                routing_ctx.chat_id.as_str(),
                Some(content),
            )
            .await;

        let history_messages = Self::convert_history(&history);
        let (tool_defs, tool_names) = self.get_tool_info().await;
        let tool_name_refs: Vec<&str> = tool_names.iter().map(|s| s.as_str()).collect();

        let result = self
            .runtime
            .process_message(
                content,
                history_messages,
                &tool_defs,
                &tool_name_refs,
                routing_ctx,
                Some(&system_prompt),
                event_tx.clone(),
                cancel_token,
            )
            .await?;

        info!(
            "AgentRuntime: agent={}, mode={}",
            result.agent_name, result.mode_used
        );

        Ok(result.content)
    }

    /// Process a message directly (for CLI mode).
    ///
    /// Returns the agent's response text directly instead of publishing to the bus.
    /// Shared session setup: log preview, add message to session, embed, return history.
    async fn setup_session(
        &self,
        content: &str,
        session_key: &str,
        label: &str,
    ) -> Result<Vec<session::SessionMessage>> {
        let preview = if content.len() > 80 {
            format!("{}...", truncate_at_boundary(content, 80))
        } else {
            content.to_string()
        };
        debug!("Processing {} message: {}", label, preview);

        let session_arc = self.session_manager.get_or_create(session_key).await?;
        let (history, embed_msg_id) = {
            let mut session = session_arc.lock().await;
            session.add_message("user", content);
            let msg_id = session.messages.last().map(|m| m.id.clone());
            let history = session.get_history(self.history_limit).to_vec();
            (history, msg_id)
        };

        if let Some(msg_id) = embed_msg_id {
            self.spawn_embed_message(session_key, "user", content, &msg_id);
        }

        Ok(history)
    }

    pub async fn process_direct(&self, content: String, session_key: String) -> Result<String> {
        let history = self.setup_session(&content, &session_key, "direct").await?;

        // Run through pipeline
        let routing_ctx = RoutingContext::new("cli".into(), session_key.clone().into());
        let response_content = self
            .run_pipeline(&content, history, &routing_ctx, None, None)
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
        let history = self
            .setup_session(&content, &session_key, "streaming direct")
            .await?;

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
        let cancel_clone = cancel_token.clone();

        // Clone Arcs for the spawned task
        let agent = Arc::clone(self);
        let sk = session_key.clone();

        let handle = tokio::spawn(async move {
            let pipeline_event_tx = event_tx.clone();
            let result = match agent
                .run_pipeline(
                    &content,
                    history,
                    &routing_ctx,
                    Some(pipeline_event_tx),
                    Some(cancel_clone),
                )
                .await
            {
                Ok(response) => {
                    // ContentChunk events are emitted per-token inside run_cycle
                    // (via call_provider_streaming), so we only emit Done here.
                    let _ = event_tx
                        .send(AgentEvent::Done {
                            content: response.clone(),
                        })
                        .await;
                    Ok(response)
                }
                Err(e) => {
                    let _ = event_tx
                        .send(AgentEvent::Error {
                            message: e.to_string(),
                        })
                        .await;
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

    /// Get all tool definitions from the registry (for /api/status).
    pub async fn list_tools(&self) -> std::sync::Arc<Vec<serde_json::Value>> {
        self.tool_registry.read().await.get_definitions()
    }

    /// Get all registered tool names (for /api/status).
    pub async fn tool_names(&self) -> Vec<String> {
        self.tool_registry.read().await.tool_names()
    }

    /// Get the model name from config (for display purposes).
    pub fn model_name(&self) -> &str {
        &self.config.agents.defaults.model
    }
}

/// Map emoji reactions to satisfaction scores.
/// Returns None for unrecognized emoji (silently ignored).
fn reaction_to_satisfaction(emoji: &str) -> Option<f32> {
    match emoji.trim() {
        "\u{1F44D}" => Some(1.0),                     // 👍
        "\u{2764}\u{FE0F}" | "\u{2764}" => Some(1.0), // ❤️ / ❤
        "\u{1F389}" => Some(1.0),                     // 🎉
        "\u{1F44E}" => Some(0.0),                     // 👎
        "\u{1F615}" => Some(0.0),                     // 😕
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bus::{LearningEvent, LearningEventBus};

    #[test]
    fn test_reaction_to_satisfaction_positive() {
        assert_eq!(reaction_to_satisfaction("\u{1F44D}"), Some(1.0));
        assert_eq!(reaction_to_satisfaction("\u{2764}\u{FE0F}"), Some(1.0));
        assert_eq!(reaction_to_satisfaction("\u{2764}"), Some(1.0));
        assert_eq!(reaction_to_satisfaction("\u{1F389}"), Some(1.0));
    }

    #[test]
    fn test_reaction_to_satisfaction_negative() {
        assert_eq!(reaction_to_satisfaction("\u{1F44E}"), Some(0.0));
        assert_eq!(reaction_to_satisfaction("\u{1F615}"), Some(0.0));
    }

    #[test]
    fn test_reaction_to_satisfaction_unknown() {
        assert_eq!(reaction_to_satisfaction("\u{1F914}"), None);
        assert_eq!(reaction_to_satisfaction("hello"), None);
        assert_eq!(reaction_to_satisfaction(""), None);
    }

    /// AC-I2.3/2.4: Learning subscriber updates ConfidenceSource threshold
    /// when LearningService publishes a ThresholdChanged event.
    #[tokio::test]
    async fn test_learning_subscriber_updates_confidence_threshold() {
        use crate::context_sources::ConfidenceSource;
        use std::sync::atomic::Ordering;

        let source = ConfidenceSource::new(0.70);
        let threshold_handle = source.threshold_handle();

        let event_bus = Arc::new(LearningEventBus::new(16));
        let handle_for_subscriber = threshold_handle.clone();
        let mut rx = event_bus.subscribe();

        // Spawn subscriber (same pattern as AgentLoop::new_with_cron wires it)
        let handle = tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                if let LearningEvent::ThresholdChanged { new_threshold, .. } = event {
                    handle_for_subscriber.store(new_threshold.to_bits(), Ordering::Relaxed);
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

        let actual = f32::from_bits(threshold_handle.load(Ordering::Relaxed));
        assert!(
            (actual - 0.82).abs() < f32::EPSILON,
            "Expected threshold 0.82, got {}",
            actual
        );

        handle.abort();
    }
}
