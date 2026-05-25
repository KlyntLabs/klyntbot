use desktop_macros::klynt_command;
use desktop_shared::types::EntityKind;

// ── Response type ────────────────────────────────────────────────────────

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PendingMemoryResponse {
    pub id: String,
    #[specta(type = desktop_shared::specta_helpers::JsonValue)]
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

#[klynt_command]
pub async fn list_pending_memories(limit: Option<i64>) -> Vec<PendingMemoryResponse> {
    let repo = state.pending_memory_repo()?;
    let rows = repo.list_pending(limit.unwrap_or(20)).await;
    Ok(rows.into_iter().map(PendingMemoryResponse::from).collect())
}

#[klynt_command]
pub async fn approve_pending_memory(id: String) -> () {
    state.approve_pending_memory(&id).await?;
    emitter.emit_entity_updated(EntityKind::PendingMemory, &id);
    Ok(())
}

#[klynt_command]
pub async fn dismiss_pending_memory(id: String) -> () {
    state.dismiss_pending_memory(&id).await?;
    emitter.emit_entity_updated(EntityKind::PendingMemory, &id);
    Ok(())
}
