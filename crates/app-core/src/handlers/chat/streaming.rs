//! Streaming relay, chat send/cancel, and interaction response handlers.

use std::collections::HashSet;
use std::sync::Arc;

use agent::AgentEvent;
use common::EntityCard;
use desktop_shared::commands::{ChatMessageResponse, SessionContextInput};
use desktop_shared::errors::ApiError;
use desktop_shared::events;
use storage::{Repos, SessionContextParams};
use tokio::sync::mpsc;
use tracing::Instrument;

use klynt_hooks::engine::HookFireInput;
use klynt_hooks::events::user_prompt_submit::UserPromptSubmitInput;

use crate::errors::map_storage_err;
use crate::state::AppCore;

/// Type aliases for the DashMap types used across both AppCore variants.
pub type ActiveStreams = dashmap::DashMap<String, ActiveStreamEntry>;
pub(super) type PendingInteractions =
    dashmap::DashMap<String, (String, tokio::sync::oneshot::Sender<common::FormResponse>)>;

#[derive(Clone)]
pub struct ActiveStreamEntry {
    pub guard_id: u64,
    pub generation: u32,
    pub cancel: tokio_util::sync::CancellationToken,
}

static STREAM_GUARD_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn next_guard_id() -> u64 {
    STREAM_GUARD_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

// ── ChatStreamInfo ──────────────────────────────────────────────────────

/// Returned from `chat_send` so the caller can wire up the
/// streaming relay with its own event emitter.
pub struct ChatStreamInfo {
    pub session_key: String,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
    pub has_context: bool,
    /// True when the session was just created (not a follow-up in an existing thread).
    pub is_new_session: bool,
    /// The user message text, forwarded to `relay_chat_stream` so it can publish
    /// `ChatTurnCompleted` after the assistant response is persisted.
    pub user_message: Option<String>,
    /// Value-identity guard for this stream so StreamGuard::drop only removes
    /// entries that still belong to us (not overwritten by a later send).
    pub guard_id: u64,
}

// ── Helper functions (private) ──────────────────────────────────────────

/// Build a human-readable summary line from a FormResponse.
fn format_interaction_summary(response: &common::FormResponse) -> String {
    match response {
        common::FormResponse::Cancelled => "Cancelled interaction".to_string(),
        common::FormResponse::Completed(answers) => {
            let parts: Vec<String> = answers
                .iter()
                .map(|a| match &a.value {
                    common::AnswerValue::Selected { value } => value.clone(),
                    common::AnswerValue::MultiSelected { values } => values.join(", "),
                    common::AnswerValue::YesNo { answer } => {
                        if *answer { "Yes" } else { "No" }.to_string()
                    }
                    common::AnswerValue::Text { content } => {
                        common::truncate_chars(content, 57, "...")
                    }
                    common::AnswerValue::Skipped => "Skipped".to_string(),
                })
                .collect();
            format!("You answered: {}", parts.join(" · "))
        }
    }
}

/// Map a tool name to its domain category.
pub fn tool_domain(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "todo" | "tasks" => Some("task"),
        "project" => Some("project"),
        "area" => Some("area"),
        "okr" => Some("objective"),
        "finance" => Some("finance"),
        _ => None,
    }
}

/// Map a tool name to the EntityKind it modifies, derived from `tool_domain`.
pub fn entity_kind_for_tool(tool_name: &str) -> Option<desktop_shared::types::EntityKind> {
    tool_domain(tool_name).and_then(desktop_shared::types::EntityKind::parse)
}

/// Returns `true` when the action is a write (create/update/delete/toggle/etc.)
/// rather than a read-only query (list/get/search).
pub fn is_mutating_action(action: Option<&str>) -> bool {
    !matches!(
        action,
        Some("list" | "get" | "search" | "search-semantic" | "search-hybrid")
    )
}

/// Map an entity_type from EntityCard to a session context entity_kind.
fn entity_kind_for(entity_type: &str) -> Option<&'static str> {
    match entity_type {
        "task" => Some("task"),
        "project" => Some("project"),
        "area" => Some("area"),
        "objective" | "key_result" => Some("objective"),
        s if s.starts_with("finance") => Some("finance"),
        _ => None,
    }
}

