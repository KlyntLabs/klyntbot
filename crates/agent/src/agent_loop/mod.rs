//! Agent loop: the core processing engine.

mod builder;

#[cfg(test)]
mod refactor_tests;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use bus::{InboundMessage, MessageBus, OutboundMessage};
use common::{truncate_at_boundary, Result};
use config::Config;
use providers::Message;
use session::SessionManager;
use tokio::sync::mpsc;
use tools::RoutingContext;

use super::AgentEvent;

/// Window (in minutes) to retroactively mark shadow log entries as corrected.
const CORRECTION_WINDOW_MINUTES: i32 = 15;

/// Truncate `content` to `max` bytes, appending "..." if truncated.
fn preview_text(content: &str, max: usize) -> String {
    if content.len() > max {
        format!("{}...", truncate_at_boundary(content, max))
    } else {
        content.to_string()
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
pub(crate) type LastActiveChannel = Arc<RwLock<Option<(common::ChannelName, common::ChatId)>>>;

/// Agent loop - the core processing engine
pub struct AgentLoop {
    pub(crate) bus: Arc<MessageBus>,
    pub(crate) inbound_rx: Option<mpsc::Receiver<InboundMessage>>,
    pub(crate) config: Config,
    pub(crate) session_manager: SessionManager,
    pub(crate) tool_registry: Arc<RwLock<tools::registry::ToolRegistry>>,
    pub(crate) running: Arc<AtomicBool>,
    pub(crate) last_active_channel: Option<LastActiveChannel>,
    pub(crate) recurring_task_spawner: Option<Arc<RwLock<super::RecurringTaskSpawner>>>,
    /// Conversation recall handler for semantic memory
    pub(crate) conversation_recall_handler: Option<Arc<dyn tools::ConversationRecallHandler>>,
    /// Background learning service for adaptive threshold updates (None if learning disabled)
    pub(crate) learning_service: Option<Arc<RwLock<crate::learning::LearningService>>>,
    /// Agent runtime: agent-first execution pipeline.
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
    /// Wrapped in Arc<Mutex> so the health check background task can share access.
    pub(crate) mcp_manager: Arc<tokio::sync::Mutex<Option<mcp::McpManager>>>,
    /// Cancellation token for the MCP health check background service.
    pub(crate) _mcp_health_check_token: Option<CancellationToken>,
    /// Shared DomainEventBus for cross-feature communication (cognitive + coaching + autotuner).
    pub(crate) domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    /// Trial repo for marking shadow log entries as corrected (autotuner).
    pub(crate) trial_repo: Option<storage::TrialRepo>,
    /// Background consolidation service for cognitive memory (kept alive for graceful shutdown).
    /// Wrapped in Mutex so `shutdown(&self)` can take ownership for graceful stop.
    pub(crate) cognitive_bg_service:
        tokio::sync::Mutex<Option<cognitive::background::BackgroundConsolidationService>>,
    /// Background session memory service (per-session scratchpad maintenance).
    /// Stored to prevent drop which would cancel the background task.
    pub(crate) _session_memory_service: Option<cognitive::SessionMemoryService>,
    /// Cancellation token for the work context inference loop.
    pub(crate) _inference_loop_token: Option<CancellationToken>,
    /// Parent cancellation token for all tree builder subscriber tasks.
    pub(crate) _tree_builder_token: Option<CancellationToken>,
    /// Activity ingestion service for chat message logging.
    pub(crate) activity_svc: Option<Arc<activity_log::ActivityIngestionService>>,
    /// Shared skill store — flat file-based skill loading, used for hot-reload.
    pub(crate) skill_store: Arc<RwLock<skill_system::SkillStore>>,
    /// Shared hot-reloadable config — updated by ConfigWatcherService without restart.
    pub(crate) hot_config: Arc<RwLock<config::HotConfig>>,
    /// Subagent manager for background task spawning (kept alive for tool_kit injection).
    pub(crate) subagent_manager: Option<Arc<crate::SubagentManager>>,
}

impl AgentLoop {
    /// Public accessor for the tool registry.
    /// Used by `klyntbot-server` to bridge internal tools to MCP.
    pub fn tool_registry(&self) -> Arc<RwLock<tools::registry::ToolRegistry>> {
        Arc::clone(&self.tool_registry)
    }

    pub fn runtime(&self) -> Arc<crate::agent_runtime::AgentRuntime> {
        Arc::clone(&self.runtime)
    }

    /// Public accessor for the subagent manager.
    pub fn subagent_manager(&self) -> Option<Arc<crate::SubagentManager>> {
        self.subagent_manager.clone()
    }

    /// Public accessor for the skill store.
    /// Used by `klyntbot-server` to expose active skills via MCP resources.
    pub fn skill_store(&self) -> Arc<RwLock<skill_system::SkillStore>> {
        Arc::clone(&self.skill_store)
    }

    /// Public accessor for the shared hot-reloadable config.
    pub fn hot_config(&self) -> Arc<RwLock<config::HotConfig>> {
        Arc::clone(&self.hot_config)
    }

    /// Inject the tool-kit builder into the subagent manager (called by app-core init).
    pub fn set_subagent_tool_kit(&self, kit: Arc<klynt_core::ToolKitBuilder>) {
        if let Some(ref mgr) = self.subagent_manager {
            mgr.set_tool_kit(kit);
        }
    }

    /// Inject the hook engine into the subagent manager (called by app-core init).
    pub fn set_subagent_hook_engine(&self, engine: Arc<klynt_hooks::HookEngine>) {
        if let Some(ref mgr) = self.subagent_manager {
            mgr.set_hook_engine(engine);
        }
    }

    /// Inject the event sender into the subagent manager (called by app-core init).
    pub fn set_subagent_event_sender(
        &self,
        tx: tokio::sync::broadcast::Sender<crate::subagent_events::SubagentLifecycleEvent>,
    ) {
        if let Some(ref mgr) = self.subagent_manager {
            mgr.set_event_sender(tx);
        }
    }

    /// Reload skill files from disk (hot-reload after UI edits).
    pub async fn reload_agents(&self) -> common::Result<()> {
        let mut store = self.skill_store.write().await;
        store.reload()?;
        info!("Skills reloaded ({} skills)", store.names().len());
        Ok(())
    }

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
            let window_minutes: i64 = 30; // Default satisfaction window
            let since = jiff::Timestamp::now()
                .checked_sub(jiff::SignedDuration::from_secs(window_minutes * 60))
                .unwrap();
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

        // Emit correction signal for negative reactions
        if score == 0.0 {
            // Read last assistant message from session
            let session_key = msg.session_key();
            let last_assistant = if let Ok(session_arc) = self
                .session_manager
                .get_or_create(session_key.as_str())
                .await
            {
                let session = session_arc.lock().await;
                session
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "assistant")
                    .map(|m| m.content.clone())
            } else {
                None
            };

            let active_skill = self.resolve_active_skill(session_key.as_str()).await;
            self.emit_correction_signal(
                msg.chat_id.as_str(),
                last_assistant.unwrap_or_default(),
                msg.content.clone(),
                bus::CorrectionKind::Reaction,
                1.0,
                session_key.to_string(),
                active_skill,
            )
            .await;
        }

        Ok(())
    }

    /// Resolve the active skill for a session from the most recent strategy record.
    async fn resolve_active_skill(&self, session_key: &str) -> Option<String> {
        if let Some(ref repo) = self.strategy_repo {
            repo.latest_skill_for_session(session_key)
                .await
                .ok()
                .flatten()
        } else {
            None
        }
    }

    /// Emit a UserCorrectedAI correction signal and mark shadow log entries.
    #[allow(clippy::too_many_arguments)]
    async fn emit_correction_signal(
        &self,
        chat_id: &str,
        original: String,
        correction: String,
        kind: bus::CorrectionKind,
        strength: f64,
        session_key: String,
        active_skill: Option<String>,
    ) {
        if let Some(ref bus) = self.domain_event_bus {
            bus.publish(bus::DomainEvent::UserCorrectedAI {
                original,
                correction,
                kind,
                strength,
                session_key,
                active_skill,
            });
        }
        if let Some(ref trial_repo) = self.trial_repo {
            if let Err(e) = trial_repo
                .mark_recent_messages_corrected(chat_id, CORRECTION_WINDOW_MINUTES)
                .await
            {
                warn!("Failed to mark shadow log entries as corrected: {}", e);
            }
        }
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
    ///
    /// When a `DomainEventBus` is available the loop also listens for
    /// `FocusSessionStarted` / `FocusSessionEnded` events. While a focus
    /// session is active, inbound messages are deferred (buffered) and a
    /// `MessageDeferred` event is emitted. Once the focus session ends the
    /// deferred messages are drained and processed in order.
    pub async fn run_with_rx(&self, mut inbound_rx: mpsc::Receiver<InboundMessage>) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);

        info!("Agent loop started");

        // Subscribe to domain events for focus-session awareness.
        let mut event_rx = self.domain_event_bus.as_ref().map(|bus| bus.subscribe());
        let mut focus_active = false;
        let mut deferred_messages: Vec<InboundMessage> = Vec::new();
        // Tracks which (channel, sender) pairs have already received an auto-reply this focus
        // session so we only send one per sender per focus session.
        let mut auto_replied_senders: HashSet<(String, String)> = HashSet::new();

        while self.running.load(Ordering::SeqCst) {
            tokio::select! {
                msg = inbound_rx.recv() => {
                    let Some(msg) = msg else {
                        // Channel closed — exit loop.
                        break;
                    };

                    if focus_active {
                        info!(
                            channel = %msg.channel,
                            sender = %msg.sender_id,
                            "Deferring message during focus session"
                        );
                        // MessageDeferred variant deleted; no-op.

                        // Auto-reply once per sender per focus session when enabled.
                        let focus_bubble = &self.config.productivity.focus_bubble;
                        if focus_bubble.auto_reply_enabled {
                            let key = (msg.channel.to_string(), msg.sender_id.clone());
                            if auto_replied_senders.insert(key) {
                                let reply = OutboundMessage::new(
                                    msg.channel.clone(),
                                    msg.chat_id.clone(),
                                    focus_bubble.auto_reply_text.clone(),
                                );
                                if let Err(e) = self.bus.publish_outbound(reply).await {
                                    warn!("Failed to send focus auto-reply: {}", e);
                                } else {
                                    info!(
                                        sender = %msg.sender_id,
                                        "Sent focus auto-reply"
                                    );
                                }
                            }
                        }

                        deferred_messages.push(msg);
                    } else if let Err(e) = self.process_message(msg).await {
                        error!("Error processing message: {}", e);
                    }
                }
                result = async {
                    match event_rx.as_mut() {
                        Some(rx) => rx.recv().await.map(Some),
                        None => std::future::pending::<std::result::Result<Option<bus::DomainEvent>, _>>().await,
                    }
                } => {
                    match result {
                        Ok(Some(bus::DomainEvent::FocusSessionStarted { .. })) => {
                            info!("Focus session started — deferring inbound messages");
                            focus_active = true;
                            // Reset per-session dedup state on each new focus session.
                            auto_replied_senders.clear();
                        }
                        Ok(Some(bus::DomainEvent::FocusSessionEnded { .. })) => {
                            info!(
                                deferred = deferred_messages.len(),
                                "Focus session ended — draining deferred messages"
                            );
                            focus_active = false;
                            auto_replied_senders.clear();
                            for deferred in deferred_messages.drain(..) {
                                if let Err(e) = self.process_message(deferred).await {
                                    error!("Error processing deferred message: {}", e);
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("Domain event bus lagged by {n} events");
                        }
                        _ => {
                            // Other domain events — not relevant here.
                        }
                    }
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

        // Stop the work context inference loop
        if let Some(token) = &self._inference_loop_token {
            token.cancel();
        }

        // Stop the MCP health check background service
        if let Some(token) = &self._mcp_health_check_token {
            token.cancel();
        }

        // Stop all tree builder subscriber tasks
        if let Some(token) = &self._tree_builder_token {
            token.cancel();
        }

        // Stop the cognitive background consolidation service (cancels + awaits JoinHandle)
        if let Some(mut svc) = self.cognitive_bg_service.lock().await.take() {
            svc.stop().await;
        }

        // Stop the session memory service
        if let Some(ref svc) = self._session_memory_service {
            svc.shutdown();
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
                *manager_guard = Some(
                    mcp::McpManager::connect_all(
                        &self.config.mcp,
                        None,
                        mcp::McpClientOptions::default(),
                    )
                    .await,
                );
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

    /// Fire-and-forget ingestion of a chat message into the activity log.
    fn ingest_chat_message(&self, session_key: &str, role: &str, content: &str) {
        if let Some(ref svc) = self.activity_svc {
            let input = activity_log::ChatMessageInput {
                session_key: session_key.to_string(),
                role: role.to_string(),
                content: content.to_string(),
            };
            if let Some(entry) = activity_log::ActivityNormalizer::normalize(
                &activity_log::ChatMessageNormalizer,
                &input as &dyn std::any::Any,
            ) {
                svc.ingest_fire_and_forget(entry);
            }
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
            let new = Some((msg.channel.clone(), msg.chat_id.clone()));
            let mut guard = last_active.write().await;
            if *guard != new {
                *guard = new;
            }
        }

        let preview = preview_text(&msg.content, 80);

        info!(
            "Processing message from {}:{}: {}",
            msg.channel, msg.sender_id, preview
        );

        // Detect correction prefix and memory miss BEFORE acquiring session lock
        let correction_strength = detect_correction_prefix(&msg.content);
        let is_memory_miss = detect_memory_miss(&msg.content);

        // Get or create session — returns per-session Arc<Mutex<Session>>
        let session_key = msg.session_key();
        let session_arc = self
            .session_manager
            .get_or_create(session_key.as_str())
            .await?;

        // Mutate session and collect data under the per-session lock
        let (history, embed_msg_id, last_assistant_content) = {
            let mut session = session_arc.lock().await;
            // Capture last assistant message if a correction or memory miss was detected
            let last_assistant = if correction_strength.is_some() || is_memory_miss {
                session
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "assistant")
                    .map(|m| m.content.clone())
            } else {
                None
            };
            if session.correction_cooldown > 0 {
                session.correction_cooldown -= 1;
            }
            session.add_message("user", &msg.content);
            let msg_id = session.messages.last().map(|m| m.id.clone());
            let history = session.get_history(self.history_limit).to_vec();
            (history, msg_id, last_assistant)
            // per-session lock released here
        };

        // Async conversation embedding hook for user message
        if let Some(ref msg_id) = embed_msg_id {
            self.spawn_embed_message(session_key.as_str(), "user", &msg.content, msg_id);
        }

        // Ingest user message into activity log (fire-and-forget)
        self.ingest_chat_message(session_key.as_str(), "user", &msg.content);

        // Keyword-based correction detection — emit signal when user corrects the AI
        // Rate-limited: max 1 keyword correction per 3 messages per session.
        // Resolve active skill once for all correction branches.
        let correction_skill = if correction_strength.is_some() || is_memory_miss {
            self.resolve_active_skill(session_key.as_str()).await
        } else {
            None
        };
        if let Some(strength) = correction_strength {
            if let Some(ref original) = last_assistant_content {
                let should_emit = {
                    let mut session = session_arc.lock().await;
                    if session.correction_cooldown > 0 {
                        false
                    } else {
                        session.correction_cooldown = 3;
                        true
                    }
                };

                if should_emit {
                    // Use MemoryMiss kind if the message also indicates a memory miss
                    let kind = if is_memory_miss {
                        bus::CorrectionKind::MemoryMiss
                    } else {
                        bus::CorrectionKind::KeywordPrefix
                    };
                    self.emit_correction_signal(
                        msg.chat_id.as_str(),
                        original.clone(),
                        msg.content.clone(),
                        kind,
                        strength,
                        session_key.to_string(),
                        correction_skill.clone(),
                    )
                    .await;
                }
            }
        } else if is_memory_miss {
            // Memory miss without a general correction prefix — emit standalone
            if let Some(ref original) = last_assistant_content {
                self.emit_correction_signal(
                    msg.chat_id.as_str(),
                    original.clone(),
                    msg.content.clone(),
                    bus::CorrectionKind::MemoryMiss,
                    0.8,
                    session_key.to_string(),
                    correction_skill,
                )
                .await;
            }
        }

        // Build correction context for query rewriting (if a correction was detected)
        let correction = if correction_strength.is_some() {
            last_assistant_content
                .as_ref()
                .map(|original| context_engine::CorrectionContext {
                    rejected_topic: crate::adapters::query_rewriter::extract_key_terms_from(
                        &original.chars().take(200).collect::<String>(),
                    ),
                    corrected_to: msg.content.clone(),
                })
        } else {
            None
        };

        // Run through pipeline
        let mut routing_ctx = RoutingContext::new(msg.channel.clone(), msg.chat_id.clone());
        routing_ctx.session_key = Some(session_key.clone());
        routing_ctx.message_id = embed_msg_id.map(|id| id.to_string());
        let response_content = self
            .run_pipeline(&msg.content, history, &routing_ctx, None, None, correction)
            .await?;

        // Prepend acknowledgement when a keyword correction was detected
        let response_content = if correction_strength.is_some() && last_assistant_content.is_some()
        {
            format!("Noted — adjusting for next time.\n\n{response_content}")
        } else {
            response_content
        };

        // Save assistant response to session
        self.save_to_session(session_key.as_str(), &response_content)
            .await;

        // Ingest assistant response into activity log (fire-and-forget)
        self.ingest_chat_message(session_key.as_str(), "assistant", &response_content);

        // Publish chat turn to cognitive consolidation pipeline
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                session_key: session_key.to_string(),
                user_message: Some(msg.content.clone()),
            });
        }

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
            session.add_message("system", &system_msg_content);
            session.get_history(self.history_limit).to_vec()
            // per-session lock released here
        };

        // Run through pipeline
        let routing_ctx = RoutingContext::new(origin_channel.into(), origin_chat_id.into());
        let response_content = self
            .run_pipeline(&system_msg_content, history, &routing_ctx, None, None, None)
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
        if let Some(handler) = &self.conversation_recall_handler {
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

    /// Persist the in-memory session to SQL without adding a new message.
    /// Used in error paths to ensure the user message is not lost.
    async fn persist_session(&self, session_key: &str) {
        if let Ok(session_arc) = self.session_manager.get_or_create(session_key).await {
            let session_clone = {
                let session = session_arc.lock().await;
                session.clone()
            };
            if let Err(e) = self.session_manager.save(&session_clone).await {
                warn!("Failed to persist session on error: {}", e);
            }
        }
    }

    /// Save assistant response to session and return the persisted message ID.
    async fn save_to_session(&self, session_key: &str, content: &str) -> Option<String> {
        if let Ok(session_arc) = self.session_manager.get_or_create(session_key).await {
            // Mutate under per-session lock, clone for async save
            let (session_clone, msg_id) = {
                let mut session = session_arc.lock().await;
                session.add_message("assistant", content);

                let msg_id = session.messages.last().map(|m| m.id.clone());
                if let Some(ref id) = msg_id {
                    self.spawn_embed_message(session_key, "assistant", content, id);
                }

                (session.clone(), msg_id)
                // per-session lock released here
            };

            if let Err(e) = self.session_manager.save(&session_clone).await {
                warn!("Failed to save session: {}", e);
                return None;
            }
            return msg_id;
        }
        None
    }

    /// Convert session history to provider Messages.
    fn convert_history(history: &[session::SessionMessage]) -> Vec<Message> {
        history
            .iter()
            .map(|m| match m.role.as_str() {
                "system" => Message::system(&m.content),
                "user" => Message::user(&m.content),
                "assistant" => Message::assistant(&m.content),
                other => {
                    tracing::warn!("Unknown message role '{}', treating as user", other);
                    Message::user(&m.content)
                }
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
        _correction: Option<context_engine::CorrectionContext>,
    ) -> Result<String> {
        let history_messages = Self::convert_history(&history);
        let (tool_defs, _tool_names) = self.get_tool_info().await;
        let channel = common::tool_channel::Channel::from_name(routing_ctx.channel.as_str());
        let registry = self.tool_registry.read().await;
        let filtered_defs: Arc<Vec<serde_json::Value>> = Arc::new(
            tool_defs
                .iter()
                .filter(|def| {
                    let name = def
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    registry
                        .get(name)
                        .map(|tool| tool.allowed_channels().allows(channel))
                        .unwrap_or(true)
                })
                .cloned()
                .collect(),
        );
        drop(registry);

        let result = self
            .runtime
            .process_message(
                content,
                history_messages,
                &filtered_defs,
                routing_ctx,
                event_tx.clone(),
                cancel_token,
                crate::execution::DepthMode::Normal,
            )
            .await?;

        info!(
            "AgentRuntime: agent={}, mode={}",
            result.agent_name, result.mode_used
        );

        // Publish SkillRouted so mirror sources and the routing metric registry
        // observe one event per message. In the flat runtime there is no
        // discrete skill-selection step, so the agent + mode serve as the
        // effective routing decision. Confidence is fixed at 1.0 because this
        // is a deterministic passthrough, not a scored selection.
        if let Some(ref bus) = self.domain_event_bus {
            bus.publish(bus::DomainEvent::SkillRouted {
                skill_name: result.mode_used.clone(),
                confidence: 1.0,
                source: format!("flat_runtime/{}", result.agent_name),
                trigger_phrases: Vec::new(),
                session_key: routing_ctx.chat_id.as_str().to_string(),
            });
        }

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
    ) -> Result<(Vec<session::SessionMessage>, Option<String>)> {
        let preview = preview_text(content, 80);
        debug!("Processing {} message: {}", label, preview);

        let session_arc = self.session_manager.get_or_create(session_key).await?;
        let (history, embed_msg_id) = {
            let mut session = session_arc.lock().await;
            session.add_message("user", content);
            let msg_id = session.messages.last().map(|m| m.id.to_string());
            let history = session.get_history(self.history_limit).to_vec();
            (history, msg_id)
        };

        if let Some(ref msg_id) = embed_msg_id {
            self.spawn_embed_message(session_key, "user", content, msg_id);
        }

        Ok((history, embed_msg_id))
    }

    pub async fn process_direct(&self, content: String, session_key: String) -> Result<String> {
        let (history, user_msg_id) = self.setup_session(&content, &session_key, "direct").await?;

        // Run through pipeline
        let mut routing_ctx = RoutingContext::new("cli".into(), session_key.clone().into());
        routing_ctx.session_key = Some(session_key.clone().into());
        routing_ctx.message_id = user_msg_id;
        let response_content = self
            .run_pipeline(&content, history, &routing_ctx, None, None, None)
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
        mode: Option<String>,
    ) -> Result<StreamingHandle> {
        // Detect correction prefix and memory miss BEFORE setup_session adds the user message
        let correction_strength = detect_correction_prefix(&content);
        let is_memory_miss = detect_memory_miss(&content);

        // Read last assistant message (if correction detected) and tick the cooldown
        // counter before setup_session mutates the session. Mirrors the two-phase
        // lock pattern in process_message: first lock decrements cooldown + captures
        // last assistant; second lock gates the emit.
        let (last_assistant_content, cooldown_after_decrement) =
            if let Ok(session_arc) = self.session_manager.get_or_create(&session_key).await {
                let mut session = session_arc.lock().await;
                let last_asst = if correction_strength.is_some() || is_memory_miss {
                    session
                        .messages
                        .iter()
                        .rev()
                        .find(|m| m.role == "assistant")
                        .map(|m| m.content.clone())
                } else {
                    None
                };
                if session.correction_cooldown > 0 {
                    session.correction_cooldown -= 1;
                }
                let cd = session.correction_cooldown;
                (last_asst, cd)
            } else {
                (None, 0)
            };

        // Emit correction signal (rate-limited: max 1 keyword correction per 3 messages)
        let stream_correction_skill = if correction_strength.is_some() || is_memory_miss {
            self.resolve_active_skill(session_key.as_str()).await
        } else {
            None
        };
        let correction_emitted = if let Some(strength) = correction_strength {
            if let Some(ref original) = last_assistant_content {
                if cooldown_after_decrement == 0 {
                    // Set cooldown under a fresh lock (mirrors process_message's second lock)
                    if let Ok(session_arc) = self.session_manager.get_or_create(&session_key).await
                    {
                        let mut session = session_arc.lock().await;
                        session.correction_cooldown = 3;
                    }
                    let kind = if is_memory_miss {
                        bus::CorrectionKind::MemoryMiss
                    } else {
                        bus::CorrectionKind::KeywordPrefix
                    };
                    self.emit_correction_signal(
                        &session_key,
                        original.clone(),
                        content.clone(),
                        kind,
                        strength,
                        session_key.clone(),
                        stream_correction_skill.clone(),
                    )
                    .await;
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else if is_memory_miss {
            // Memory miss without a general correction prefix — emit standalone
            if let Some(ref original) = last_assistant_content {
                self.emit_correction_signal(
                    &session_key,
                    original.clone(),
                    content.clone(),
                    bus::CorrectionKind::MemoryMiss,
                    0.8,
                    session_key.clone(),
                    stream_correction_skill,
                )
                .await;
            }
            true
        } else {
            false
        };

        let (history, user_msg_id) = self
            .setup_session(&content, &session_key, "streaming direct")
            .await?;

        // Create event channel and interaction channel
        let (event_tx, event_rx) = mpsc::channel(64);
        let (interaction_tx, interaction_rx) = mpsc::channel(4);

        // Authoritative session mode comes from the session row itself.
        // The legacy `mode: Option<String>` parameter is now an override hint
        // only used when the row does not yet exist (first turn).
        let session_mode: common::SessionMode = match self
            .session_manager
            .get_session_row(&session_key)
            .await
        {
            Ok(row) => row.session_mode(),
            Err(_) => mode
                .as_deref()
                .and_then(common::SessionMode::parse)
                .unwrap_or(common::SessionMode::Assistant),
        };

        let channel: common::ChannelName = match session_mode {
            common::SessionMode::Coding => common::CODING_CHANNEL.into(),
            common::SessionMode::Assistant => "desktop".into(),
        };

        // Routing context with interaction channel for ask_user tool
        let mut routing_ctx =
            RoutingContext::with_interaction(channel, session_key.clone().into(), interaction_tx);
        routing_ctx.session_mode = session_mode;
        routing_ctx.session_key = Some(session_key.clone().into());
        routing_ctx.message_id = user_msg_id;

        let cancel_token = CancellationToken::new();
        let cancel_clone = cancel_token.clone();

        // Whether to prepend correction acknowledgement to response
        let prepend_correction_ack = correction_emitted;

        // Build correction context for query rewriting (if a correction was detected)
        let correction = if correction_emitted {
            last_assistant_content
                .as_ref()
                .map(|original| context_engine::CorrectionContext {
                    rejected_topic: crate::adapters::query_rewriter::extract_key_terms_from(
                        &original.chars().take(200).collect::<String>(),
                    ),
                    corrected_to: content.clone(),
                })
        } else {
            None
        };

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
                    correction,
                )
                .await
            {
                Ok(response) => {
                    // Prepend acknowledgement when a keyword correction was detected
                    let response = if prepend_correction_ack {
                        format!("Noted — adjusting for next time.\n\n{response}")
                    } else {
                        response
                    };

                    // Save to session BEFORE emitting Done so the message ID
                    // is available and the DB row exists when the streaming
                    // relay tries to update metadata.
                    let message_id = agent.save_to_session(&sk, &response).await;
                    let _ = event_tx
                        .send(AgentEvent::Done {
                            content: response.clone(),
                            message_id,
                        })
                        .await;
                    if let Some(engine) = agent.runtime.hook_engine() {
                        let message_count = {
                            if let Ok(session_arc) = agent.session_manager.get_or_create(&sk).await
                            {
                                let session = session_arc.lock().await;
                                session.messages.len() as u64
                            } else {
                                0
                            }
                        };
                        let stop_input = klynt_hooks::events::stop::StopInput {
                            session_id: sk.clone(),
                            message_count,
                            base: Default::default(),
                        };
                        let _ = engine
                            .fire(klynt_hooks::engine::HookFireInput::Stop(stop_input))
                            .await;
                    }
                    Ok(response)
                }
                Err(e) => {
                    // Persist the session so the user message is saved even on error.
                    // Without this, the session row exists (created by chat_send)
                    // but has zero messages — a ghost thread in the sidebar.
                    agent.persist_session(&sk).await;
                    if let Some(engine) = agent.runtime.hook_engine() {
                        let error_input = klynt_hooks::events::error::ErrorInput {
                            session_id: sk.clone(),
                            kind: "agent_loop_error".to_string(),
                            message: e.to_string(),
                            recoverable: false,
                            base: Default::default(),
                        };
                        let _ = engine
                            .fire(klynt_hooks::engine::HookFireInput::Error(error_input))
                            .await;
                    }
                    let _ = event_tx
                        .send(AgentEvent::Error {
                            message: e.to_string(),
                        })
                        .await;
                    Err(e)
                }
            };

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

/// Case-insensitive `starts_with` for ASCII prefixes.
fn istarts_with(s: &str, prefix: &str) -> bool {
    let mut s_chars = s.chars();
    for b in prefix.chars() {
        match s_chars.next() {
            Some(a) if a.eq_ignore_ascii_case(&b) => {}
            _ => return false,
        }
    }
    true
}

/// Case-insensitive `contains` for ASCII needles.
fn icontains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut chars = haystack.chars();
    loop {
        if chars.clone().zip(needle.chars()).all(|(a, b)| a.eq_ignore_ascii_case(&b)) {
            return true;
        }
        if chars.next().is_none() {
            break;
        }
    }
    false
}

/// Detects if a user message starts with a correction phrase.
/// Returns the correction strength (1.0 for strong, 0.8 for soft) or None.
fn detect_correction_prefix(message: &str) -> Option<f64> {
    let check = &message[..message.len().min(80)];

    const STRONG: &[&str] = &["no,", "no ", "wrong", "that's not", "incorrect"];
    const SOFT: &[&str] = &["i meant", "try again", "redo", "not quite", "never mind"];

    for prefix in STRONG {
        if istarts_with(check, prefix) {
            return Some(1.0);
        }
    }
    for prefix in SOFT {
        if istarts_with(check, prefix) {
            return Some(0.8);
        }
    }
    None
}

/// Detects if a user message indicates the AI forgot a previously mentioned fact.
/// Returns true when the user signals a memory retrieval miss.
fn detect_memory_miss(message: &str) -> bool {
    let check = &message[..message.len().min(120)];

    const PHRASES: &[&str] = &[
        "i already told you",
        "i mentioned",
        "as i said",
        "i said before",
        "remember when i",
        "i told you",
        "you should know",
        "we talked about",
        "we discussed",
        "you forgot",
        "don't you remember",
        "i already said",
    ];

    PHRASES.iter().any(|phrase| icontains(check, phrase))
}

#[cfg(test)]
mod correction_tests {
    use super::detect_correction_prefix;

    #[test]
    fn detects_strong_corrections() {
        assert_eq!(detect_correction_prefix("No, that's wrong"), Some(1.0));
        assert_eq!(detect_correction_prefix("wrong answer"), Some(1.0));
        assert_eq!(detect_correction_prefix("incorrect, I wanted"), Some(1.0));
    }

    #[test]
    fn detects_soft_corrections() {
        assert_eq!(detect_correction_prefix("I meant the other one"), Some(0.8));
        assert_eq!(detect_correction_prefix("try again please"), Some(0.8));
    }

    #[test]
    fn ignores_normal_messages() {
        assert_eq!(detect_correction_prefix("What's the weather?"), None);
        assert_eq!(detect_correction_prefix("Hello there"), None);
    }

    #[test]
    fn rate_limiter_cooldown_mechanics() {
        use session::Session;

        // Fresh session: cooldown is 0 (ready to fire)
        let session = Session::new("test:chat");
        assert_eq!(session.correction_cooldown, 0);

        // Simulate emission: set cooldown to 3
        let mut session = session;
        session.correction_cooldown = 3;

        // Simulate 3 messages decrementing
        for i in (0..3).rev() {
            assert!(session.correction_cooldown > 0, "Should be rate-limited");
            session.correction_cooldown -= 1;
            assert_eq!(session.correction_cooldown, i as u32);
        }

        // After 3 messages: ready to fire again
        assert_eq!(session.correction_cooldown, 0);
    }

    #[test]
    fn detects_memory_miss_phrases() {
        use super::detect_memory_miss;
        assert!(detect_memory_miss("I already told you about my meeting"));
        assert!(detect_memory_miss("We discussed this yesterday"));
        assert!(detect_memory_miss("You forgot that I prefer dark mode"));
        assert!(detect_memory_miss("Don't you remember my schedule?"));
        assert!(detect_memory_miss("I mentioned my deadline earlier"));
        assert!(detect_memory_miss("As I said, the project is due Friday"));
    }

    #[test]
    fn memory_miss_ignores_normal_messages() {
        use super::detect_memory_miss;
        assert!(!detect_memory_miss("What's the weather?"));
        assert!(!detect_memory_miss("Help me plan my day"));
        assert!(!detect_memory_miss("No, that's wrong")); // correction but not memory miss
        assert!(!detect_memory_miss("Tell me about rust traits"));
    }

    #[test]
    fn excluded_phrases_return_none() {
        assert_eq!(detect_correction_prefix("not sure about that"), None);
        assert_eq!(detect_correction_prefix("hold on let me think"), None);
        assert_eq!(detect_correction_prefix("actually that reminds me"), None);
        assert_eq!(detect_correction_prefix("wait before that"), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
