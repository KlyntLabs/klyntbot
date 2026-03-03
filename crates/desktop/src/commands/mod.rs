pub mod areas;
pub mod calendar;
pub mod chat;
pub mod finance;
pub mod key_results;
pub mod objectives;
pub mod projects;
pub mod status;
pub mod tasks;
pub mod window;

use chrono::{DateTime, Utc};
use desktop_shared::errors::ApiError;
use desktop_shared::events::{EntityUpdatedPayload, ENTITY_UPDATED};
use desktop_shared::types::EntityKind;
use tauri::Emitter;

/// Convert a `StorageError` into an `ApiError`, preserving specific error codes
/// for NotFound and Conflict variants.
pub(crate) fn map_storage_err(e: storage::StorageError) -> ApiError {
    match e {
        storage::StorageError::NotFound(msg) => ApiError::new("NOT_FOUND", msg),
        storage::StorageError::Conflict(msg) => ApiError::new("CONFLICT", msg),
        other => ApiError::new("STORAGE_ERROR", other.to_string()),
    }
}

/// Parse a "YYYY-MM-DD" string into a midnight UTC DateTime.
pub fn parse_date(s: &str) -> Option<DateTime<Utc>> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .ok()
        .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
}

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
