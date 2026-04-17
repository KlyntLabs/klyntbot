//! Repository for retrieval feedback tracking.
//!
//! Records which retrieved facts the LLM actually referenced in its response,
//! enabling the autotuner to evaluate retrieval quality.

use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct RetrievalFeedbackRepo {
    pool: SqlitePool,
}

impl RetrievalFeedbackRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a feedback record. Computes precision as `referenced / retrieved`.
    pub async fn insert(
        &self,
        retrieved_ids: &[String],
        referenced_ids: &[String],
        session_key: &str,
        score_breakdown: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        if retrieved_ids.is_empty() {
            return Ok(());
        }
        let precision = referenced_ids.len() as f64 / retrieved_ids.len() as f64;
        let id = uuid::Uuid::new_v4().to_string();
        let retrieved_json = serde_json::to_string(retrieved_ids).unwrap_or_default();
        let referenced_json = serde_json::to_string(referenced_ids).unwrap_or_default();

        sqlx::query(
            "INSERT INTO retrieval_feedback (id, retrieved_fact_ids, referenced_fact_ids, precision, session_key, score_breakdown)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )
        .bind(&id)
        .bind(&retrieved_json)
        .bind(&referenced_json)
        .bind(precision)
        .bind(session_key)
        .bind(score_breakdown)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Average precision over the past N days.
    pub async fn avg_precision_since(&self, days: i64) -> Result<f64, sqlx::Error> {
        let cutoff = (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(days * 24)).as_millisecond();
        let row: (f64,) = sqlx::query_as(
            "SELECT COALESCE(AVG(precision), 0.0) FROM retrieval_feedback WHERE created_at >= ?1",
        )
        .bind(&cutoff)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    /// Average retrieval precision grouped by fact domain.
    /// Joins retrieval_feedback against semantic_facts to determine domain.
    pub async fn avg_precision_by_domain_since(
        &self,
        days: i64,
    ) -> Result<Vec<(String, f64)>, sqlx::Error> {
        let since = (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(days * 24)).as_millisecond();
        let rows: Vec<(String, f64)> = sqlx::query_as(
            "SELECT f.domain, AVG(rf.precision) as avg_precision
             FROM retrieval_feedback rf,
                  json_each(rf.retrieved_fact_ids) je
             JOIN semantic_facts f ON f.id = je.value
             WHERE rf.created_at > ?1
             GROUP BY f.domain
             HAVING COUNT(*) >= 3
             ORDER BY avg_precision ASC",
        )
        .bind(&since)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count feedback entries in the past N days.
    pub async fn count_since(&self, days: i64) -> Result<i64, sqlx::Error> {
        let cutoff = (jiff::Timestamp::now() - jiff::SignedDuration::from_hours(days * 24)).as_millisecond();
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM retrieval_feedback WHERE created_at >= ?1")
                .bind(&cutoff)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }
}
