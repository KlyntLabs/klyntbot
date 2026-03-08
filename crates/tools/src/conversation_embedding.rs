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
    /// Applies time-decay scoring: `adjusted_score = similarity × decay_factor^days_old`.
    /// Use `decay_factor = 1.0` to disable decay.
    pub async fn search_similar(
        &self,
        query_embedding: &[f32],
        limit: usize,
        threshold: f64,
        decay_factor: f64,
    ) -> Result<Vec<(ConversationEmbeddingRecord, f64)>> {
        // Fetch extra results before decay filtering so we still return enough.
        let fetch_limit = if decay_factor < 1.0 { limit * 2 } else { limit };
        let hits = self
            .store
            .search_conv_embeddings(query_embedding, fetch_limit, threshold)
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        let now = Utc::now();
        let mut records: Vec<(ConversationEmbeddingRecord, f64)> = hits
            .into_iter()
            .map(
                |(id, session_key, role, content_preview, full_content, created_at, score)| {
                    let embedded_at = chrono::DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or(now);

                    let adjusted_score = if decay_factor < 1.0 {
                        let days_old = (now - embedded_at).num_seconds().max(0) as f64 / 86400.0;
                        score * decay_factor.powf(days_old)
                    } else {
                        score
                    };

                    let record = ConversationEmbeddingRecord {
                        id,
                        session_key,
                        role,
                        content_preview,
                        content_full: full_content,
                        embedding: Vec::new(),
                        model: String::new(),
                        embedded_at,
                    };
                    (record, adjusted_score)
                },
            )
            .collect();

        // Re-sort by decayed score, filter below-threshold, then apply limit.
        if decay_factor < 1.0 {
            records.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            records.retain(|(_, score)| *score >= threshold);
            records.truncate(limit);
        }

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

    #[test]
    fn test_decay_factor_math() {
        // decay_factor = 0.995 should give ~50% weight at 138 days (half-life).
        let decay_factor = 0.995_f64;
        let days_old = 138.0_f64;
        let weight = decay_factor.powf(days_old);
        assert!(
            (weight - 0.5).abs() < 0.01,
            "Expected ~0.5 at 138-day half-life, got {weight}"
        );

        // A 1-day-old result should retain ~99.5% of its score.
        let one_day = decay_factor.powf(1.0);
        assert!(
            (one_day - 0.995).abs() < 0.001,
            "Expected ~0.995 at 1 day, got {one_day}"
        );

        // A 365-day-old result should be ~16% of its original score.
        let one_year = decay_factor.powf(365.0);
        assert!(
            one_year < 0.20 && one_year > 0.10,
            "Expected 10-20% at 365 days, got {one_year}"
        );
    }

    #[test]
    fn test_decay_factor_one_means_no_decay() {
        let decay_factor = 1.0_f64;
        let weight = decay_factor.powf(1000.0);
        assert_eq!(weight, 1.0, "decay_factor=1.0 should mean no decay");
    }
}
