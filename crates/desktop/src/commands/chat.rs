//! Chat commands — wired to AgentLoop with streaming events.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use agent::AgentEvent;
use common::EntityCard;
use desktop_shared::commands::{ChatMessageResponse, ChatThreadResponse, SessionContextInput};
use desktop_shared::errors::ApiError;
use desktop_shared::events::{self, *};
use storage::{ProjectFilter, Repos};
use tauri::{Emitter, State};
use tokio_util::sync::CancellationToken;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn chat_threads(state: State<'_, AppCore>) -> Result<Vec<ChatThreadResponse>, ApiError> {
    // Fetch sessions, contexts, and name lookups concurrently (4 queries total)
    let default_filter = ProjectFilter::default();
    let (sessions, visible_contexts, all_areas, all_projects) = tokio::join!(
        state.repos.sessions.list_sessions(),
        state.repos.session_context.list_visible(),
        state.repos.areas.list(None),
        state.repos.projects.list(&default_filter),
    );
    let sessions = sessions.map_err(super::map_storage_err)?;
    let visible_contexts = visible_contexts.map_err(super::map_storage_err)?;
    let all_areas = all_areas.map_err(super::map_storage_err)?;
    let all_projects = all_projects.map_err(super::map_storage_err)?;

    // Build context lookup by session_key
    let ctx_map: HashMap<&str, _> = visible_contexts
        .iter()
        .map(|c| (c.session_key.as_str(), c))
        .collect();

    // Build name lookup maps from full lists
    let area_names: HashMap<&str, &str> = all_areas
        .iter()
        .map(|a| (a.id.as_str(), a.name.as_str()))
        .collect();
    let project_names: HashMap<&str, &str> = all_projects
        .iter()
        .map(|p| (p.id.as_str(), p.name.as_str()))
        .collect();

    Ok(sessions
        .iter()
        .map(|s| {
            let title = s
                .metadata
                .get("title")
                .and_then(|v| v.as_str())
                .unwrap_or(&s.key)
                .to_string();
            let ctx = ctx_map.get(s.key.as_str());

            ChatThreadResponse {
                session_key: s.key.clone(),
                title,
                message_count: s.message_count,
                updated_at: s.updated_at,
                context_type: ctx.map(|c| c.context_type.clone()),
                entity_kind: ctx.and_then(|c| c.entity_kind.clone()),
                entity_id: ctx.and_then(|c| c.entity_id.clone()),
                area_id: ctx.and_then(|c| c.area_id.clone()),
                area_name: ctx.and_then(|c| {
                    c.area_id
                        .as_deref()
                        .and_then(|id| area_names.get(id).map(|s| s.to_string()))
                }),
                project_id: ctx.and_then(|c| c.project_id.clone()).or_else(|| {
                    s.metadata
                        .get("projectId")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                }),
                project_name: ctx.and_then(|c| {
                    c.project_id
                        .as_deref()
                        .and_then(|id| project_names.get(id).map(|s| s.to_string()))
                }),
            }
        })
        .collect())
}

