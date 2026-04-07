//! Repository for the `episodic_memories` table.

use sqlx::SqlitePool;

use crate::types::EpisodicMemory;

#[derive(Debug, Clone)]
pub struct EpisodicMemoryRepo {
    pool: SqlitePool,
}

impl EpisodicMemoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Access the underlying SQLite connection pool.
    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Insert a new episodic memory.
    pub async fn insert(&self, mem: &EpisodicMemory) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO episodic_memories (id, domain, content, summary, importance,
                occurred_at, recorded_at, stability, last_accessed, access_count, project_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
        )
        .bind(&mem.id)
        .bind(&mem.domain)
        .bind(&mem.content)
        .bind(&mem.summary)
        .bind(mem.importance)
        .bind(&mem.occurred_at)
        .bind(&mem.recorded_at)
        .bind(mem.stability)
        .bind(&mem.last_accessed)
        .bind(mem.access_count)
        .bind(&mem.project_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// List episodic memories in a date range (occurred_at between start and end).
    pub async fn list_range(
        &self,
        start: &str,
        end: &str,
    ) -> Result<Vec<EpisodicMemory>, sqlx::Error> {
        sqlx::query_as::<_, EpisodicMemory>(
            "SELECT * FROM episodic_memories WHERE occurred_at >= ?1 AND occurred_at <= ?2 ORDER BY occurred_at DESC",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
    }

    /// List recent episodic memories for a domain.
    pub async fn list_by_domain(
        &self,
        domain: &str,
        limit: i64,
    ) -> Result<Vec<EpisodicMemory>, sqlx::Error> {
        sqlx::query_as::<_, EpisodicMemory>(
            "SELECT * FROM episodic_memories WHERE domain = ?1 ORDER BY occurred_at DESC LIMIT ?2",
        )
        .bind(domain)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Record an access event on an episodic memory.
    pub async fn record_access(&self, id: &str, new_stability: f64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE episodic_memories SET access_count = access_count + 1, last_accessed = datetime('now'), stability = ?2 WHERE id = ?1",
        )
        .bind(id)
        .bind(new_stability)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get a single episodic memory by ID.
    pub async fn get(&self, id: &str) -> Result<Option<EpisodicMemory>, sqlx::Error> {
        sqlx::query_as::<_, EpisodicMemory>("SELECT * FROM episodic_memories WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// Count all episodic memories.
    pub async fn count_all(&self) -> Result<i64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// List recent episodic memories across all domains.
    pub async fn list_recent(&self, limit: i64) -> Result<Vec<EpisodicMemory>, sqlx::Error> {
        sqlx::query_as::<_, EpisodicMemory>(
            "SELECT * FROM episodic_memories ORDER BY occurred_at DESC LIMIT ?1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    /// Full-text search via FTS5.
    pub async fn search_fts(
        &self,
        query: &str,
        domain: Option<&str>,
        limit: usize,
    ) -> Result<Vec<EpisodicMemory>, sqlx::Error> {
        let sql = r#"
            SELECT e.* FROM episodic_memories e
            INNER JOIN episodic_memories_fts fts ON e.id = fts.id
            WHERE episodic_memories_fts MATCH ?1
            AND (?2 IS NULL OR e.domain = ?2)
            ORDER BY fts.rank
            LIMIT ?3
        "#;
        sqlx::query_as::<_, EpisodicMemory>(sql)
            .bind(query)
            .bind(domain)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
    }

    /// Delete old episodic memories with low access count (for archival).
    pub async fn delete_old(
        &self,
        older_than_days: i64,
        min_access_count: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM episodic_memories
            WHERE julianday('now') - julianday(occurred_at) > ?1
              AND access_count < ?2
            "#,
        )
        .bind(older_than_days)
        .bind(min_access_count)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_memory(id: &str, domain: &str, content: &str, occurred_at: &str) -> EpisodicMemory {
        EpisodicMemory {
            id: id.into(),
            domain: domain.into(),
            content: content.into(),
            summary: None,
            importance: 0.7,
            occurred_at: occurred_at.into(),
            recorded_at: "2026-03-06T12:00:00".into(),
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            scope_type: "system".to_string(),
            scope_id: None,
        }
    }

    #[tokio::test]
    async fn test_insert_and_list_by_domain() {
        let pool = setup().await;
        let repo = EpisodicMemoryRepo::new(pool);

        let m1 = test_memory(
            "m1",
            "productivity",
            "High focus morning session",
            "2026-03-06T10:00:00",
        );
        let m2 = test_memory(
            "m2",
            "finance",
            "Overspent on dining",
            "2026-03-06T12:00:00",
        );
        repo.insert(&m1).await.unwrap();
        repo.insert(&m2).await.unwrap();

        let prod = repo.list_by_domain("productivity", 10).await.unwrap();
        assert_eq!(prod.len(), 1);
        assert_eq!(prod[0].id, "m1");
    }

    #[tokio::test]
    async fn test_list_range() {
        let pool = setup().await;
        let repo = EpisodicMemoryRepo::new(pool);

        let m1 = test_memory(
            "m1",
            "productivity",
            "Monday session",
            "2026-03-02T10:00:00",
        );
        let m2 = test_memory(
            "m2",
            "productivity",
            "Wednesday session",
            "2026-03-04T10:00:00",
        );
        let m3 = test_memory(
            "m3",
            "productivity",
            "Friday session",
            "2026-03-06T10:00:00",
        );
        repo.insert(&m1).await.unwrap();
        repo.insert(&m2).await.unwrap();
        repo.insert(&m3).await.unwrap();

        let range = repo.list_range("2026-03-03", "2026-03-05").await.unwrap();
        assert_eq!(range.len(), 1);
        assert_eq!(range[0].id, "m2");
    }

    #[tokio::test]
    async fn test_record_access() {
        let pool = setup().await;
        let repo = EpisodicMemoryRepo::new(pool);

        let m = test_memory("m1", "productivity", "Session", "2026-03-06T10:00:00");
        repo.insert(&m).await.unwrap();

        repo.record_access("m1", 1.5).await.unwrap();

        let updated = repo.list_by_domain("productivity", 1).await.unwrap();
        assert_eq!(updated[0].access_count, 1);
        assert!((updated[0].stability - 1.5).abs() < f64::EPSILON);
    }
}
