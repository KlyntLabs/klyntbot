//! Repository for notes, notebooks, tags, links, and version history.

use std::collections::HashMap;

use sqlx::SqlitePool;
use storage::StorageError;

use crate::models::*;

/// Convert a tri-state `Option<Option<&str>>` to a sentinel value for SQL:
/// - `None` → `None`  (bind NULL → COALESCE keeps existing)
/// - `Some(None)` → `Some("")` (bind "" → CASE sets NULL)
/// - `Some(Some(v))` → `Some(v)` (bind v → CASE sets v)
fn nullable_to_sentinel(v: Option<Option<&str>>) -> Option<&str> {
    v.map(|opt| opt.unwrap_or(""))
}

/// UTC "now" as an ISO-8601 string (`YYYY-MM-DDTHH:MM:SSZ`).
pub fn utc_now_str() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

#[derive(Debug, Clone)]
pub struct NoteRepo {
    pool: SqlitePool,
}

impl NoteRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ── Notes ────────────────────────────────────────

    pub async fn create_note(&self, row: &NoteRow) -> Result<NoteRow, StorageError> {
        let result = sqlx::query_as::<_, NoteRow>(
            "INSERT INTO notes (id, notebook_id, title, body, body_html, pinned, archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             RETURNING *",
        )
        .bind(&row.id)
        .bind(&row.notebook_id)
        .bind(&row.title)
        .bind(&row.body)
        .bind(&row.body_html)
        .bind(row.pinned)
        .bind(row.archived)
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
    ) -> Result<NoteRow, StorageError> {
        let nb_sentinel = nullable_to_sentinel(notebook_id);
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
                      n.pinned, n.archived, n.created_at, n.updated_at,
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
                    pinned, archived, created_at, updated_at
             FROM scored
             WHERE score > 0
             ORDER BY score DESC, updated_at DESC",
        )
        .bind(&pattern)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // ── Tags ─────────────────────────────────────────

    pub async fn get_tags(&self, note_id: &str) -> Result<Vec<String>, StorageError> {
        let rows: Vec<(String,)> =
            sqlx::query_as("SELECT tag FROM note_tags WHERE note_id = ?1 ORDER BY tag")
                .bind(note_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Fetch tags for multiple notes in a single query. Returns a map of note_id → tags.
    pub async fn get_tags_batch(
        &self,
        note_ids: &[String],
    ) -> Result<HashMap<String, Vec<String>>, StorageError> {
        if note_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let placeholders: Vec<String> = note_ids
            .iter()
            .enumerate()
            .map(|(i, _)| format!("?{}", i + 1))
            .collect();
        let sql = format!(
            "SELECT note_id, tag FROM note_tags WHERE note_id IN ({}) ORDER BY tag",
            placeholders.join(", ")
        );
        let mut query = sqlx::query_as::<_, (String, String)>(&sql);
        for id in note_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(&self.pool).await?;
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (note_id, tag) in rows {
            map.entry(note_id).or_default().push(tag);
        }
        Ok(map)
    }

    pub async fn set_tags(&self, note_id: &str, tags: &[String]) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM note_tags WHERE note_id = ?1")
            .bind(note_id)
            .execute(&mut *tx)
            .await?;
        for tag in tags {
            sqlx::query("INSERT INTO note_tags (note_id, tag) VALUES (?1, ?2)")
                .bind(note_id)
                .bind(tag)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    // ── Notebooks ────────────────────────────────────

    pub async fn create_notebook(&self, row: &NotebookRow) -> Result<NotebookRow, StorageError> {
        let result = sqlx::query_as::<_, NotebookRow>(
            "INSERT INTO notebooks (id, parent_id, title, icon, sort_order, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             RETURNING *",
        )
        .bind(&row.id)
        .bind(&row.parent_id)
        .bind(&row.title)
        .bind(&row.icon)
        .bind(row.sort_order)
        .bind(&row.created_at)
        .bind(&row.updated_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    pub async fn list_notebooks(&self) -> Result<Vec<NotebookRow>, StorageError> {
        let rows =
            sqlx::query_as::<_, NotebookRow>("SELECT * FROM notebooks ORDER BY sort_order, title")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn update_notebook(
        &self,
        id: &str,
        title: Option<&str>,
        icon: Option<&str>,
        parent_id: Option<Option<&str>>,
    ) -> Result<NotebookRow, StorageError> {
        let pid_sentinel = nullable_to_sentinel(parent_id);
        let row = sqlx::query_as::<_, NotebookRow>(
            "UPDATE notebooks SET
                title = COALESCE(?2, title),
                icon = COALESCE(?3, icon),
                parent_id = CASE
                    WHEN ?4 IS NULL THEN parent_id
                    WHEN ?4 = '' THEN NULL
                    ELSE ?4
                END,
                updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
             WHERE id = ?1
             RETURNING *",
        )
        .bind(id)
        .bind(title)
        .bind(icon)
        .bind(pid_sentinel)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn count_notes_in_notebook(&self, notebook_id: &str) -> Result<i64, StorageError> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM notes WHERE notebook_id = ?1 AND archived = 0")
                .bind(notebook_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(count.0)
    }

    /// Count notes per notebook in a single query. Returns notebook_id → count.
    pub async fn count_notes_by_notebook(&self) -> Result<HashMap<String, i64>, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT notebook_id, COUNT(*) FROM notes WHERE notebook_id IS NOT NULL AND archived = 0 GROUP BY notebook_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    pub async fn delete_notebook(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM notebooks WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Check if setting `notebook_id`'s parent to `proposed_parent_id` would create a cycle.
    /// Returns `true` if a cycle would be created.
    pub async fn would_create_cycle(
        &self,
        notebook_id: &str,
        proposed_parent_id: &str,
    ) -> Result<bool, StorageError> {
        // Trivial self-cycle
        if notebook_id == proposed_parent_id {
            return Ok(true);
        }
        // Walk up the ancestor chain from proposed_parent_id.
        // If we encounter notebook_id, setting it as parent would create a cycle.
        let result = sqlx::query_scalar::<_, i32>(
            "WITH RECURSIVE ancestors(nid) AS (
                SELECT parent_id FROM notebooks WHERE id = ?1
                UNION ALL
                SELECT nb.parent_id FROM notebooks nb JOIN ancestors a ON nb.id = a.nid
                WHERE nb.parent_id IS NOT NULL
            )
            SELECT 1 FROM ancestors WHERE nid = ?2 LIMIT 1",
        )
        .bind(proposed_parent_id)
        .bind(notebook_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.is_some())
    }

    // ── Entity Mentions ──────────────────────────

    pub async fn set_entity_mentions(
        &self,
        note_id: &str,
        mentions: &[(String, String)], // (entity_type, entity_id)
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM note_entity_mentions WHERE note_id = ?1")
            .bind(note_id)
            .execute(&mut *tx)
            .await?;
        for (entity_type, entity_id) in mentions {
            sqlx::query(
                "INSERT INTO note_entity_mentions (note_id, entity_type, entity_id) VALUES (?1, ?2, ?3)",
            )
            .bind(note_id)
            .bind(entity_type)
            .bind(entity_id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Find notes that mention a specific entity (task, project, etc.).
    pub async fn list_notes_by_entity(
        &self,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<NoteRow>, StorageError> {
        let rows = sqlx::query_as::<_, NoteRow>(
            "SELECT n.* FROM notes n
             INNER JOIN note_entity_mentions m ON m.note_id = n.id
             WHERE m.entity_type = ?1 AND m.entity_id = ?2 AND n.archived = 0
             ORDER BY n.updated_at DESC",
        )
        .bind(entity_type)
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Find note IDs by their titles (case-insensitive). Returns (title, id) pairs.
    pub async fn resolve_titles_to_ids(
        &self,
        titles: &[String],
    ) -> Result<Vec<(String, String)>, StorageError> {
        if titles.is_empty() {
            return Ok(vec![]);
        }
        // Build dynamic IN clause
        let placeholders: Vec<String> = titles
            .iter()
            .enumerate()
            .map(|(i, _)| format!("LOWER(?{})", i + 1))
            .collect();
        let sql = format!(
            "SELECT title, id FROM notes WHERE LOWER(title) IN ({}) AND archived = 0",
            placeholders.join(", ")
        );
        let mut query = sqlx::query_as::<_, (String, String)>(&sql);
        for title in titles {
            query = query.bind(title.to_lowercase());
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    // ── Links ────────────────────────────────────────

    pub async fn set_links(
        &self,
        source_id: &str,
        target_ids: &[String],
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM note_links WHERE source_id = ?1")
            .bind(source_id)
            .execute(&mut *tx)
            .await?;
        for target_id in target_ids {
            sqlx::query("INSERT INTO note_links (source_id, target_id) VALUES (?1, ?2)")
                .bind(source_id)
                .bind(target_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn get_links_from(&self, source_id: &str) -> Result<Vec<NoteLinkRow>, StorageError> {
        let rows =
            sqlx::query_as::<_, NoteLinkRow>("SELECT * FROM note_links WHERE source_id = ?1")
                .bind(source_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn get_links_to(&self, target_id: &str) -> Result<Vec<NoteLinkRow>, StorageError> {
        let rows =
            sqlx::query_as::<_, NoteLinkRow>("SELECT * FROM note_links WHERE target_id = ?1")
                .bind(target_id)
                .fetch_all(&self.pool)
                .await?;
        Ok(rows)
    }

    pub async fn get_all_links(&self) -> Result<Vec<NoteLinkRow>, StorageError> {
        let rows = sqlx::query_as::<_, NoteLinkRow>("SELECT * FROM note_links")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    // ── Versions ─────────────────────────────────────

    pub async fn create_version(
        &self,
        row: &NoteVersionRow,
    ) -> Result<NoteVersionRow, StorageError> {
        let result = sqlx::query_as::<_, NoteVersionRow>(
            "INSERT INTO note_versions (id, note_id, body, created_at)
             VALUES (?1, ?2, ?3, ?4)
             RETURNING *",
        )
        .bind(&row.id)
        .bind(&row.note_id)
        .bind(&row.body)
        .bind(&row.created_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(result)
    }

    pub async fn get_version(&self, id: &str) -> Result<Option<NoteVersionRow>, StorageError> {
        let row = sqlx::query_as::<_, NoteVersionRow>("SELECT * FROM note_versions WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn list_versions(&self, note_id: &str) -> Result<Vec<NoteVersionRow>, StorageError> {
        let rows = sqlx::query_as::<_, NoteVersionRow>(
            "SELECT * FROM note_versions WHERE note_id = ?1 ORDER BY created_at DESC",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn prune_versions(
        &self,
        note_id: &str,
        max_versions: i64,
    ) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "DELETE FROM note_versions WHERE id IN (
                SELECT id FROM note_versions WHERE note_id = ?1
                ORDER BY created_at DESC
                LIMIT -1 OFFSET ?2
            )",
        )
        .bind(note_id)
        .bind(max_versions)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> NoteRepo {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON;")
            .execute(&pool)
            .await
            .unwrap();
        let sql = crate::NotesFeature::migration_sql();
        sqlx::query(sql).execute(&pool).await.unwrap();
        NoteRepo::new(pool)
    }

    fn sample_note(id: &str, title: &str) -> NoteRow {
        let now = utc_now_str();
        NoteRow {
            id: id.to_string(),
            notebook_id: None,
            title: title.to_string(),
            body: "hello world".to_string(),
            body_html: None,
            pinned: 0,
            archived: 0,
            created_at: now.clone(),
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_note() {
        let repo = setup().await;
        let row = sample_note("n1", "Test Note");
        repo.create_note(&row).await.unwrap();

        let fetched = repo.get_note("n1").await.unwrap().unwrap();
        assert_eq!(fetched.title, "Test Note");
        assert_eq!(fetched.body, "hello world");
    }

    #[tokio::test]
    async fn test_list_notes() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "First")).await.unwrap();
        repo.create_note(&sample_note("n2", "Second"))
            .await
            .unwrap();

        let notes = repo.list_notes(None).await.unwrap();
        assert_eq!(notes.len(), 2);
    }

    #[tokio::test]
    async fn test_update_note() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Original"))
            .await
            .unwrap();

        let updated = repo
            .update_note("n1", Some("Updated"), None, None, Some(true), None)
            .await
            .unwrap();
        assert_eq!(updated.title, "Updated");
        assert_eq!(updated.pinned, 1);
    }

    #[tokio::test]
    async fn test_delete_note() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "To Delete"))
            .await
            .unwrap();

        assert!(repo.delete_note("n1").await.unwrap());
        assert!(repo.get_note("n1").await.unwrap().is_none());
        assert!(!repo.delete_note("n1").await.unwrap());
    }

    #[tokio::test]
    async fn test_health_check() {
        let repo = setup().await;
        repo.check_health().await.unwrap();
        repo.create_note(&sample_note("n1", "One")).await.unwrap();
        repo.check_health().await.unwrap();
    }

    #[tokio::test]
    async fn test_search_notes() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Rust Programming"))
            .await
            .unwrap();
        repo.create_note(&sample_note("n2", "Go Programming"))
            .await
            .unwrap();

        let results = repo.search_notes("Rust").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Rust Programming");
    }

    #[tokio::test]
    async fn test_tags() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Tagged"))
            .await
            .unwrap();

        repo.set_tags("n1", &["rust".to_string(), "notes".to_string()])
            .await
            .unwrap();
        let tags = repo.get_tags("n1").await.unwrap();
        assert_eq!(tags, vec!["notes", "rust"]);

        // Replace tags
        repo.set_tags("n1", &["only-this".to_string()])
            .await
            .unwrap();
        let tags = repo.get_tags("n1").await.unwrap();
        assert_eq!(tags, vec!["only-this"]);
    }

    #[tokio::test]
    async fn test_notebooks_crud() {
        let repo = setup().await;
        let now = utc_now_str();
        let nb = NotebookRow {
            id: "nb1".to_string(),
            parent_id: None,
            title: "My Notebook".to_string(),
            icon: Some("📓".to_string()),
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_notebook(&nb).await.unwrap();

        let notebooks = repo.list_notebooks().await.unwrap();
        assert_eq!(notebooks.len(), 1);
        assert_eq!(notebooks[0].title, "My Notebook");

        let updated = repo
            .update_notebook("nb1", Some("Renamed"), None, None)
            .await
            .unwrap();
        assert_eq!(updated.title, "Renamed");

        assert!(repo.delete_notebook("nb1").await.unwrap());
        assert!(repo.list_notebooks().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_links() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Source"))
            .await
            .unwrap();
        repo.create_note(&sample_note("n2", "Target A"))
            .await
            .unwrap();
        repo.create_note(&sample_note("n3", "Target B"))
            .await
            .unwrap();

        repo.set_links("n1", &["n2".to_string(), "n3".to_string()])
            .await
            .unwrap();

        let from = repo.get_links_from("n1").await.unwrap();
        assert_eq!(from.len(), 2);

        let to = repo.get_links_to("n2").await.unwrap();
        assert_eq!(to.len(), 1);
        assert_eq!(to[0].source_id, "n1");

        let all = repo.get_all_links().await.unwrap();
        assert_eq!(all.len(), 2);

        // Replace links
        repo.set_links("n1", &["n3".to_string()]).await.unwrap();
        assert_eq!(repo.get_links_from("n1").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_versions() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Versioned"))
            .await
            .unwrap();

        for i in 0..5 {
            let v = NoteVersionRow {
                id: format!("v{i}"),
                note_id: "n1".to_string(),
                body: format!("version {i}"),
                created_at: format!("2026-01-0{i}T00:00:00Z", i = i + 1),
            };
            repo.create_version(&v).await.unwrap();
        }

        let versions = repo.list_versions("n1").await.unwrap();
        assert_eq!(versions.len(), 5);
        // Most recent first
        assert_eq!(versions[0].body, "version 4");

        // Prune to 2
        let pruned = repo.prune_versions("n1", 2).await.unwrap();
        assert_eq!(pruned, 3);
        assert_eq!(repo.list_versions("n1").await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_notes_by_entity() {
        let repo = setup().await;
        repo.create_note(&sample_note("n1", "Meeting notes"))
            .await
            .unwrap();
        repo.create_note(&sample_note("n2", "Other note"))
            .await
            .unwrap();

        // Link n1 to task t1
        repo.set_entity_mentions("n1", &[("task".to_string(), "t1".to_string())])
            .await
            .unwrap();

        let linked = repo.list_notes_by_entity("task", "t1").await.unwrap();
        assert_eq!(linked.len(), 1);
        assert_eq!(linked[0].id, "n1");

        // No notes for unknown entity
        let empty = repo.list_notes_by_entity("task", "t99").await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_notes_filtered_by_notebook() {
        let repo = setup().await;
        let now = utc_now_str();
        let nb = NotebookRow {
            id: "nb1".to_string(),
            parent_id: None,
            title: "Work".to_string(),
            icon: None,
            sort_order: 0,
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_notebook(&nb).await.unwrap();

        let mut note_in_nb = sample_note("n1", "In notebook");
        note_in_nb.notebook_id = Some("nb1".to_string());
        repo.create_note(&note_in_nb).await.unwrap();
        repo.create_note(&sample_note("n2", "No notebook"))
            .await
            .unwrap();

        let filtered = repo.list_notes(Some("nb1")).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "n1");

        let all = repo.list_notes(None).await.unwrap();
        assert_eq!(all.len(), 2);
    }
}
