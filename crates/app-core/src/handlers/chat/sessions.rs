//! Session CRUD handlers — get, list by project, delete stale.

use std::collections::HashMap;

use desktop_shared::commands::{ChatSessionResponse, ChatThreadResponse};
use desktop_shared::errors::ApiError;
use storage::Repos;

use crate::errors::map_storage_err;
use crate::state::AppCore;

pub(crate) fn extract_title(metadata: &serde_json::Value) -> String {
    metadata
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string()
}

fn session_row_to_response(
    row: &storage::rows::session::SessionRow,
    message_count: i64,
) -> ChatSessionResponse {
    ChatSessionResponse {
        session_key: row.key.clone(),
        title: extract_title(&row.metadata),
        message_count,
        created_at: *row.created_at,
        updated_at: *row.updated_at,
        project_id: row.project_id.clone(),
        conversation_type: row.conversation_type.clone(),
        pinned: row.pinned,
        squad_id: row.squad_id.clone(),
    }
}

#[tracing::instrument(skip(repos), err)]
pub async fn chat_get_session(
    repos: &Repos,
    session_key: String,
) -> Result<ChatSessionResponse, ApiError> {
    let (session, msg_count) = tokio::try_join!(
        repos.sessions.get_session(&session_key),
        repos.sessions.count_messages(&session_key),
    )
    .map_err(map_storage_err)?;

    Ok(session_row_to_response(&session, msg_count))
}

#[tracing::instrument(skip(repos), err)]
pub async fn chat_list_sessions_by_project(
    repos: &Repos,
    project_id: String,
) -> Result<Vec<ChatThreadResponse>, ApiError> {
    let rows = repos
        .sessions
        .list_by_project(&project_id)
        .await
        .map_err(map_storage_err)?;

    Ok(rows
        .iter()
        .map(|s| ChatThreadResponse {
            session_key: s.key.clone(),
            title: extract_title(&s.metadata),
            message_count: s.message_count,
            updated_at: *s.updated_at,
            context_type: None,
            entity_kind: None,
            entity_id: None,
            area_id: None,
            area_name: None,
            project_id: s.project_id.clone(),
            project_name: None,
            squad_id: s.squad_id.clone(),
            squad_name: None,
            squad_icon: None,
        })
        .collect())
}

#[tracing::instrument(skip(repos), err)]
pub async fn chat_delete_stale_sessions(repos: &Repos, before_days: u32) -> Result<u64, ApiError> {
    repos
        .sessions
        .delete_stale_sessions(before_days)
        .await
        .map_err(map_storage_err)
}

// ── AppCore convenience methods ─────────────────────────────────────────

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn chat_get_session(
        &self,
        session_key: String,
    ) -> Result<ChatSessionResponse, ApiError> {
        chat_get_session(&self.repos, session_key).await
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn chat_list_sessions_by_project(
        &self,
        project_id: String,
    ) -> Result<Vec<ChatThreadResponse>, ApiError> {
        let mut threads = chat_list_sessions_by_project(&self.repos, project_id).await?;

        // Enrich squad_name and squad_icon via post-fetch lookup if squad_repo is available.
        if let Some(squad_repo) = &self.squad_repo {
            // Collect unique squad IDs that need resolving.
            let squad_ids: Vec<String> = threads
                .iter()
                .filter_map(|t| t.squad_id.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            if !squad_ids.is_empty() {
                // Fetch squad rows concurrently and build a lookup map.
                let futures: Vec<_> = squad_ids.iter().map(|id| squad_repo.get(id)).collect();
                let results = futures_util::future::join_all(futures).await;
                let mut squad_map: HashMap<String, (String, String)> =
                    HashMap::with_capacity(squad_ids.len());
                for result in results {
                    if let Ok(Some(row)) = result {
                        squad_map.insert(row.id.clone(), (row.name.clone(), row.icon.clone()));
                    }
                }

                // Apply squad_name and squad_icon to each thread.
                for thread in &mut threads {
                    if let Some(sid) = &thread.squad_id {
                        if let Some((name, icon)) = squad_map.get(sid) {
                            thread.squad_name = Some(name.clone());
                            thread.squad_icon = Some(icon.clone());
                        }
                    }
                }
            }
        }

        Ok(threads)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn chat_delete_stale_sessions(&self, before_days: u32) -> Result<u64, ApiError> {
        chat_delete_stale_sessions(&self.repos, before_days).await
    }
}