#[tauri::command]
pub async fn chat_messages(
    state: State<'_, AppCore>,
    session_key: String,
    limit: Option<i64>,
) -> Result<Vec<ChatMessageResponse>, ApiError> {
    let rows = if let Some(lim) = limit {
        state
            .repos
            .sessions
            .get_recent_messages(&session_key, lim)
            .await
    } else {
        state.repos.sessions.get_messages(&session_key).await
    }
    .map_err(super::map_storage_err)?;

    Ok(rows
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant" || m.role == "interaction")
        .map(|m| {
            let segments: Option<Vec<events::MessageSegment>> = m
                .metadata
                .as_ref()
                .and_then(|meta| meta.get("segments"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let transparency: Option<events::TransparencyData> = m
                .metadata
                .as_ref()
                .and_then(|meta| meta.get("transparency"))
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            ChatMessageResponse {
                id: m.id.to_string(),
                role: m.role.clone(),
                content: m.content.clone(),
                timestamp: m.timestamp,
                segments,
                transparency,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn chat_send(
    app: tauri::AppHandle,
    state: State<'_, AppCore>,
    content: String,
    session_key: String,
    context: Option<SessionContextInput>,
) -> Result<ChatMessageResponse, ApiError> {
    // 1. Ensure session exists (title derived from first message, truncated to 60 chars)
    let title: String = content
        .chars()
        .take(60)
        .collect::<String>()
        .trim()
        .to_string();
    let metadata = serde_json::json!({ "title": title });
    state
        .repos
        .sessions
        .upsert_session(&session_key, &metadata)
        .await
        .map_err(super::map_storage_err)?;

    // 2. Upsert session_context if provided
    if let Some(ctx) = &context {
        state
            .repos
            .session_context
            .upsert(
                &session_key,
                ctx.context_type.as_deref().unwrap_or("general"),
                ctx.entity_kind.as_deref(),
                ctx.entity_id.as_deref(),
                None, // area_id resolved later
                None, // project_id resolved later
                ctx.is_ephemeral.unwrap_or(false),
            )
            .await
            .map_err(super::map_storage_err)?;
    }

    // 3. Call agent with streaming (agent loop stores user + assistant messages)
    let msg_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();
    let streaming_handle = state
        .agent
        .process_direct_streaming(content.clone(), session_key.clone())
        .await
        .map_err(ApiError::from)?;

    // 5. Track the cancel token
    state
        .active_streams
        .insert(session_key.clone(), streaming_handle.cancel_token);

    // 6. Spawn background task to relay AgentEvents → Tauri events
    let sk = session_key.clone();
    let active_streams = Arc::clone(&state.active_streams);
    let pending_interactions = Arc::clone(&state.pending_interactions);
    let repos = state.repos.clone();
    let has_context = context.is_some();
    let mut event_rx = streaming_handle.event_rx;
    let mut interaction_rx = streaming_handle.interaction_rx;

    tokio::spawn(async move {
        // Guard ensures active_streams cleanup even on panic
        struct StreamGuard {
            key: String,
            streams: Arc<dashmap::DashMap<String, CancellationToken>>,
        }
        impl Drop for StreamGuard {
            fn drop(&mut self) {
                self.streams.remove(&self.key);
            }
        }
        let _guard = StreamGuard {
            key: sk.clone(),
            streams: Arc::clone(&active_streams),
        };

        // Collect signals for auto-detection
        let mut tool_names: Vec<String> = Vec::with_capacity(4);
        let mut entity_cards: Vec<common::EntityCard> = Vec::new();

        // Pending action names: ToolStart stashes the action extracted from args,
        // ToolEnd pops it to build the qualified name (e.g. "task:list").
        let mut pending_actions: HashMap<String, VecDeque<String>> = HashMap::new();
        let mut tool_token_sum: u32 = 0;

        // Segment accumulation for structured message persistence
        let mut segments: Vec<events::MessageSegment> = Vec::with_capacity(8);
        let mut current_text = String::new();

        // Transparency data accumulation
        let mut transparency = desktop_shared::events::TransparencyData::default();

        // Flush accumulated text into a finalized text segment.
        let flush_text = |text: &mut String, segs: &mut Vec<events::MessageSegment>| {
            if !text.is_empty() {
                segs.push(events::MessageSegment::Text {
                    content: std::mem::take(text),
                });
            }
        };

        loop {
            tokio::select! {
                biased;
                Some(bundle) = interaction_rx.recv() => {
                    let request_id = uuid::Uuid::new_v4().to_string();
                    let _ = app.emit(
                        AGENT_INTERACTION_REQUEST,
                        InteractionRequestPayload {
                            session_key: sk.clone(),
                            request_id: request_id.clone(),
                            request: bundle.request,
                        },
                    );
                    pending_interactions.insert(sk.clone(), (request_id, bundle.response_tx));
                }
                Some(event) = event_rx.recv() => {
                    match event {
                        AgentEvent::ContentChunk { data } => {
                            current_text.push_str(&data);
                            let _ = app.emit(
                                AGENT_CONTENT_CHUNK,
                                ContentChunkPayload {
                                    session_key: sk.clone(),
                                    data,
                                },
                            );
                        }
                        AgentEvent::ToolStart { name, args } => {
                            flush_text(&mut current_text, &mut segments);
                            tool_names.push(name.clone());
                            let action = args.get("action").and_then(|v| v.as_str()).map(String::from);
                            if let Some(ref a) = action {
                                pending_actions.entry(name.clone()).or_default().push_back(a.clone());
                            }
                            let _ = app.emit(
                                AGENT_TOOL_START,
                                ToolStartPayload {
                                    session_key: sk.clone(),
                                    name,
                                    action,
                                },
                            );
                        }
                        AgentEvent::ToolEnd { name, success, duration_ms, result } => {
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
                            });
                            transparency.tools.push(desktop_shared::events::TransparencyTool {
                                name: name.clone(),
                                action: action.clone(),
                                success,
                                duration_ms,
                                estimated_tokens,
                            });
                            // Emit entity:updated so the UI refreshes affected lists
                            // (e.g. tasks page updates after a status change via chat).
                            // Skip read-only actions to avoid needless refetches.
                            if success && is_mutating_action(action.as_deref()) {
                                if let Some(kind) = entity_kind_for_tool(&name) {
                                    super::emit_entity_updated(&app, kind, "");
                                }
                            }
                            let _ = app.emit(
                                AGENT_TOOL_END,
                                ToolEndPayload {
                                    session_key: sk.clone(),
                                    name,
                                    action,
                                    success,
                                    duration_ms,
                                    result,
                                    estimated_tokens,
                                },
                            );
                        }
                        AgentEvent::EntityCreated(card) => {
                            let _ = app.emit(
                                AGENT_ENTITY_CREATED,
                                EntityCreatedPayload {
                                    session_key: sk.clone(),
                                    entity_type: card.entity_type.clone(),
                                    entity_id: card.entity_id.clone(),
                                },
                            );
                            if let Some(kind) = desktop_shared::types::EntityKind::parse(&card.entity_type)
                            {
                                super::emit_entity_updated(&app, kind, &card.entity_id);
                            }
                            entity_cards.push(card);
                        }
                        AgentEvent::Done { content } => {
                            flush_text(&mut current_text, &mut segments);
                            if tool_token_sum > 0 {
                                transparency.tool_tokens_total = Some(tool_token_sum);
                            }
                            // Persist segments + transparency to the assistant message metadata
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
                            match repos.sessions.update_last_assistant_metadata(
                                &sk, None, Some(&meta_value),
                            ).await {
                                Ok(true) => {}
                                Ok(false) => {
                                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                    match repos.sessions.update_last_assistant_metadata(
                                        &sk, None, Some(&meta_value),
                                    ).await {
                                        Ok(false) => tracing::warn!("metadata persist: no assistant message found for {sk}"),
                                        Err(e) => tracing::warn!("metadata persist retry failed for {sk}: {e}"),
                                        _ => {}
                                    }
                                }
                                Err(e) => tracing::warn!("metadata persist failed for {sk}: {e}"),
                            }
                            let _ = app.emit(
                                AGENT_DONE,
                                DonePayload {
                                    session_key: sk.clone(),
                                    content,
                                },
                            );
                            break;
                        }
                        AgentEvent::Error { message } => {
                            let _ = app.emit(
                                AGENT_ERROR,
                                AgentErrorPayload {
                                    session_key: sk.clone(),
                                    message,
                                },
                            );
                            break;
                        }
                        AgentEvent::ClassificationComplete { strategy, confidence, source, duration_ms } => {
                            transparency.classification = Some(desktop_shared::events::TransparencyClassification {
                                strategy: strategy.clone(),
                                confidence,
                                source: source.clone(),
                            });
                            transparency.timing.get_or_insert_with(Default::default).classification_ms = Some(duration_ms);
                            let _ = app.emit(
                                AGENT_CLASSIFICATION_COMPLETE,
                                ClassificationCompletePayload {
                                    session_key: sk.clone(),
                                    strategy,
                                    confidence,
                                    source,
                                },
                            );
                        }
                        AgentEvent::ExecutionStarted { engine, max_iterations } => {
                            transparency.execution = Some(desktop_shared::events::TransparencyExecution {
                                engine: engine.clone(),
                                iterations: 0,
                                max_iterations: max_iterations as u32,
                                escalations: 0,
                            });
                            let _ = app.emit(
                                AGENT_EXECUTION_STARTED,
                                ExecutionStartedPayload {
                                    session_key: sk.clone(),
                                    engine,
                                    max_iterations,
                                },
                            );
                        }
                        AgentEvent::ContextAssembled { duration_ms, .. } => {
                            transparency.timing.get_or_insert_with(Default::default).context_assembly_ms = Some(duration_ms);
                        }
                        AgentEvent::IterationStart { iteration, max } => {
                            if let Some(ref mut exec) = transparency.execution {
                                exec.iterations = iteration as u32;
                            }
                            let _ = app.emit(
                                events::AGENT_ITERATION_START,
                                events::IterationStartPayload {
                                    session_key: sk.clone(),
                                    iteration,
                                    max_iterations: max,
                                },
                            );
                        }
                        AgentEvent::ConfidenceAssessed { score, action } => {
                            let _ = app.emit(
                                events::AGENT_CONFIDENCE_ASSESSED,
                                events::ConfidenceAssessedPayload {
                                    session_key: sk.clone(),
                                    score,
                                    action,
                                },
                            );
                        }
                        AgentEvent::UsageReport {
                            prompt_tokens, completion_tokens,
                            cache_read_tokens, cache_write_tokens,
                            estimated_cost_usd, model, response_time_ms,
                        } => {
                            transparency.usage = Some(desktop_shared::events::TransparencyUsage {
                                prompt_tokens,
                                completion_tokens,
                                cache_read_tokens,
                                cache_write_tokens,
                            });
                            transparency.cost = Some(desktop_shared::events::TransparencyCost {
                                estimated_usd: estimated_cost_usd,
                                model: model.clone(),
                            });
                            transparency.timing.get_or_insert_with(Default::default).total_ms = response_time_ms;
                            let _ = app.emit(
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
                                },
                            );
                        }
                        AgentEvent::MemoryAccess { action, query, results_count } => {
                            transparency.memory_accesses.push(desktop_shared::events::TransparencyMemoryAccess {
                                action: action.clone(),
                                query: query.clone(),
                                results_count,
                            });
                            let _ = app.emit(
                                events::AGENT_MEMORY_ACCESS,
                                events::MemoryAccessPayload {
                                    session_key: sk.clone(),
                                    action,
                                    query,
                                    results_count,
                                },
                            );
                        }
                        AgentEvent::SkillLoaded { name, trigger } => {
                            transparency.skills.push(desktop_shared::events::TransparencySkill {
                                name: name.clone(),
                                trigger: trigger.clone(),
                            });
                            let _ = app.emit(
                                events::AGENT_SKILL_LOADED,
                                events::SkillLoadedPayload {
                                    session_key: sk.clone(),
                                    name,
                                    trigger,
                                },
                            );
                        }
                        AgentEvent::LearningEvent { event_type, detail } => {
                            transparency.learning.push(desktop_shared::events::TransparencyLearning {
                                event_type: event_type.clone(),
                                detail: detail.clone(),
                            });
                            let _ = app.emit(
                                events::AGENT_LEARNING_EVENT,
                                events::LearningEventPayload {
                                    session_key: sk.clone(),
                                    event_type,
                                    detail,
                                },
                            );
                        }
                        AgentEvent::SubagentSpawned { label, profile } => {
                            transparency.subagents.push(desktop_shared::events::TransparencySubagent {
                                label: label.clone(),
                                profile: profile.clone(),
                            });
                            let _ = app.emit(
                                events::AGENT_SUBAGENT_SPAWNED,
                                events::SubagentSpawnedPayload {
                                    session_key: sk.clone(),
                                    label,
                                    profile,
                                },
                            );
                        }
                        AgentEvent::McpServerStatus { server_name, status, tool_count, error } => {
                            let _ = app.emit(
                                events::MCP_SERVER_STATUS,
                                events::McpServerStatusPayload {
                                    server_name,
                                    status,
                                    tool_count,
                                    error,
                                },
                            );
                        }
                        AgentEvent::McpStartupComplete { ready, failed, skipped } => {
                            let _ = app.emit(
                                events::MCP_STARTUP_COMPLETE,
                                events::McpStartupCompletePayload {
                                    ready,
                                    failed,
                                    skipped,
                                },
                            );
                        }
                    }
                }
                else => break,
            }
        }

        // Auto-detect session context from tool usage (only if not already set)
        if !has_context {
            if let Err(e) = auto_detect_context(&repos, &sk, &tool_names, &entity_cards).await {
                tracing::debug!("auto-detect context skipped for {sk}: {e}");
            }
        }
    });

    // 7. Return user message immediately (streaming handles the AI response)
    Ok(ChatMessageResponse {
        id: msg_id.to_string(),
        role: common::MessageRole::User.to_string(),
        content,
        timestamp: now,
        segments: None,
        transparency: None,
    })
}

#[tauri::command]
pub async fn chat_pin_thread(
    state: State<'_, AppCore>,
    session_key: String,
) -> Result<(), ApiError> {
    state
        .repos
        .session_context
        .pin(&session_key)
        .await
        .map_err(super::map_storage_err)?;
    Ok(())
}

#[tauri::command]
pub async fn chat_rename_thread(
    state: State<'_, AppCore>,
    session_key: String,
    title: String,
) -> Result<(), ApiError> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::new("INVALID_PARAMS", "title cannot be empty"));
    }
    state
        .repos
        .sessions
        .rename_session(&session_key, &title)
        .await
        .map_err(super::map_storage_err)?;
    Ok(())
}

#[tauri::command]
pub async fn chat_delete_thread(
    state: State<'_, AppCore>,
    session_key: String,
) -> Result<(), ApiError> {
    // Cancel any in-flight stream before deleting to avoid dangling writes
    if let Some((_, token)) = state.active_streams.remove(&session_key) {
        token.cancel();
    }
    // Cancel any pending interaction to avoid leaking the oneshot
    if let Some((_, (_, tx))) = state.pending_interactions.remove(&session_key) {
        let _ = tx.send(common::FormResponse::Cancelled);
    }
    state
        .repos
        .sessions
        .delete_session(&session_key)
        .await
        .map_err(super::map_storage_err)?;
    Ok(())
}

#[tauri::command]
pub async fn chat_respond_interaction(
    state: State<'_, AppCore>,
    session_key: String,
    request_id: String,
    response: common::FormResponse,
) -> Result<(), ApiError> {
    // 1. Find and remove the pending oneshot sender, validating request_id
    let (_, (stored_id, sender)) = state
        .pending_interactions
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
    if let Err(e) = state
        .repos
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

    Ok(())
}

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
                        common::utils::truncate_chars(content, 57, "...")
                    }
                    common::AnswerValue::Skipped => "Skipped".to_string(),
                })
                .collect();
            format!("You answered: {}", parts.join(" · "))
        }
    }
}

