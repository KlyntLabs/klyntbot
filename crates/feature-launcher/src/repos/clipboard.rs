use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use storage::StorageError;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, sqlx::FromRow)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content: String,
    pub content_type: String,
    pub source_app: Option<String>,
    pub preview: Option<String>,
    pub file_path: Option<String>,
    pub pinned: bool,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct ClipboardRepo {
    pool: SqlitePool,
}

impl ClipboardRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(
        &self,
        content: &str,
        content_type: &str,
        source_app: Option<&str>,
        file_path: Option<&str>,
    ) -> Result<i64, StorageError> {
        let now = Timestamp::now().to_string();
        let preview: String = content.chars().take(200).collect();
        let result = sqlx::query(
            "INSERT INTO clipboard_history (content, content_type, source_app, preview, file_path, created_at) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(content)
        .bind(content_type)
        .bind(source_app)
        .bind(&preview)
        .bind(file_path)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(result.last_insert_rowid())
    }

    pub async fn get(&self, id: i64) -> Result<Option<ClipboardEntry>, StorageError> {
        let entry =
            sqlx::query_as::<_, ClipboardEntry>("SELECT * FROM clipboard_history WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(entry)
    }

    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<ClipboardEntry>, StorageError> {
        let entries = sqlx::query_as::<_, ClipboardEntry>(
            "SELECT * FROM clipboard_history ORDER BY created_at DESC LIMIT ? OFFSET ?",
        )
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    pub async fn search(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<ClipboardEntry>, StorageError> {
        let fts_query = format!("{}*", query);
        let entries = sqlx::query_as::<_, ClipboardEntry>(
            "SELECT ch.* FROM clipboard_history ch \
             JOIN clipboard_fts fts ON ch.id = fts.rowid \
             WHERE clipboard_fts MATCH ? \
             ORDER BY rank LIMIT ?",
        )
        .bind(&fts_query)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    pub async fn pin(&self, id: i64, pinned: bool) -> Result<(), StorageError> {
        sqlx::query("UPDATE clipboard_history SET pinned = ? WHERE id = ?")
            .bind(pinned)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM clipboard_history WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn evict_to_max(&self, max_entries: i64) -> Result<i64, StorageError> {
        let result = sqlx::query(
            "DELETE FROM clipboard_history WHERE id IN ( \
                SELECT id FROM clipboard_history \
                WHERE pinned = 0 \
                ORDER BY created_at ASC \
                LIMIT MAX(0, (SELECT COUNT(*) FROM clipboard_history) - ?) \
             )",
        )
        .bind(max_entries)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as i64)
    }
}

#[async_trait::async_trait]
impl crate::search::SearchSource for ClipboardRepo {
    fn name(&self) -> &'static str {
        "clipboard"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<crate::LauncherItem> {
        let entries = match self.search(query, limit as i64).await {
            Ok(e) => e,
            Err(_) => return vec![],
        };
        entries
            .into_iter()
            .map(|e| {
                let content_type = match e.content_type.as_str() {
                    "image" => crate::ClipboardContentType::Image,
                    "file" => crate::ClipboardContentType::File,
                    _ => crate::ClipboardContentType::Text,
                };
                let preview: String = e
                    .preview
                    .clone()
                    .unwrap_or_else(|| e.content.chars().take(80).collect());
                crate::LauncherItem {
                    id: format!("clip:{}", e.id),
                    title: preview,
                    subtitle: e.source_app.clone(),
                    icon: Some("clipboard".to_string()),
                    kind: crate::LauncherItemKind::ClipboardEntry {
                        entry_id: e.id,
                        content_type,
                    },
                    score: 0.5,
                    no_view: false,
                    arguments: vec![],
                    pinned: false,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    async fn setup() -> ClipboardRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(pool.inner(), &crate::launcher_migrations())
            .await
            .unwrap();
        ClipboardRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_insert_and_list() {
        let repo = setup().await;
        repo.insert("hello world", "text", Some("Safari"), None)
            .await
            .unwrap();
        repo.insert("second entry", "text", Some("VSCode"), None)
            .await
            .unwrap();
        let entries = repo.list(10, 0).await.unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].content, "second entry"); // most recent first
    }

    #[tokio::test]
    async fn test_search_fts() {
        let repo = setup().await;
        repo.insert("rust programming language", "text", None, None)
            .await
            .unwrap();
        repo.insert("python scripting", "text", None, None)
            .await
            .unwrap();
        let results = repo.search("rust", 10).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].content.contains("rust"));
    }

    #[tokio::test]
    async fn test_pin_and_delete() {
        let repo = setup().await;
        repo.insert("keep me", "text", None, None).await.unwrap();
        let entries = repo.list(10, 0).await.unwrap();
        let id = entries[0].id;
        repo.pin(id, true).await.unwrap();
        let entry = repo.get(id).await.unwrap().unwrap();
        assert!(entry.pinned);
        repo.delete(id).await.unwrap();
        assert!(repo.get(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_eviction_respects_pins() {
        let repo = setup().await;
        // Insert 3 entries, pin the first
        repo.insert("first", "text", None, None).await.unwrap();
        let entries = repo.list(10, 0).await.unwrap();
        repo.pin(entries[0].id, true).await.unwrap();
        repo.insert("second", "text", None, None).await.unwrap();
        repo.insert("third", "text", None, None).await.unwrap();

        // Evict to max 2 entries
        repo.evict_to_max(2).await.unwrap();
        let remaining = repo.list(10, 0).await.unwrap();
        // Pinned entry survives, oldest unpinned evicted
        assert!(remaining.iter().any(|e| e.content == "first")); // pinned
        assert_eq!(remaining.len(), 2);
    }
}
