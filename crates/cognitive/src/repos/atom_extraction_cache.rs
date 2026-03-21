use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct AtomExtractionCache {
    pool: SqlitePool,
}

impl AtomExtractionCache {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Check if note content has already been extracted with this hash.
    pub async fn is_cached(&self, note_id: &str, content_hash: &str) -> Result<bool, sqlx::Error> {
        let row: Option<(String,)> = sqlx::query_as(
            "SELECT note_id FROM atom_extraction_cache WHERE note_id = ?1 AND content_hash = ?2",
        )
        .bind(note_id)
        .bind(content_hash)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.is_some())
    }

    /// Update or insert the cache entry for a note.
    pub async fn set(&self, note_id: &str, content_hash: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO atom_extraction_cache (note_id, content_hash, extracted_at) VALUES (?1, ?2, ?3) ON CONFLICT(note_id) DO UPDATE SET content_hash = ?2, extracted_at = ?3",
        )
        .bind(note_id)
        .bind(content_hash)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::cognitive_test_pool;

    #[tokio::test]
    async fn test_cache_miss_then_hit() {
        let pool = cognitive_test_pool().await;
        let cache = AtomExtractionCache::new(pool);

        assert!(!cache.is_cached("note-1", "hash-abc").await.unwrap());
        cache.set("note-1", "hash-abc").await.unwrap();
        assert!(cache.is_cached("note-1", "hash-abc").await.unwrap());
        // Different hash = miss
        assert!(!cache.is_cached("note-1", "hash-def").await.unwrap());
    }

    #[tokio::test]
    async fn test_cache_update_overwrites() {
        let pool = cognitive_test_pool().await;
        let cache = AtomExtractionCache::new(pool);

        cache.set("note-1", "hash-old").await.unwrap();
        cache.set("note-1", "hash-new").await.unwrap();
        assert!(!cache.is_cached("note-1", "hash-old").await.unwrap());
        assert!(cache.is_cached("note-1", "hash-new").await.unwrap());
    }
}
