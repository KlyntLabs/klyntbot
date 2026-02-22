//! Row structs for `todo_embeddings` and `conversation_embeddings` tables.

use chrono::{DateTime, Utc};
use pgvector::Vector;
use sqlx::FromRow;

/// Row struct for the `todo_embeddings` table.
#[derive(Debug, Clone, FromRow)]
pub struct EmbeddingRow {
    pub todo_id: String,
    pub embedding: Vector,
    pub model: String,
    pub updated_at: DateTime<Utc>,
}

/// Row struct for the `conversation_embeddings` table.
#[derive(Debug, Clone, FromRow)]
pub struct ConvEmbeddingRow {
    pub id: uuid::Uuid,
    pub session_key: String,
    pub embedding: Vector,
    pub role: String,
    pub content_preview: String,
    pub content_full: String,
    pub created_at: DateTime<Utc>,
}
