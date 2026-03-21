use desktop_shared::commands::{
    AtomAcceptParams, AtomDismissParams, AtomMigrationStatusResponse, AtomNextCardParams,
    AtomRestoreParams, AtomsForNoteParams, FlashcardResponse, KnowledgeAtomResponse,
};
use desktop_shared::errors::ApiError;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn atoms_for_note(
    state: State<'_, Arc<AppCore>>,
    params: AtomsForNoteParams,
) -> Result<Vec<KnowledgeAtomResponse>, ApiError> {
    state.atoms_for_note(params).await
}

#[tauri::command]
pub async fn atom_accept(
    state: State<'_, Arc<AppCore>>,
    params: AtomAcceptParams,
) -> Result<KnowledgeAtomResponse, ApiError> {
    state.atom_accept(params).await
}

#[tauri::command]
pub async fn atom_dismiss(
    state: State<'_, Arc<AppCore>>,
    params: AtomDismissParams,
) -> Result<(), ApiError> {
    state.atom_dismiss(params).await
}

#[tauri::command]
pub async fn atom_restore(
    state: State<'_, Arc<AppCore>>,
    params: AtomRestoreParams,
) -> Result<KnowledgeAtomResponse, ApiError> {
    state.atom_restore(params).await
}

#[tauri::command]
pub async fn atom_next_card(
    state: State<'_, Arc<AppCore>>,
    params: AtomNextCardParams,
) -> Result<Option<FlashcardResponse>, ApiError> {
    state.atom_next_card(params).await
}

#[tauri::command]
pub async fn atoms_migration_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<AtomMigrationStatusResponse, ApiError> {
    state.atoms_migration_status().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "atoms_for_note",
    "atom_accept",
    "atom_dismiss",
    "atom_restore",
    "atom_next_card",
    "atoms_migration_status",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "atoms_for_note" => dev::val(core.atoms_for_note(try_field!(dev::parse_params(body))).await),
        "atom_accept" => dev::val(core.atom_accept(try_field!(dev::parse_params(body))).await),
        "atom_dismiss" => dev::val(core.atom_dismiss(try_field!(dev::parse_params(body))).await),
        "atom_restore" => dev::val(core.atom_restore(try_field!(dev::parse_params(body))).await),
        "atom_next_card" => dev::val(core.atom_next_card(try_field!(dev::parse_params(body))).await),
        "atoms_migration_status" => dev::val(core.atoms_migration_status().await),
        _ => return None,
    })
}
