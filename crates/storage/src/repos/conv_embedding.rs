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
    ) -> Result<ConvEmbeddingRow, StorageError> {
        let row = sqlx::query_as::<_, ConvEmbeddingRow>(
            "INSERT INTO conversation_embeddings (id, session_key, embedding, role, content_preview, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)
             RETURNING *",
        )
        .bind(id)
        .bind(session_key)
        .bind(embedding)
        .bind(role)
        .bind(content_preview)
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

    /// Search for similar conversation embeddings using cosine distance.
    pub async fn search_similar(
        &self,
        query_embedding: &Vector,
        limit: i64,
        threshold: f64,
    ) -> Result<Vec<(ConvEmbeddingRow, f64)>, StorageError> {
        let max_distance = 1.0 - threshold;

        let rows: Vec<ConvEmbeddingWithDistance> = sqlx::query_as(
            "SELECT id, session_key, embedding, role, content_preview, created_at,
                    (embedding <=> $1) AS distance
             FROM conversation_embeddings
             WHERE (embedding <=> $1) <= $2
             ORDER BY embedding <=> $1
             LIMIT $3",
        )
        .bind(query_embedding)
        .bind(max_distance)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let similarity = 1.0 - r.distance;
                let row = ConvEmbeddingRow {
                    id: r.id,
                    session_key: r.session_key,
                    embedding: r.embedding,
                    role: r.role,
                    content_preview: r.content_preview,
                    created_at: r.created_at,
                };
                (row, similarity)
            })
            .collect())
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

/// Internal helper row with distance column for ANN queries.
#[derive(sqlx::FromRow)]
struct ConvEmbeddingWithDistance {
    pub id: Uuid,
    pub session_key: String,
    pub embedding: Vector,
    pub role: String,
    pub content_preview: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub distance: f64,
}
