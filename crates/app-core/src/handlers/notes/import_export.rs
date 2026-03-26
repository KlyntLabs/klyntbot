use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use desktop_shared::commands::{NoteImportParams, NoteImportResult, SkippedFile};
use desktop_shared::errors::ApiError;
use feature_notes::front_matter;
use feature_notes::models::{NoteRow, NotebookRow};
use feature_notes::repo::utc_now_str;

use crate::state::AppCore;

const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB

struct CollectedFile {
    path: PathBuf,
    content: String,
    notebook_id: Option<String>,
}

impl AppCore {
    pub async fn note_import_files(
        &self,
        params: NoteImportParams,
    ) -> Result<NoteImportResult, ApiError> {
        let mut files: Vec<CollectedFile> = Vec::new();
        let mut skipped: Vec<SkippedFile> = Vec::new();
        #[cfg(unix)]
        let mut visited_inodes: HashSet<u64> = HashSet::new();

        // Phase 1: Validate paths and collect files
        for raw_path in &params.paths {
            let path = PathBuf::from(raw_path);

            // Reject non-absolute paths
            if !path.is_absolute() {
                skipped.push(SkippedFile {
                    path: raw_path.clone(),
                    reason: "path must be absolute".to_string(),
                });
                continue;
            }

            // Reject paths with parent-dir components (directory traversal)
            if path
                .components()
                .any(|c| c == std::path::Component::ParentDir)
            {
                skipped.push(SkippedFile {
                    path: raw_path.clone(),
                    reason: "path must not contain '..' components".to_string(),
                });
                continue;
            }

            // Canonicalize — broken symlinks get skipped (not abort)
            let canonical = match std::fs::canonicalize(&path) {
                Ok(p) => p,
                Err(e) => {
                    skipped.push(SkippedFile {
                        path: raw_path.clone(),
                        reason: format!("cannot resolve path: {e}"),
                    });
                    continue;
                }
            };

            if canonical.is_dir() {
                Box::pin(self.collect_dir(
                    &canonical,
                    params.notebook_id.clone(),
                    &mut files,
                    &mut skipped,
                    #[cfg(unix)]
                    &mut visited_inodes,
                ))
                .await?;
            } else {
                self.collect_file(
                    &canonical,
                    params.notebook_id.clone(),
                    &mut files,
                    &mut skipped,
                );
            }
        }

        if files.is_empty() {
            return Ok(NoteImportResult {
                imported: 0,
                skipped,
            });
        }

        // Phase 2: Bulk insert in a single transaction
        let mut tx = self.note_repo.pool().begin().await.map_err(|e| {
            ApiError::new("STORAGE_ERROR", format!("failed to begin transaction: {e}"))
        })?;

        let now = utc_now_str();
        let mut created_notes: Vec<(NoteRow, Option<Vec<String>>)> = Vec::new();

        for file in &files {
            let parsed = front_matter::parse(&file.content);
            let fm = parsed.front_matter.unwrap_or_default();

            // Title: front matter title > file stem
            let title = fm
                .title
                .or_else(|| {
                    file.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| "Untitled".to_string());

            let id = uuid::Uuid::new_v4().to_string();
            let created_at = fm.created.unwrap_or_else(|| now.clone());
            let updated_at = fm.updated.unwrap_or_else(|| now.clone());

            let row = NoteRow {
                id: id.clone(),
                notebook_id: file.notebook_id.clone(),
                title,
                body: parsed.body.clone(),
                body_html: None,
                pinned: fm.pinned.map_or(0, |p| if p { 1 } else { 0 }),
                archived: 0,
                icon: fm.icon,
                color: fm.color,
                embedding_updated_at: None,
                split_content: None,
                split_mode: None,
                perspective_config: None,
                last_visited_at: None,
                created_at,
                updated_at,
            };

            sqlx::query(
                "INSERT INTO notes (id, notebook_id, title, body, body_html, pinned, archived, icon, color, embedding_updated_at, split_content, split_mode, perspective_config, last_visited_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            )
            .bind(&row.id)
            .bind(&row.notebook_id)
            .bind(&row.title)
            .bind(&row.body)
            .bind(&row.body_html)
            .bind(row.pinned)
            .bind(row.archived)
            .bind(&row.icon)
            .bind(&row.color)
            .bind(&row.embedding_updated_at)
            .bind(&row.split_content)
            .bind(&row.split_mode)
            .bind(&row.perspective_config)
            .bind(&row.last_visited_at)
            .bind(&row.created_at)
            .bind(&row.updated_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| ApiError::new("STORAGE_ERROR", format!("failed to insert note: {e}")))?;

            // Insert tags within the same transaction
            if let Some(ref tags) = fm.tags {
                for tag in tags {
                    sqlx::query("INSERT OR IGNORE INTO note_tags (note_id, tag) VALUES (?1, ?2)")
                        .bind(&id)
                        .bind(tag)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| {
                            ApiError::new("STORAGE_ERROR", format!("failed to insert tag: {e}"))
                        })?;
                }
            }

            created_notes.push((row, fm.tags));
        }

        let imported = created_notes.len() as u32;

        tx.commit().await.map_err(|e| {
            ApiError::new(
                "STORAGE_ERROR",
                format!("failed to commit transaction: {e}"),
            )
        })?;

        // Phase 3: Post-commit — fire domain events and queue embeddings
        if let Ok(bus) = self.domain_event_bus() {
            for (note, _) in &created_notes {
                bus.publish(bus::DomainEvent::NoteCreated {
                    note_id: note.id.clone(),
                    title: note.title.clone(),
                });
                if !note.body.is_empty() {
                    bus.publish(bus::DomainEvent::NoteContentChanged {
                        note_id: note.id.clone(),
                        content: note.body.clone(),
                    });
                }
            }
        }

        // Fire-and-forget embeddings for all imported notes
        if let Some(ref handler) = self.note_embedding_handler {
            for (note, _) in created_notes {
                let handler = Arc::clone(handler);
                let repo = self.note_repo.clone();
                let note_row = note;
                tokio::spawn(async move {
                    if let Err(e) = handler.embed_note(&note_row).await {
                        tracing::warn!(note_id = %note_row.id, "import embedding failed (non-fatal): {e}");
                    } else {
                        let _ = repo.update_embedding_timestamp(&note_row.id).await;
                    }
                });
            }
        }

        Ok(NoteImportResult { imported, skipped })
    }

    fn collect_file(
        &self,
        path: &Path,
        notebook_id: Option<String>,
        files: &mut Vec<CollectedFile>,
        skipped: &mut Vec<SkippedFile>,
    ) {
        let path_str = path.to_string_lossy().to_string();

        // Check .md extension (case-insensitive)
        let is_md = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("md"));

        if !is_md {
            skipped.push(SkippedFile {
                path: path_str,
                reason: "not a .md file".to_string(),
            });
            return;
        }

        // Check file size
        match std::fs::metadata(path) {
            Ok(meta) => {
                if meta.len() > MAX_FILE_SIZE {
                    skipped.push(SkippedFile {
                        path: path_str,
                        reason: format!(
                            "file too large ({} bytes, max {})",
                            meta.len(),
                            MAX_FILE_SIZE
                        ),
                    });
                    return;
                }
            }
            Err(e) => {
                skipped.push(SkippedFile {
                    path: path_str,
                    reason: format!("cannot read metadata: {e}"),
                });
                return;
            }
        }

        // Read file content
        match std::fs::read_to_string(path) {
            Ok(content) => {
                files.push(CollectedFile {
                    path: path.to_path_buf(),
                    content,
                    notebook_id,
                });
            }
            Err(e) => {
                skipped.push(SkippedFile {
                    path: path_str,
                    reason: format!("cannot read file: {e}"),
                });
            }
        }
    }

