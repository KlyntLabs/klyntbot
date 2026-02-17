//! Mock ConversationEmbeddingHandler for Phase 4.1 tests.
//!
//! Provides a controllable mock following the 3-mode pattern from Sprint 5.
//! Avoids loading the 420MB fastembed model in tests.
//!
//! ## Mock Modes
//!   1. `new()` — available, returns deterministic embeddings based on text hash
//!   2. `unavailable()` — simulates model not loaded / download failure
//!   3. `with_store(store)` — persists embeddings to ConversationEmbeddingStore (like real impl)

use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tools::conversation_embedding::{
    ConversationEmbeddingHandler, ConversationEmbeddingRecord, ConversationEmbeddingStatus,
    ConversationEmbeddingStore, PurgeFilter,
};
use tools::EMBEDDING_DIM;

/// Record of an embed_message call for test assertions
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EmbedCallRecord {
    pub session_key: String,
    pub role: String,
    pub content: String,
    pub message_id: String,
}

/// Mock conversation embedding handler for testing.
pub struct MockConversationEmbeddingHandler {
    /// Pre-loaded embeddings keyed by message ID
    pub embeddings: Mutex<HashMap<String, Vec<f32>>>,
    /// Whether the mock should report as available
    pub available: bool,
    /// Optional store to persist embeddings (like the real impl)
    pub store: Option<Arc<RwLock<ConversationEmbeddingStore>>>,
    /// Track embed_message calls for assertions
    pub embed_message_calls: Mutex<Vec<EmbedCallRecord>>,
    /// Track search calls for assertions
    pub search_calls: Mutex<Vec<String>>,
}

impl Default for MockConversationEmbeddingHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl MockConversationEmbeddingHandler {
    /// Create an available mock that generates deterministic embeddings.
    pub fn new() -> Self {
        Self {
            embeddings: Mutex::new(HashMap::new()),
            available: true,
            store: None,
            embed_message_calls: Mutex::new(Vec::new()),
            search_calls: Mutex::new(Vec::new()),
        }
    }

    /// Create an available mock backed by a ConversationEmbeddingStore (persists embeddings).
    #[allow(dead_code)]
    pub fn with_store(store: Arc<RwLock<ConversationEmbeddingStore>>) -> Self {
        Self {
            embeddings: Mutex::new(HashMap::new()),
            available: true,
            store: Some(store),
            embed_message_calls: Mutex::new(Vec::new()),
            search_calls: Mutex::new(Vec::new()),
        }
    }

    /// Create a mock that reports as unavailable (simulates model download failure).
    pub fn unavailable() -> Self {
        Self {
            embeddings: Mutex::new(HashMap::new()),
            available: false,
            store: None,
            embed_message_calls: Mutex::new(Vec::new()),
            search_calls: Mutex::new(Vec::new()),
        }
    }

    /// Generate a deterministic 384-dim embedding from text.
    /// Uses a simple hash-based approach so identical text produces identical vectors.
    /// Follows the same pattern as MockEmbeddingHandler from Sprint 5.
    pub fn deterministic_embedding(text: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; EMBEDDING_DIM];
        // Simple deterministic fill: use byte values spread across dims
        for (i, byte) in text.bytes().enumerate() {
            let idx = i % EMBEDDING_DIM;
            vec[idx] += (byte as f32) / 255.0;
        }
        // Normalize to unit vector
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in &mut vec {
                *v /= norm;
            }
        }
        vec
    }

    /// Get the number of embed_message calls made.
    #[allow(dead_code)]
    pub fn embed_message_call_count(&self) -> usize {
        self.embed_message_calls.lock().unwrap().len()
    }

    /// Get the number of search calls made.
    #[allow(dead_code)]
    pub fn search_call_count(&self) -> usize {
        self.search_calls.lock().unwrap().len()
    }

    /// Get the embed_message call records for detailed assertions.
    #[allow(dead_code)]
    pub fn embed_message_calls(&self) -> Vec<EmbedCallRecord> {
        self.embed_message_calls.lock().unwrap().clone()
    }
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    (dot / (norm_a * norm_b)) as f64
}

