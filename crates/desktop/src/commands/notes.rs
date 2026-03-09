use std::sync::Arc;

use desktop_shared::commands::{
    NoteCreateParams, NoteLinkResponse, NoteResponse, NoteUpdateParams, NoteVersionResponse,
    NotebookCreateParams, NotebookResponse, NotebookUpdateParams,
};
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

// ── Note commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn note_list(
    state: State<'_, Arc<AppCore>>,
    notebook_id: Option<String>,
) -> Result<Vec<NoteResponse>, ApiError> {
    state.note_list(notebook_id).await
}

#[tauri::command]
pub async fn note_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<NoteResponse, ApiError> {
    state.note_get(id).await
}

#[tauri::command]
pub async fn note_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NoteCreateParams,
) -> Result<NoteResponse, ApiError> {
    let (result, updates) = state.note_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn note_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NoteUpdateParams,
) -> Result<NoteResponse, ApiError> {
    let (result, updates) = state.note_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn note_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.note_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn note_search(
    state: State<'_, Arc<AppCore>>,
    query: String,
) -> Result<Vec<NoteResponse>, ApiError> {
    state.note_search(query).await
}

#[tauri::command]
pub async fn note_links_all(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<NoteLinkResponse>, ApiError> {
    state.note_links_all().await
}

#[tauri::command]
pub async fn note_list_by_entity(
    state: State<'_, Arc<AppCore>>,
    entity_type: String,
    entity_id: String,
) -> Result<Vec<NoteResponse>, ApiError> {
    state.note_list_by_entity(entity_type, entity_id).await
}

// ── Version commands ────────────────────────────────────────────────────

#[tauri::command]
pub async fn note_version_list(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<Vec<NoteVersionResponse>, ApiError> {
    state.note_version_list(note_id).await
}

#[tauri::command]
pub async fn note_version_create(
    state: State<'_, Arc<AppCore>>,
    note_id: String,
) -> Result<NoteVersionResponse, ApiError> {
    state.note_version_create(note_id).await
}

#[tauri::command]
pub async fn note_version_restore(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    version_id: String,
    note_id: String,
) -> Result<NoteResponse, ApiError> {
    let (result, updates) = state.note_version_restore(version_id, note_id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Attachment commands ─────────────────────────────────────────────────

#[tauri::command]
pub async fn note_save_attachment(
    state: State<'_, Arc<AppCore>>,
    data: String,
    filename: String,
) -> Result<String, ApiError> {
    state.note_save_attachment(data, filename).await
}

// ── Notebook commands ───────────────────────────────────────────────────

#[tauri::command]
pub async fn notebook_list(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<NotebookResponse>, ApiError> {
    state.notebook_list().await
}

#[tauri::command]
pub async fn notebook_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NotebookCreateParams,
) -> Result<NotebookResponse, ApiError> {
    let (result, updates) = state.notebook_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn notebook_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NotebookUpdateParams,
) -> Result<NotebookResponse, ApiError> {
    let (result, updates) = state.notebook_update(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
pub async fn notebook_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let (result, updates) = state.notebook_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "note_list",
    "note_get",
    "note_create",
    "note_update",
    "note_delete",
    "note_search",
    "note_links_all",
    "note_list_by_entity",
    "note_version_list",
    "note_version_create",
    "note_version_restore",
    "note_save_attachment",
    "notebook_list",
    "notebook_create",
    "notebook_update",
    "notebook_delete",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "note_list" => dev::val(core.note_list(dev::get(body, "notebook_id")).await),
        "note_get" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.note_get(id).await)
        }
        "note_create" => dev::val_rh(core.note_create(try_field!(dev::parse_params(body))).await),
        "note_update" => dev::val_rh(core.note_update(try_field!(dev::parse_params(body))).await),
        "note_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.note_delete(id).await)
        }
        "note_search" => {
            let query = try_field!(dev::get_str(body, "query"));
            dev::val(core.note_search(query).await)
        }
        "note_links_all" => dev::val(core.note_links_all().await),
        "note_list_by_entity" => {
            let entity_type = try_field!(dev::get_str(body, "entity_type"));
            let entity_id = try_field!(dev::get_str(body, "entity_id"));
            dev::val(core.note_list_by_entity(entity_type, entity_id).await)
        }
        "note_version_list" => {
            let note_id = try_field!(dev::get_str(body, "note_id"));
            dev::val(core.note_version_list(note_id).await)
        }
        "note_version_create" => {
            let note_id = try_field!(dev::get_str(body, "note_id"));
            dev::val(core.note_version_create(note_id).await)
        }
        "note_version_restore" => {
            let version_id = try_field!(dev::get_str(body, "version_id"));
            let note_id = try_field!(dev::get_str(body, "note_id"));
            dev::val_rh(core.note_version_restore(version_id, note_id).await)
        }
        "note_save_attachment" => {
            let data = try_field!(dev::get_str(body, "data"));
            let filename = try_field!(dev::get_str(body, "filename"));
            dev::val(core.note_save_attachment(data, filename).await)
        }
        "notebook_list" => dev::val(core.notebook_list().await),
        "notebook_create" => dev::val_rh(
            core.notebook_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "notebook_update" => dev::val_rh(
            core.notebook_update(try_field!(dev::parse_params(body)))
                .await,
        ),
        "notebook_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.notebook_delete(id).await)
        }
        _ => return None,
    })
}
