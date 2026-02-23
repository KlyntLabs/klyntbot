//! Repository for the `memory_note_embeddings` table.

use pgvector::Vector;
use sqlx::PgPool;

use crate::error::StorageError;

/// Search result with similarity score.
pub struct MemoryNoteMatch {
    pub note_key: String,
    pub content: String,
    pub similarity: f64,
}

/// Repository for memory note embeddings (pgvector ANN search).
#[derive(Debug, Clone)]
pub struct MemoryNoteEmbeddingRepo {
    pool: PgPool,
}

impl MemoryNoteEmbeddingRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Upsert an embedding for a memory note.
    pub async fn upsert(&self, note_key: &str, embedding: &[f32]) -> Result<(), StorageError> {
        let vec = Vector::from(embedding.to_vec());
        sqlx::query(
            r#"
            INSERT INTO memory_note_embeddings (note_key, embedding)
            VALUES ($1, $2)
            ON CONFLICT (note_key)
            DO UPDATE SET embedding = $2, updated_at = now()
            "#,
        )
        .bind(note_key)
        .bind(vec)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Find memory notes similar to a query embedding.
    /// Returns notes joined with `memory_notes.content`, ordered by similarity descending.
    pub async fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: i64,
        threshold: f64,
    ) -> Result<Vec<MemoryNoteMatch>, StorageError> {
        let vec = Vector::from(query_embedding.to_vec());
        let rows: Vec<(String, String, f64)> = sqlx::query_as(
            r#"
            SELECT e.note_key, m.content,
                   (1.0 - (e.embedding <=> $1))::float8 AS similarity
            FROM memory_note_embeddings e
            JOIN memory_notes m ON m.note_key = e.note_key
            WHERE (1.0 - (e.embedding <=> $1)) >= $3
            ORDER BY e.embedding <=> $1
            LIMIT $2
            "#,
        )
        .bind(vec)
        .bind(limit)
        .bind(threshold)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|(note_key, content, similarity)| MemoryNoteMatch {
                note_key,
                content,
                similarity,
            })
            .collect())
    }

    /// Delete an embedding by note key.
    pub async fn delete(&self, note_key: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM memory_note_embeddings WHERE note_key = $1")
            .bind(note_key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
