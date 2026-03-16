use storage::StorageError;

use super::NoteRepo;

impl NoteRepo {
    /// Find notes that share graph neighbors with the given note but aren't directly linked.
    /// Returns (note_id, shared_neighbor_count) sorted by count descending.
    pub async fn find_structural_holes(
        &self,
        note_id: &str,
    ) -> Result<Vec<(String, i64)>, StorageError> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT nl2.target_id, COUNT(*) AS shared
             FROM note_links nl1
             JOIN note_links nl2 ON nl2.source_id = nl1.target_id
             WHERE nl1.source_id = ?1
               AND nl2.target_id != ?1
               AND nl2.target_id NOT IN (
                   SELECT target_id FROM note_links WHERE source_id = ?1
               )
             GROUP BY nl2.target_id
             HAVING shared >= 2
             ORDER BY shared DESC
             LIMIT 10",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Find notes that mention the same entities as the given note.
    /// Returns (note_id, shared_entity_count) sorted by count descending.
    pub async fn find_entity_cooccurrences(
        &self,
        note_id: &str,
    ) -> Result<Vec<(String, i64)>, StorageError> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT nem2.note_id, COUNT(*) AS shared
             FROM note_entity_mentions nem1
             JOIN note_entity_mentions nem2
               ON nem2.entity_type = nem1.entity_type
              AND nem2.entity_id = nem1.entity_id
             WHERE nem1.note_id = ?1
               AND nem2.note_id != ?1
             GROUP BY nem2.note_id
             ORDER BY shared DESC
             LIMIT 10",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Find notes that share tags but aren't linked to the given note.
    /// Returns (note_id, shared_tag_count) sorted by count descending.
    pub async fn find_tag_overlaps(
        &self,
        note_id: &str,
    ) -> Result<Vec<(String, i64)>, StorageError> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT nt2.note_id, COUNT(*) AS shared
             FROM note_tags nt1
             JOIN note_tags nt2 ON nt2.tag = nt1.tag
             WHERE nt1.note_id = ?1
               AND nt2.note_id != ?1
               AND nt2.note_id NOT IN (
                   SELECT target_id FROM note_links WHERE source_id = ?1
               )
             GROUP BY nt2.note_id
             ORDER BY shared DESC
             LIMIT 10",
        )
        .bind(note_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get all unique tags with usage counts, sorted by count descending.
    pub async fn get_all_tags(&self) -> Result<Vec<(String, i64)>, StorageError> {
        let rows = sqlx::query_as::<_, (String, i64)>(
            "SELECT tag, COUNT(*) AS cnt FROM note_tags GROUP BY tag ORDER BY cnt DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
