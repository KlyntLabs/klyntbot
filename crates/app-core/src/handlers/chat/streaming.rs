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
use tokio_util::sync::CancellationToken;

use crate::errors::map_storage_err;
use crate::state::AppCore;

/// Type aliases for the DashMap types used across both AppCore variants.
pub(super) type ActiveStreams = dashmap::DashMap<String, CancellationToken>;
pub(super) type PendingInteractions =
    dashmap::DashMap<String, (String, tokio::sync::oneshot::Sender<common::FormResponse>)>;

// ── ChatStreamInfo ──────────────────────────────────────────────────────

/// Returned from `chat_send` so the caller can wire up the
/// streaming relay with its own event emitter.
pub struct ChatStreamInfo {
    pub session_key: String,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
    pub has_context: bool,
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
            } else if let Ok(Some(action)) = repos.actions.get(id).await {
                (Some(action.area_id.clone()), action.project_id.clone())
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

pub async fn chat_send(
    repos: &Repos,
    agent: &Arc<agent::AgentLoop>,
    active_streams: &ActiveStreams,
    content: String,
    session_key: String,
    context: Option<SessionContextInput>,
) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
    // 1. Ensure session exists (title derived from first message, truncated to 60 chars)
    let title: String = content
        .chars()
        .take(60)
        .collect::<String>()
        .trim()
        .to_string();
    let metadata = serde_json::json!({ "title": title });
    let squad_id_ref = context.as_ref().and_then(|c| c.squad_id.as_deref());
    repos
        .sessions
        .upsert_session(&session_key, &metadata, squad_id_ref)
        .await
        .map_err(map_storage_err)?;

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

    // 3. Call agent with streaming (agent loop stores user + assistant messages)
    let msg_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();
    let squad_mode = context.as_ref().and_then(|c| c.squad_mode.clone());
    let streaming_handle = agent
        .process_direct_streaming(content.clone(), session_key.clone(), squad_mode)
        .await
        .map_err(ApiError::from)?;

    // 4. Track the cancel token
    active_streams.insert(session_key.clone(), streaming_handle.cancel_token);

    // 5. Build the user message response
    let user_msg = ChatMessageResponse {
        id: msg_id.to_string(),
        role: common::MessageRole::User.to_string(),
        content,
        timestamp: now,
        segments: None,
        transparency: None,
        persona_id: None,
        persona_name: None,
    };

    // 6. Build stream info for the caller to wire up
    let stream_info = ChatStreamInfo {
        session_key,
        event_rx: streaming_handle.event_rx,
        interaction_rx: streaming_handle.interaction_rx,
        has_context,
    };

    Ok((user_msg, stream_info))
}

pub async fn chat_cancel(
    active_streams: &ActiveStreams,
    pending_interactions: &PendingInteractions,
    session_key: String,
) -> Result<(), ApiError> {
    // Cancel stream
    if let Some((_, token)) = active_streams.remove(&session_key) {
        token.cancel();
    }
    // Cancel any pending interaction
    if let Some((_, (_, tx))) = pending_interactions.remove(&session_key) {
        let _ = tx.send(common::FormResponse::Cancelled);
    }
    Ok(())
}

pub async fn chat_respond_interaction(
    repos: &Repos,
    pending_interactions: &PendingInteractions,
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
            None, // persona_id
        )
        .await
    {
        tracing::warn!("failed to persist interaction message for {session_key}: {e}");
    }

    Ok(())
}

// ── Streaming relay ─────────────────────────────────────────────────────