/// After streaming completes, infer session context from tool usage and entity
/// creation events. Only sets context when a single domain dominates.
pub async fn auto_detect_context(
    repos: &Repos,
    session_key: &str,
    tool_names: &[String],
    entity_cards: &[EntityCard],
) -> Result<(), ApiError> {
    // Determine dominant domain from tool names
    let domains: HashSet<&str> = tool_names.iter().filter_map(|n| tool_domain(n)).collect();
    if domains.len() != 1 {
        return Ok(()); // ambiguous or no tools → skip
    }
    let Some(&domain) = domains.iter().next() else {
        return Ok(());
    };

    // Pick the best entity card for this domain (first match)
    let card = entity_cards
        .iter()
        .find(|c| entity_kind_for(&c.entity_type) == Some(domain));

    let entity_id = card.map(|c| c.entity_id.as_str());

    // Resolve PARA ancestry
    let (area_id, project_id) = resolve_ancestry(repos, domain, entity_id).await;

    repos
        .session_context
        .upsert(SessionContextParams {
            session_key,
            context_type: "auto",
            entity_kind: Some(domain),
            entity_id,
            area_id: area_id.as_deref(),
            project_id: project_id.as_deref(),
            is_ephemeral: false, // non-ephemeral — auto-detected sessions are persistent
        })
        .await
        .map_err(map_storage_err)?;

    Ok(())
}

/// Look up area_id and project_id for a given entity.
pub async fn resolve_ancestry(
    repos: &Repos,
    domain: &str,
    entity_id: Option<&str>,
) -> (Option<String>, Option<String>) {
    let Some(id) = entity_id else {
        return (None, None);
    };

    match domain {
        "task" => {
            if let Ok(Some(task)) = repos.tasks.get(id).await {
                (Some(task.area_id.clone()), task.project_id.clone())
            } else {
                (None, None)
            }
        }
        "project" => {
            if let Ok(Some(proj)) = repos.projects.get(id).await {
                (Some(proj.area_id.clone()), Some(proj.id.clone()))
            } else {
                (None, None)
            }
        }
        "objective" => {
            if let Ok(Some(obj)) = repos.objectives.get(id).await {
                // Objective → project → area
                if let Ok(Some(proj)) = repos.projects.get(&obj.project_id).await {
                    (Some(proj.area_id.clone()), Some(proj.id.clone()))
                } else {
                    (None, Some(obj.project_id.clone()))
                }
            } else {
                (None, None)
            }
        }
        "area" => {
            // The entity IS the area
            (Some(id.to_string()), None)
        }
        // finance — no PARA ancestry
        _ => (None, None),
    }
}

// ── Public free functions ────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
#[tracing::instrument(skip(repos, agent, active_streams), err)]
pub async fn chat_send(
    repos: &Repos,
    agent: &Arc<agent::AgentLoop>,
    active_streams: &ActiveStreams,
    content: String,
    session_key: String,
    context: Option<SessionContextInput>,
    is_voice: bool,
    mode: Option<String>,
) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
    // 1. Ensure session exists (title derived from first message, truncated to 60 chars)
    let title: String = content
        .chars()
        .take(60)
        .collect::<String>()
        .trim()
        .to_string();
    let metadata = if is_voice {
        serde_json::json!({ "title": title, "is_voice_session": true })
    } else {
        serde_json::json!({ "title": title })
    };
    let session_mode = common::SessionMode::Assistant;
    let session_row = repos
        .sessions
        .upsert_session_with_mode(&session_key, session_mode, &metadata)
        .await
        .map_err(map_storage_err)?;
    let is_new_session = session_row.created_at == session_row.updated_at;

    // Set conversation_type based on mode if provided
    if let Some(ref m) = mode {
        repos
            .sessions
            .update_conversation_type(&session_key, m)
            .await
            .map_err(map_storage_err)?;
    }

    // 2. Upsert session_context if provided
    let has_context = context.is_some();
    if let Some(ctx) = &context {
        repos
            .session_context
            .upsert(SessionContextParams {
                session_key: &session_key,
                context_type: ctx.context_type.as_deref().unwrap_or("general"),
                entity_kind: ctx.entity_kind.as_deref(),
                entity_id: ctx.entity_id.as_deref(),
                area_id: None,    // area_id resolved later
                project_id: None, // project_id resolved later
                is_ephemeral: ctx.is_ephemeral.unwrap_or(false),
            })
            .await
            .map_err(map_storage_err)?;
    }

    // 3. Fire UserPromptSubmit hook before entering the agent loop
    if let Some(engine) = agent.runtime().hook_engine() {
        let input = UserPromptSubmitInput {
            session_id: session_key.clone(),
            prompt: content.clone(),
            base: Default::default(),
        };
        let _ = engine.fire(HookFireInput::UserPromptSubmit(input)).await;
    }

    // 4. Call agent with streaming (agent loop stores user + assistant messages)
    let msg_id = uuid::Uuid::new_v4();
    let now = jiff::Timestamp::now();
    let user_message = content.clone();
    let streaming_handle = agent
        .process_direct_streaming(content.clone(), session_key.clone(), mode.clone())
        .await
        .map_err(ApiError::from)?;

    // 5. Track the cancel token with value-identity guard
    let guard_id = next_guard_id();
    active_streams.insert(
        session_key.clone(),
        ActiveStreamEntry {
            guard_id,
            generation: 0,
            cancel: streaming_handle.cancel_token.clone(),
        },
    );

    // 6. Build the user message response
    let user_msg = ChatMessageResponse {
        id: msg_id.to_string(),
        role: common::MessageRole::User.to_string(),
        content,
        timestamp: now,
        segments: None,
        transparency: None,
    };

    // 7. Build stream info for the caller to wire up
    let stream_info = ChatStreamInfo {
        session_key,
        event_rx: streaming_handle.event_rx,
        interaction_rx: streaming_handle.interaction_rx,
        has_context,
        is_new_session,
        user_message: Some(user_message),
        guard_id,
    };

    Ok((user_msg, stream_info))
}

