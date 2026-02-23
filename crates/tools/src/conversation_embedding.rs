//! Conversation embedding storage and handler for semantic memory search.
//!
//! LanceDB-backed wrapper around `storage::VectorStore`.
//! All persistence is handled by LanceDB — no pgvector, no JSONL journals.
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
    pub content_full: String,    // Full message content
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

/// LanceDB-backed conversation embedding store.
///
/// All methods delegate to the underlying `VectorStore`. No in-memory caching,
/// no JSONL journals, no locks.
#[derive(Debug, Clone)]
pub struct ConversationEmbeddingStore {
    store: storage::VectorStore,
}

impl ConversationEmbeddingStore {
    /// Create a new store from a `VectorStore`.
    pub fn new(store: storage::VectorStore) -> Self {
        Self { store }
    }

    /// Upsert an embedding record (delete-then-insert semantics).
    pub async fn upsert(&self, record: ConversationEmbeddingRecord) -> Result<()> {
        self.store
            .upsert_embedding(
                "conv_embeddings",
                &record.id,
                &record.embedding,
                &[
                    ("session_key", &record.session_key),
                    ("role", &record.role),
                    ("content_preview", &record.content_preview),
                    ("full_content", &record.content_full),
                ],
            )
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;
        Ok(())
    }

    /// Search for similar embeddings. Returns `(record, score)` pairs.
    ///
    /// `decay_factor` is accepted for API compatibility but is not applied —
    /// LanceDB returns raw cosine similarity scores.
    pub async fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: usize,
        threshold: f64,
        _decay_factor: f64,
    ) -> Result<Vec<(ConversationEmbeddingRecord, f64)>> {
        let hits = self
            .store
            .search_conv_embeddings(query_embedding, limit, threshold)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        let records = hits
            .into_iter()
            .map(
                |(id, session_key, role, content_preview, full_content, score)| {
                    let record = ConversationEmbeddingRecord {
                        id,
                        session_key,
                        role,
                        content_preview,
                        content_full: full_content,
                        embedding: Vec::new(), // not re-hydrated from search results
                        model: String::new(),
                        embedded_at: Utc::now(), // approximate — not stored in search path
                    };
                    (record, score)
                },
            )
            .collect();
        Ok(records)
    }

    /// Get status information about the store.
    pub async fn status(&self) -> Result<ConversationEmbeddingStatus> {
        let total = self
            .store
            .count("conv_embeddings")
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ConversationEmbeddingStatus {
            total_embeddings: total,
            // Distinct session_key listing requires a full table scan not
            // supported by the current VectorStore API; omitted for now.
            indexed_channels: Vec::new(),
            oldest_embedding: None,
            newest_embedding: None,
            is_available: true,
        })
    }

    /// Purge embeddings matching the filter. Returns count of deleted records.
    pub async fn purge(&self, filter: PurgeFilter) -> Result<usize> {
        let before = self
            .store
            .count("conv_embeddings")
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        let predicate = match &filter {
            PurgeFilter::BySessionKey(sk) => {
                let safe = sk.replace('\'', "''");
                format!("session_key = '{safe}'")
            }
            PurgeFilter::Before(cutoff) => {
                let ts = cutoff.to_rfc3339();
                format!("created_at < '{ts}'")
            }
            PurgeFilter::All => "id IS NOT NULL".to_string(),
        };

        self.store
            .delete_where("conv_embeddings", &predicate)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        let after = self.store.count("conv_embeddings").await.unwrap_or(before);

        Ok(before.saturating_sub(after))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_has_content_full() {
        let record = ConversationEmbeddingRecord {
            id: "test".to_string(),
            session_key: "cli:default".to_string(),
            role: "user".to_string(),
            content_preview: "Hello...".to_string(),
            content_full: "Hello, how are you doing today?".to_string(),
            embedding: vec![0.0; 384],
            model: "test".to_string(),
            embedded_at: chrono::Utc::now(),
        };
        assert_eq!(record.content_full, "Hello, how are you doing today?");
    }
}
