use desktop_shared::commands::{
    NoteCreateParams, NoteResponse, NoteUpdateParams, NotebookCreateParams, NotebookResponse,
};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use feature_notes::models::{NoteRow, NotebookRow};
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

// ── Row → Response converters ───────────────────────────────────────────

fn note_row_to_response(row: &NoteRow, tags: Vec<String>) -> NoteResponse {
    NoteResponse {
        id: row.id.clone(),
        notebook_id: row.notebook_id.clone(),
        title: row.title.clone(),
        body: row.body.clone(),
        body_html: row.body_html.clone(),
        pinned: row.pinned != 0,
        archived: row.archived != 0,
        tags,
        created_at: row.created_at.clone(),
        updated_at: row.updated_at.clone(),
    }
}

async fn note_with_tags(core: &AppCore, row: &NoteRow) -> Result<NoteResponse, ApiError> {
    let tags = core
        .note_repo
        .get_tags(&row.id)
        .await
        .map_err(super::map_storage_err)?;
    Ok(note_row_to_response(row, tags))
}

// ── Note commands ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn note_list(
    state: State<'_, Arc<AppCore>>,
    notebook_id: Option<String>,
) -> Result<Vec<NoteResponse>, ApiError> {
    let rows = state
        .note_repo
        .list_notes(notebook_id.as_deref())
        .await
        .map_err(super::map_storage_err)?;

    let mut results = Vec::with_capacity(rows.len());
    for row in &rows {
        results.push(note_with_tags(&state, row).await?);
    }
    Ok(results)
}

#[tauri::command]
pub async fn note_get(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<NoteResponse, ApiError> {
    let row = state
        .note_repo
        .get_note(&id)
        .await
        .map_err(super::map_storage_err)?
        .ok_or_else(|| ApiError::new("NOT_FOUND", format!("note '{id}' not found")))?;
    note_with_tags(&state, &row).await
}

#[tauri::command]
pub async fn note_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NoteCreateParams,
) -> Result<NoteResponse, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    let row = NoteRow {
        id: id.clone(),
        notebook_id: params.notebook_id,
        title: params.title,
        body: params.body.unwrap_or_default(),
        body_html: None,
        pinned: 0,
        archived: 0,
        created_at: now.clone(),
        updated_at: now,
    };

    let created = state
        .note_repo
        .create_note(&row)
        .await
        .map_err(super::map_storage_err)?;

    if let Some(tags) = params.tags {
        state
            .note_repo
            .set_tags(&id, &tags)
            .await
            .map_err(super::map_storage_err)?;
    }

    super::emit_entity_updated(&app, EntityKind::Note, &id);
    note_with_tags(&state, &created).await
}

#[tauri::command]
pub async fn note_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NoteUpdateParams,
) -> Result<NoteResponse, ApiError> {
    let updated = state
        .note_repo
        .update_note(
            &params.id,
            params.title.as_deref(),
            params.body.as_deref(),
            params.body_html.as_deref(),
            params.pinned,
        )
        .await
        .map_err(super::map_storage_err)?;

    if let Some(tags) = params.tags {
        state
            .note_repo
            .set_tags(&params.id, &tags)
            .await
            .map_err(super::map_storage_err)?;
    }

    super::emit_entity_updated(&app, EntityKind::Note, &params.id);
    note_with_tags(&state, &updated).await
}

#[tauri::command]
pub async fn note_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let deleted = state
        .note_repo
        .delete_note(&id)
        .await
        .map_err(super::map_storage_err)?;

    if deleted {
        super::emit_entity_updated(&app, EntityKind::Note, &id);
    }
    Ok(deleted)
}

#[tauri::command]
pub async fn note_search(
    state: State<'_, Arc<AppCore>>,
    query: String,
) -> Result<Vec<NoteResponse>, ApiError> {
    let rows = state
        .note_repo
        .search_notes(&query)
        .await
        .map_err(super::map_storage_err)?;

    let mut results = Vec::with_capacity(rows.len());
    for row in &rows {
        results.push(note_with_tags(&state, row).await?);
    }
    Ok(results)
}

// ── Notebook commands ───────────────────────────────────────────────────

#[tauri::command]
pub async fn notebook_list(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<NotebookResponse>, ApiError> {
    let rows = state
        .note_repo
        .list_notebooks()
        .await
        .map_err(super::map_storage_err)?;

    let mut results = Vec::with_capacity(rows.len());
    for row in &rows {
        let count = state
            .note_repo
            .count_notes_in_notebook(&row.id)
            .await
            .map_err(super::map_storage_err)?;
        results.push(NotebookResponse {
            id: row.id.clone(),
            parent_id: row.parent_id.clone(),
            title: row.title.clone(),
            icon: row.icon.clone(),
            sort_order: row.sort_order,
            note_count: count,
        });
    }
    Ok(results)
}

#[tauri::command]
pub async fn notebook_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: NotebookCreateParams,
) -> Result<NotebookResponse, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now()
        .format("%Y-%m-%dT%H:%M:%SZ")
        .to_string();

    let row = NotebookRow {
        id: id.clone(),
        parent_id: params.parent_id,
        title: params.title,
        icon: params.icon,
        sort_order: 0,
        created_at: now.clone(),
        updated_at: now,
    };

    let created = state
        .note_repo
        .create_notebook(&row)
        .await
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Notebook, &id);

    Ok(NotebookResponse {
        id: created.id,
        parent_id: created.parent_id,
        title: created.title,
        icon: created.icon,
        sort_order: created.sort_order,
        note_count: 0,
    })
}

#[tauri::command]
pub async fn notebook_update(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
    title: Option<String>,
    icon: Option<String>,
) -> Result<NotebookResponse, ApiError> {
    let updated = state
        .note_repo
        .update_notebook(&id, title.as_deref(), icon.as_deref(), None)
        .await
        .map_err(super::map_storage_err)?;

    let count = state
        .note_repo
        .count_notes_in_notebook(&id)
        .await
        .map_err(super::map_storage_err)?;

    super::emit_entity_updated(&app, EntityKind::Notebook, &id);

    Ok(NotebookResponse {
        id: updated.id,
        parent_id: updated.parent_id,
        title: updated.title,
        icon: updated.icon,
        sort_order: updated.sort_order,
        note_count: count,
    })
}

#[tauri::command]
pub async fn notebook_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> Result<bool, ApiError> {
    let deleted = state
        .note_repo
        .delete_notebook(&id)
        .await
        .map_err(super::map_storage_err)?;

    if deleted {
        super::emit_entity_updated(&app, EntityKind::Notebook, &id);
    }
    Ok(deleted)
}
