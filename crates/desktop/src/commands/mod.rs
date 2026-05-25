pub mod agents;
pub mod annotations;
pub mod app_icon;
pub mod areas;
pub mod autotuner;
pub mod capture;
pub mod chat;
pub mod cognitive;
pub mod columns;
pub mod cron;
pub mod distraction;
pub mod entities;
pub mod entity_links;
pub mod fabric;
pub mod focus;
pub mod git;
pub mod groups;
pub mod integrations;
pub mod journey;
pub mod key_results;
pub mod knowledge_health;
pub mod language;
pub mod launcher;
pub mod mirror;
pub mod morning_briefing;
pub mod notes;
pub mod objectives;
pub mod pending_memory;
pub mod permissions;
pub mod practice;
pub mod productivity;
pub mod project_conversations;
pub mod project_memories;
pub mod project_sources;
pub mod projects;
pub mod retention_history;
pub mod review_stats;
pub mod settings;
pub mod shortcuts;
pub mod status;
pub mod status_badge;
pub mod subagent;
pub mod tasks;
pub mod timeline;
pub mod tracing;
pub mod view;
pub mod voice;
pub mod voice_conversation;
pub mod window;
pub mod work_context;
pub mod workflows;
pub mod workspace;
pub mod workspace_lifecycle;

#[cfg(debug_assertions)]
pub(crate) mod dev_helpers;

use desktop_shared::events::EntityUpdatedPayload;
use desktop_shared::types::EntityKind;
use tauri_specta::Event;

pub fn emit_updates(app: &tauri::AppHandle, updates: &[::app_core::EntityUpdate]) {
    for u in updates {
        emit_entity_updated(app, u.kind.clone(), &u.id);
    }
}

/// `AppEventEmitter` backed by a Tauri `AppHandle`. Built by `#[klynt_command]`
/// for the Tauri IPC adapter so one command body can emit through the same
/// transport-neutral seam the dev HTTP adapter uses.
pub struct TauriEmitter {
    app: tauri::AppHandle,
}

impl TauriEmitter {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl ::app_core::events::AppEventEmitter for TauriEmitter {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value) {
        use tauri::Emitter;
        if let Err(e) = self.app.emit(event_name, payload) {
            ::tracing::warn!("failed to emit '{event_name}' event: {e}");
        }
    }
}

/// Emit entity updates through the transport-neutral emitter seam. Used by
/// commands migrated off the `app: tauri::AppHandle` + `emit_updates` pattern.
pub fn emit_updates_ev(
    emitter: &dyn ::app_core::events::AppEventEmitter,
    updates: &[::app_core::EntityUpdate],
) {
    for u in updates {
        emitter.emit_entity_updated(u.kind.clone(), &u.id);
    }
}

pub fn emit_entity_updated(app: &tauri::AppHandle, kind: EntityKind, id: &str) {
    let payload = EntityUpdatedPayload {
        entity_kind: kind,
        id: id.to_string(),
    };
    if let Err(e) = payload.emit(app) {
        ::tracing::warn!("failed to emit entity:updated event: {e}");
    }
}
