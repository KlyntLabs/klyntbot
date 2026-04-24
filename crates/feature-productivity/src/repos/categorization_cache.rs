use sqlx::SqlitePool;

use crate::types::CategorizationCacheEntry;

#[derive(Debug, Clone)]
pub struct CategorizationCacheRepo {
    pool: SqlitePool,
}

impl CategorizationCacheRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get(&self, cache_key: &str) -> common::Result<Option<CategorizationCacheEntry>> {
        let row = sqlx::query_as::<_, CategorizationCacheEntry>(
            "SELECT * FROM productivity_categorization_cache WHERE cache_key = ?1 AND expires_at > datetime('now')",
        )
        .bind(cache_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row)
    }

    pub async fn upsert(&self, entry: &CategorizationCacheEntry) -> common::Result<()> {
        sqlx::query(
            r#"INSERT INTO productivity_categorization_cache (cache_key, category, session_type, confidence, source, created_at, expires_at)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
               ON CONFLICT(cache_key) DO UPDATE SET
                   category = excluded.category,
                   session_type = excluded.session_type,
                   confidence = excluded.confidence,
                   source = excluded.source,
                   created_at = excluded.created_at,
                   expires_at = excluded.expires_at"#,
        )
        .bind(&entry.cache_key)
        .bind(&entry.category)
        .bind(&entry.session_type)
        .bind(entry.confidence)
        .bind(&entry.source)
        .bind(&entry.created_at)
        .bind(&entry.expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn purge_expired(&self) -> common::Result<u64> {
        let result = sqlx::query(
            "DELETE FROM productivity_categorization_cache WHERE expires_at < datetime('now')",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProductivityFeature;

    async fn setup_pool() -> SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    fn sample_entry(key: &str, expires_at: &str) -> CategorizationCacheEntry {
        CategorizationCacheEntry {
            cache_key: key.to_string(),
            category: "coding".to_string(),
            session_type: "focus".to_string(),
            confidence: 0.95,
            source: "rule".to_string(),
            created_at: "2026-03-09T10:00:00Z".to_string(),
            expires_at: expires_at.to_string(),
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get() {
        let pool = setup_pool().await;
        let repo = CategorizationCacheRepo::new(pool);

        let entry = sample_entry("app:vscode", "2026-12-31T23:59:59Z");
        repo.upsert(&entry).await.unwrap();

        let fetched = repo.get("app:vscode").await.unwrap();
        assert!(fetched.is_some());
        let fetched = fetched.unwrap();
        assert_eq!(fetched.category, "coding");
        assert_eq!(fetched.confidence, 0.95);

        // Upsert with updated category
        let updated = CategorizationCacheEntry {
            category: "documentation".to_string(),
            confidence: 0.80,
            ..entry
        };
        repo.upsert(&updated).await.unwrap();
        let fetched = repo.get("app:vscode").await.unwrap().unwrap();
        assert_eq!(fetched.category, "documentation");
        assert_eq!(fetched.confidence, 0.80);
    }

    #[tokio::test]
    async fn test_purge_expired() {
        let pool = setup_pool().await;
        let repo = CategorizationCacheRepo::new(pool);

        // One expired, one valid
        let expired = sample_entry("expired-key", "2020-01-01T00:00:00Z");
        let valid = sample_entry("valid-key", "2030-12-31T23:59:59Z");
        repo.upsert(&expired).await.unwrap();
        repo.upsert(&valid).await.unwrap();

        let purged = repo.purge_expired().await.unwrap();
        assert_eq!(purged, 1);

        assert!(repo.get("expired-key").await.unwrap().is_none());
        assert!(repo.get("valid-key").await.unwrap().is_some());
    }
}
