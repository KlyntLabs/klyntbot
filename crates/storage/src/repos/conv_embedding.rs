//! Conversation embedding repository — conversation_embeddings table (pgvector).

use chrono::{DateTime, Utc};
use pgvector::Vector;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::StorageError;
use crate::rows::embedding::ConvEmbeddingRow;

/// Repository for conversation embedding persistence and ANN search.
#[derive(Debug, Clone)]
pub struct ConvEmbeddingRepo {
    pool: PgPool,
}

impl ConvEmbeddingRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insert a conversation embedding.
    pub async fn insert(
        &self,
        id: Uuid,
        session_key: &str,
        embedding: &Vector,
        role: &str,
        content_preview: &str,
        content_full: &str,
    ) -> Result<ConvEmbeddingRow, StorageError> {
        let row = sqlx::query_as::<_, ConvEmbeddingRow>(
            "INSERT INTO conversation_embeddings (id, session_key, embedding, role, content_preview, content_full, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             RETURNING *",
        )
        .bind(id)
        .bind(session_key)
        .bind(embedding)
        .bind(role)
        .bind(content_preview)
        .bind(content_full)
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get a conversation embedding by ID.
    pub async fn get(&self, id: Uuid) -> Result<ConvEmbeddingRow, StorageError> {
        sqlx::query_as::<_, ConvEmbeddingRow>("SELECT * FROM conversation_embeddings WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::NotFound(format!("conversation embedding '{}'", id)))
    }

    /// Delete a conversation embedding by ID.
    pub async fn delete(&self, id: Uuid) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM conversation_embeddings WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Search for similar conversation embeddings using cosine distance with time-decay scoring.
    ///
    /// The final score is `cosine_similarity * decay_factor^days_old`, giving recent
    /// embeddings higher weight. Use `decay_factor = 1.0` to disable decay.
    /// A typical value is `0.995` (half-life ≈ 138 days: `0.995^138 ≈ 0.5`).
    pub async fn search_similar(
        &self,
        query_embedding: &Vector,
        limit: i64,
        threshold: f64,
        decay_factor: f64,
    ) -> Result<Vec<(ConvEmbeddingRow, f64)>, StorageError> {
        let rows: Vec<ConvEmbeddingWithScore> = sqlx::query_as(
            "SELECT id, session_key, embedding, role, content_preview, content_full, created_at,
                    (1.0 - (embedding <=> $1)) * power($4, EXTRACT(EPOCH FROM now() - created_at) / 86400.0) AS score
             FROM conversation_embeddings
             WHERE (1.0 - (embedding <=> $1)) >= $3
             ORDER BY score DESC
             LIMIT $2",
        )
        .bind(query_embedding)
        .bind(limit)
        .bind(threshold)
        .bind(decay_factor)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let row = ConvEmbeddingRow {
                    id: r.id,
                    session_key: r.session_key,
                    embedding: r.embedding,
                    role: r.role,
                    content_preview: r.content_preview,
                    content_full: r.content_full,
                    created_at: r.created_at,
                };
                (row, r.score)
            })
            .collect())
    }

    /// Delete embeddings older than `max_age_days`. Returns count deleted.
    pub async fn delete_old_embeddings(&self, max_age_days: u32) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "DELETE FROM conversation_embeddings WHERE created_at < now() - ($1 || ' days')::interval",
        )
        .bind(max_age_days as i64)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Get all conversation embeddings.
    pub async fn get_all(&self) -> Result<Vec<ConvEmbeddingRow>, StorageError> {
        let rows = sqlx::query_as::<_, ConvEmbeddingRow>(
            "SELECT * FROM conversation_embeddings ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get conversation embeddings by session key.
    pub async fn get_by_session_key(
        &self,
        session_key: &str,
    ) -> Result<Vec<ConvEmbeddingRow>, StorageError> {
        let rows = sqlx::query_as::<_, ConvEmbeddingRow>(
            "SELECT * FROM conversation_embeddings WHERE session_key = $1 ORDER BY created_at",
        )
        .bind(session_key)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count total conversation embeddings.
    pub async fn count(&self) -> Result<i64, StorageError> {
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM conversation_embeddings")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// List distinct session keys.
    pub async fn list_session_keys(&self) -> Result<Vec<String>, StorageError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT session_key FROM conversation_embeddings ORDER BY session_key",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    /// Get the oldest embedding timestamp.
    pub async fn oldest_created_at(&self) -> Result<Option<DateTime<Utc>>, StorageError> {
        let row: Option<(DateTime<Utc>,)> =
            sqlx::query_as("SELECT MIN(created_at) FROM conversation_embeddings")
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Get the newest embedding timestamp.
    pub async fn newest_created_at(&self) -> Result<Option<DateTime<Utc>>, StorageError> {
        let row: Option<(DateTime<Utc>,)> =
            sqlx::query_as("SELECT MAX(created_at) FROM conversation_embeddings")
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.map(|r| r.0))
    }

    /// Delete all embeddings for a session key. Returns count deleted.
    pub async fn purge_by_session_key(&self, session_key: &str) -> Result<u64, StorageError> {
        let result = sqlx::query("DELETE FROM conversation_embeddings WHERE session_key = $1")
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete all embeddings created before a cutoff date. Returns count deleted.
    pub async fn purge_before(&self, cutoff: DateTime<Utc>) -> Result<u64, StorageError> {
        let result = sqlx::query("DELETE FROM conversation_embeddings WHERE created_at < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    /// Delete all conversation embeddings. Returns count deleted.
    pub async fn purge_all(&self) -> Result<u64, StorageError> {
        let result = sqlx::query("DELETE FROM conversation_embeddings")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }
}

/// Internal helper row with time-decayed score column for ANN queries.
#[derive(sqlx::FromRow)]
struct ConvEmbeddingWithScore {
    pub id: Uuid,
    pub session_key: String,
    pub embedding: Vector,
    pub role: String,
    pub content_preview: String,
    pub content_full: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub score: f64,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_decay_factor_calculation() {
        let decay_factor = 0.995_f64;
        let days = 138.0;
        let weight = decay_factor.powf(days);
        assert!(
            (weight - 0.5).abs() < 0.01,
            "Expected ~0.5 at 138 days with decay_factor=0.995, got {}",
            weight
        );
    }

    #[test]
    fn test_decay_factor_no_decay() {
        // decay_factor=1.0 means no decay (score == similarity)
        let decay_factor = 1.0_f64;
        let weight = decay_factor.powf(365.0);
        assert_eq!(weight, 1.0, "decay_factor=1.0 should give weight 1.0 at any age");
    }
}
