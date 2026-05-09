//! Thread management handlers — list, messages, pin, rename, delete.

use std::collections::HashMap;

use desktop_shared::commands::{ChatMessageResponse, ChatThreadResponse};
use desktop_shared::errors::ApiError;
use desktop_shared::events;
use storage::{ProjectFilter, Repos};

use super::streaming::{ActiveStreams, PendingInteractions};
use crate::errors::map_storage_err;
use crate::state::AppCore;

// ── Public free functions ────────────────────────────────────────────────

#[tracing::instrument(skip(repos), err)]
pub async fn chat_threads(repos: &Repos) -> Result<Vec<ChatThreadResponse>, ApiError> {
    let default_filter = ProjectFilter::default();
    let (sessions, visible_contexts, all_areas, all_projects) = tokio::join!(
        repos
            .sessions
            .list_sessions_by_mode(common::SessionMode::Assistant),
        repos.session_context.list_visible(),
        repos.areas.list(None),
        repos.projects.list(&default_filter),
    );
    let sessions = sessions.map_err(map_storage_err)?;
    let visible_contexts = visible_contexts.map_err(map_storage_err)?;
    let all_areas = all_areas.map_err(map_storage_err)?;
    let all_projects = all_projects.map_err(map_storage_err)?;

    let ctx_map: HashMap<&str, _> = visible_contexts
        .iter()
        .map(|c| (c.session_key.as_str(), c))
        .collect();

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
                .filter(|t| !t.is_empty())
                .unwrap_or(&s.key)
                .to_string();
            let ctx = ctx_map.get(s.key.as_str());

            ChatThreadResponse {
                session_key: s.key.clone(),
                title,
                message_count: s.message_count,
                updated_at: *s.updated_at,
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

#[tracing::instrument(skip(repos), err)]
pub async fn chat_messages(
    repos: &Repos,
    session_key: String,
    limit: Option<i64>,
) -> Result<Vec<ChatMessageResponse>, ApiError> {
    let lim = limit.unwrap_or(100).min(500);
    let rows = repos
        .sessions
        .get_recent_messages(&session_key, lim)
        .await
        .map_err(map_storage_err)?;

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
                timestamp: *m.timestamp,
                segments,
                transparency,
            }
        })
        .collect())
}

#[tracing::instrument(skip(repos), err)]
pub async fn chat_pin_thread(repos: &Repos, session_key: String) -> Result<(), ApiError> {
    repos
        .session_context
        .pin(&session_key)
        .await
        .map_err(map_storage_err)?;
    Ok(())
}

#[tracing::instrument(skip(repos), err)]
pub async fn chat_rename_thread(
    repos: &Repos,
    session_key: String,
    title: String,
) -> Result<(), ApiError> {
    let title = title.trim().to_string();
    if title.is_empty() {
        return Err(ApiError::new("INVALID_PARAMS", "title cannot be empty"));
    }
    repos
        .sessions
        .rename_session(&session_key, &title)
        .await
        .map_err(map_storage_err)?;
    Ok(())
}

#[tracing::instrument(skip(repos, active_streams, pending_interactions), err)]
pub async fn chat_delete_thread(
    repos: &Repos,
    active_streams: &ActiveStreams,
    pending_interactions: &PendingInteractions,
    session_key: String,
) -> Result<(), ApiError> {
    // Cancel any in-flight stream before deleting to avoid dangling writes
    if let Some((_, token)) = active_streams.remove(&session_key) {
        token.cancel();
    }
    // Cancel any pending interaction to avoid leaking the oneshot
    if let Some((_, (_, tx))) = pending_interactions.remove(&session_key) {
        let _ = tx.send(common::FormResponse::Cancelled);
    }
    repos
        .sessions
        .delete_session(&session_key)
        .await
        .map_err(map_storage_err)?;
    Ok(())
}

// ── AppCore convenience methods ─────────────────────────────────────────

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn chat_threads(&self) -> Result<Vec<ChatThreadResponse>, ApiError> {
        chat_threads(&self.repos).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn chat_messages(
        &self,
        session_key: String,
        limit: Option<i64>,
    ) -> Result<Vec<ChatMessageResponse>, ApiError> {
        chat_messages(&self.repos, session_key, limit).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn chat_pin_thread(&self, session_key: String) -> Result<(), ApiError> {
        chat_pin_thread(&self.repos, session_key.clone()).await?;
        self.event_emitter.emit_chat_thread(false, &session_key);
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn chat_rename_thread(
        &self,
        session_key: String,
        title: String,
    ) -> Result<(), ApiError> {
        chat_rename_thread(&self.repos, session_key.clone(), title).await?;
        self.event_emitter.emit_chat_thread(false, &session_key);
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn chat_delete_thread(&self, session_key: String) -> Result<(), ApiError> {
        // Phase 2.3a — reap any live background bash jobs before deleting the thread
        if let Some(ref supervisor) = self.job_supervisor {
            match supervisor.reap_session(&session_key).await {
                Ok(n) if n > 0 => tracing::info!(session = %session_key, count = n, "reaped background jobs on thread delete"),
                Ok(_) => {}
                Err(e) => tracing::warn!(session = %session_key, "failed to reap background jobs: {e}"),
            }
        }
        chat_delete_thread(
            &self.repos,
            &self.active_streams,
            &self.pending_interactions,
            session_key.clone(),
        )
        .await?;
        self.event_emitter.emit_chat_thread(false, &session_key);
        Ok(())
    }
}
