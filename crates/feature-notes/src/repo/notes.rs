use storage::StorageError;

use super::{nullable_to_sentinel, NoteRepo};
use crate::models::{NoteRow, NoteSearchResult};

impl NoteRepo {
    // ── Notes ────────────────────────────────────────

    pub async fn create_note(&self, row: &NoteRow) -> Result<NoteRow, StorageError> {
        let result = sqlx::query_as::<_, NoteRow>(
            "INSERT INTO notes (id, notebook_id, title, body, body_html, body_json, pinned, archived, icon, color, embedding_updated_at, split_content, split_mode, perspective_config, last_visited_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
             RETURNING *",
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
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    pub async fn get_note(&self, id: &str) -> Result<Option<NoteRow>, StorageError> {
        let result = sqlx::query_as::<_, NoteRow>("SELECT * FROM notes WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(result)
    }

    pub async fn list_notes(
        &self,
        notebook_id: Option<&str>,
    ) -> Result<Vec<NoteRow>, StorageError> {
        let rows = sqlx::query_as::<_, NoteRow>(
            "SELECT * FROM notes
             WHERE (notebook_id = ?1 OR ?1 IS NULL) AND archived = 0
             ORDER BY pinned DESC, updated_at DESC",
        )
        .bind(notebook_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Fetch notes created or updated within a date range (for timeline display).
    pub async fn notes_in_date_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<NoteRow>, StorageError> {
        let end_bound = format!("{end_date}T23:59:59Z");
        let rows = sqlx::query_as::<_, NoteRow>(
            r#"
            SELECT * FROM notes
            WHERE archived = 0
              AND ((created_at >= ?1 AND created_at < ?2)
                   OR (updated_at >= ?1 AND updated_at < ?2))
            ORDER BY COALESCE(updated_at, created_at) ASC
            "#,
        )
        .bind(start_date)
        .bind(&end_bound)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_note(
        &self,
        id: &str,
        title: Option<&str>,
        body: Option<&str>,
        body_html: Option<&str>,
        body_json: Option<&str>,
        pinned: Option<bool>,
        notebook_id: Option<Option<&str>>,
        icon: Option<Option<&str>>,
        color: Option<Option<&str>>,
        split_content: Option<Option<&str>>,
        split_mode: Option<Option<&str>>,
        perspective_config: Option<Option<&str>>,
    ) -> Result<NoteRow, StorageError> {
        let nb_sentinel = nullable_to_sentinel(notebook_id);
        let icon_sentinel = nullable_to_sentinel(icon);
        let color_sentinel = nullable_to_sentinel(color);
        let split_content_sentinel = nullable_to_sentinel(split_content);
        let split_mode_sentinel = nullable_to_sentinel(split_mode);
        let perspective_config_sentinel = nullable_to_sentinel(perspective_config);
        let row = sqlx::query_as::<_, NoteRow>(
            "UPDATE notes SET
                title = COALESCE(?2, title),
                body = COALESCE(?3, body),
                body_html = COALESCE(?4, body_html),
                body_json = COALESCE(?5, body_json),
                pinned = COALESCE(?6, pinned),
                notebook_id = CASE
                    WHEN ?7 IS NULL THEN notebook_id
                    WHEN ?7 = '' THEN NULL
                    ELSE ?7
                END,
                icon = CASE
                    WHEN ?8 IS NULL THEN icon
                    WHEN ?8 = '' THEN NULL
                    ELSE ?8
                END,
                color = CASE
                    WHEN ?9 IS NULL THEN color
                    WHEN ?9 = '' THEN NULL
                    ELSE ?9
                END,
                split_content = CASE
                    WHEN ?10 IS NULL THEN split_content
                    WHEN ?10 = '' THEN NULL
                    ELSE ?10
                END,
                split_mode = CASE
                    WHEN ?11 IS NULL THEN split_mode
                    WHEN ?11 = '' THEN NULL
                    ELSE ?11
                END,
                perspective_config = CASE
                    WHEN ?12 IS NULL THEN perspective_config
                    WHEN ?12 = '' THEN NULL
                    ELSE ?12
                END,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?1
             RETURNING *",
        )
        .bind(id)
        .bind(title)
        .bind(body)
        .bind(body_html)
        .bind(body_json)
        .bind(pinned.map(|p| p as i32))
        .bind(nb_sentinel)
        .bind(icon_sentinel)
        .bind(color_sentinel)
        .bind(split_content_sentinel)
        .bind(split_mode_sentinel)
        .bind(perspective_config_sentinel)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn delete_note(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM notes WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List notes with optional filters and pagination.
    ///
    /// When `tags` is `Some`, only notes matching **all** given tags are returned (AND logic).
    pub async fn list_notes_paginated(
        &self,
        notebook_id: Option<&str>,
        tags: Option<&[String]>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<NoteRow>, StorageError> {
        let mut sql = String::from("SELECT * FROM notes WHERE archived = 0");
        let mut bind_index: usize = 1;

        if notebook_id.is_some() {
            sql.push_str(&format!(" AND notebook_id = ?{bind_index}"));
            bind_index += 1;
        }

        if let Some(tags) = tags {
            for _ in tags {
                sql.push_str(&format!(
                    " AND id IN (SELECT note_id FROM note_tags WHERE tag = ?{bind_index})"
                ));
                bind_index += 1;
            }
        }

        sql.push_str(&format!(
            " ORDER BY pinned DESC, updated_at DESC LIMIT ?{} OFFSET ?{}",
            bind_index,
            bind_index + 1
        ));

        let mut query = sqlx::query_as::<_, NoteRow>(&sql);

        if let Some(nb) = notebook_id {
            query = query.bind(nb.to_string());
        }

        if let Some(tags) = tags {
            for tag in tags {
                query = query.bind(tag.clone());
            }
        }

        query = query.bind(limit).bind(offset);

        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Set `archived = 1` and update `updated_at`.
    pub async fn archive_note(&self, id: &str) -> Result<NoteRow, StorageError> {
        let row = sqlx::query_as::<_, NoteRow>(
            "UPDATE notes SET archived = 1, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?1
             RETURNING *",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Set `archived = 0` and update `updated_at`.
    pub async fn unarchive_note(&self, id: &str) -> Result<NoteRow, StorageError> {
        let row = sqlx::query_as::<_, NoteRow>(
            "UPDATE notes SET archived = 0, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?1
             RETURNING *",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List all archived notes, most recently updated first.
    pub async fn list_archived_notes(&self) -> Result<Vec<NoteRow>, StorageError> {
        let rows = sqlx::query_as::<_, NoteRow>(
            "SELECT * FROM notes WHERE archived = 1 ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Quick liveness check — verifies the notes table is accessible.
    pub async fn check_health(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1 FROM notes LIMIT 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Search notes by title, body, and tags using LIKE matching.
    /// Results are ranked by a weighted score: title matches (3) > tag matches (2) > body matches (1).
    /// Only non-archived notes with score > 0 are returned.
    pub async fn search_notes(&self, query: &str) -> Result<Vec<NoteRow>, StorageError> {
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let rows = sqlx::query_as::<_, NoteRow>(
            "WITH scored AS (
               SELECT n.id, n.notebook_id, n.title, n.body, n.body_html, n.body_json,
                      n.pinned, n.archived, n.icon, n.color, n.embedding_updated_at,
                      n.split_content, n.split_mode, n.perspective_config, n.last_visited_at,
                      n.created_at, n.updated_at,
                      (CASE WHEN n.title LIKE ?1 ESCAPE '\\' THEN 3 ELSE 0 END
                       + CASE WHEN n.body LIKE ?1 ESCAPE '\\' THEN 1 ELSE 0 END
                       + CASE WHEN EXISTS (
                           SELECT 1 FROM note_tags t
                           WHERE t.note_id = n.id AND t.tag LIKE ?1 ESCAPE '\\'
                         ) THEN 2 ELSE 0 END) AS score
               FROM notes n
               WHERE n.archived = 0
             )
             SELECT id, notebook_id, title, body, body_html, body_json,
                    pinned, archived, icon, color, embedding_updated_at,
                    split_content, split_mode, perspective_config, last_visited_at,
                    created_at, updated_at
             FROM scored
             WHERE score > 0
             ORDER BY score DESC, updated_at DESC",
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update the `embedding_updated_at` timestamp for a note after successful embedding.
    pub async fn update_embedding_timestamp(&self, note_id: &str) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE notes SET embedding_updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
        )
        .bind(note_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List notes that have no embedding yet (for background catch-up).
    pub async fn list_notes_needing_embedding(
        &self,
        limit: i64,
    ) -> Result<Vec<NoteRow>, StorageError> {
        let rows = sqlx::query_as::<_, NoteRow>(
            "SELECT * FROM notes WHERE embedding_updated_at IS NULL AND archived = 0 ORDER BY updated_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Find notes whose body contains the title of the given note as a phrase,
    /// but which don't have an explicit `[[wiki-link]]` to it.
    /// Skips titles shorter than 3 words AND shorter than 8 chars to avoid false positives.
    pub async fn get_unlinked_mentions(&self, note_id: &str) -> Result<Vec<NoteRow>, StorageError> {
        let note = sqlx::query_as::<_, NoteRow>("SELECT * FROM notes WHERE id = ?1")
            .bind(note_id)
            .fetch_optional(&self.pool)
            .await?;

        let note = match note {
            Some(n) => n,
            None => return Ok(vec![]),
        };

        let title = note.title.trim().to_string();

        // Skip short titles to avoid false positives
        let word_count = title.split_whitespace().count();
        if word_count < 3 && title.len() < 8 {
            return Ok(vec![]);
        }

        // FTS5 phrase matching: wrap title in double quotes for exact phrase match.
        // Escape any embedded double quotes by doubling them.
        let escaped = title.replace('"', "\"\"");
        let fts_query = format!("\"{escaped}\"");

        let rows = sqlx::query_as::<_, NoteRow>(
            r#"
            SELECT n.*
            FROM notes_fts fts
            JOIN notes n ON n.rowid = fts.rowid
            WHERE notes_fts MATCH ?1
              AND n.id != ?2
              AND n.archived = 0
              AND n.id NOT IN (
                SELECT source_id FROM note_links WHERE target_id = ?2
              )
            ORDER BY n.updated_at DESC
            LIMIT 20
            "#,
        )
        .bind(&fts_query)
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    /// Full-text search using FTS5 with BM25 ranking.
    /// Title matches are weighted 5x, body matches 1x.
    /// Returns results sorted by relevance (highest first).
    pub async fn search_fts(&self, query: &str) -> Result<Vec<NoteSearchResult>, StorageError> {
        let rows = sqlx::query_as::<_, NoteSearchResult>(
            "SELECT n.id, n.notebook_id, n.title, n.body, n.body_html, n.body_json,
                    n.pinned, n.archived, n.icon, n.color, n.embedding_updated_at,
                    n.split_content, n.split_mode, n.perspective_config, n.last_visited_at,
                    n.created_at, n.updated_at,
                    -bm25(notes_fts, 5.0, 1.0) AS rank
             FROM notes_fts fts
             JOIN notes n ON n.rowid = fts.rowid
             WHERE notes_fts MATCH ?1 AND n.archived = 0
             ORDER BY rank DESC",
        )
        .bind(query)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
