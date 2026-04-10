use std::sync::Arc;

use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use tauri::State;

use crate::app_core::AppCore;

// ── Response type ────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingMemoryResponse {
    pub id: String,
    pub fact: serde_json::Value,
    pub reason: String,
    pub created_at: String,
}

impl From<cognitive::repos::PendingMemoryRow> for PendingMemoryResponse {
    fn from(row: cognitive::repos::PendingMemoryRow) -> Self {
        Self {
            id: row.id.clone(),
            fact: serde_json::from_str(&row.fact_json).unwrap_or(serde_json::Value::Null),
            reason: row.reason,
            created_at: row.created_at,
        }
    }
}

// ── Commands ─────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_pending_memories(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
) -> Result<Vec<PendingMemoryResponse>, ApiError> {
    let repo = state.pending_memory_repo()?;
    let rows = repo.list_pending(limit.unwrap_or(20)).await;
    Ok(rows.into_iter().map(PendingMemoryResponse::from).collect())
}

#[tauri::command]
pub async fn approve_pending_memory(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), ApiError> {
    state.approve_pending_memory(&id).await?;
    super::emit_entity_updated(&app, EntityKind::PendingMemory, &id);
    Ok(())
}

#[tauri::command]
pub async fn dismiss_pending_memory(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<(), ApiError> {
    state.dismiss_pending_memory(&id).await?;
    super::emit_entity_updated(&app, EntityKind::PendingMemory, &id);
    Ok(())
}

// ── Dev server dispatch ──────────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "list_pending_memories",
    "approve_pending_memory",
    "dismiss_pending_memory",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};

    Some(match cmd {
        "list_pending_memories" => {
            let repo = match core.pending_memory_repo() {
                Ok(r) => r,
                Err(e) => return Some(Err(e)),
            };
            let limit: Option<i64> = dev::get(body, "limit");
            let rows = repo.list_pending(limit.unwrap_or(20)).await;
            let responses: Vec<PendingMemoryResponse> =
                rows.into_iter().map(PendingMemoryResponse::from).collect();
            dev::val(Ok::<_, ApiError>(responses))
        }
        "approve_pending_memory" => {
            let id: String = try_field!(dev::get(body, "id")
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: id")));
            dev::val(core.approve_pending_memory(&id).await)
        }
        "dismiss_pending_memory" => {
            let id: String = try_field!(dev::get(body, "id")
                .ok_or_else(|| ApiError::new("VALIDATION", "missing required field: id")));
            dev::val(core.dismiss_pending_memory(&id).await)
        }
        _ => return None,
    })
}
