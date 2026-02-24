//! GET /api/status — agent status overview.

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::ApiError;
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub version: &'static str,
    pub model: String,
    pub uptime_seconds: u64,
    pub storage: StorageStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub task_count: i64,
    pub session_count: i64,
}

pub async fn get_status(
    State(state): State<AppState>,
) -> Result<Json<StatusResponse>, ApiError> {
    let task_count = state
        .repos
        .todos
        .summary()
        .await
        .map(|s| s.total)
        .unwrap_or(0);

    let session_count = state
        .repos
        .sessions
        .list_sessions()
        .await
        .map(|s| s.len() as i64)
        .unwrap_or(0);

    Ok(Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION"),
        model: state.config.agents.defaults.model.clone(),
        uptime_seconds: state.started_at.elapsed().as_secs(),
        storage: StorageStats { task_count, session_count },
    }))
}
