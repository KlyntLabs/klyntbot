//! Streaming relay, chat send/cancel, and interaction response handlers.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use agent::AgentEvent;
use common::EntityCard;
use desktop_shared::commands::{ChatMessageResponse, SessionContextInput};
use desktop_shared::errors::ApiError;
use desktop_shared::events::{self, *};
use storage::{Repos, SessionContextParams};
use tokio::sync::mpsc;
use tracing::Instrument;

use klynt_hooks::engine::HookFireInput;
use klynt_hooks::events::{
    notification::NotificationInput, session_end::SessionEndInput,
    session_start::SessionStartInput, user_prompt_submit::UserPromptSubmitInput,
};

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
fn tool_domain(tool_name: &str) -> Option<&'static str> {
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
fn entity_kind_for_tool(tool_name: &str) -> Option<desktop_shared::types::EntityKind> {
    tool_domain(tool_name).and_then(desktop_shared::types::EntityKind::parse)
}

/// Returns `true` when the action is a write (create/update/delete/toggle/etc.)
/// rather than a read-only query (list/get/search).
fn is_mutating_action(action: Option<&str>) -> bool {
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
async fn auto_detect_context(
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
async fn resolve_ancestry(
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
    let session_mode = match mode.as_deref() {
        Some("coding") => common::SessionMode::Coding,
        _ => common::SessionMode::Assistant,
    };
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

/// Relay agent streaming events to a transport-agnostic event emitter.
///
/// This contains the entire streaming loop extracted from the desktop
/// `chat_send` command. The caller is responsible for spawning this as a
/// background task and providing the appropriate emitter implementation.
#[tracing::instrument(skip(
    repos,
    active_streams,
    pending_interactions,
    event_rx,
    interaction_rx,
    emitter,
    journey_tracker,
    domain_event_bus,
    user_message,
    hook_engine
))]
#[allow(clippy::too_many_arguments)]
pub async fn relay_chat_stream(
    repos: Repos,
    session_key: String,
    active_streams: Arc<ActiveStreams>,
    pending_interactions: Arc<PendingInteractions>,
    mut event_rx: mpsc::Receiver<AgentEvent>,
    mut interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
    emitter: Arc<dyn crate::events::AppEventEmitter>,
    has_context: bool,
    journey_tracker: Option<crate::journey::JourneyTracker>,
    domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    user_message: Option<String>,
    hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    session_start_fired: Arc<dashmap::DashMap<String, ()>>,
    session_end_fired: Arc<dashmap::DashMap<String, ()>>,
    guard_id: u64,
) {
    // Guard ensures active_streams + pending_interactions cleanup even on panic
    struct StreamGuard {
        key: String,
        guard_id: u64,
        streams: Arc<ActiveStreams>,
        pending: Arc<PendingInteractions>,
    }
    impl Drop for StreamGuard {
        fn drop(&mut self) {
            // Value-identity removal: only delete the entry if it still belongs to us.
            // If a later send overwrote the slot, we leave the new entry alone.
            if let Some(entry) = self.streams.get(&self.key) {
                if entry.guard_id == self.guard_id {
                    drop(entry); // release the read lock before write
                    self.streams.remove(&self.key);
                }
            }
            // Same idea for pending_interactions — remove by key for now
            // (interactions don't have the same overwrite race).
            self.pending.remove(&self.key);
        }
    }
    let _guard = StreamGuard {
        key: session_key.clone(),
        guard_id,
        streams: Arc::clone(&active_streams),
        pending: Arc::clone(&pending_interactions),
    };

    let sk = &session_key;
    let generation = active_streams
        .get(sk)
        .map(|e| e.generation)
        .unwrap_or(0);

    // Collect signals for auto-detection
    let mut tool_names: Vec<String> = Vec::with_capacity(4);
    let mut entity_cards: Vec<EntityCard> = Vec::new();

    // Pending action names: ToolStart stashes the action extracted from args,
    // ToolEnd pops it to build the qualified name (e.g. "task:list").
    let mut pending_actions: HashMap<String, VecDeque<String>> = HashMap::new();
    // Pending approvals: request_id -> (tool_name, path) for enriching ApprovalResolved events.
    let mut pending_approvals: HashMap<String, (String, Option<String>)> = HashMap::new();
    let mut tool_token_sum: u32 = 0;

    // Segment accumulation for structured message persistence
    let mut segments: Vec<events::MessageSegment> = Vec::with_capacity(8);
    let mut current_text = String::new();

    // Transparency data accumulation
    let mut transparency = TransparencyData::default();

    // Flush accumulated text into a finalized text segment.
    let flush_text = |text: &mut String, segs: &mut Vec<events::MessageSegment>| {
        if !text.is_empty() {
            segs.push(events::MessageSegment::Text {
                content: std::mem::take(text),
            });
        }
    };

    // Helper to emit via the emitter (serializes payload to JSON value)
    macro_rules! emit {
        ($event:expr, $payload:expr) => {
            if let Ok(val) = serde_json::to_value(&$payload) {
                emitter.emit_event($event, val);
            }
        };
    }

    // Merge pipeline events and domain-bus agent events into a single stream
    // so that tools (e.g. BashTool) that publish via the domain bus are relayed
    // to the UI just like native pipeline events.
    // Capacity 256: bursty providers can emit ~50 events/sec at peak; 64 is too
    // small once we add token-by-token streaming + parallel tool calls.
    let (merged_tx, mut merged_rx) = mpsc::channel::<AgentEvent>(256);
    let merged_tx2 = merged_tx.clone();

    tokio::spawn(
        async move {
            while let Some(evt) = event_rx.recv().await {
                if merged_tx.send(evt).await.is_err() {
                    break;
                }
            }
        }
        .in_current_span(),
    );

    if let Some(ref bus) = domain_event_bus {
        let mut rx = bus.subscribe();
        tokio::spawn(
            async move {
                while let Ok(evt) = rx.recv().await {
                    if let bus::DomainEvent::Generic { kind, payload } = evt {
                        if kind == "agent_event" {
                            if let Ok(agent_evt) = serde_json::from_value::<AgentEvent>(payload) {
                                if merged_tx2.send(agent_evt).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            .in_current_span(),
        );
    }

    // Heartbeat: emit every 30s so the frontend knows the turn is still alive.
    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut interaction_closed = false;
    loop {
        tokio::select! {
            biased;
            _ = heartbeat.tick() => {
                let hb = desktop_shared::thread_event_v2::ThreadEvent::Heartbeat {
                    generation,
                    session_key: sk.clone(),
                    server_time: jiff::Timestamp::now().as_millisecond(),
                };
                if let Ok(val) = serde_json::to_value(&hb) {
                    emitter.emit_event("thread:event", val);
                }
            }
            bundle = interaction_rx.recv(), if !interaction_closed => {
                match bundle {
                    Some(bundle) => {
                        let request_id = uuid::Uuid::new_v4().to_string();
                        emit!(
                            AGENT_INTERACTION_REQUEST,
                            InteractionRequestPayload {
                                session_key: sk.clone(),
                                request_id: request_id.clone(),
                                request: bundle.request,
                            }
                        );
                        pending_interactions.insert(sk.clone(), (request_id, bundle.response_tx));
                    }
                    None => {
                        // Agent task is done but merged_rx may still have buffered
                        // terminal events (Done, Error). Disable this branch so
                        // select! falls through to merged_rx until it also closes.
                        interaction_closed = true;
                    }
                }
            }
            event = merged_rx.recv() => {
                match event {
                    Some(event) => {
                        // Emit v2 event (parallel with v1 during migration window)
                        if let Some(te) = super::thread_event_v2_translator::agent_event_to_thread_event(
                            event.clone(), sk.clone(), generation
                        ) {
                            if let Ok(val) = serde_json::to_value(&te) {
                                emitter.emit_event("thread:event", val);
                            }
                        }
                        match event {
                        AgentEvent::ContentChunk { data } => {
                            current_text.push_str(&data);
                            emit!(
                                AGENT_CONTENT_CHUNK,
                                ContentChunkPayload {
                                    session_key: sk.clone(),
                                    data,
                                }
                            );
                        }
                        AgentEvent::ToolStart { name, args, agent, .. } => {
                            flush_text(&mut current_text, &mut segments);
                            tool_names.push(name.clone());
                            let action = args.get("action").and_then(|v| v.as_str()).map(String::from);
                            if let Some(ref a) = action {
                                pending_actions.entry(name.clone()).or_default().push_back(a.clone());
                            }
                            emit!(
                                AGENT_TOOL_START,
                                ToolStartPayload {
                                    session_key: sk.clone(),
                                    name,
                                    action,
                                    agent,
                                }
                            );
                        }
                        AgentEvent::ToolEnd { name, success, duration_ms, result, agent, .. } => {
                            // Pop the stashed action from ToolStart (FIFO per tool name)
                            let action = match pending_actions.entry(name.clone()) {
                                std::collections::hash_map::Entry::Occupied(mut e) => {
                                    let a = e.get_mut().pop_front();
                                    if e.get().is_empty() { e.remove(); }
                                    a
                                }
                                std::collections::hash_map::Entry::Vacant(_) => None,
                            };
                            // Estimate tokens from result length (~4 chars per token)
                            let estimated_tokens = result.as_ref().map(|r| (r.len() as u32).saturating_add(3) / 4);
                            if let Some(t) = estimated_tokens { tool_token_sum += t; }
                            segments.push(events::MessageSegment::Tool {
                                name: name.clone(),
                                action: action.clone(),
                                success,
                                duration_ms,
                                result: result.clone(),
                                estimated_tokens,
                                agent: agent.clone(),
                            });
                            transparency.tools.push(TransparencyTool {
                                name: name.clone(),
                                action: action.clone(),
                                success,
                                duration_ms,
                                estimated_tokens,
                                agent: agent.clone(),
                            });
                            // Emit entity:updated so the UI refreshes affected lists
                            if success && is_mutating_action(action.as_deref()) {
                                if let Some(kind) = entity_kind_for_tool(&name) {
                                    let payload = serde_json::json!({
                                        "entityKind": kind,
                                        "id": ""
                                    });
                                    emitter.emit_event(ENTITY_UPDATED, payload);
                                }
                            }
                            emit!(
                                AGENT_TOOL_END,
                                ToolEndPayload {
                                    session_key: sk.clone(),
                                    name,
                                    action,
                                    success,
                                    duration_ms,
                                    result,
                                    estimated_tokens,
                                    agent,
                                }
                            );
                        }
                        AgentEvent::EntityCreated(card) => {
                            emit!(
                                AGENT_ENTITY_CREATED,
                                EntityCreatedPayload {
                                    session_key: sk.clone(),
                                    entity_type: card.entity_type.clone(),
                                    entity_id: card.entity_id.clone(),
                                }
                            );
                            if let Some(kind) = desktop_shared::types::EntityKind::parse(&card.entity_type)
                            {
                                let payload = serde_json::json!({
                                    "entityKind": kind,
                                    "id": card.entity_id
                                });
                                emitter.emit_event(ENTITY_UPDATED, payload);
                            }
                            entity_cards.push(card);
                        }
                        AgentEvent::Done { content, message_id } => {
                            flush_text(&mut current_text, &mut segments);
                            if tool_token_sum > 0 {
                                transparency.tool_tokens_total = Some(tool_token_sum);
                            }
                            // Persist segments + transparency to the assistant message metadata.
                            // Use targeted update by message ID when available to avoid
                            // overwriting the wrong message in multi-turn conversations.
                            let mut meta = serde_json::Map::new();
                            if !segments.is_empty() {
                                meta.insert(
                                    "segments".to_string(),
                                    serde_json::to_value(&segments).unwrap_or_default(),
                                );
                            }
                            meta.insert(
                                "transparency".to_string(),
                                serde_json::to_value(&transparency).unwrap_or_default(),
                            );
                            let meta_value = serde_json::Value::Object(meta);
                            let persist_outcome = if let Some(ref mid) = message_id {
                                repos.sessions
                                    .update_assistant_metadata_by_id(mid, None, Some(&meta_value))
                                    .await
                            } else {
                                repos.sessions
                                    .update_last_assistant_metadata(sk, None, Some(&meta_value))
                                    .await
                            };
                            if let Err(e) = &persist_outcome {
                                tracing::warn!("metadata persist sync failed for {sk}: {e}");
                            }
                            // If the call returned Ok(false) (no row), spawn a detached retry.
                            // We DO NOT block the relay on this.
                            if matches!(persist_outcome, Ok(false)) {
                                let repos_clone = repos.clone();
                                let sk_owned = sk.to_string();
                                let meta_clone = meta_value.clone();
                                tokio::spawn(
                                    async move {
                                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                        match repos_clone
                                            .sessions
                                            .update_last_assistant_metadata(&sk_owned, None, Some(&meta_clone))
                                            .await
                                        {
                                            Ok(true) => {}
                                            Ok(false) => tracing::warn!("metadata persist retry: no row {sk_owned}"),
                                            Err(e) => tracing::warn!("metadata persist retry failed {sk_owned}: {e}"),
                                        }
                                    }
                                    .in_current_span(),
                                );
                            }
                            // Publish ChatTurnCompleted AFTER response is saved to session
                            if let Some(ref bus) = domain_event_bus {
                                bus.publish(bus::DomainEvent::ChatTurnCompleted {
                                    session_key: sk.to_string(),
                                    user_message: user_message.clone(),
                                });
                            }
                            // Wire: FirstChatResponse journey milestone
                            if let Some(ref tracker) = journey_tracker {
                                if !tracker
                                    .is_complete(crate::journey::Milestone::FirstChatResponse)
                                    .await
                                {
                                    tracker
                                        .mark_complete(
                                            crate::journey::Milestone::FirstChatResponse,
                                        )
                                        .await;
                                }
                            }

                            // Eagerly remove the active_streams entry BEFORE emitting terminal
                            // events. This eliminates the race where a fast consumer sees
                            // agent:done and immediately calls chat_send before StreamGuard drops.
                            if let Some(entry) = active_streams.get(sk) {
                                if entry.guard_id == guard_id {
                                    drop(entry);
                                    active_streams.remove(sk);
                                }
                            }

                            emit!(
                                AGENT_DONE,
                                DonePayload {
                                    session_key: sk.clone(),
                                    content,
                                }
                            );
                            emit!(
                                CHAT_MESSAGE_ADDED,
                                ChatMessagePayload {
                                    session_key: sk.clone(),
                                    source: "chat".to_string(),
                                }
                            );
                            if let Some(ref engine) = hook_engine {
                                if !session_end_fired.contains_key(sk.as_str()) {
                                    session_end_fired.insert(sk.clone(), ());
                                    session_start_fired.remove(sk.as_str());
                                    let input = SessionEndInput {
                                        session_id: sk.clone(),
                                        reason: "complete".to_string(),
                                        duration_ms: 0,
                                        base: Default::default(),
                                    };
                                    let _ = engine.fire(HookFireInput::SessionEnd(input)).await;
                                    // Now that the hook has fired exactly once, drop the marker
                                    // so the next turn on this session can fire SessionStart cleanly.
                                    session_end_fired.remove(sk.as_str());
                                }
                            }
                            break;
                        }
                        AgentEvent::Error { message } => {
                            // Eager cleanup: remove active_streams entry before emitting
                            // terminal events so consumers can retry immediately.
                            if let Some(entry) = active_streams.get(sk) {
                                if entry.guard_id == guard_id {
                                    drop(entry);
                                    active_streams.remove(sk);
                                }
                            }
                            emit!(
                                AGENT_ERROR,
                                AgentErrorPayload {
                                    session_key: sk.clone(),
                                    message: message.clone(),
                                }
                            );
                            // ALSO emit chat:message_added so the FE re-reads the session.
                            // Without this, FE consumers that gate on chat:message_added to refresh
                            // history will never see the error message even though it's persisted.
                            emit!(
                                CHAT_MESSAGE_ADDED,
                                ChatMessagePayload {
                                    session_key: sk.clone(),
                                    source: "agent_error".to_string(),
                                }
                            );
                            break;
                        }
                        AgentEvent::Cancelled { partial_content, partial_reasoning } => {
                            // Eager cleanup before emitting terminal event.
                            if let Some(entry) = active_streams.get(sk) {
                                if entry.guard_id == guard_id {
                                    drop(entry);
                                    active_streams.remove(sk);
                                }
                            }
                            emit!(
                                AGENT_CANCELLED,
                                CancelledPayload {
                                    session_key: sk.clone(),
                                    partial_content,
                                    partial_reasoning,
                                }
                            );
                            // Cancellation is terminal — break so StreamGuard drops.
                            break;
                        }
                        AgentEvent::ExecutionStarted { engine, max_iterations } => {
                            transparency.execution = Some(TransparencyExecution {
                                engine: engine.clone(),
                                iterations: 0,
                                max_iterations: max_iterations as u32,
                                escalations: 0,
                            });
                            emit!(
                                AGENT_EXECUTION_STARTED,
                                ExecutionStartedPayload {
                                    session_key: sk.clone(),
                                    engine,
                                    max_iterations,
                                }
                            );
                        }
                        AgentEvent::PipelineStarted => {
                            emit!(
                                AGENT_PIPELINE_STARTED,
                                PipelineStartedPayload {
                                    session_key: sk.clone(),
                                }
                            );
                        }
                        AgentEvent::ContextAssembled { total_tokens, budget: _, duration_ms } => {
                            transparency.timing.get_or_insert_with(Default::default).context_assembly_ms = Some(duration_ms);
                            emit!(
                                AGENT_CONTEXT_ASSEMBLED,
                                ContextAssembledPayload {
                                    session_key: sk.clone(),
                                    total_tokens,
                                    duration_ms,
                                }
                            );
                        }
                        AgentEvent::RetrievalEnhanced { stages, total_latency_ms, total_llm_calls } => {
                            let stage_payloads: Vec<events::EnhancementStagePayload> = stages
                                .iter()
                                .map(|s| {
                                    let (status, detail) = s.status.to_parts();
                                    events::EnhancementStagePayload {
                                        name: s.name.to_string(),
                                        status: status.to_string(),
                                        status_detail: detail.map(String::from),
                                        latency_ms: s.latency_ms,
                                        llm_calls: s.llm_calls,
                                        output_summary: s.output_summary.clone(),
                                    }
                                })
                                .collect();
                            transparency.enhancement = Some(events::TransparencyEnhancement {
                                stages: stage_payloads.clone(),
                                total_latency_ms,
                                total_llm_calls,
                            });
                            emit!(
                                events::AGENT_RETRIEVAL_ENHANCED,
                                events::RetrievalEnhancedPayload {
                                    session_key: sk.clone(),
                                    stages: stage_payloads,
                                    total_latency_ms,
                                    total_llm_calls,
                                }
                            );
                        }
                        AgentEvent::IterationStart { iteration, max } => {
                            if let Some(ref mut exec) = transparency.execution {
                                exec.iterations = iteration as u32;
                            }
                            emit!(
                                events::AGENT_ITERATION_START,
                                events::IterationStartPayload {
                                    session_key: sk.clone(),
                                    iteration,
                                    max_iterations: max,
                                }
                            );
                        }
                        AgentEvent::ConfidenceAssessed { score, action } => {
                            emit!(
                                events::AGENT_CONFIDENCE_ASSESSED,
                                events::ConfidenceAssessedPayload {
                                    session_key: sk.clone(),
                                    score,
                                    action,
                                }
                            );
                        }
                        AgentEvent::UsageReport {
                            prompt_tokens, completion_tokens,
                            cache_read_tokens, cache_write_tokens,
                            estimated_cost_usd, model, response_time_ms,
                            ..
                        } => {
                            transparency.usage = Some(TransparencyUsage {
                                prompt_tokens,
                                completion_tokens,
                                cache_read_tokens,
                                cache_write_tokens,
                            });
                            transparency.cost = Some(TransparencyCost {
                                estimated_usd: estimated_cost_usd,
                                model: model.clone(),
                            });
                            transparency.timing.get_or_insert_with(Default::default).total_ms = response_time_ms;
                            emit!(
                                events::AGENT_USAGE_REPORT,
                                events::UsageReportPayload {
                                    session_key: sk.clone(),
                                    prompt_tokens,
                                    completion_tokens,
                                    cache_read_tokens,
                                    cache_write_tokens,
                                    estimated_cost_usd,
                                    model,
                                    response_time_ms,
                                }
                            );
                        }
                        AgentEvent::MemoryAccess { action, query, results_count } => {
                            transparency.memory_accesses.push(TransparencyMemoryAccess {
                                action: action.clone(),
                                query: query.clone(),
                                results_count,
                            });
                            emit!(
                                events::AGENT_MEMORY_ACCESS,
                                events::MemoryAccessPayload {
                                    session_key: sk.clone(),
                                    action,
                                    query,
                                    results_count,
                                }
                            );
                        }
                        AgentEvent::SkillLoaded { name, trigger, agent } => {
                            transparency.skills.push(TransparencySkill {
                                name: name.clone(),
                                trigger: trigger.clone(),
                                agent: agent.clone(),
                            });
                            emit!(
                                events::AGENT_SKILL_LOADED,
                                events::SkillLoadedPayload {
                                    session_key: sk.clone(),
                                    name,
                                    trigger,
                                    agent,
                                }
                            );
                        }
                        AgentEvent::LearningEvent { event_type, detail } => {
                            transparency.learning.push(TransparencyLearning {
                                event_type: event_type.clone(),
                                detail: detail.clone(),
                            });
                            emit!(
                                events::AGENT_LEARNING_EVENT,
                                events::LearningEventPayload {
                                    session_key: sk.clone(),
                                    event_type,
                                    detail,
                                }
                            );
                        }
                        AgentEvent::AgentSelected { name, description } => {
                            transparency.agent_selected = Some(TransparencyAgentSelected {
                                name: name.clone(),
                                description: description.clone(),
                            });
                            emit!(
                                events::AGENT_SELECTED,
                                events::AgentSelectedPayload {
                                    session_key: sk.clone(),
                                    name,
                                    description,
                                }
                            );
                        }
                        AgentEvent::SubagentSpawned { label, profile, .. } => {
                            transparency.subagents.push(TransparencySubagent {
                                label: label.clone(),
                                profile: profile.clone(),
                            });
                            emit!(
                                events::AGENT_SUBAGENT_SPAWNED,
                                events::SubagentSpawnedPayload {
                                    session_key: sk.clone(),
                                    label,
                                    profile,
                                }
                            );
                        }
                        AgentEvent::DelegationStarted { from_agent, to_agent, query, depth } => {
                            transparency.delegations.push(TransparencyDelegation {
                                from_agent: from_agent.clone(),
                                to_agent: to_agent.clone(),
                                query: query.clone(),
                                depth,
                                status: "active".to_string(),
                                duration_ms: None,
                            });
                            emit!(
                                events::AGENT_DELEGATION_STARTED,
                                events::DelegationStartedPayload {
                                    session_key: sk.clone(),
                                    from_agent,
                                    to_agent,
                                    query,
                                    depth,
                                }
                            );
                        }
                        AgentEvent::DelegationCompleted { from_agent, to_agent, success, duration_ms } => {
                            // Update the matching delegation entry
                            if let Some(d) = transparency.delegations.iter_mut().find(|d| d.to_agent == to_agent && d.status == "active") {
                                d.status = if success { "completed".to_string() } else { "failed".to_string() };
                                d.duration_ms = Some(duration_ms);
                            }
                            emit!(
                                events::AGENT_DELEGATION_COMPLETED,
                                events::DelegationCompletedPayload {
                                    session_key: sk.clone(),
                                    from_agent,
                                    to_agent,
                                    success,
                                    duration_ms,
                                }
                            );
                        }
                        AgentEvent::McpServerStatus { server_name, status, tool_count, error } => {
                            emit!(
                                events::MCP_SERVER_STATUS,
                                events::McpServerStatusPayload {
                                    server_name,
                                    status,
                                    tool_count,
                                    error,
                                }
                            );
                        }
                        AgentEvent::McpStartupComplete { ready, failed, skipped } => {
                            emit!(
                                events::MCP_STARTUP_COMPLETE,
                                events::McpStartupCompletePayload {
                                    ready,
                                    failed,
                                    skipped,
                                }
                            );
                        }
                        AgentEvent::PlanningStarted { .. } => {}
                        AgentEvent::PlanGenerated { steps, raw_plan } => {
                            transparency.plan = Some(events::TransparencyPlan {
                                steps: steps.clone(),
                                completed_steps: Vec::new(),
                            });
                            emit!(
                                events::AGENT_PLAN_GENERATED,
                                events::PlanGeneratedPayload {
                                    session_key: sk.to_string(),
                                    steps,
                                    raw_plan,
                                }
                            );
                        }
                        AgentEvent::PlanStepCompleted { step_index, description, tool_name } => {
                            if let Some(ref mut plan) = transparency.plan {
                                plan.completed_steps.push(step_index);
                            }
                            emit!(
                                events::AGENT_PLAN_STEP_COMPLETED,
                                events::PlanStepCompletedPayload {
                                    session_key: sk.to_string(),
                                    step_index,
                                    description,
                                    tool_name,
                                }
                            );
                        }
                        AgentEvent::BudgetWarning { monthly_spend_usd, monthly_budget_usd, usage_percent } => {
                            emit!(
                                events::AGENT_BUDGET_WARNING,
                                events::BudgetWarningPayload {
                                    session_key: sk.to_string(),
                                    monthly_spend_usd,
                                    monthly_budget_usd,
                                    usage_percent,
                                }
                            );
                        }

                        AgentEvent::MemoryPromoted { fact_id, from_scope, to_scope, subject, predicate } => {
                            emit!(
                                events::AGENT_MEMORY_PROMOTED,
                                events::MemoryPromotedPayload {
                                    session_key: sk.to_string(),
                                    fact_id,
                                    from_scope,
                                    to_scope,
                                    subject,
                                    predicate,
                                }
                            );
                        }
                        // AutoTuner events — forwarded to the UI for toast notifications and panel updates.
                        AgentEvent::AutoTunerReport(report) => {
                            emit!(events::AUTOTUNER_REPORT, report);
                        }
                        AgentEvent::AutoTunerPromotion(promotion) => {
                            emit!(events::AUTOTUNER_PROMOTION, promotion);
                        }
                        AgentEvent::AutoTunerRollback(rollback) => {
                            emit!(events::AUTOTUNER_ROLLBACK, rollback);
                        }
                        AgentEvent::ContextCompressed { before_tokens, after_tokens, iteration } => {
                            tracing::info!(
                                before_tokens,
                                after_tokens,
                                iteration,
                                "mid-loop context compression applied"
                            );
                        }
                        AgentEvent::ContextTieredCompressed { tier0_kept, tier1_tokens, tier2_tokens, cognitive_scoring_used, delta_only } => {
                            tracing::info!(
                                tier0_kept,
                                tier1_tokens,
                                tier2_tokens,
                                cognitive_scoring_used,
                                delta_only,
                                "tiered history compression applied"
                            );
                        }
                        AgentEvent::LoopDetected { iteration, tools_summary, suggestion } => {
                            tracing::info!(
                                iteration,
                                tools_summary = %tools_summary,
                                suggestion = %suggestion,
                                "loop detected: repeating tool pattern"
                            );
                        }
                        AgentEvent::LoopHardStop { iteration, tools_summary } => {
                            tracing::warn!(
                                iteration,
                                tools_summary = %tools_summary,
                                "loop hard-stop: forcing synthesis"
                            );
                        }
                        AgentEvent::ContextReassembled { updates, tokens_added } => {
                            tracing::info!(
                                updates_count = updates.len(),
                                tokens_added,
                                "live context reassembled during execution"
                            );
                        }
                        // Budget-bounded execution events — logged for now, UI integration later.
                        AgentEvent::BudgetUpdate { .. }
                        | AgentEvent::DepthSuggestion { .. }
                        | AgentEvent::EnrichmentStarted { .. }
                        | AgentEvent::EnrichmentComplete { .. }
                        | AgentEvent::TurnComplete { .. } => {}
                        AgentEvent::ApprovalRequested { requires_user_input, .. } if !requires_user_input => {
                            // Auto-allow / auto-deny / privacy: telemetry only — UI doesn't need them.
                        }
                        AgentEvent::ApprovalRequested { ref request_id, ref tool, ref args_hash, ref layer, ref rule_matched, ref mirror_history, ref sandbox_summary, requires_user_input, ref args, ref cwd, ref layer_reason } => {
                            let path = args.as_ref().and_then(approval::extract_path_str_from_args);
                            pending_approvals.insert(request_id.clone(), (tool.clone(), path));
                            if let Some(ref bus) = domain_event_bus {
                                bus.publish(bus::DomainEvent::ApprovalRequested {
                                    request_id: request_id.clone(),
                                    tool: tool.clone(),
                                    args_hash: args_hash.clone(),
                                    layer: layer.clone(),
                                    repo_id: None,
                                });
                            }
                            let payload = serde_json::json!({
                                "request_id": request_id,
                                "tool": tool,
                                "args_hash": args_hash,
                                "layer": layer,
                                "rule_matched": rule_matched,
                                "mirror_history": mirror_history,
                                "sandbox_summary": sandbox_summary,
                                "requires_user_input": requires_user_input,
                                "args": args,
                                "cwd": cwd,
                                "layer_reason": layer_reason,
                            });
                            emitter.emit_event("agent:approval_requested", payload);
                            if let Some(ref engine) = hook_engine {
                                let input = NotificationInput {
                                    session_id: sk.clone(),
                                    kind: "approval_card_opened".to_string(),
                                    message: format!("Approval requested for {} (layer: {})", tool, layer),
                                    tool: Some(tool.clone()),
                                    base: Default::default(),
                                };
                                let _ = engine.fire(HookFireInput::Notification(input)).await;
                            }
                        }
                        AgentEvent::ApprovalResolved { ref request_id, ref decision, ref decision_reason, ref latency_ms, ref persisted_rule, ref decided_by } => {
                            let (tool_name, path) = pending_approvals
                                .remove(request_id)

                                .unwrap_or_default();
                            if let Some(ref bus) = domain_event_bus {
                                bus.publish(bus::DomainEvent::ApprovalResolved {
                                    request_id: request_id.clone(),
                                    user_id: None,
                                    tool_name,
                                    path,
                                    decision: decision.clone(),
                                    pattern_used: persisted_rule.clone(),
                                    decided_by: decided_by.clone(),
                                    occurred_at: jiff::Timestamp::now().as_second(),
                                });
                            }
                            let payload = serde_json::json!({
                                "request_id": request_id,
                                "decision": decision,
                                "decision_reason": decision_reason,
                                "latency_ms": latency_ms,
                                "persisted_rule": persisted_rule,
                                "decided_by": decided_by,
                            });
                            emitter.emit_event("agent:approval_resolved", payload);
                        }
                        AgentEvent::SandboxPolicyApplied { ref tool, ref policy_summary, ref policy_hash, fallback_unsandboxed, ref fs_constraints, ref network_constraints } => {
                            let payload = serde_json::json!({
                                "tool": tool,
                                "policy_summary": policy_summary,
                                "policy_hash": policy_hash,
                                "fallback_unsandboxed": fallback_unsandboxed,
                                "fs_constraints": fs_constraints,
                                "network_constraints": network_constraints,
                            });
                            emitter.emit_event("agent:sandbox_policy_applied", payload);
                        }
                        AgentEvent::FileEditWithSymbols { ref path, ref op, bytes, ref diff_full, .. } => {
                            let payload = serde_json::json!({
                                "path": path,
                                "op": op,
                                "bytes": bytes,
                                "diff": diff_full,
                            });
                            emitter.emit_event("agent:file_edit_with_symbols", payload);
                        }
                        AgentEvent::RecallInjected { ref memory_ids, coverage_score, ref escalation_chain, dead_end_warning, budget_used_tokens, budget_limit_tokens } => {
                            let payload = serde_json::json!({
                                "session_key": sk.clone(),
                                "memory_ids": memory_ids,
                                "coverage_score": coverage_score,
                                "escalation_chain": escalation_chain,
                                "dead_end_warning": dead_end_warning,
                                "budget_used_tokens": budget_used_tokens,
                                "budget_limit_tokens": budget_limit_tokens,
                            });
                            emitter.emit_event("agent:recall_injected", payload);
                        }
                        AgentEvent::DeadEndWarningSurfaced { ref approach_summary, ref prior_attempt_id, confidence } => {
                            let payload = serde_json::json!({
                                "session_key": sk.clone(),
                                "approach_summary": approach_summary,
                                "prior_attempt_id": prior_attempt_id,
                                "confidence": confidence,
                            });
                            emitter.emit_event("agent:dead_end_warning_surfaced", payload);
                        }
                        AgentEvent::PlanModeChanged { ref session_key, active, ref requested_by } => {
                            let payload = serde_json::json!({
                                "session_key": session_key, "active": active, "requested_by": requested_by,
                            });
                            emitter.emit_event("agent:plan_mode_changed", payload);
                        }
                        // Telemetry / internal events — intentionally not relayed to FE.
                        AgentEvent::ReasoningChunk { .. }
                        | AgentEvent::SubagentProgress { .. }
                        | AgentEvent::SubagentCompleted { .. }
                        | AgentEvent::SubagentCancelled { .. }
                        | AgentEvent::SkillActivationConsidered { .. }
                        | AgentEvent::SkillActivated { .. }
                        | AgentEvent::SkillReferenceLoaded { .. }
                        | AgentEvent::ContextEngineDecision { .. }
                        | AgentEvent::ToolCallStreamChunk { .. }
                        | AgentEvent::MCPSubcallTrace { .. }
                        | AgentEvent::ProviderRequest { .. }
                        | AgentEvent::ProviderResponse { .. }
                        | AgentEvent::MidLoopCompressionTriggered { .. }
                        | AgentEvent::TestRunDetailed { .. }
                        | AgentEvent::PowerModeToggled { .. }
                        | AgentEvent::TurnInterrupted { .. } => {
                            tracing::debug!(event_type = ?event, "chat relay: dropped event (no v1 FE relay)");
                        }
                        // Safety net for future AgentEvent variants added without updating this match.
                        _ => {
                            tracing::warn!(event_type = ?event, "chat relay: unknown event variant dropped");
                        }
                        }
                    }
                    None => {
                        // Event stream closed — agent task finished or panicked.
                        if let Some(entry) = active_streams.get(sk) {
                            if entry.guard_id == guard_id {
                                drop(entry);
                                active_streams.remove(sk);
                            }
                        }
                        break;
                    }
                }
            }
            else => break,
        }
    }

    // Auto-detect session context from tool usage (only if not already set)
    if !has_context {
        if let Err(e) = auto_detect_context(&repos, sk, &tool_names, &entity_cards).await {
            tracing::debug!("auto-detect context skipped for {sk}: {e}");
        }
    }
}

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

        // Fire SessionStart hook once per session on first coding-mode message.
        if mode.as_deref() == Some("coding") && !self.session_start_fired.contains_key(&session_key)
        {
            if let Some(engine) = self.agent.runtime().hook_engine() {
                let input = SessionStartInput {
                    session_id: session_key.clone(),
                    cwd: std::env::current_dir()
                        .ok()
                        .and_then(|p| p.to_str().map(String::from))
                        .unwrap_or_default(),
                    base: Default::default(),
                };
                let _ = engine.fire(HookFireInput::SessionStart(input)).await;
            }
            self.session_start_fired.insert(session_key.clone(), ());
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
        if let Some(engine) = self.agent.runtime().hook_engine() {
            if !self.session_end_fired.contains_key(&session_key) {
                self.session_end_fired.insert(session_key.clone(), ());
                self.session_start_fired.remove(&session_key);
                let input = SessionEndInput {
                    session_id: session_key.clone(),
                    reason: "user_cancel".to_string(),
                    duration_ms: 0,
                    base: Default::default(),
                };
                let _ = engine.fire(HookFireInput::SessionEnd(input)).await;
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
        let journey_tracker = self.journey_tracker.clone();
        let domain_event_bus = self.domain_event_bus.clone();
        let hook_engine = self.agent.runtime().hook_engine();

        tokio::spawn(
            relay_chat_stream(
                repos,
                stream_info.session_key,
                active_streams,
                pending_interactions,
                stream_info.event_rx,
                stream_info.interaction_rx,
                emitter,
                stream_info.has_context,
                journey_tracker,
                domain_event_bus,
                stream_info.user_message,
                hook_engine,
                Arc::clone(&self.session_start_fired),
                Arc::clone(&self.session_end_fired),
                stream_info.guard_id,
            )
            .in_current_span(),
        );
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
        if let Some(ref bus) = self.domain_event_bus {
            bus.publish(bus::DomainEvent::Generic {
                kind: "thread:event".to_string(),
                payload,
            });
        }

        Ok(())
    }
}
