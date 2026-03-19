//! Repository for the `annotations` table.

use sqlx::SqlitePool;

use crate::types::Annotation;

#[derive(Debug, Clone)]
pub struct AnnotationRepo {
    pool: SqlitePool,
}

impl AnnotationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, annotation: &Annotation) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO annotations (id, target_type, target_id, content, tags, author,
                priority, created_at, updated_at, expires_at, access_count,
                mark_id, quoted_text, range_start, range_end, ai_suggestion)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            ON CONFLICT (id) DO UPDATE SET
                target_type = excluded.target_type,
                target_id = excluded.target_id,
                content = excluded.content,
                tags = excluded.tags,
                author = excluded.author,
                priority = excluded.priority,
                updated_at = excluded.updated_at,
                expires_at = excluded.expires_at,
                mark_id = excluded.mark_id,
                quoted_text = excluded.quoted_text,
                range_start = excluded.range_start,
                range_end = excluded.range_end,
                ai_suggestion = excluded.ai_suggestion
            "#,
        )
        .bind(&annotation.id)
        .bind(&annotation.target_type)
        .bind(&annotation.target_id)
        .bind(&annotation.content)
        .bind(&annotation.tags)
        .bind(&annotation.author)
        .bind(annotation.priority)
        .bind(&annotation.created_at)
        .bind(&annotation.updated_at)
        .bind(&annotation.expires_at)
        .bind(annotation.access_count)
        .bind(&annotation.mark_id)
        .bind(&annotation.quoted_text)
        .bind(annotation.range_start)
        .bind(annotation.range_end)
        .bind(&annotation.ai_suggestion)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_for_target(
        &self,
        target_type: &str,
        target_id: &str,
    ) -> Result<Vec<Annotation>, sqlx::Error> {
        sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE target_type = ?1 AND target_id = ?2 ORDER BY priority DESC, updated_at DESC",
        )
        .bind(target_type)
        .bind(target_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<Annotation>, sqlx::Error> {
        let sql = r#"
            SELECT a.* FROM annotations a
            INNER JOIN annotations_fts fts ON a.id = fts.id
            WHERE annotations_fts MATCH ?1
            AND (a.expires_at IS NULL OR a.expires_at >= datetime('now'))
            ORDER BY fts.rank
            LIMIT ?2
        "#;
        sqlx::query_as::<_, Annotation>(sql)
            .bind(query)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
    }

    pub async fn list_all(&self) -> Result<Vec<Annotation>, sqlx::Error> {
        sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE expires_at IS NULL OR expires_at >= datetime('now') ORDER BY priority DESC, updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn delete(&self, id: &str) -> Result<bool, sqlx::Error> {
        let result = sqlx::query("DELETE FROM annotations WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn delete_expired(&self) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            "DELETE FROM annotations WHERE expires_at IS NOT NULL AND expires_at < datetime('now')",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn increment_access(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE annotations SET access_count = access_count + 1 WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Batch-increment access counts for multiple annotations in a single query.
    pub async fn increment_access_batch(&self, ids: &[&str]) -> Result<(), sqlx::Error> {
        if ids.is_empty() {
            return Ok(());
        }
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "UPDATE annotations SET access_count = access_count + 1 WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut query = sqlx::query(&sql);
        for id in ids {
            query = query.bind(id);
        }
        query.execute(&self.pool).await?;
        Ok(())
    }

    pub async fn list_for_note(
        &self,
        note_id: &str,
        limit: i64,
    ) -> Result<Vec<Annotation>, sqlx::Error> {
        sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE target_type = 'note' AND target_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        )
        .bind(note_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_by_min_priority(
        &self,
        min_priority: i32,
    ) -> Result<Vec<Annotation>, sqlx::Error> {
        sqlx::query_as::<_, Annotation>(
            "SELECT * FROM annotations WHERE priority >= ?1 AND (expires_at IS NULL OR expires_at >= datetime('now')) ORDER BY priority DESC, updated_at DESC",
        )
        .bind(min_priority)
        .fetch_all(&self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_annotation(id: &str, target_type: &str, target_id: &str, content: &str) -> Annotation {
        Annotation {
            id: id.into(),
            target_type: target_type.into(),
            target_id: target_id.into(),
            content: content.into(),
            tags: "".into(),
            author: "agent".into(),
            priority: 0,
            created_at: "2026-03-10T10:00:00Z".into(),
            updated_at: "2026-03-10T10:00:00Z".into(),
            expires_at: None,
            access_count: 0,
            mark_id: None,
            quoted_text: None,
            range_start: None,
            range_end: None,
            ai_suggestion: None,
        }
    }

    #[tokio::test]
    async fn test_create_annotation_with_mark_id() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let annotation = Annotation {
            id: "ann-1".into(),
            target_type: "note".into(),
            target_id: "note-123".into(),
            content: "My annotation".into(),
            tags: "learning".into(),
            author: "user".into(),
            priority: 0,
            created_at: "2026-03-18T00:00:00Z".into(),
            updated_at: "2026-03-18T00:00:00Z".into(),
            expires_at: None,
            access_count: 0,
            mark_id: Some("mark-abc".into()),
            quoted_text: Some("selected text".into()),
            range_start: Some(42),
            range_end: Some(55),
            ai_suggestion: Some("Related to X".into()),
        };

        repo.upsert(&annotation).await.unwrap();
        let results = repo.get_for_target("note", "note-123").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].mark_id, Some("mark-abc".into()));
        assert_eq!(results[0].quoted_text, Some("selected text".into()));
        assert_eq!(results[0].range_start, Some(42));
        assert_eq!(results[0].range_end, Some(55));
        assert_eq!(results[0].ai_suggestion, Some("Related to X".into()));
    }

    #[tokio::test]
    async fn test_list_for_note() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let mut ann1 = test_annotation("ann-1", "note", "note-1", "First note");
        ann1.mark_id = Some("m1".into());
        let mut ann2 = test_annotation("ann-2", "note", "note-1", "Second note");
        ann2.mark_id = Some("m2".into());

        repo.upsert(&ann1).await.unwrap();
        repo.upsert(&ann2).await.unwrap();

        let results = repo.list_for_note("note-1", 10).await.unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_upsert_and_get_for_target() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let ann = test_annotation("a1", "tool", "search", "Use BM25 for keyword queries");
        repo.upsert(&ann).await.unwrap();

        let results = repo.get_for_target("tool", "search").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "Use BM25 for keyword queries");
    }

    #[tokio::test]
    async fn test_upsert_deduplicates() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let ann = test_annotation("a1", "tool", "search", "Same content");
        repo.upsert(&ann).await.unwrap();
        repo.upsert(&ann).await.unwrap();

        let results = repo.get_for_target("tool", "search").await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_delete() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let ann = test_annotation("a1", "tool", "search", "Temp note");
        repo.upsert(&ann).await.unwrap();

        let deleted = repo.delete("a1").await.unwrap();
        assert!(deleted);

        let results = repo.get_for_target("tool", "search").await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_delete_nonexistent() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let deleted = repo.delete("nonexistent").await.unwrap();
        assert!(!deleted);
    }

    #[tokio::test]
    async fn test_search_fts() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        repo.upsert(&test_annotation(
            "a1",
            "tool",
            "search",
            "BM25 ranking algorithm",
        ))
        .await
        .unwrap();
        repo.upsert(&test_annotation(
            "a2",
            "api",
            "stripe",
            "Webhook requires raw body",
        ))
        .await
        .unwrap();

        let results = repo.search("BM25 ranking", 10).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "a1");
    }

    #[tokio::test]
    async fn test_list_all() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        repo.upsert(&test_annotation("a1", "tool", "search", "Note 1"))
            .await
            .unwrap();
        repo.upsert(&test_annotation("a2", "api", "stripe", "Note 2"))
            .await
            .unwrap();

        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_increment_access() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        repo.upsert(&test_annotation("a1", "tool", "search", "Note"))
            .await
            .unwrap();
        repo.increment_access("a1").await.unwrap();
        repo.increment_access("a1").await.unwrap();

        let results = repo.get_for_target("tool", "search").await.unwrap();
        assert_eq!(results[0].access_count, 2);
    }

    #[tokio::test]
    async fn test_delete_expired() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let mut ann = test_annotation("a1", "tool", "search", "Expiring note");
        ann.expires_at = Some("2026-03-01T00:00:00Z".into()); // already expired
        repo.upsert(&ann).await.unwrap();

        let count = repo.delete_expired().await.unwrap();
        assert_eq!(count, 1);

        let all = repo.list_all().await.unwrap();
        assert!(all.is_empty());
    }

    #[tokio::test]
    async fn test_get_by_priority() {
        let pool = setup().await;
        let repo = AnnotationRepo::new(pool);

        let mut critical = test_annotation("a1", "tool", "search", "Critical: API change");
        critical.priority = 2;
        repo.upsert(&critical).await.unwrap();

        let mut normal = test_annotation("a2", "tool", "other", "Normal note");
        normal.priority = 0;
        repo.upsert(&normal).await.unwrap();

        let results = repo.get_by_min_priority(2).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "a1");
    }
}
