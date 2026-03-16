use storage::StorageError;

use super::{nullable_to_sentinel, NoteRepo};
use crate::models::{NoteRow, NoteSearchResult};

impl NoteRepo {
    // ── Notes ────────────────────────────────────────

    pub async fn create_note(&self, row: &NoteRow) -> Result<NoteRow, StorageError> {
        let result = sqlx::query_as::<_, NoteRow>(
            "INSERT INTO notes (id, notebook_id, title, body, body_html, pinned, archived, icon, embedding_updated_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             RETURNING *",
        )
        .bind(&row.id)
        .bind(&row.notebook_id)
        .bind(&row.title)
        .bind(&row.body)
        .bind(&row.body_html)
        .bind(row.pinned)
        .bind(row.archived)
        .bind(&row.icon)
        .bind(&row.embedding_updated_at)
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

    pub async fn update_note(
        &self,
        id: &str,
        title: Option<&str>,
        body: Option<&str>,
        body_html: Option<&str>,
        pinned: Option<bool>,
        notebook_id: Option<Option<&str>>,
        icon: Option<Option<&str>>,
    ) -> Result<NoteRow, StorageError> {
        let nb_sentinel = nullable_to_sentinel(notebook_id);
        let icon_sentinel = nullable_to_sentinel(icon);
        let row = sqlx::query_as::<_, NoteRow>(
            "UPDATE notes SET
                title = COALESCE(?2, title),
                body = COALESCE(?3, body),
                body_html = COALESCE(?4, body_html),
                pinned = COALESCE(?5, pinned),
                notebook_id = CASE
                    WHEN ?6 IS NULL THEN notebook_id
                    WHEN ?6 = '' THEN NULL
                    ELSE ?6
                END,
                icon = CASE
                    WHEN ?7 IS NULL THEN icon
                    WHEN ?7 = '' THEN NULL
                    ELSE ?7
                END,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?1
             RETURNING *",
        )
        .bind(id)
        .bind(title)
        .bind(body)
        .bind(body_html)
        .bind(pinned.map(|p| p as i32))
        .bind(nb_sentinel)
        .bind(icon_sentinel)
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
               SELECT n.id, n.notebook_id, n.title, n.body, n.body_html,
                      n.pinned, n.archived, n.icon, n.embedding_updated_at,
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
             SELECT id, notebook_id, title, body, body_html,
                    pinned, archived, icon, embedding_updated_at, created_at, updated_at
             FROM scored
             WHERE score > 0
             ORDER BY score DESC, updated_at DESC",
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Full-text search using FTS5 with BM25 ranking.
    /// Title matches are weighted 5x, body matches 1x.
    /// Returns results sorted by relevance (highest first).
    pub async fn search_fts(&self, query: &str) -> Result<Vec<NoteSearchResult>, StorageError> {
        let rows = sqlx::query_as::<_, NoteSearchResult>(
            "SELECT n.id, n.notebook_id, n.title, n.body, n.body_html,
                    n.pinned, n.archived, n.icon, n.embedding_updated_at,
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
