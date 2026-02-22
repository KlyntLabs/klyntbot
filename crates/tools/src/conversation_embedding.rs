//! Conversation embedding storage and handler for semantic memory search.
//!
//! SQL-only wrapper around `ConvEmbeddingRepo` (pgvector).
//! All persistence is handled by PostgreSQL — no JSONL journals or in-memory indexes.
//!
//! The `ConversationEmbeddingHandler` trait provides dependency inversion for
//! the embedding pipeline (defined in tools crate L3, implemented in agent crate L5).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use common::Result;

/// A single conversation embedding record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEmbeddingRecord {
    pub id: String,              // Message UUID from SessionMessage.id
    pub session_key: String,     // "channel:chat_id"
    pub role: String,            // "user" | "assistant"
    pub content_preview: String, // First 100 chars
    pub embedding: Vec<f32>,     // 384 dimensions
    pub model: String,
    pub embedded_at: DateTime<Utc>,
}

/// Filter for purging embeddings.
#[derive(Debug, Clone)]
pub enum PurgeFilter {
    /// Delete all embeddings for a specific session.
    BySessionKey(String),
    /// Delete all embeddings embedded before a specific date.
    Before(DateTime<Utc>),
    /// Delete all embeddings.
    All,
}

/// Status information about the conversation embedding store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEmbeddingStatus {
    pub total_embeddings: usize,
    pub indexed_channels: Vec<String>,
    pub oldest_embedding: Option<DateTime<Utc>>,
    pub newest_embedding: Option<DateTime<Utc>>,
    pub is_available: bool,
}

/// SQL-only conversation embedding store backed by `ConvEmbeddingRepo` (pgvector).
///
/// All methods delegate directly to the repository. No in-memory caching,
/// no JSONL journals, no locks.
#[derive(Debug, Clone)]
pub struct ConversationEmbeddingStore {
    repo: storage::ConvEmbeddingRepo,
}

impl ConversationEmbeddingStore {
    /// Create a new SQL-only store from a `ConvEmbeddingRepo`.
    pub fn new(repo: storage::ConvEmbeddingRepo) -> Self {
        Self { repo }
    }

    /// Upsert an embedding record. Deletes existing record with the same ID first.
    pub async fn upsert(&self, record: ConversationEmbeddingRecord) -> Result<()> {
        let id = uuid::Uuid::parse_str(&record.id).unwrap_or_else(|_| uuid::Uuid::new_v4());
        let vector = pgvector::Vector::from(record.embedding);
        // Delete existing first (upsert semantics)
        let _ = self.repo.delete(id).await;
        self.repo
            .insert(
                id,
                &record.session_key,
                &vector,
                &record.role,
                &record.content_preview,
            )
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        Ok(())
    }

    /// Get a single embedding record by message ID.
    pub async fn get(&self, id: &str) -> Result<Option<ConversationEmbeddingRecord>> {
        let uuid = match uuid::Uuid::parse_str(id) {
            Ok(u) => u,
            Err(_) => return Ok(None),
        };
        match self.repo.get(uuid).await {
            Ok(row) => Ok(Some(row_to_record(row))),
            Err(storage::StorageError::NotFound(_)) => Ok(None),
            Err(e) => Err(common::ToolError::ExecutionFailed(e.to_string()).into()),
        }
    }

    /// Get all embedding records matching a session key.
    pub async fn get_by_session_key(
        &self,
        session_key: &str,
    ) -> Result<Vec<ConversationEmbeddingRecord>> {
        let rows = self
            .repo
            .get_by_session_key(session_key)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        Ok(rows.into_iter().map(row_to_record).collect())
    }

    /// Search for similar embeddings using pgvector ANN (cosine distance).
    ///
    /// Cross-channel: searches ALL conversation embeddings regardless of session_key.
    pub async fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<(ConversationEmbeddingRecord, f64)>> {
        let vector = pgvector::Vector::from(query_embedding.to_vec());
        let rows = self
            .repo
            .search_similar(&vector, limit as i64, threshold)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|(row, sim)| (row_to_record(row), sim))
            .collect())
    }

    /// Get status information about the store.
    pub async fn status(&self) -> Result<ConversationEmbeddingStatus> {
        let total = self
            .repo
            .count()
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        let indexed_channels = self
            .repo
            .list_session_keys()
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        let oldest_embedding = self
            .repo
            .oldest_created_at()
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        let newest_embedding = self
            .repo
            .newest_created_at()
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ConversationEmbeddingStatus {
            total_embeddings: total as usize,
            indexed_channels,
            oldest_embedding,
            newest_embedding,
            is_available: true,
        })
    }

    /// Purge embeddings matching the filter. Returns count of deleted records.
    pub async fn purge(&self, filter: PurgeFilter) -> Result<usize> {
        let count = match filter {
            PurgeFilter::BySessionKey(session_key) => self
                .repo
                .purge_by_session_key(&session_key)
                .await
                .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?,
            PurgeFilter::Before(cutoff) => self
                .repo
                .purge_before(cutoff)
                .await
                .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?,
            PurgeFilter::All => self
                .repo
                .purge_all()
                .await
                .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?,
        };
        Ok(count as usize)
    }
}

/// Convert a `ConvEmbeddingRow` to a `ConversationEmbeddingRecord`.
fn row_to_record(row: storage::ConvEmbeddingRow) -> ConversationEmbeddingRecord {
    ConversationEmbeddingRecord {
        id: row.id.to_string(),
        session_key: row.session_key,
        role: row.role,
        content_preview: row.content_preview,
        embedding: row.embedding.to_vec(),
        model: String::new(), // model info not stored in DB row
        embedded_at: row.created_at,
    }
}

/// ConversationEmbeddingHandler trait for dependency inversion.
/// Implemented by ConversationEmbeddingHandlerImpl in agent crate (Layer 5).
/// Defined here in tools crate (Layer 3) to break circular dependency.
#[async_trait]
pub trait ConversationEmbeddingHandler: Send + Sync {
    /// Embed a message and store it.
    async fn embed_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
        message_id: &str,
    ) -> Result<()>;

    /// Search for similar conversations using cosine similarity.
    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<(ConversationEmbeddingRecord, f64)>>;

    /// Purge embeddings matching the filter.
    async fn purge(&self, filter: PurgeFilter) -> Result<usize>;

    /// Get status information about the embedding store.
    async fn status(&self) -> Result<ConversationEmbeddingStatus>;

    /// Check if the handler is available (model loaded, etc.).
    fn is_available(&self) -> bool;
}
