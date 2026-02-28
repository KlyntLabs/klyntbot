//! Handlers for `/api/sessions` — list, get with messages, and delete.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;
use storage::{SessionListRow, SessionMessageRow, SessionRow};

use crate::api::deleted_or_not_found;
use crate::error::ApiError;
use crate::state::AppState;

// ── Response types ────────────────────────────────────────────────────────────

/// Response for `GET /api/sessions/:key` — session row plus all messages.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionWithMessages {
    pub session: SessionRow,
    pub messages: Vec<SessionMessageRow>,
}

// ── GET /api/sessions ─────────────────────────────────────────────────────────

pub async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<SessionListRow>>, ApiError> {
    let rows = state
        .repos
        .sessions
        .list_sessions()
        .await
        .map_err(ApiError::from)?;
    Ok(Json(rows))
}

// ── GET /api/sessions/:key ────────────────────────────────────────────────────

pub async fn get_session(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<Json<SessionWithMessages>, ApiError> {
    let (session, messages) = tokio::join!(
        state.repos.sessions.get_session(&key),
        state.repos.sessions.get_messages(&key),
    );
    let session = session.map_err(ApiError::from)?;
    let messages = messages.map_err(ApiError::from)?;

    Ok(Json(SessionWithMessages { session, messages }))
}

// ── DELETE /api/sessions/:key ─────────────────────────────────────────────────

pub async fn delete_session(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> Result<StatusCode, ApiError> {
    let deleted = state
        .repos
        .sessions
        .delete_session(&key)
        .await
        .map_err(ApiError::from)?;

    deleted_or_not_found(deleted, "session", &key)
}