    async fn collect_dir(
        &self,
        dir: &Path,
        parent_notebook_id: Option<String>,
        files: &mut Vec<CollectedFile>,
        skipped: &mut Vec<SkippedFile>,
        #[cfg(unix)] visited_inodes: &mut HashSet<u64>,
    ) -> Result<(), ApiError> {
        // Inode cycle detection (unix only)
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(meta) = std::fs::metadata(dir) {
                if !visited_inodes.insert(meta.ino()) {
                    // Already visited this directory — skip to avoid cycles
                    return Ok(());
                }
            }
        }

        let dir_name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();

        // Deduplicate notebooks: reuse existing or create new
        let notebook_id = match self
            .note_repo
            .find_notebook_by_parent_and_title(parent_notebook_id.as_deref(), &dir_name)
            .await
        {
            Ok(Some(existing)) => existing.id,
            _ => {
                let now = utc_now_str();
                let nb = NotebookRow {
                    id: uuid::Uuid::new_v4().to_string(),
                    parent_id: parent_notebook_id,
                    title: dir_name,
                    icon: None,
                    color: None,
                    sort_order: 0,
                    created_at: now.clone(),
                    updated_at: now,
                };
                self.note_repo.create_notebook(&nb).await.map_err(|e| {
                    ApiError::new("STORAGE_ERROR", format!("failed to create notebook: {e}"))
                })?;
                nb.id
            }
        };

        // Read directory entries
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                skipped.push(SkippedFile {
                    path: dir.to_string_lossy().to_string(),
                    reason: format!("cannot read directory: {e}"),
                });
                return Ok(());
            }
        };

        let mut subdirs = Vec::new();
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!("skipping unreadable dir entry: {e}");
                    continue;
                }
            };

            let entry_path = entry.path();

            // Canonicalize to resolve symlinks
            let canonical = match std::fs::canonicalize(&entry_path) {
                Ok(p) => p,
                Err(e) => {
                    skipped.push(SkippedFile {
                        path: entry_path.to_string_lossy().to_string(),
                        reason: format!("cannot resolve path: {e}"),
                    });
                    continue;
                }
            };

            if canonical.is_dir() {
                subdirs.push(canonical);
            } else {
                self.collect_file(&canonical, Some(notebook_id.clone()), files, skipped);
            }
        }

        // Recurse into subdirectories
        for subdir in subdirs {
            Box::pin(self.collect_dir(
                &subdir,
                Some(notebook_id.clone()),
                files,
                skipped,
                #[cfg(unix)]
                visited_inodes,
            ))
            .await?;
        }

        Ok(())
    }
}