#[async_trait]
impl ConversationEmbeddingHandler for MockConversationEmbeddingHandler {
    async fn embed_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
        message_id: &str,
    ) -> Result<()> {
        // Track the call
        self.embed_message_calls.lock().unwrap().push(EmbedCallRecord {
            session_key: session_key.to_string(),
            role: role.to_string(),
            content: content.to_string(),
            message_id: message_id.to_string(),
        });

        if !self.available {
            // Unavailable mode: return Ok(()) to simulate best-effort embedding
            return Ok(());
        }

        // Compose text with role prefix (same as real impl)
        let text = format!("{}: {}", role, content);
        let embedding = Self::deterministic_embedding(&text);

        // Store in-memory
        self.embeddings
            .lock()
            .unwrap()
            .insert(message_id.to_string(), embedding.clone());

        // Persist to store if available
        if let Some(ref store) = self.store {
            let record = ConversationEmbeddingRecord {
                id: message_id.to_string(),
                session_key: session_key.to_string(),
                role: role.to_string(),
                content_preview: content.chars().take(100).collect(),
                embedding,
                model: "mock-model".to_string(),
                embedded_at: Utc::now(),
            };
            let store = store.write().await;
            store.upsert(record).await?;
        }

        Ok(())
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> Result<Vec<(ConversationEmbeddingRecord, f64)>> {
        // Track the call
        self.search_calls.lock().unwrap().push(query.to_string());

        if !self.available {
            return Err(common::ToolError::ExecutionFailed(
                "Mock conversation embedding handler unavailable".to_string(),
            )
            .into());
        }

        // If we have a store, search it
        if let Some(ref store) = self.store {
            let store = store.write().await;
            let query_embedding = Self::deterministic_embedding(query);
            let all_records = store.get_all().await?;

            // Compute cosine similarity for each record
            let mut results: Vec<(ConversationEmbeddingRecord, f64)> = all_records
                .values()
                .map(|record| {
                    let similarity = cosine_similarity(&query_embedding, &record.embedding);
                    (record.clone(), similarity)
                })
                .filter(|(_, score)| *score >= threshold)
                .collect();

            // Sort by similarity descending
            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Limit results
            results.truncate(limit);

            Ok(results)
        } else {
            // No store: search in-memory embeddings
            let embeddings = self.embeddings.lock().unwrap();
            let query_embedding = Self::deterministic_embedding(query);

            let mut results: Vec<(ConversationEmbeddingRecord, f64)> = embeddings
                .iter()
                .map(|(id, emb)| {
                    let similarity = cosine_similarity(&query_embedding, emb);
                    let record = ConversationEmbeddingRecord {
                        id: id.clone(),
                        session_key: "mock:session".to_string(),
                        role: "user".to_string(),
                        content_preview: "Mock content".to_string(),
                        embedding: emb.clone(),
                        model: "mock-model".to_string(),
                        embedded_at: Utc::now(),
                    };
                    (record, similarity)
                })
                .filter(|(_, score)| *score >= threshold)
                .collect();

            results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            results.truncate(limit);

            Ok(results)
        }
    }

    async fn purge(&self, filter: PurgeFilter) -> Result<usize> {
        if let Some(ref store) = self.store {
            let store = store.write().await;
            store.purge(filter).await
        } else {
            // No store: just clear in-memory embeddings
            let count = self.embeddings.lock().unwrap().len();
            self.embeddings.lock().unwrap().clear();
            Ok(count)
        }
    }

    async fn status(&self) -> Result<ConversationEmbeddingStatus> {
        if let Some(ref store) = self.store {
            let store = store.write().await;
            store.status().await
        } else {
            Ok(ConversationEmbeddingStatus {
                total_embeddings: self.embeddings.lock().unwrap().len(),
                indexed_channels: vec![],
                oldest_embedding: None,
                newest_embedding: None,
                is_available: self.available,
            })
        }
    }

    fn is_available(&self) -> bool {
        self.available
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_deterministic_embedding_consistency() {
        let v1 = MockConversationEmbeddingHandler::deterministic_embedding("hello world");
        let v2 = MockConversationEmbeddingHandler::deterministic_embedding("hello world");
        assert_eq!(v1, v2, "Same text should produce same embedding");
    }

    #[test]
    fn test_mock_deterministic_embedding_dimension() {
        let v = MockConversationEmbeddingHandler::deterministic_embedding("test");
        assert_eq!(v.len(), EMBEDDING_DIM, "Should be 384 dimensions");
    }

    #[test]
    fn test_mock_deterministic_embedding_unit_norm() {
        let v = MockConversationEmbeddingHandler::deterministic_embedding("test input");
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "Expected unit norm, got {}",
            norm
        );
    }

    #[tokio::test]
    async fn test_mock_unavailable_returns_ok() {
        let mock = MockConversationEmbeddingHandler::unavailable();
        let result = mock
            .embed_message("telegram:12345", "user", "test", "msg1")
            .await;
        assert!(result.is_ok(), "Unavailable mode should return Ok (best-effort)");
    }

    #[tokio::test]
    async fn test_mock_unavailable_search_returns_error() {
        let mock = MockConversationEmbeddingHandler::unavailable();
        let result = mock.search("test", 10, 0.5).await;
        assert!(result.is_err(), "Unavailable mode should error on search");
    }

    #[tokio::test]
    async fn test_mock_tracks_calls() {
        let mock = MockConversationEmbeddingHandler::new();
        mock.embed_message("telegram:12345", "user", "Hello", "msg1")
            .await
            .unwrap();
        mock.search("test query", 10, 0.5).await.unwrap();

        assert_eq!(mock.embed_message_call_count(), 1);
        assert_eq!(mock.search_call_count(), 1);

        let calls = mock.embed_message_calls();
        assert_eq!(calls[0].role, "user");
        assert_eq!(calls[0].content, "Hello");
    }
}
