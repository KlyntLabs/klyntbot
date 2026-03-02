//! Chat commands — wired to AgentLoop with streaming events.

use std::collections::HashMap;
use std::sync::Arc;

use agent::AgentEvent;
use desktop_shared::commands::{ChatMessageResponse, ChatThreadResponse, SessionContextInput};
use desktop_shared::events::*;
use storage::ProjectFilter;
use tauri::{Emitter, State};
use tokio_util::sync::CancellationToken;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn chat_threads(state: State<'_, AppCore>) -> Result<Vec<ChatThreadResponse>, String> {
    // Fetch sessions, contexts, and name lookups concurrently (4 queries total)
    let default_filter = ProjectFilter::default();
    let (sessions, visible_contexts, all_areas, all_projects) = tokio::join!(
        state.repos.sessions.list_sessions(),
        state.repos.session_context.list_visible(),
        state.repos.areas.list(None),
        state.repos.projects.list(&default_filter),
    );
    let sessions = sessions.map_err(|e| e.to_string())?;
    let visible_contexts = visible_contexts.map_err(|e| e.to_string())?;
    let all_areas = all_areas.map_err(|e| e.to_string())?;
    let all_projects = all_projects.map_err(|e| e.to_string())?;

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
                    c.area_id.as_deref().and_then(|id| area_names.get(id).map(|s| s.to_string()))
                }),
                project_id: ctx
                    .and_then(|c| c.project_id.clone())
                    .or_else(|| {
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
) -> Result<Vec<ChatMessageResponse>, String> {
    let rows = if let Some(lim) = limit {
        state
            .repos
            .sessions
            .get_recent_messages(&session_key, lim)
            .await
    } else {
        state.repos.sessions.get_messages(&session_key).await
    }
    .map_err(|e| e.to_string())?;

    Ok(rows
        .iter()
        .filter(|m| m.role == "user" || m.role == "assistant")
        .map(|m| ChatMessageResponse {
            id: m.id.to_string(),
            role: m.role.clone(),
            content: m.content.clone(),
            timestamp: m.timestamp,
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
) -> Result<ChatMessageResponse, String> {
    // 1. Ensure session exists
    let metadata = serde_json::json!({ "title": "Desktop Chat" });
    state
        .repos
        .sessions
        .upsert_session(&session_key, &metadata)
        .await
        .map_err(|e| e.to_string())?;

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
            .map_err(|e| e.to_string())?;
    }

    // 3. Store user message
    let msg_id = uuid::Uuid::new_v4();
    let msg = state
        .repos
        .sessions
        .add_message(&session_key, msg_id, "user", &content, None, None, None)
        .await
        .map_err(|e| e.to_string())?;

    // 4. Call agent with streaming
    let streaming_handle = state
        .agent
        .process_direct_streaming(content, session_key.clone())
        .await
        .map_err(|e| format!("agent error: {e}"))?;

    // 5. Track the cancel token
    state
        .active_streams
        .insert(session_key.clone(), streaming_handle.cancel_token);

    // 6. Spawn background task to relay AgentEvents → Tauri events
    let sk = session_key.clone();
    let active_streams = Arc::clone(&state.active_streams);
    let mut event_rx = streaming_handle.event_rx;

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

        while let Some(event) = event_rx.recv().await {
            match event {
                AgentEvent::ContentChunk { data } => {
                    let _ = app.emit(
                        AGENT_CONTENT_CHUNK,
                        ContentChunkPayload {
                            session_key: sk.clone(),
                            data,
                        },
                    );
                }
                AgentEvent::ToolStart { name, .. } => {
                    let _ = app.emit(
                        AGENT_TOOL_START,
                        ToolStartPayload {
                            session_key: sk.clone(),
                            name,
                        },
                    );
                }
                AgentEvent::ToolEnd {
                    name, success, ..
                } => {
                    let _ = app.emit(
                        AGENT_TOOL_END,
                        ToolEndPayload {
                            session_key: sk.clone(),
                            name,
                            success,
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
                    // Also emit entity:updated for UI refetch (skip unknown kinds)
                    if let Some(kind) = desktop_shared::types::EntityKind::parse(&card.entity_type) {
                        super::emit_entity_updated(&app, kind, &card.entity_id);
                    }
                }
                AgentEvent::Done { content } => {
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
                // Other events (IterationStart, ClassificationComplete, etc.) are internal
                _ => {}
            }
        }
    });

    // 7. Return user message immediately (streaming handles the AI response)
    Ok(ChatMessageResponse {
        id: msg.id.to_string(),
        role: msg.role,
        content: msg.content,
        timestamp: msg.timestamp,
    })
}

#[tauri::command]
pub async fn chat_pin_thread(
    state: State<'_, AppCore>,
    session_key: String,
) -> Result<(), String> {
    state
        .repos
        .session_context
        .pin(&session_key)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn chat_cancel(
    state: State<'_, AppCore>,
    session_key: String,
) -> Result<(), String> {
    if let Some((_, token)) = state.active_streams.remove(&session_key) {
        token.cancel();
    }
    Ok(())
}
