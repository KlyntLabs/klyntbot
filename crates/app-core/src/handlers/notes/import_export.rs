use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use desktop_shared::commands::{NoteImportParams, NoteImportResult, SkippedFile};
use desktop_shared::errors::ApiError;
use feature_notes::front_matter;
use feature_notes::models::{NoteRow, NotebookRow};
use feature_notes::repo::utc_now_str;

use crate::errors::map_storage_err;
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
                body: parsed.body,
                body_html: None,
                body_json: None,
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
                "INSERT INTO notes (id, notebook_id, title, body, body_html, body_json, pinned, archived, icon, color, embedding_updated_at, split_content, split_mode, perspective_config, last_visited_at, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            )
            .bind(&row.id)
            .bind(&row.notebook_id)
            .bind(&row.title)
            .bind(&row.body)
            .bind(&row.body_html)
            .bind(&row.body_json)
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
            Ok(None) => {
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
                self.note_repo
                    .create_notebook(&nb)
                    .await
                    .map_err(map_storage_err)?;
                nb.id
            }
            Err(e) => {
                return Err(ApiError::new(
                    "STORAGE_ERROR",
                    format!("failed to look up notebook: {e}"),
                ));
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

    pub async fn note_export(
        &self,
        params: desktop_shared::commands::NoteExportParams,
    ) -> Result<desktop_shared::commands::NoteExportResult, ApiError> {
        use desktop_shared::commands::NoteExportResult;

        // Validate
        let dest = PathBuf::from(&params.destination);
        if !dest.is_absolute() {
            return Err(ApiError::new(
                "VALIDATION",
                "Destination must be an absolute path",
            ));
        }
        if dest
            .components()
            .any(|c| c == std::path::Component::ParentDir)
        {
            return Err(ApiError::new("VALIDATION", "Path traversal not allowed"));
        }

        let has_notes = params.note_ids.as_ref().is_some_and(|v| !v.is_empty());
        let has_notebooks = params.notebook_ids.as_ref().is_some_and(|v| !v.is_empty());
        if !has_notes && !has_notebooks {
            return Err(ApiError::new(
                "VALIDATION",
                "At least one of noteIds or notebookIds must be provided",
            ));
        }

        // Collect notes: Vec<(NoteRow, Option<String>)> where String is subdirectory path
        let mut notes_with_subdir: Vec<(NoteRow, Option<String>)> = Vec::new();

        if let Some(ids) = &params.note_ids {
            for id in ids {
                if let Some(row) = self.note_repo.get_note(id).await.map_err(map_storage_err)? {
                    notes_with_subdir.push((row, None));
                }
            }
        }

        if let Some(nb_ids) = &params.notebook_ids {
            let all_notebooks = self
                .note_repo
                .list_notebooks()
                .await
                .map_err(map_storage_err)?;
            for nb_id in nb_ids {
                self.collect_notebook_notes_for_export(
                    nb_id,
                    &all_notebooks,
                    &mut notes_with_subdir,
                )
                .await?;
            }
        }

        // Get data_dir from config (same pattern as note_save_attachment in crud.rs)
        let config = self.config.read().await;
        let data_dir = config.data_dir_path();
        drop(config);
        let attachment_prefix = format!("{}/attachments/", data_dir.display());
        let mut exported = 0u32;

        let note_ids: Vec<String> = notes_with_subdir
            .iter()
            .map(|(n, _)| n.id.clone())
            .collect();
        let tags_map = self
            .note_repo
            .get_tags_batch(&note_ids)
            .await
            .map_err(map_storage_err)?;

        for (note, subdir) in &notes_with_subdir {
            let tags = tags_map.get(&note.id).cloned().unwrap_or_default();
            let fm = front_matter::NoteFrontMatter {
                title: Some(note.title.clone()),
                tags: if tags.is_empty() { None } else { Some(tags) },
                created: Some(note.created_at.clone()),
                updated: Some(note.updated_at.clone()),
                pinned: if note.pinned != 0 { Some(true) } else { None },
                icon: note.icon.clone(),
                color: note.color.clone(),
            };

            // Handle attachments: rewrite absolute paths to relative
            let mut body = note.body.clone();
            if body.contains(&attachment_prefix) {
                let out_attachments = match subdir {
                    Some(sd) => dest.join(sd).join("attachments"),
                    None => dest.join("attachments"),
                };
                let _ = std::fs::create_dir_all(&out_attachments);
                // Find and replace each attachment reference
                let mut search_from = 0;
                while let Some(start) = body[search_from..].find(&attachment_prefix) {
                    let abs_start = search_from + start;
                    let after = &body[abs_start + attachment_prefix.len()..];
                    let end = after.find([')', '"', ' ']).unwrap_or(after.len());
                    let filename = after[..end].to_string();
                    let src = data_dir.join("attachments").join(&filename);
                    if src.exists() {
                        let _ = std::fs::copy(&src, out_attachments.join(&filename));
                    }
                    let abs_ref = format!("{}{}", attachment_prefix, filename);
                    let rel_ref = format!("./attachments/{}", filename);
                    body = body.replacen(&abs_ref, &rel_ref, 1);
                    search_from = abs_start + rel_ref.len();
                }
            }

            let content = front_matter::serialize(&fm, &body);

            // Determine output path
            let out_dir = match subdir {
                Some(sd) => dest.join(sd),
                None => dest.clone(),
            };
            let _ = std::fs::create_dir_all(&out_dir);

            let filename = match (&params.output_filename, notes_with_subdir.len()) {
                (Some(name), 1) => name.clone(),
                _ => slugify_filename(&note.title, &out_dir),
            };

            let out_path = out_dir.join(&filename);
            tokio::fs::write(&out_path, content).await.map_err(|e| {
                ApiError::new(
                    "IO_ERROR",
                    format!("Failed to write {}: {e}", out_path.display()),
                )
            })?;

            exported += 1;
        }

        Ok(NoteExportResult { exported })
    }

    /// Recursively collect notes from a notebook with their subdirectory paths for export.
    async fn collect_notebook_notes_for_export(
        &self,
        notebook_id: &str,
        all_notebooks: &[NotebookRow],
        notes: &mut Vec<(NoteRow, Option<String>)>,
    ) -> Result<(), ApiError> {
        let nb_map: std::collections::HashMap<String, &NotebookRow> =
            all_notebooks.iter().map(|nb| (nb.id.clone(), nb)).collect();

        // Build path from notebook hierarchy using raw titles (not slugified)
        // so that export → import round-trip deduplicates correctly
        fn build_path(
            nb_id: &str,
            nb_map: &std::collections::HashMap<String, &NotebookRow>,
        ) -> String {
            let mut parts = vec![];
            let mut current = Some(nb_id.to_string());
            while let Some(id) = current {
                if let Some(nb) = nb_map.get(&id) {
                    parts.push(nb.title.clone());
                    current = nb.parent_id.clone();
                } else {
                    break;
                }
            }
            parts.reverse();
            parts.join(std::path::MAIN_SEPARATOR_STR)
        }

        // Collect notes in this notebook
        let rows = self
            .note_repo
            .list_notes(Some(notebook_id))
            .await
            .map_err(map_storage_err)?;
        let subdir = build_path(notebook_id, &nb_map);
        for row in rows {
            notes.push((row, Some(subdir.clone())));
        }

        // Recurse into child notebooks
        for nb in all_notebooks {
            if nb.parent_id.as_deref() == Some(notebook_id) {
                Box::pin(self.collect_notebook_notes_for_export(&nb.id, all_notebooks, notes))
                    .await?;
            }
        }

        Ok(())
    }
}

/// Create a URL-safe filename slug with collision handling.
fn slugify_filename(title: &str, dir: &Path) -> String {
    let base = title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");

    let base = if base.is_empty() {
        "untitled".to_string()
    } else {
        base
    };

    let candidate = format!("{}.md", base);
    if !dir.join(&candidate).exists() {
        return candidate;
    }
    for i in 1..1000 {
        let candidate = format!("{}-{}.md", base, i);
        if !dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{}-{}.md", base, uuid::Uuid::new_v4())
}