/// Relay agent streaming events to a transport-agnostic event emitter.
///
/// This contains the entire streaming loop extracted from the desktop
/// `chat_send` command. The caller is responsible for spawning this as a
/// background task and providing the appropriate emitter implementation.
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
) {
    // Guard ensures active_streams cleanup even on panic
    struct StreamGuard {
        key: String,
        streams: Arc<ActiveStreams>,
    }
    impl Drop for StreamGuard {
        fn drop(&mut self) {
            self.streams.remove(&self.key);
        }
    }
    let _guard = StreamGuard {
        key: session_key.clone(),
        streams: Arc::clone(&active_streams),
    };

    let sk = &session_key;

    // Collect signals for auto-detection
    let mut tool_names: Vec<String> = Vec::with_capacity(4);
    let mut entity_cards: Vec<EntityCard> = Vec::new();

    // Pending action names: ToolStart stashes the action extracted from args,
    // ToolEnd pops it to build the qualified name (e.g. "task:list").
    let mut pending_actions: HashMap<String, VecDeque<String>> = HashMap::new();
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

    loop {
        tokio::select! {
            biased;
            Some(bundle) = interaction_rx.recv() => {
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
            Some(event) = event_rx.recv() => {
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
                    AgentEvent::ToolStart { name, args, agent } => {
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
                    AgentEvent::ToolEnd { name, success, duration_ms, result, agent } => {
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
                        if let Some(ref mid) = message_id {
                            // Targeted update by message ID — no race condition
                            if let Err(e) = repos.sessions.update_assistant_metadata_by_id(
                                mid, None, Some(&meta_value),
                            ).await {
                                tracing::warn!("metadata persist by id failed for {sk}/{mid}: {e}");
                            }
                        } else {
                            // Fallback: legacy path for messages without ID
                            match repos.sessions.update_last_assistant_metadata(
                                sk, None, Some(&meta_value),
                            ).await {
                                Ok(true) => {}
                                Ok(false) => {
                                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                    match repos.sessions.update_last_assistant_metadata(
                                        sk, None, Some(&meta_value),
                                    ).await {
                                        Ok(false) => tracing::warn!("metadata persist: no assistant message found for {sk}"),
                                        Err(e) => tracing::warn!("metadata persist retry failed for {sk}: {e}"),
                                        _ => {}
                                    }
                                }
                                Err(e) => tracing::warn!("metadata persist failed for {sk}: {e}"),
                            }
                        }
                        emit!(
                            AGENT_DONE,
                            DonePayload {
                                session_key: sk.clone(),
                                content,
                            }
                        );
                        break;
                    }
                    AgentEvent::Error { message } => {
                        emit!(
                            AGENT_ERROR,
                            AgentErrorPayload {
                                session_key: sk.clone(),
                                message,
                            }
                        );
                        break;
                    }
                    AgentEvent::ClassificationComplete { strategy, confidence, source, duration_ms } => {
                        transparency.classification = Some(TransparencyClassification {
                            strategy: strategy.clone(),
                            confidence,
                            source: source.clone(),
                        });
                        transparency.timing.get_or_insert_with(Default::default).classification_ms = Some(duration_ms);
                        emit!(
                            AGENT_CLASSIFICATION_COMPLETE,
                            ClassificationCompletePayload {
                                session_key: sk.clone(),
                                strategy,
                                confidence,
                                source,
                            }
                        );
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
                    AgentEvent::ContextAssembled { duration_ms, .. } => {
                        transparency.timing.get_or_insert_with(Default::default).context_assembly_ms = Some(duration_ms);
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
                    AgentEvent::SubagentSpawned { label, profile } => {
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
                    AgentEvent::PersonaPerspective { persona_id, persona_name, persona_icon, persona_role, content, challenge } => {
                        emit!(
                            events::AGENT_PERSONA_PERSPECTIVE,
                            events::PersonaPerspectivePayload {
                                session_key: sk.to_string(),
                                persona_id,
                                persona_name,
                                persona_icon,
                                persona_role,
                                content,
                                challenge,
                            }
                        );
                    }
                    AgentEvent::DebateRoundStarted { round, total_rounds, phase } => {
                        emit!(
                            events::AGENT_DEBATE_ROUND_STARTED,
                            events::DebateRoundStartedPayload {
                                session_key: sk.to_string(),
                                round,
                                total_rounds,
                                phase,
                            }
                        );
                    }
                    AgentEvent::DebateJudgeDecision { round, consensus_score, decision, speaking_order, reasoning } => {
                        emit!(
                            events::AGENT_DEBATE_JUDGE_DECISION,
                            events::DebateJudgeDecisionPayload {
                                session_key: sk.to_string(),
                                round,
                                consensus_score,
                                decision,
                                speaking_order,
                                reasoning,
                            }
                        );
                    }
                    AgentEvent::DebateRoundCompleted { round, consensus_score } => {
                        emit!(
                            events::AGENT_DEBATE_ROUND_COMPLETED,
                            events::DebateRoundCompletedPayload {
                                session_key: sk.to_string(),
                                round,
                                consensus_score,
                            }
                        );
                    }
                    AgentEvent::ConsensusReached { round, consensus_score, summary } => {
                        emit!(
                            events::AGENT_CONSENSUS_REACHED,
                            events::ConsensusReachedPayload {
                                session_key: sk.to_string(),
                                round,
                                consensus_score,
                                summary,
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
    pub async fn chat_send(
        &self,
        content: String,
        session_key: String,
        context: Option<SessionContextInput>,
    ) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
        let result = chat_send(
            &self.repos,
            &self.agent,
            &self.active_streams,
            content.clone(),
            session_key.clone(),
            context,
        )
        .await?;

        // Publish chat turn to cognitive consolidation pipeline
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                user_message: content,
                session_key,
            });
        }

        Ok(result)
    }

    pub async fn chat_cancel(&self, session_key: String) -> Result<(), ApiError> {
        chat_cancel(
            &self.active_streams,
            &self.pending_interactions,
            session_key,
        )
        .await
    }

    pub async fn chat_respond_interaction(
        &self,
        session_key: String,
        request_id: String,
        response: common::FormResponse,
    ) -> Result<(), ApiError> {
        chat_respond_interaction(
            &self.repos,
            &self.pending_interactions,
            session_key,
            request_id,
            response,
        )
        .await
    }

    /// Spawn the streaming relay as a background task with the given emitter.
    pub fn spawn_chat_relay(
        &self,
        stream_info: ChatStreamInfo,
        emitter: Arc<dyn crate::events::AppEventEmitter>,
    ) {
        let repos = self.repos.clone();
        let active_streams = Arc::clone(&self.active_streams);
        let pending_interactions = Arc::clone(&self.pending_interactions);

        tokio::spawn(relay_chat_stream(
            repos,
            stream_info.session_key,
            active_streams,
            pending_interactions,
            stream_info.event_rx,
            stream_info.interaction_rx,
            emitter,
            stream_info.has_context,
        ));
    }
}
