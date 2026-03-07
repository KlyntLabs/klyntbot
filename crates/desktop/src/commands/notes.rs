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
