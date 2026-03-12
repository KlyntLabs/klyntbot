use std::collections::HashMap;

use storage::StorageError;

use super::NoteRepo;

impl NoteRepo {
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
}
