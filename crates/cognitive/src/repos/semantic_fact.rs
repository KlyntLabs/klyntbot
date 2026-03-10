//! Repository for the `semantic_facts` table.

use sqlx::SqlitePool;

use crate::types::SemanticFact;

#[derive(Debug, Clone)]
pub struct SemanticFactRepo {
    pool: SqlitePool,
}

impl SemanticFactRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or replace a semantic fact.
    pub async fn upsert(&self, fact: &SemanticFact) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO semantic_facts (id, domain, subject, predicate, object, confidence, source,
                valid_from, valid_until, recorded_at, superseded_at, superseded_by,
                stability, last_accessed, access_count, project_id, memory_type)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
            ON CONFLICT (id) DO UPDATE SET
                domain = excluded.domain,
                subject = excluded.subject,
                predicate = excluded.predicate,
                object = excluded.object,
                confidence = excluded.confidence,
                source = excluded.source,
                valid_from = excluded.valid_from,
                valid_until = excluded.valid_until,
                superseded_at = excluded.superseded_at,
                superseded_by = excluded.superseded_by,
                stability = excluded.stability,
                last_accessed = excluded.last_accessed,
                access_count = excluded.access_count,
                project_id = excluded.project_id,
                memory_type = excluded.memory_type
            "#,
        )
        .bind(&fact.id)
        .bind(&fact.domain)
        .bind(&fact.subject)
        .bind(&fact.predicate)
        .bind(&fact.object)
        .bind(fact.confidence)
        .bind(&fact.source)
        .bind(&fact.valid_from)
        .bind(&fact.valid_until)
        .bind(&fact.recorded_at)
        .bind(&fact.superseded_at)
        .bind(&fact.superseded_by)
        .bind(fact.stability)
        .bind(&fact.last_accessed)
        .bind(fact.access_count)
        .bind(&fact.project_id)
        .bind(&fact.memory_type)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get a fact by ID.
    pub async fn get(&self, id: &str) -> Result<Option<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>("SELECT * FROM semantic_facts WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// List all active (non-superseded) facts for a domain.
    pub async fn list_active(&self, domain: &str) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE domain = ?1 AND valid_until IS NULL AND superseded_at IS NULL",
        )
        .bind(domain)
        .fetch_all(&self.pool)
        .await
    }

    /// List ALL active facts across all domains.
    pub async fn list_all_active(&self) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE valid_until IS NULL AND superseded_at IS NULL ORDER BY recorded_at DESC",
        )
        .fetch_all(&self.pool)
        .await
    }

    /// Find facts with matching subject and predicate (for consolidation).
    pub async fn find_similar(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE subject = ?1 AND predicate = ?2 AND superseded_at IS NULL",
        )
        .bind(subject)
        .bind(predicate)
        .fetch_all(&self.pool)
        .await
    }

    /// Supersede a fact: set superseded_at and superseded_by on the old fact.
    pub async fn supersede(&self, old_id: &str, new_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE semantic_facts SET superseded_at = datetime('now'), superseded_by = ?2 WHERE id = ?1",
        )
        .bind(old_id)
        .bind(new_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record an access event: increment count, update last_accessed and stability.
    pub async fn record_access(&self, id: &str, new_stability: f64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE semantic_facts SET access_count = access_count + 1, last_accessed = datetime('now'), stability = ?2 WHERE id = ?1",
        )
        .bind(id)
        .bind(new_stability)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fetch multiple facts by ID in a single query.
    pub async fn get_batch(&self, ids: &[&str]) -> Result<Vec<SemanticFact>, sqlx::Error> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // SQLite doesn't support array params — build IN clause with positional placeholders
        let placeholders: Vec<String> = (1..=ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT * FROM semantic_facts WHERE id IN ({})",
            placeholders.join(", ")
        );
        let mut query = sqlx::query_as::<_, SemanticFact>(&sql);
        for id in ids {
            query = query.bind(id);
        }
        query.fetch_all(&self.pool).await
    }

    /// List facts with retrievability below a threshold (candidates for compaction).
    pub async fn list_low_stability(
        &self,
        max_stability: f64,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE stability < ?1 AND superseded_at IS NULL",
        )
        .bind(max_stability)
        .fetch_all(&self.pool)
        .await
    }

    /// List all active facts for a project (non-superseded, non-expired).
    pub async fn list_by_project(&self, project_id: &str) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE project_id = ?1 AND valid_until IS NULL AND superseded_at IS NULL ORDER BY recorded_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await
    }

    /// List active facts for a project filtered by memory type.
    pub async fn list_by_project_and_type(
        &self,
        project_id: &str,
        memory_type: &str,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE project_id = ?1 AND memory_type = ?2 AND valid_until IS NULL AND superseded_at IS NULL ORDER BY recorded_at DESC",
        )
        .bind(project_id)
        .bind(memory_type)
        .fetch_all(&self.pool)
        .await
    }

    /// Count facts in the archive table.
    pub async fn count_archived(&self) -> Result<u64, sqlx::Error> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM semantic_facts_archive")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0 as u64)
    }

    /// Search archived facts by domain and/or keyword in subject/predicate/object.
    pub async fn search_archived(
        &self,
        domain: Option<&str>,
        keyword: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        let mut conditions = Vec::new();
        if domain.is_some() {
            conditions.push("domain = ?1".to_string());
        }
        if keyword.is_some() {
            let kw_param = if domain.is_some() { "?2" } else { "?1" };
            conditions.push(format!(
                "(subject LIKE '%' || {kw} || '%' OR predicate LIKE '%' || {kw} || '%' OR object LIKE '%' || {kw} || '%')",
                kw = kw_param
            ));
        }
        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };
        let limit_param = if domain.is_some() && keyword.is_some() {
            "?3"
        } else if domain.is_some() || keyword.is_some() {
            "?2"
        } else {
            "?1"
        };
        let sql = format!(
            "SELECT id, domain, subject, predicate, object, confidence, source, \
             valid_from, valid_until, recorded_at, superseded_at, superseded_by, \
             stability, last_accessed, access_count, project_id, memory_type \
             FROM semantic_facts_archive {where_clause} ORDER BY recorded_at DESC LIMIT {limit_param}"
        );
        let mut query = sqlx::query_as::<_, SemanticFact>(&sql);
        if let Some(d) = domain {
            query = query.bind(d);
        }
        if let Some(kw) = keyword {
            query = query.bind(kw);
        }
        query = query.bind(limit);
        query.fetch_all(&self.pool).await
    }

    /// Reinstate an archived fact back into the active table.
    pub async fn reinstate_archived(&self, id: &str) -> Result<bool, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let rows = sqlx::query(
            r#"
            INSERT INTO semantic_facts
                (id, domain, subject, predicate, object, confidence, source,
                 valid_from, valid_until, recorded_at, superseded_at, superseded_by,
                 stability, last_accessed, access_count, project_id, memory_type)
            SELECT id, domain, subject, predicate, object, confidence, source,
                   valid_from, NULL, recorded_at, NULL, NULL,
                   stability, last_accessed, access_count, project_id, memory_type
            FROM semantic_facts_archive
            WHERE id = ?1
            "#,
        )
        .bind(id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        if rows > 0 {
            sqlx::query("DELETE FROM semantic_facts_archive WHERE id = ?1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }

        tx.commit().await?;
        Ok(rows > 0)
    }

    /// Move superseded facts older than N days to the archive table.
    pub async fn archive_superseded(&self, older_than_days: i64) -> Result<u64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query(
            r#"
            INSERT INTO semantic_facts_archive
                (id, domain, subject, predicate, object, confidence, source,
                 valid_from, valid_until, recorded_at, superseded_at, superseded_by,
                 stability, last_accessed, access_count, project_id, memory_type)
            SELECT id, domain, subject, predicate, object, confidence, source,
                   valid_from, valid_until, recorded_at, superseded_at, superseded_by,
                   stability, last_accessed, access_count, project_id, memory_type
            FROM semantic_facts
            WHERE superseded_at IS NOT NULL
              AND julianday('now') - julianday(superseded_at) > ?1
            "#,
        )
        .bind(older_than_days)
        .execute(&mut *tx)
        .await?;

        let archived = result.rows_affected();

        sqlx::query(
            r#"
            DELETE FROM semantic_facts
            WHERE superseded_at IS NOT NULL
              AND julianday('now') - julianday(superseded_at) > ?1
            "#,
        )
        .bind(older_than_days)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(archived)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DEFAULT_MEMORY_TYPE;

    async fn setup() -> SqlitePool {
        crate::repos::cognitive_test_pool().await
    }

    fn test_fact(id: &str, domain: &str, predicate: &str, object: &str) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: domain.into(),
            subject: "user".into(),
            predicate: predicate.into(),
            object: object.into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            project_id: None,
            memory_type: DEFAULT_MEMORY_TYPE.to_string(),
        }
    }

    #[tokio::test]
    async fn test_upsert_and_get_active_facts() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let fact = test_fact("f1", "productivity", "peak_hours", "10am-12pm");
        repo.upsert(&fact).await.unwrap();

        let active = repo.list_active("productivity").await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].predicate, "peak_hours");
    }

    #[tokio::test]
    async fn test_supersede_fact() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let old = test_fact("f1", "productivity", "peak_hours", "10am-12pm");
        repo.upsert(&old).await.unwrap();

        let new = test_fact("f2", "productivity", "peak_hours", "9am-11am");
        repo.upsert(&new).await.unwrap();
        repo.supersede("f1", "f2").await.unwrap();

        let active = repo.list_active("productivity").await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "f2");
    }

    #[tokio::test]
    async fn test_record_access_increases_stability() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let fact = test_fact("f1", "productivity", "peak_hours", "10am-12pm");
        repo.upsert(&fact).await.unwrap();

        repo.record_access("f1", 1.2).await.unwrap();
        let updated = repo.get("f1").await.unwrap().unwrap();
        assert!((updated.stability - 1.2).abs() < f64::EPSILON);
        assert_eq!(updated.access_count, 1);
    }

    #[tokio::test]
    async fn test_get_batch_returns_matching_facts() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let f1 = test_fact("batch1", "productivity", "pred1", "val1");
        let f2 = test_fact("batch2", "productivity", "pred2", "val2");
        let f3 = test_fact("batch3", "productivity", "pred3", "val3");
        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();
        repo.upsert(&f3).await.unwrap();

        let results = repo.get_batch(&["batch1", "batch3"]).await.unwrap();
        assert_eq!(results.len(), 2);
        let ids: Vec<&str> = results.iter().map(|f| f.id.as_str()).collect();
        assert!(ids.contains(&"batch1"));
        assert!(ids.contains(&"batch3"));
    }

    #[tokio::test]
    async fn test_get_batch_empty_ids() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);
        let results = repo.get_batch(&[]).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_find_similar() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let f1 = test_fact("f1", "productivity", "peak_hours", "10am-12pm");
        let f2 = test_fact("f2", "productivity", "peak_hours", "9am-11am");
        let f3 = test_fact("f3", "productivity", "break_pattern", "every 90min");
        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();
        repo.upsert(&f3).await.unwrap();

        let similar = repo.find_similar("user", "peak_hours").await.unwrap();
        assert_eq!(similar.len(), 2);
    }

    #[tokio::test]
    async fn test_count_archived_empty() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);
        let count = repo.count_archived().await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_archive_and_count() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        // Create and supersede a fact, then archive it
        let old = test_fact("f1", "productivity", "peak_hours", "10am-12pm");
        repo.upsert(&old).await.unwrap();
        let new = test_fact("f2", "productivity", "peak_hours", "9am-11am");
        repo.upsert(&new).await.unwrap();
        repo.supersede("f1", "f2").await.unwrap();

        // Manually set superseded_at to > 0 days ago so archive_superseded(0) picks it up
        let archived = repo.archive_superseded(0).await.unwrap();
        assert_eq!(archived, 1);

        let count = repo.count_archived().await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_search_archived_by_domain() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let f1 = test_fact("f1", "productivity", "peak_hours", "10am-12pm");
        repo.upsert(&f1).await.unwrap();
        repo.supersede("f1", "f2").await.unwrap();
        repo.archive_superseded(0).await.unwrap();

        let results = repo
            .search_archived(Some("productivity"), None, 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "f1");

        let empty = repo
            .search_archived(Some("finance"), None, 10)
            .await
            .unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_search_archived_by_keyword() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let f1 = test_fact("f1", "productivity", "peak_hours", "10am-12pm");
        repo.upsert(&f1).await.unwrap();
        repo.supersede("f1", "f2").await.unwrap();
        repo.archive_superseded(0).await.unwrap();

        let results = repo.search_archived(None, Some("peak"), 10).await.unwrap();
        assert_eq!(results.len(), 1);

        let empty = repo
            .search_archived(None, Some("nonexistent"), 10)
            .await
            .unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_reinstate_archived() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let f1 = test_fact("f1", "productivity", "peak_hours", "10am-12pm");
        repo.upsert(&f1).await.unwrap();
        repo.supersede("f1", "f2").await.unwrap();
        repo.archive_superseded(0).await.unwrap();

        // Fact should be in archive, not active
        assert!(repo.get("f1").await.unwrap().is_none());
        assert_eq!(repo.count_archived().await.unwrap(), 1);

        // Reinstate it
        let reinstated = repo.reinstate_archived("f1").await.unwrap();
        assert!(reinstated);

        // Now it's active again with cleared supersession
        let fact = repo.get("f1").await.unwrap().unwrap();
        assert!(fact.superseded_at.is_none());
        assert!(fact.superseded_by.is_none());
        assert!(fact.valid_until.is_none());

        // Archive is empty
        assert_eq!(repo.count_archived().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_reinstate_nonexistent_returns_false() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);
        let reinstated = repo.reinstate_archived("nonexistent").await.unwrap();
        assert!(!reinstated);
    }
}
