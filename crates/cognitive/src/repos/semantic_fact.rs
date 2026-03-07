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
                stability, last_accessed, access_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
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
                access_count = excluded.access_count
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

    /// Move superseded facts older than N days to the archive table.
    pub async fn archive_superseded(&self, older_than_days: i64) -> Result<u64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query(
            r#"
            INSERT INTO semantic_facts_archive
                (id, domain, subject, predicate, object, confidence, source,
                 valid_from, valid_until, recorded_at, superseded_at, superseded_by,
                 stability, last_accessed, access_count)
            SELECT id, domain, subject, predicate, object, confidence, source,
                   valid_from, valid_until, recorded_at, superseded_at, superseded_by,
                   stability, last_accessed, access_count
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
}
