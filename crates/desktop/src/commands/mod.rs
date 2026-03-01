pub mod areas;
pub mod calendar;
pub mod chat;
pub mod status;
pub mod tasks;

use desktop_shared::events::{EntityUpdatedPayload, ENTITY_UPDATED};
use desktop_shared::types::EntityKind;
use tauri::Emitter;

/// Emit an entity-updated event so the frontend can refetch affected data.
pub fn emit_entity_updated(app: &tauri::AppHandle, kind: EntityKind, id: &str) {
    let payload = EntityUpdatedPayload {
        entity_kind: kind,
        id: id.to_string(),
    };
    if let Err(e) = app.emit(ENTITY_UPDATED, &payload) {
        tracing::warn!("failed to emit entity:updated event: {e}");
    }
}
