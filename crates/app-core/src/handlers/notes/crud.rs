use std::sync::Arc;

use desktop_shared::commands::{
    BacklinkResponse, HybridSearchResponse, NoteCreateParams, NoteLinkResponse, NoteResponse,
    NoteUpdateParams, NoteVersionResponse,
};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use feature_notes::models::{NoteRow, NoteVersionRow};
use feature_notes::repo::utc_now_str;

use super::converters::{
    extract_links_and_mentions, note_row_to_response, note_with_tags, notes_with_tags_batch,
    version_row_to_response,
};
use crate::errors::map_storage_err;
use crate::state::{AppCore, EntityUpdate, HandlerResult};

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
            icon: None,
            color: None,
            embedding_updated_at: None,
            split_content: None,
            split_mode: None,
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

        // Extract links and mentions if the note has a body (e.g. wiki-link creation)
        if !created.body.is_empty() {
            extract_links_and_mentions(self, &id, &created).await?;
        }

        // Emit domain events for timeline tracking + BookIndex
        if let Ok(bus) = self.domain_event_bus() {
            bus.publish(bus::DomainEvent::NoteCreated {
                note_id: id.clone(),
                title: created.title.clone(),
            });
            if !created.body.is_empty() {
                bus.publish(bus::DomainEvent::NoteContentChanged {
                    note_id: id.clone(),
                    content: created.body.clone(),
                });
            }
        }

        // Fire-and-forget embedding for the new note
        if let Some(ref handler) = self.note_embedding_handler {
            let handler = Arc::clone(handler);
            let note_row = created.clone();
            let repo = self.note_repo.clone();
            tokio::spawn(async move {
                if let Err(e) = handler.embed_note(&note_row).await {
                    tracing::warn!("note embedding failed (non-fatal): {e}");
                } else {
                    let _ = repo.update_embedding_timestamp(&note_row.id).await;
                }
            });
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
                params.icon.as_ref().map(|o| o.as_deref()),
                params.color.as_ref().map(|o| o.as_deref()),
                params.split_content.as_ref().map(|o| o.as_deref()),
                params.split_mode.as_ref().map(|o| o.as_deref()),
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

        // Emit domain events for timeline tracking + BookIndex
        if let Ok(bus) = self.domain_event_bus() {
            bus.publish(bus::DomainEvent::NoteUpdated {
                note_id: params.id.clone(),
                title: updated.title.clone(),
            });
            if params.body.is_some() || params.body_html.is_some() {
                bus.publish(bus::DomainEvent::NoteContentChanged {
                    note_id: params.id.clone(),
                    content: updated.body.clone(),
                });
            }
        }

        // Fire-and-forget embedding for the updated note
        if let Some(ref handler) = self.note_embedding_handler {
            let handler = Arc::clone(handler);
            let note_row = updated.clone();
            let repo = self.note_repo.clone();
            tokio::spawn(async move {
                if let Err(e) = handler.embed_note(&note_row).await {
                    tracing::warn!("note embedding failed (non-fatal): {e}");
                } else {
                    let _ = repo.update_embedding_timestamp(&note_row.id).await;
                }
            });
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

        if deleted {
            if let Ok(bus) = self.domain_event_bus() {
                bus.publish(bus::DomainEvent::NoteDeleted {
                    note_id: id.clone(),
                });
            }
        }

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

        // Prune old versions
        let max_versions = {
            let config = self.config.read().await;
            config.notes.max_versions_per_note as i64
        };
        if let Err(e) = self.note_repo.prune_versions(&note_id, max_versions).await {
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
            .update_note(
                &note_id,
                None,
                Some(&version.body),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .map_err(map_storage_err)?;

        let response = note_with_tags(self, &updated).await?;
        let updates = vec![EntityUpdate {
            kind: EntityKind::Note,
            id: note_id,
        }];
        Ok((response, updates))
    }

    // ── Hybrid search (FTS5 + semantic) ────────────────────────────

    /// Hybrid search: FTS5 keyword search + semantic vector search, merged by RRF-like scoring.
    pub async fn note_search_hybrid(&self, query: &str) -> Result<HybridSearchResponse, ApiError> {
        // 1. FTS5 keyword search (always available)
        let keyword_results = self.note_repo.search_notes(query).await.unwrap_or_default();
        let keyword_notes = notes_with_tags_batch(self, &keyword_results).await?;

        // 2. Semantic search (only if embedding handler available)
        let semantic_notes = if let Some(ref handler) = self.note_embedding_handler {
            match handler.embed_query(query).await {
                Ok(query_vec) => {
                    match handler.search_similar(&query_vec, 10, 0.35).await {
                        Ok(results) => {
                            let keyword_ids: std::collections::HashSet<&str> =
                                keyword_notes.iter().map(|n| n.id.as_str()).collect();
                            let mut notes = Vec::new();
                            for (note_id, _score) in results {
                                // Skip notes already in keyword results
                                if keyword_ids.contains(note_id.as_str()) {
                                    continue;
                                }
                                if let Ok(Some(row)) = self.note_repo.get_note(&note_id).await {
                                    let tags =
                                        self.note_repo.get_tags(&note_id).await.unwrap_or_default();
                                    notes.push(note_row_to_response(&row, tags));
                                }
                            }
                            notes
                        }
                        Err(_) => Vec::new(),
                    }
                }
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };

        Ok(HybridSearchResponse {
            exact: keyword_notes,
            related: semantic_notes,
        })
    }

    // ── Semantic search ─────────────────────────────────────────────

    pub async fn note_search_semantic(&self, query: &str) -> Result<Vec<NoteResponse>, ApiError> {
        let handler = self
            .note_embedding_handler
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "semantic search not available"))?;

        let query_vec = handler
            .embed_query(query)
            .await
            .map_err(|e| ApiError::new("EMBEDDING_ERROR", e.to_string()))?;

        let results = handler
            .search_similar(&query_vec, 20, 0.3)
            .await
            .map_err(|e| ApiError::new("SEARCH_ERROR", e.to_string()))?;

        let mut notes = Vec::new();
        for (note_id, _score) in results {
            if let Ok(Some(row)) = self.note_repo.get_note(&note_id).await {
                let tags = self.note_repo.get_tags(&note_id).await.unwrap_or_default();
                notes.push(note_row_to_response(&row, tags));
            }
        }
        Ok(notes)
    }

    // ── Archive handlers ─────────────────────────────────────────────

    pub async fn note_archive(&self, id: &str) -> HandlerResult<()> {
        self.note_repo
            .archive_note(id)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Note,
            id: id.to_string(),
        }];
        Ok(((), updates))
    }

    pub async fn note_unarchive(&self, id: &str) -> HandlerResult<()> {
        self.note_repo
            .unarchive_note(id)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Note,
            id: id.to_string(),
        }];
        Ok(((), updates))
    }

    pub async fn note_list_archived(&self) -> Result<Vec<NoteResponse>, ApiError> {
        let rows = self
            .note_repo
            .list_archived_notes()
            .await
            .map_err(map_storage_err)?;

        notes_with_tags_batch(self, &rows).await
    }

    // ── Unlinked mentions handler ────────────────────────────────────

    pub async fn note_unlinked_mentions(
        &self,
        note_id: &str,
    ) -> Result<Vec<NoteResponse>, ApiError> {
        let rows = self
            .note_repo
            .get_unlinked_mentions(note_id)
            .await
            .map_err(map_storage_err)?;

        notes_with_tags_batch(self, &rows).await
    }

    // ── Backlinks handler ───────────────────────────────────────────

    pub async fn note_backlinks(&self, note_id: &str) -> Result<Vec<BacklinkResponse>, ApiError> {
        let rows = self
            .note_repo
            .get_backlinks_with_context(note_id)
            .await
            .map_err(map_storage_err)?;

        let mut results = Vec::with_capacity(rows.len());
        for (row, context) in &rows {
            let note = note_with_tags(self, row).await?;
            results.push(BacklinkResponse {
                note,
                context: context.clone(),
            });
        }
        Ok(results)
    }

    // ── Attachment handler ──────────────────────────────────────────

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
}
