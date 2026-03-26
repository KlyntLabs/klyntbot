//! Session CRUD handlers — get, list by project, delete stale.

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
        created_at: row.created_at,
        updated_at: row.updated_at,
        project_id: row.project_id.clone(),
        conversation_type: row.conversation_type.clone(),
        pinned: row.pinned,
        squad_id: row.squad_id.clone(),
    }
}

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
            updated_at: s.updated_at,
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

pub async fn chat_delete_stale_sessions(repos: &Repos, before_days: u32) -> Result<u64, ApiError> {
    repos
        .sessions
        .delete_stale_sessions(before_days)
        .await
        .map_err(map_storage_err)
}

// ── AppCore convenience methods ─────────────────────────────────────────

impl AppCore {
    pub async fn chat_get_session(
        &self,
        session_key: String,
    ) -> Result<ChatSessionResponse, ApiError> {
        chat_get_session(&self.repos, session_key).await
    }

    pub async fn chat_list_sessions_by_project(
        &self,
        project_id: String,
    ) -> Result<Vec<ChatThreadResponse>, ApiError> {
        chat_list_sessions_by_project(&self.repos, project_id).await
    }

    pub async fn chat_delete_stale_sessions(&self, before_days: u32) -> Result<u64, ApiError> {
        chat_delete_stale_sessions(&self.repos, before_days).await
    }
}