#[tauri::command]
pub async fn chat_cancel(state: State<'_, AppCore>, session_key: String) -> Result<(), ApiError> {
    // Cancel stream
    if let Some((_, token)) = state.active_streams.remove(&session_key) {
        token.cancel();
    }
    // Cancel any pending interaction
    if let Some((_, (_, tx))) = state.pending_interactions.remove(&session_key) {
        let _ = tx.send(common::FormResponse::Cancelled);
    }
    Ok(())
}

// ── Entity-update helpers ────────────────────────────────────────────────

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

// ── Auto-detection helpers ──────────────────────────────────────────────

/// Map a tool name to its domain category.
fn tool_domain(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "todo" => Some("task"),
        "project" => Some("project"),
        "area" => Some("area"),
        "okr" => Some("objective"),
        "finance" => Some("finance"),
        "calendar" => Some("calendar"),
        _ => None,
    }
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
    // Skip if session already has context
    if let Ok(Some(_)) = repos.session_context.get(session_key).await {
        return Ok(());
    }

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
        .upsert(
            session_key,
            "auto",
            Some(domain),
            entity_id,
            area_id.as_deref(),
            project_id.as_deref(),
            false, // non-ephemeral — auto-detected sessions are persistent
        )
        .await
        .map_err(super::map_storage_err)?;

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
            if let Ok(Some(task)) = repos.actions.get(id).await {
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
        // finance, calendar — no PARA ancestry
        _ => (None, None),
    }
}
