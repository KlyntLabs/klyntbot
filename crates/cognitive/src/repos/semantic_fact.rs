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

    /// Access the underlying pool (needed by sibling repos in the same service).
    pub fn pool(&self) -> &sqlx::SqlitePool {
        &self.pool
    }

    /// Insert or replace a semantic fact.
    pub async fn upsert(&self, fact: &SemanticFact) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO semantic_facts (id, domain, subject, predicate, object, confidence, source,
                valid_from, valid_until, recorded_at, superseded_at, superseded_by,
                stability, last_accessed, access_count, project_id, memory_type,
                scope_type, scope_id)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
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
                memory_type = excluded.memory_type,
                scope_type = excluded.scope_type,
                scope_id = excluded.scope_id
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
        .bind(&fact.scope_type)
        .bind(&fact.scope_id)
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

    /// List active facts for a specific scope.
    pub async fn list_by_scope(
        &self,
        scope_type: &str,
        scope_id: Option<&str>,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        if let Some(sid) = scope_id {
            sqlx::query_as::<_, SemanticFact>(
                "SELECT * FROM semantic_facts WHERE scope_type = ?1 AND scope_id = ?2 AND superseded_at IS NULL ORDER BY recorded_at DESC",
            )
            .bind(scope_type)
            .bind(sid)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as::<_, SemanticFact>(
                "SELECT * FROM semantic_facts WHERE scope_type = ?1 AND scope_id IS NULL AND superseded_at IS NULL ORDER BY recorded_at DESC",
            )
            .bind(scope_type)
            .fetch_all(&self.pool)
            .await
        }
    }

    /// List active facts visible to a scope chain (e.g., system + squad + persona).
    /// Returns facts matching ANY tier in the chain, deduplicated by ID.
    ///
    /// Uses N separate `list_by_scope()` calls + dedup to avoid dynamic SQL bind issues.
    pub async fn list_by_scope_chain(
        &self,
        chain: &[(&str, Option<&str>)],
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        if chain.is_empty() {
            return Ok(Vec::new());
        }
        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();
        for (scope_type, scope_id) in chain {
            let facts = self.list_by_scope(scope_type, *scope_id).await?;
            for fact in facts {
                if seen.insert(fact.id.clone()) {
                    results.push(fact);
                }
            }
        }
        results.sort_by(|a, b| b.recorded_at.cmp(&a.recorded_at));
        Ok(results)
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
    pub async fn list_by_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
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

    /// Full-text search via FTS5 with BM25 ranking.
    pub async fn search_fts(
        &self,
        query: &str,
        domain: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        let sql = r#"
            SELECT f.* FROM semantic_facts f
            INNER JOIN semantic_facts_fts fts ON f.id = fts.id
            WHERE semantic_facts_fts MATCH ?1
            AND (?2 IS NULL OR f.domain = ?2)
            AND f.superseded_at IS NULL
            ORDER BY fts.rank
            LIMIT ?3
        "#;
        sqlx::query_as::<_, SemanticFact>(sql)
            .bind(query)
            .bind(domain)
            .bind(limit as i64)
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
             stability, last_accessed, access_count, project_id, memory_type, \
             scope_type, scope_id \
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
                 stability, last_accessed, access_count, project_id, memory_type,
                 scope_type, scope_id)
            SELECT id, domain, subject, predicate, object, confidence, source,
                   valid_from, NULL, recorded_at, NULL, NULL,
                   stability, last_accessed, access_count, project_id, memory_type,
                   scope_type, scope_id
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

    /// Prune low-salience facts: delete non-superseded facts where
    /// confidence < threshold AND last_accessed is older than the given number of days
    /// (or never accessed). Returns the number of facts deleted.
    pub async fn prune_low_salience(
        &self,
        confidence_threshold: f64,
        inactive_days: i64,
    ) -> Result<u64, sqlx::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM semantic_facts
            WHERE superseded_at IS NULL
              AND confidence < ?1
              AND (last_accessed IS NULL
                   OR julianday('now') - julianday(last_accessed) > ?2)
            "#,
        )
        .bind(confidence_threshold)
        .bind(inactive_days)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Find all facts (including superseded) with matching subject and predicate.
    pub async fn find_by_subject_predicate(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT * FROM semantic_facts WHERE subject = ?1 AND predicate = ?2 ORDER BY valid_from DESC",
        )
        .bind(subject)
        .bind(predicate)
        .fetch_all(&self.pool)
        .await
    }

    /// Find archived facts with matching subject and predicate.
    /// Must list columns explicitly because archive has an extra `archived_at` column.
    pub async fn search_archived_by_subject_predicate(
        &self,
        subject: &str,
        predicate: &str,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        sqlx::query_as::<_, SemanticFact>(
            "SELECT id, domain, subject, predicate, object, confidence, source, \
             valid_from, valid_until, recorded_at, superseded_at, superseded_by, \
             stability, last_accessed, access_count, project_id, memory_type, \
             scope_type, scope_id \
             FROM semantic_facts_archive WHERE subject = ?1 AND predicate = ?2 ORDER BY valid_from DESC",
        )
        .bind(subject)
        .bind(predicate)
        .fetch_all(&self.pool)
        .await
    }

    /// List facts created since a given timestamp that have not been superseded.
    /// Optionally filtered to specific domains.
    pub async fn list_created_since(
        &self,
        since: &str,
        domains: Option<&[&str]>,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        if let Some(domains) = domains {
            if domains.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders: Vec<String> =
                (2..=domains.len() + 1).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "SELECT * FROM semantic_facts WHERE recorded_at >= ?1 AND superseded_at IS NULL \
                 AND domain IN ({}) ORDER BY recorded_at DESC",
                placeholders.join(", ")
            );
            let mut query = sqlx::query_as::<_, SemanticFact>(&sql).bind(since);
            for d in domains {
                query = query.bind(*d);
            }
            query.fetch_all(&self.pool).await
        } else {
            sqlx::query_as::<_, SemanticFact>(
                "SELECT * FROM semantic_facts WHERE recorded_at >= ?1 AND superseded_at IS NULL ORDER BY recorded_at DESC",
            )
            .bind(since)
            .fetch_all(&self.pool)
            .await
        }
    }

    /// List facts superseded since a given timestamp.
    /// Optionally filtered to specific domains.
    pub async fn list_superseded_since(
        &self,
        since: &str,
        domains: Option<&[&str]>,
    ) -> Result<Vec<SemanticFact>, sqlx::Error> {
        if let Some(domains) = domains {
            if domains.is_empty() {
                return Ok(Vec::new());
            }
            let placeholders: Vec<String> =
                (2..=domains.len() + 1).map(|i| format!("?{i}")).collect();
            let sql = format!(
                "SELECT * FROM semantic_facts WHERE superseded_at >= ?1 \
                 AND domain IN ({}) ORDER BY superseded_at DESC",
                placeholders.join(", ")
            );
            let mut query = sqlx::query_as::<_, SemanticFact>(&sql).bind(since);
            for d in domains {
                query = query.bind(*d);
            }
            query.fetch_all(&self.pool).await
        } else {
            sqlx::query_as::<_, SemanticFact>(
                "SELECT * FROM semantic_facts WHERE superseded_at >= ?1 ORDER BY superseded_at DESC",
            )
            .bind(since)
            .fetch_all(&self.pool)
            .await
        }
    }

    /// Move superseded facts older than N days to the archive table.
    pub async fn archive_superseded(&self, older_than_days: i64) -> Result<u64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query(
            r#"
            INSERT INTO semantic_facts_archive
                (id, domain, subject, predicate, object, confidence, source,
                 valid_from, valid_until, recorded_at, superseded_at, superseded_by,
                 stability, last_accessed, access_count, project_id, memory_type,
                 scope_type, scope_id)
            SELECT id, domain, subject, predicate, object, confidence, source,
                   valid_from, valid_until, recorded_at, superseded_at, superseded_by,
                   stability, last_accessed, access_count, project_id, memory_type,
                   scope_type, scope_id
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
            scope_type: "system".to_string(),
            scope_id: None,
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
    async fn test_search_fts_basic() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let f1 = test_fact("f1", "productivity", "peak_hours", "morning routine at 9am");
        let f2 = test_fact("f2", "productivity", "break_pattern", "every 90 minutes");
        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let results = repo.search_fts("morning routine", None, 10).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].id, "f1");
    }

    #[tokio::test]
    async fn test_search_fts_with_domain_filter() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let f1 = test_fact("f1", "productivity", "peak_hours", "morning");
        let f2 = test_fact("f2", "finance", "budget", "morning expenses");
        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();

        let results = repo
            .search_fts("morning", Some("productivity"), 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "f1");
    }

    #[tokio::test]
    async fn test_prune_low_salience() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        // Low confidence, never accessed — should be pruned
        let mut f1 = test_fact("f1", "productivity", "noise", "irrelevant");
        f1.confidence = 0.02;

        // High confidence — should survive
        let f2 = test_fact("f2", "productivity", "peak_hours", "10am-12pm");

        // Low confidence but recently accessed — should survive (inactive_days = 180)
        let mut f3 = test_fact("f3", "productivity", "weak", "maybe");
        f3.confidence = 0.01;

        repo.upsert(&f1).await.unwrap();
        repo.upsert(&f2).await.unwrap();
        repo.upsert(&f3).await.unwrap();

        // Mark f3 as recently accessed
        repo.record_access("f3", 0.5).await.unwrap();

        // Prune: confidence < 0.05 AND inactive > 0 days
        // f1 has never been accessed (last_accessed IS NULL) → pruned
        // f3 was just accessed → survives (0 < 180 days inactive)
        let pruned = repo.prune_low_salience(0.05, 180).await.unwrap();
        assert_eq!(pruned, 1);

        // f1 gone, f2 and f3 remain
        assert!(repo.get("f1").await.unwrap().is_none());
        assert!(repo.get("f2").await.unwrap().is_some());
        assert!(repo.get("f3").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_scoped_fact_upsert_and_list() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        // Insert a squad-scoped fact
        let mut fact = test_fact("squad-f1", "finance", "recommended_by", "Deep Analyst");
        fact.scope_type = "squad".into();
        fact.scope_id = Some("builtin-squad-finance".into());
        fact.confidence = 0.9;
        repo.upsert(&fact).await.unwrap();

        // Insert a system-scoped fact
        let mut sys_fact = test_fact("system-f1", "finance", "index_funds", "low risk");
        sys_fact.scope_type = "system".into();
        sys_fact.scope_id = None;
        repo.upsert(&sys_fact).await.unwrap();

        // list_by_scope should return only squad-scoped
        let squad_facts = repo
            .list_by_scope("squad", Some("builtin-squad-finance"))
            .await
            .unwrap();
        assert_eq!(squad_facts.len(), 1);
        assert_eq!(squad_facts[0].id, "squad-f1");

        // list_by_scope for system should return only system-scoped
        let sys_facts = repo.list_by_scope("system", None).await.unwrap();
        assert_eq!(sys_facts.len(), 1);
        assert_eq!(sys_facts[0].id, "system-f1");

        // list_by_scope_chain should return both system + squad
        let chain = vec![("system", None), ("squad", Some("builtin-squad-finance"))];
        let all = repo.list_by_scope_chain(&chain).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_scoped_upsert_persists_scope_fields() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);

        let mut fact = test_fact("scoped-1", "finance", "pred", "val");
        fact.scope_type = "persona".into();
        fact.scope_id = Some("builtin-deep-analyst".into());
        repo.upsert(&fact).await.unwrap();

        let retrieved = repo.get("scoped-1").await.unwrap().unwrap();
        assert_eq!(retrieved.scope_type, "persona");
        assert_eq!(retrieved.scope_id.as_deref(), Some("builtin-deep-analyst"));
    }

    #[tokio::test]
    async fn test_reinstate_nonexistent_returns_false() {
        let pool = setup().await;
        let repo = SemanticFactRepo::new(pool);
        let reinstated = repo.reinstate_archived("nonexistent").await.unwrap();
        assert!(!reinstated);
    }
}
