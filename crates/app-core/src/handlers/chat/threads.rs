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

pub async fn chat_threads(
    repos: &Repos,
    squad_repo: Option<&cognitive::SquadRepo>,
) -> Result<Vec<ChatThreadResponse>, ApiError> {
    let default_filter = ProjectFilter::default();
    let (sessions, visible_contexts, all_areas, all_projects) = tokio::join!(
        repos.sessions.list_sessions(),
        repos.session_context.list_visible(),
        repos.areas.list(None),
        repos.projects.list(&default_filter),
    );
    let sessions = sessions.map_err(map_storage_err)?;
    let visible_contexts = visible_contexts.map_err(map_storage_err)?;
    let all_areas = all_areas.map_err(map_storage_err)?;
    let all_projects = all_projects.map_err(map_storage_err)?;

    // Collect unique squad_ids and batch-resolve names/icons
    let squad_ids: Vec<&str> = sessions
        .iter()
        .filter_map(|s| s.squad_id.as_deref())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let squad_map: HashMap<String, (String, String)> = if !squad_ids.is_empty() {
        if let Some(repo) = squad_repo {
            let mut map = HashMap::new();
            for id in &squad_ids {
                if let Ok(Some(squad)) = repo.get(id).await {
                    map.insert(squad.id.clone(), (squad.name.clone(), squad.icon.clone()));
                }
            }
            map
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

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

            let (squad_name, squad_icon) = s
                .squad_id
                .as_deref()
                .and_then(|id| squad_map.get(id))
                .map(|(name, icon)| (Some(name.clone()), Some(icon.clone())))
                .unwrap_or((None, None));

            ChatThreadResponse {
                session_key: s.key.clone(),
                title,
                message_count: s.message_count,
                updated_at: common::time::bridge::jiff_to_chrono(*s.updated_at),
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
                squad_id: s.squad_id.clone(),
                squad_name,
                squad_icon,
            }
        })
        .collect())
}

pub async fn chat_messages(
    repos: &Repos,
    persona_repo: Option<&cognitive::PersonaRepo>,
    session_key: String,
    limit: Option<i64>,
) -> Result<Vec<ChatMessageResponse>, ApiError> {
    let lim = limit.unwrap_or(100).min(500);
    let rows = repos
        .sessions
        .get_recent_messages(&session_key, lim)
        .await
        .map_err(map_storage_err)?;

    // Batch-resolve persona names for messages that have persona_id
    let persona_ids: Vec<&str> = rows
        .iter()
        .filter_map(|m| m.persona_id.as_deref())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let persona_names: HashMap<String, String> = if !persona_ids.is_empty() {
        if let Some(repo) = persona_repo {
            let mut map = HashMap::new();
            for id in &persona_ids {
                if let Ok(Some(persona)) = repo.get(id).await {
                    map.insert(persona.id.clone(), persona.name.clone());
                }
            }
            map
        } else {
            HashMap::new()
        }
    } else {
        HashMap::new()
    };

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
            let persona_name = m
                .persona_id
                .as_deref()
                .and_then(|id| persona_names.get(id).cloned());
            ChatMessageResponse {
                id: m.id.to_string(),
                role: m.role.clone(),
                content: m.content.clone(),
                timestamp: common::time::bridge::jiff_to_chrono(*m.timestamp),
                segments,
                transparency,
                persona_id: m.persona_id.clone(),
                persona_name,
            }
        })
        .collect())
}

pub async fn chat_pin_thread(repos: &Repos, session_key: String) -> Result<(), ApiError> {
    repos
        .session_context
        .pin(&session_key)
        .await
        .map_err(map_storage_err)?;
    Ok(())
}

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
    pub async fn chat_threads(&self) -> Result<Vec<ChatThreadResponse>, ApiError> {
        chat_threads(&self.repos, self.squad_repo.as_ref()).await
    }

    pub async fn chat_messages(
        &self,
        session_key: String,
        limit: Option<i64>,
    ) -> Result<Vec<ChatMessageResponse>, ApiError> {
        chat_messages(&self.repos, self.persona_repo.as_ref(), session_key, limit).await
    }

    pub async fn chat_pin_thread(&self, session_key: String) -> Result<(), ApiError> {
        chat_pin_thread(&self.repos, session_key.clone()).await?;
        self.event_emitter.emit_chat_thread(false, &session_key);
        Ok(())
    }

    pub async fn chat_rename_thread(
        &self,
        session_key: String,
        title: String,
    ) -> Result<(), ApiError> {
        chat_rename_thread(&self.repos, session_key.clone(), title).await?;
        self.event_emitter.emit_chat_thread(false, &session_key);
        Ok(())
    }

    pub async fn chat_delete_thread(&self, session_key: String) -> Result<(), ApiError> {
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
