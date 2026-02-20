//! Embedding repository — todo_embeddings table (pgvector).

use chrono::Utc;
use pgvector::Vector;
use sqlx::PgPool;

use crate::error::StorageError;
use crate::rows::embedding::EmbeddingRow;

/// Repository for todo embedding persistence and ANN search.
#[derive(Debug, Clone)]
pub struct EmbeddingRepo {
    pool: PgPool,
}

impl EmbeddingRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Upsert an embedding for a todo.
    pub async fn upsert(
        &self,
        todo_id: &str,
        embedding: &Vector,
        model: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO todo_embeddings (todo_id, embedding, model, updated_at)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (todo_id) DO UPDATE SET embedding = $2, model = $3, updated_at = $4",
        )
        .bind(todo_id)
        .bind(embedding)
        .bind(model)
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get the embedding for a todo.
    pub async fn get(&self, todo_id: &str) -> Result<EmbeddingRow, StorageError> {
        sqlx::query_as::<_, EmbeddingRow>("SELECT * FROM todo_embeddings WHERE todo_id = $1")
            .bind(todo_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| StorageError::NotFound(format!("embedding for todo '{}'", todo_id)))
    }

    /// Delete the embedding for a todo.
    pub async fn delete(&self, todo_id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM todo_embeddings WHERE todo_id = $1")
            .bind(todo_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Search for similar embeddings using cosine distance (`<=>`).
    ///
    /// Uses a CTE so the distance is computed once per row instead of
    /// three times (SELECT, WHERE, ORDER BY).
    pub async fn search_similar(
        &self,
        query_embedding: &Vector,
        limit: i64,
        threshold: f64,
    ) -> Result<Vec<(EmbeddingRow, f64)>, StorageError> {
        // pgvector <=> returns cosine distance (0 = identical, 2 = opposite).
        // Convert threshold from similarity (1 = identical) to distance.
        let max_distance = 1.0 - threshold;

        let rows: Vec<EmbeddingWithDistance> = sqlx::query_as(
            "WITH q AS (
                 SELECT todo_id, embedding, model, updated_at,
                        (embedding <=> $1) AS distance
                 FROM todo_embeddings
             )
             SELECT todo_id, embedding, model, updated_at, distance
             FROM q
             WHERE distance <= $2
             ORDER BY distance
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
                let row = EmbeddingRow {
                    todo_id: r.todo_id,
                    embedding: r.embedding,
                    model: r.model,
                    updated_at: r.updated_at,
                };
                (row, similarity)
            })
            .collect())
    }

    /// Convenience: upsert from a `Vec<f32>` (avoids pgvector dependency in consumers).
    pub async fn upsert_vec(
        &self,
        todo_id: &str,
        embedding: &[f32],
        model: &str,
    ) -> Result<(), StorageError> {
        let vec = Vector::from(embedding.to_vec());
        self.upsert(todo_id, &vec, model).await
    }

    /// Convenience: search using a `Vec<f32>` query (avoids pgvector dependency in consumers).
    pub async fn search_similar_vec(
        &self,
        query_embedding: &[f32],
        limit: i64,
        threshold: f64,
    ) -> Result<Vec<(String, f64)>, StorageError> {
        let vec = Vector::from(query_embedding.to_vec());
        let results = self.search_similar(&vec, limit, threshold).await?;
        Ok(results
            .into_iter()
            .map(|(row, sim)| (row.todo_id, sim))
            .collect())
    }

    /// Count the total number of embeddings.
    pub async fn count(&self) -> Result<i64, StorageError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM todo_embeddings")
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Bulk upsert embeddings (for backfill).
    pub async fn bulk_upsert(
        &self,
        items: &[(String, Vector, String)],
    ) -> Result<u64, StorageError> {
        let now = Utc::now();
        let mut count = 0u64;
        for (todo_id, embedding, model) in items {
            sqlx::query(
                "INSERT INTO todo_embeddings (todo_id, embedding, model, updated_at)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (todo_id) DO UPDATE SET embedding = $2, model = $3, updated_at = $4",
            )
            .bind(todo_id)
            .bind(embedding)
            .bind(model)
            .bind(now)
            .execute(&self.pool)
            .await?;
            count += 1;
        }
        Ok(count)
    }
}

/// Internal helper row with distance column for ANN queries.
#[derive(sqlx::FromRow)]
struct EmbeddingWithDistance {
    pub todo_id: String,
    pub embedding: Vector,
    pub model: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub distance: f64,
}
