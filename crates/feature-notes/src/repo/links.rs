use storage::StorageError;

use super::NoteRepo;
use crate::models::{NoteLinkRow, NoteRow, NoteVersionRow};

impl NoteRepo {
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

    /// Get notes that link TO the given note, returning the source note row
    /// and an optional context snippet (the line containing `[[`).
    pub async fn get_backlinks_with_context(
        &self,
        note_id: &str,
    ) -> Result<Vec<(NoteRow, Option<String>)>, StorageError> {
        let rows: Vec<NoteRow> = sqlx::query_as(
            r#"SELECT n.* FROM note_links nl
               JOIN notes n ON n.id = nl.source_id
               WHERE nl.target_id = ?1 AND n.archived = 0
               ORDER BY n.updated_at DESC"#,
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                let context = row
                    .body
                    .lines()
                    .find(|line| line.contains("[["))
                    .map(|line| line.trim().to_string());
                (row, context)
            })
            .collect())
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
