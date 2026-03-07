use desktop_shared::commands::{
    NoteCreateParams, NoteLinkResponse, NoteResponse, NoteUpdateParams, NoteVersionResponse,
    NotebookCreateParams, NotebookResponse, NotebookUpdateParams,
};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use feature_notes::link_parser;
use feature_notes::models::{NoteRow, NoteVersionRow, NotebookRow};
use feature_notes::repo::utc_now_str;

use crate::errors::map_storage_err;
use crate::state::{AppCore, EntityUpdate, HandlerResult};

// ── Row → Response converters ───────────────────────────────────────────

pub(crate) fn note_row_to_response(row: &NoteRow, tags: Vec<String>) -> NoteResponse {
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

pub(crate) async fn note_with_tags(
    core: &AppCore,
    row: &NoteRow,
) -> Result<NoteResponse, ApiError> {
    let tags = core
        .note_repo
        .get_tags(&row.id)
        .await
        .map_err(map_storage_err)?;
    Ok(note_row_to_response(row, tags))
}

pub(crate) fn notebook_row_to_response(row: &NotebookRow, note_count: i64) -> NotebookResponse {
    NotebookResponse {
        id: row.id.clone(),
        parent_id: row.parent_id.clone(),
        title: row.title.clone(),
        icon: row.icon.clone(),
        sort_order: row.sort_order,
        note_count,
    }
}

/// Convert a list of NoteRows to NoteResponses with batch-fetched tags (single query).
pub(crate) async fn notes_with_tags_batch(
    core: &AppCore,
    rows: &[NoteRow],
) -> Result<Vec<NoteResponse>, ApiError> {
    let ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
    let mut tag_map = core
        .note_repo
        .get_tags_batch(&ids)
        .await
        .map_err(map_storage_err)?;
    Ok(rows
        .iter()
        .map(|row| {
            let tags = tag_map.remove(&row.id).unwrap_or_default();
            note_row_to_response(row, tags)
        })
        .collect())
}

pub(crate) fn version_row_to_response(row: &NoteVersionRow) -> NoteVersionResponse {
    NoteVersionResponse {
        id: row.id.clone(),
        note_id: row.note_id.clone(),
        body: row.body.clone(),
        created_at: row.created_at.clone(),
    }
}

/// Extract wiki-links and entity mentions from note content, updating the
/// `note_links` and `note_entity_mentions` tables.
/// Always writes to DB (even with empty vecs) to clear stale rows.
pub(crate) async fn extract_links_and_mentions(
    core: &AppCore,
    note_id: &str,
    row: &NoteRow,
) -> Result<(), ApiError> {
    // 1. Extract link targets from HTML (data-note-id attributes)
    let mut target_ids: Vec<String> = Vec::new();
    if let Some(html) = &row.body_html {
        target_ids.extend(link_parser::extract_wiki_link_ids(html));
    }

    // 2. Also resolve [[Title]] from plain text body
    let titles = link_parser::extract_wiki_link_titles(&row.body);
    if !titles.is_empty() {
        let resolved = core
            .note_repo
            .resolve_titles_to_ids(&titles)
            .await
            .map_err(map_storage_err)?;
        for (_title, id) in resolved {
            if !target_ids.contains(&id) {
                target_ids.push(id);
            }
        }
    }

    // Filter out self-links
    target_ids.retain(|id| id != note_id);

    // 3. Extract entity mentions (@task:id, @project:id)
    let mentions = link_parser::extract_entity_mentions(&row.body);

    core.note_repo
        .set_links(note_id, &target_ids)
        .await
        .map_err(map_storage_err)?;

    let mention_tuples: Vec<(String, String)> = mentions
        .into_iter()
        .map(|m| (m.entity_type, m.entity_id))
        .collect();
    core.note_repo
        .set_entity_mentions(note_id, &mention_tuples)
        .await
        .map_err(map_storage_err)?;

    Ok(())
}

// ── impl AppCore ────────────────────────────────────────────────────────

impl AppCore {
    // ── Read-only note handlers ─────────────────────────────────────

    pub async fn note_list(
        &self,
        notebook_id: Option<String>,
    ) -> Result<Vec<NoteResponse>, ApiError> {
        let rows = self
            .note_repo
            .list_notes(notebook_id.as_deref())
            .await
            .map_err(map_storage_err)?;

        notes_with_tags_batch(self, &rows).await
    }

    pub async fn note_get(&self, id: String) -> Result<NoteResponse, ApiError> {
        let row = self
            .note_repo
            .get_note(&id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("note '{id}' not found")))?;
        note_with_tags(self, &row).await
    }

    pub async fn note_search(&self, query: String) -> Result<Vec<NoteResponse>, ApiError> {
        let rows = self
            .note_repo
            .search_notes(&query)
            .await
            .map_err(map_storage_err)?;

        notes_with_tags_batch(self, &rows).await
    }

    pub async fn note_links_all(&self) -> Result<Vec<NoteLinkResponse>, ApiError> {
        let rows = self
            .note_repo
            .get_all_links()
            .await
            .map_err(map_storage_err)?;

        Ok(rows
            .into_iter()
            .map(|r| NoteLinkResponse {
                source_id: r.source_id,
                target_id: r.target_id,
            })
            .collect())
    }

    pub async fn note_list_by_entity(
        &self,
        entity_type: String,
        entity_id: String,
    ) -> Result<Vec<NoteResponse>, ApiError> {
        let rows = self
            .note_repo
            .list_notes_by_entity(&entity_type, &entity_id)
            .await
            .map_err(map_storage_err)?;

        notes_with_tags_batch(self, &rows).await
    }

    pub async fn note_version_list(
        &self,
        note_id: String,
    ) -> Result<Vec<NoteVersionResponse>, ApiError> {
        let rows = self
            .note_repo
            .list_versions(&note_id)
            .await
            .map_err(map_storage_err)?;

        Ok(rows.iter().map(version_row_to_response).collect())
    }

    pub async fn notebook_list(&self) -> Result<Vec<NotebookResponse>, ApiError> {
        let rows = self
            .note_repo
            .list_notebooks()
            .await
            .map_err(map_storage_err)?;

        let counts = self
            .note_repo
            .count_notes_by_notebook()
            .await
            .map_err(map_storage_err)?;

        Ok(rows
            .iter()
            .map(|row| notebook_row_to_response(row, counts.get(&row.id).copied().unwrap_or(0)))
            .collect())
    }

    // ── Mutating note handlers ──────────────────────────────────────

    pub async fn note_create(&self, params: NoteCreateParams) -> HandlerResult<NoteResponse> {
        if params.title.trim().is_empty() {
            return Err(ApiError::new("VALIDATION", "title must not be empty"));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = utc_now_str();

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

        let created = self
            .note_repo
            .create_note(&row)
            .await
            .map_err(map_storage_err)?;

        if let Some(tags) = params.tags {
            self.note_repo
                .set_tags(&id, &tags)
                .await
                .map_err(map_storage_err)?;
        }

        let response = note_with_tags(self, &created).await?;
        let updates = vec![EntityUpdate {
            kind: EntityKind::Note,
            id,
        }];
        Ok((response, updates))
    }

    pub async fn note_update(&self, params: NoteUpdateParams) -> HandlerResult<NoteResponse> {
        let updated = self
            .note_repo
            .update_note(
                &params.id,
                params.title.as_deref(),
                params.body.as_deref(),
                params.body_html.as_deref(),
                params.pinned,
                params.notebook_id.as_ref().map(|o| o.as_deref()),
            )
            .await
            .map_err(map_storage_err)?;

        if let Some(tags) = params.tags {
            self.note_repo
                .set_tags(&params.id, &tags)
                .await
                .map_err(map_storage_err)?;
        }

        // Extract wiki-links and entity mentions only when body content changed
        if params.body.is_some() || params.body_html.is_some() {
            extract_links_and_mentions(self, &params.id, &updated).await?;
        }

        let response = note_with_tags(self, &updated).await?;
        let updates = vec![EntityUpdate {
            kind: EntityKind::Note,
            id: params.id,
        }];
        Ok((response, updates))
    }

    pub async fn note_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .note_repo
            .delete_note(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = if deleted {
            vec![EntityUpdate {
                kind: EntityKind::Note,
                id,
            }]
        } else {
            vec![]
        };
        Ok((deleted, updates))
    }

    pub async fn note_version_create(
        &self,
        note_id: String,
    ) -> Result<NoteVersionResponse, ApiError> {
        let note = self
            .note_repo
            .get_note(&note_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("note '{note_id}' not found")))?;

        let id = uuid::Uuid::new_v4().to_string();
        let now = utc_now_str();

        let row = NoteVersionRow {
            id,
            note_id: note_id.clone(),
            body: note.body,
            created_at: now,
        };

        let created = self
            .note_repo
            .create_version(&row)
            .await
            .map_err(map_storage_err)?;

        // Prune old versions (keep max 50)
        if let Err(e) = self.note_repo.prune_versions(&note_id, 50).await {
            tracing::warn!(note_id = %note_id, error = %e, "failed to prune old versions");
        }

        Ok(version_row_to_response(&created))
    }

    pub async fn note_version_restore(
        &self,
        version_id: String,
        note_id: String,
    ) -> HandlerResult<NoteResponse> {
        // Find the version
        let version = self
            .note_repo
            .get_version(&version_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| {
                ApiError::new("NOT_FOUND", format!("version '{version_id}' not found"))
            })?;

        // Create a snapshot of current state before restoring
        let current_note = self
            .note_repo
            .get_note(&note_id)
            .await
            .map_err(map_storage_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", format!("note '{note_id}' not found")))?;

        let snapshot = NoteVersionRow {
            id: uuid::Uuid::new_v4().to_string(),
            note_id: note_id.clone(),
            body: current_note.body,
            created_at: utc_now_str(),
        };
        self.note_repo
            .create_version(&snapshot)
            .await
            .map_err(map_storage_err)?;

        // Restore the version body
        let updated = self
            .note_repo
            .update_note(&note_id, None, Some(&version.body), None, None, None)
            .await
            .map_err(map_storage_err)?;

        let response = note_with_tags(self, &updated).await?;
        let updates = vec![EntityUpdate {
            kind: EntityKind::Note,
            id: note_id,
        }];
        Ok((response, updates))
    }

    pub async fn note_save_attachment(
        &self,
        data: String,
        filename: String,
    ) -> Result<String, ApiError> {
        let config = self.config.read().await;
        let attachments_dir = config.data_dir_path().join("attachments");
        drop(config);

        tokio::fs::create_dir_all(&attachments_dir)
            .await
            .map_err(|e| {
                ApiError::new("IO_ERROR", format!("failed to create attachments dir: {e}"))
            })?;

        // Determine extension from filename
        const ALLOWED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp", "svg", "bmp"];
        let ext = std::path::Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let ext = if ALLOWED_EXTENSIONS.contains(&ext) {
            ext
        } else {
            "png"
        };

        let id = uuid::Uuid::new_v4();
        let file_name = format!("{id}.{ext}");
        let file_path = attachments_dir.join(&file_name);

        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&data)
            .map_err(|e| ApiError::new("DECODE_ERROR", format!("invalid base64: {e}")))?;

        tokio::fs::write(&file_path, &bytes)
            .await
            .map_err(|e| ApiError::new("IO_ERROR", format!("failed to write attachment: {e}")))?;

        // Return the absolute path — frontend converts to asset URL
        Ok(file_path.to_string_lossy().to_string())
    }

    // ── Notebook handlers ───────────────────────────────────────────

    pub async fn notebook_create(
        &self,
        params: NotebookCreateParams,
    ) -> HandlerResult<NotebookResponse> {
        if params.title.trim().is_empty() {
            return Err(ApiError::new(
                "VALIDATION",
                "notebook title must not be empty",
            ));
        }
        let id = uuid::Uuid::new_v4().to_string();
        let now = utc_now_str();

        let row = NotebookRow {
            id: id.clone(),
            parent_id: params.parent_id,
            title: params.title,
            icon: params.icon,
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };

        let created = self
            .note_repo
            .create_notebook(&row)
            .await
            .map_err(map_storage_err)?;

        let response = notebook_row_to_response(&created, 0);
        let updates = vec![EntityUpdate {
            kind: EntityKind::Notebook,
            id,
        }];
        Ok((response, updates))
    }

    pub async fn notebook_update(
        &self,
        params: NotebookUpdateParams,
    ) -> HandlerResult<NotebookResponse> {
        // Check for cycles when changing parent
        if let Some(Some(new_parent_id)) = &params.parent_id {
            if self
                .note_repo
                .would_create_cycle(&params.id, new_parent_id)
                .await
                .map_err(map_storage_err)?
            {
                return Err(ApiError::new(
                    "VALIDATION",
                    "cannot set parent: would create a cycle",
                ));
            }
        }
        let parent_id_ref = params.parent_id.as_ref().map(|o| o.as_deref());
        let updated = self
            .note_repo
            .update_notebook(
                &params.id,
                params.title.as_deref(),
                params.icon.as_deref(),
                parent_id_ref,
            )
            .await
            .map_err(map_storage_err)?;

        let count = self
            .note_repo
            .count_notes_in_notebook(&params.id)
            .await
            .map_err(map_storage_err)?;

        let response = notebook_row_to_response(&updated, count);
        let updates = vec![EntityUpdate {
            kind: EntityKind::Notebook,
            id: params.id,
        }];
        Ok((response, updates))
    }

    pub async fn notebook_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .note_repo
            .delete_notebook(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = if deleted {
            vec![EntityUpdate {
                kind: EntityKind::Notebook,
                id,
            }]
        } else {
            vec![]
        };
        Ok((deleted, updates))
    }
}
