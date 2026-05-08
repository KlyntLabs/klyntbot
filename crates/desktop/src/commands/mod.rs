pub mod agents;
pub mod annotations;
pub mod app_icon;
pub mod approval;
pub mod approvals;
pub mod areas;
pub mod autotuner;
pub mod capture;
pub mod chat;
pub mod coding_doctor;
pub mod coding_help;
pub mod coding_mcp;
pub mod coding_memory;
pub mod coding_recall_stats;
pub mod coding_resume;
pub mod coding_review;
pub mod coding_sessions;
pub mod coding_sessions_v2;
pub mod coding_skills;
pub mod coding_status;
pub mod coding_thread;
pub mod coding_thread_metadata;
pub mod coding_todo;
pub mod coding_turn;
pub mod coding_window;
pub mod cognitive;
pub mod columns;
pub mod cron;
pub mod distraction;
pub mod entities;
pub mod entity_links;
pub mod fabric;
pub mod finance;
pub mod focus;
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
pub mod providers;
pub mod reforge;
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
pub mod workspace_files;
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

pub fn emit_entity_updated(app: &tauri::AppHandle, kind: EntityKind, id: &str) {
    let payload = EntityUpdatedPayload {
        entity_kind: kind,
        id: id.to_string(),
    };
    if let Err(e) = payload.emit(app) {
        ::tracing::warn!("failed to emit entity:updated event: {e}");
    }
}