#[tracing::instrument(skip(active_streams, pending_interactions), err)]
pub async fn chat_cancel(
    active_streams: &ActiveStreams,
    pending_interactions: &PendingInteractions,
    session_key: String,
) -> Result<(), ApiError> {
    // Cancel stream
    if let Some((_, entry)) = active_streams.remove(&session_key) {
        entry.cancel.cancel();
    }
    // Cancel any pending interaction
    if let Some((_, (_, tx))) = pending_interactions.remove(&session_key) {
        let _ = tx.send(common::FormResponse::Cancelled);
    }
    Ok(())
}

#[tracing::instrument(skip(repos, pending_interactions, emitter), err)]
pub async fn chat_respond_interaction(
    repos: &Repos,
    pending_interactions: &PendingInteractions,
    emitter: &dyn crate::events::AppEventEmitter,
    session_key: String,
    request_id: String,
    response: common::FormResponse,
) -> Result<(), ApiError> {
    // 1. Find and remove the pending oneshot sender, validating request_id
    let (_, (stored_id, sender)) = pending_interactions
        .remove(&session_key)
        .ok_or_else(|| ApiError::new("NOT_FOUND", "no pending interaction for this session"))?;
    if stored_id != request_id {
        return Err(ApiError::new(
            "INVALID_PARAMS",
            format!("request_id mismatch: expected {stored_id}, got {request_id}"),
        ));
    }

    // 2. Build collapsed summary text for the interaction message
    let summary = format_interaction_summary(&response);

    // 3. Send the response through the oneshot to unblock the agent tool immediately
    let _ = sender.send(response);

    // 4. Persist as a synthetic "interaction" message (agent already unblocked)
    let msg_id = uuid::Uuid::new_v4();
    let metadata = serde_json::json!({
        "type": "interaction_response",
        "requestId": request_id,
    });
    if let Err(e) = repos
        .sessions
        .add_message(
            &session_key,
            msg_id,
            "interaction",
            &summary,
            None,
            None,
            Some(&metadata),
        )
        .await
    {
        tracing::warn!("failed to persist interaction message for {session_key}: {e}");
    }

    if let Ok(payload) = serde_json::to_value(events::ChatMessagePayload {
        session_key,
        source: "chat".to_string(),
    }) {
        emitter.emit_event(events::CHAT_MESSAGE_ADDED, payload);
    }

    Ok(())
}

// ── Streaming relay ─────────────────────────────────────────────────────

// ── AppCore convenience methods ─────────────────────────────────────────

impl AppCore {
    #[tracing::instrument(skip(self, content), err)]
    pub async fn chat_send(
        &self,
        content: String,
        session_key: String,
        context: Option<SessionContextInput>,
        mode: Option<String>,
    ) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
        // Reject double-send while a stream is already active for this session.
        if let Some(entry) = self.active_streams.get(&session_key) {
            if !entry.cancel.is_cancelled() {
                return Err(ApiError::from(
                    common::KlyntbotError::SessionAlreadyStreaming(session_key.clone()),
                ));
            }
        }

        let result = chat_send(
            &self.repos,
            &self.agent,
            &self.active_streams,
            content,
            session_key.clone(),
            context,
            false,
            mode,
        )
        .await?;

        self.event_emitter
            .emit_chat_thread(result.1.is_new_session, &result.1.session_key);

