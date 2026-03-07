pub mod areas;
pub mod chat;
pub mod cognitive;
pub mod columns;
pub mod distraction;
pub mod finance;
pub mod groups;
pub mod key_results;
pub mod notes;
pub mod objectives;
pub mod permissions;
pub mod productivity;
pub mod projects;
pub mod session_tracker;
pub mod settings;
pub mod status;
pub mod tasks;
pub mod window;
pub mod workflows;

use desktop_shared::events::{EntityUpdatedPayload, ENTITY_UPDATED};
use desktop_shared::types::EntityKind;
use tauri::Emitter;

pub fn emit_updates(app: &tauri::AppHandle, updates: &[::app_core::EntityUpdate]) {
    for u in updates {
        emit_entity_updated(app, u.kind.clone(), &u.id);
    }
}

pub fn emit_entity_updated(app: &tauri::AppHandle, kind: EntityKind, id: &str) {
    let payload = EntityUpdatedPayload {
        entity_kind: kind,
        id: id.to_string(),
    };
    if let Err(e) = app.emit(ENTITY_UPDATED, &payload) {
        tracing::warn!("failed to emit entity:updated event: {e}");
    }
}
