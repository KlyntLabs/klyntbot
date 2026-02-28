//! Mock ConversationEmbeddingHandler for Phase 4.1 tests.
//!
//! Provides a controllable mock following the 3-mode pattern from Sprint 5.
//! Avoids loading the 420MB fastembed model in tests.

use super::embedding_utils::{cosine_similarity, deterministic_embedding};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Mutex;
use tools::conversation_embedding::{
    ConversationEmbeddingHandler, ConversationEmbeddingRecord, ConversationEmbeddingStatus,
    PurgeFilter,
};

/// Record of an embed_message call for test assertions
#[derive(Debug, Clone)]
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
            embed_message_calls: Mutex::new(Vec::new()),
            search_calls: Mutex::new(Vec::new()),
        }
    }

    /// Create a mock that reports as unavailable (simulates model download failure).
    pub fn unavailable() -> Self {
        Self {
            embeddings: Mutex::new(HashMap::new()),
            available: false,
            embed_message_calls: Mutex::new(Vec::new()),
            search_calls: Mutex::new(Vec::new()),
        }
    }

    /// Generate a deterministic 384-dim embedding from text.
    pub fn deterministic_embedding(text: &str) -> Vec<f32> {
        deterministic_embedding(text)
    }

    /// Get the number of embed_message calls made.
    pub fn embed_message_call_count(&self) -> usize {
        self.embed_message_calls.lock().unwrap().len()
    }

    /// Get the number of search calls made.
    pub fn search_call_count(&self) -> usize {
        self.search_calls.lock().unwrap().len()
    }

    /// Get the embed_message call records for detailed assertions.
    pub fn embed_message_calls(&self) -> Vec<EmbedCallRecord> {
        self.embed_message_calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl ConversationEmbeddingHandler for MockConversationEmbeddingHandler {
    async fn embed_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
        message_id: &str,
    ) -> ::common::Result<()> {
        self.embed_message_calls
            .lock()
            .unwrap()
            .push(EmbedCallRecord {
                session_key: session_key.to_string(),
                role: role.to_string(),
                content: content.to_string(),
                message_id: message_id.to_string(),
            });

        if !self.available {
            return Ok(());
        }

        let text = format!("{}: {}", role, content);
        let embedding = Self::deterministic_embedding(&text);

        self.embeddings
            .lock()
            .unwrap()
            .insert(message_id.to_string(), embedding);

        Ok(())
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> ::common::Result<Vec<(ConversationEmbeddingRecord, f64)>> {
        self.search_calls.lock().unwrap().push(query.to_string());

        if !self.available {
            return Err(::common::ToolError::ExecutionFailed(
                "Mock conversation embedding handler unavailable".to_string(),
            )
            .into());
        }

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
                    content_full: "Mock content".to_string(),
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

    async fn purge(&self, _filter: PurgeFilter) -> ::common::Result<usize> {
        let mut guard = self.embeddings.lock().unwrap();
        let count = guard.len();
        guard.clear();
        Ok(count)
    }

    async fn status(&self) -> ::common::Result<ConversationEmbeddingStatus> {
        let calls = self.embed_message_calls.lock().unwrap();
        let mut channels: Vec<String> = calls
            .iter()
            .filter_map(|c| c.session_key.split(':').next().map(String::from))
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        channels.sort();

        Ok(ConversationEmbeddingStatus {
            total_embeddings: self.embeddings.lock().unwrap().len(),
            indexed_channels: channels,
            oldest_embedding: None,
            newest_embedding: None,
            is_available: self.available,
        })
    }

    fn is_available(&self) -> bool {
        self.available
    }
}