        Ok(result)
    }

    /// Voice-specific chat_send: no session context, `is_voice` flag set to true
    /// so the session metadata includes `"is_voice_session": true`.
    #[tracing::instrument(skip(self, content), err)]
    pub async fn chat_send_voice(
        &self,
        content: String,
        session_key: String,
    ) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
        let result = chat_send(
            &self.repos,
            &self.agent,
            &self.active_streams,
            content,
            session_key.clone(),
            None,
            true,
            None,
        )
        .await?;

        self.event_emitter
            .emit_chat_thread(result.1.is_new_session, &result.1.session_key);

        Ok(result)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn chat_cancel(&self, session_key: String) -> Result<(), ApiError> {
        // Route subagent-mode sessions to SubagentRuntime::kill.
        let session = self
            .repos
            .sessions
            .get_session(&session_key)
            .await
            .map_err(|e| ApiError::from(common::KlyntbotError::from(e)))?;
        if session.session_mode() == common::SessionMode::Subagent {
            if let Some(ref sm) = self.agent.subagent_manager() {
                if let Some(row) = self
                    .repos
                    .subagent_instances
                    .get_by_session(&session_key)
                    .await
                    .map_err(|e| ApiError::from(common::KlyntbotError::from(e)))?
                {
                    sm.runtime
                        .kill(&row.agent_id)
                        .await
                        .map_err(|e| ApiError::from(common::KlyntbotError::from(e)))?;
                    return Ok(());
                }
            }
        }
        chat_cancel(
            &self.active_streams,
            &self.pending_interactions,
            session_key,
        )
        .await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn chat_respond_interaction(
        &self,
        session_key: String,
        request_id: String,
        response: common::FormResponse,
    ) -> Result<(), ApiError> {
        chat_respond_interaction(
            &self.repos,
            &self.pending_interactions,
            self.event_emitter.as_ref(),
            session_key,
            request_id,
            response,
        )
        .await
    }

    /// Spawn the streaming relay as a background task with the given emitter.
    #[tracing::instrument(skip(self, stream_info, emitter))]
    pub fn spawn_chat_relay(
        &self,
        stream_info: ChatStreamInfo,
        emitter: Arc<dyn crate::events::AppEventEmitter>,
    ) {
        let repos = self.repos.clone();
        let active_streams = Arc::clone(&self.active_streams);
        let pending_interactions = Arc::clone(&self.pending_interactions);
        let journey_tracker = self.journey_tracker();
        let domain_event_bus = self.domain_event_bus().ok();
        let _hook_engine = self.agent.runtime().hook_engine();

        let relay = super::relay::ChatRelay {
            repos,
            session_key: stream_info.session_key,
            active_streams,
            pending_interactions,
            event_rx: stream_info.event_rx,
            interaction_rx: stream_info.interaction_rx,
            emitter,
            has_context: stream_info.has_context,
            journey_tracker,
            domain_event_bus,
            user_message: stream_info.user_message,
            guard_id: stream_info.guard_id,
        };
        tokio::spawn(relay.run().in_current_span());
    }

    pub fn active_streams_len(&self) -> usize {
        self.active_streams.len()
    }

    /// Detect zombie sessions: sessions whose last message is from the user
    /// and whose updated_at is older than the threshold.
    pub async fn detect_zombie_sessions(
        &self,
        threshold_ms: i64,
    ) -> Result<Vec<storage::SessionRow>, ApiError> {
        self.repos
            .sessions
            .detect_zombie_sessions(threshold_ms)
            .await
            .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))
    }

    /// Force-reset a stuck session: clear active_streams entry and emit a
    /// synthetic Terminal event so the frontend can recover.
    pub async fn chat_force_reset(&self, session_key: String) -> Result<(), ApiError> {
        // Remove from active_streams if present.
        if let Some((_, entry)) = self.active_streams.remove(&session_key) {
            entry.cancel.cancel();
        }
        self.pending_interactions.remove(&session_key);

        // Emit synthetic terminal event so the FE unwinds its spinner.
        let terminal = desktop_shared::thread_event_v2::ThreadEvent::Terminal {
            generation: 0, // FE ignores generation on terminal events for reset
            session_key: session_key.clone(),
            kind: desktop_shared::thread_event_v2::TerminalKind::Error {
                message: "Session force-reset by user".to_string(),
            },
            transparency: None,
        };
        let payload = serde_json::to_value(&terminal)
            .map_err(|e| ApiError::new("SERIALIZE_ERROR", e.to_string()))?;

        // We don't have an emitter here — emit via the domain bus if available,
        // otherwise the caller must handle UI-level reset.
        if let Ok(bus) = self.domain_event_bus() {
            bus.publish(bus::DomainEvent::Generic {
                kind: "thread:event".to_string(),
                payload,
            });
        }

        Ok(())
    }
}
