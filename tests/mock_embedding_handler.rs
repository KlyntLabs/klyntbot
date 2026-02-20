//! Mock EmbeddingHandler for Sprint 5 tests.
//!
//! Provides a controllable mock that avoids loading the 420MB fastembed model
//! in tests. Supports three modes:
//!   1. `new()` — available, returns deterministic embeddings based on text hash
//!   2. `unavailable()` — simulates model not loaded / download failure
//!   3. `with_embeddings(map)` — returns pre-loaded embeddings for specific IDs

use async_trait::async_trait;
use chrono::Utc;
use common::Result;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tools::embedding_engine::EmbeddingHandler;
use tools::embedding_store::{EmbeddingRecord, EmbeddingStore};
use tools::todo_types::Todo;

#[path = "test_utils/embedding.rs"]
mod embedding_utils;
use embedding_utils::deterministic_embedding;

/// Mock embedding handler for testing semantic search without the real model.
pub struct MockEmbeddingHandler {
    /// Pre-loaded embeddings keyed by todo ID
    pub embeddings: Mutex<HashMap<String, Vec<f32>>>,
    /// Whether the mock should report as available
    pub available: bool,
    /// Optional store to persist embeddings (like the real EmbeddingEngineImpl)
    pub store: Option<Arc<RwLock<EmbeddingStore>>>,
    /// Track calls for assertion
    pub embed_todo_calls: Mutex<Vec<String>>,
    pub embed_query_calls: Mutex<Vec<String>>,
}

impl Default for MockEmbeddingHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl MockEmbeddingHandler {
    /// Create an available mock that generates deterministic embeddings.
    pub fn new() -> Self {
        Self {
            embeddings: Mutex::new(HashMap::new()),
            available: true,
            store: None,
            embed_todo_calls: Mutex::new(Vec::new()),
            embed_query_calls: Mutex::new(Vec::new()),
        }
    }

    /// Create an available mock backed by an EmbeddingStore (persists embeddings).
    #[allow(dead_code)]
    pub fn with_store(store: Arc<RwLock<EmbeddingStore>>) -> Self {
        Self {
            embeddings: Mutex::new(HashMap::new()),
            available: true,
            store: Some(store),
            embed_todo_calls: Mutex::new(Vec::new()),
            embed_query_calls: Mutex::new(Vec::new()),
        }
    }

    /// Create a mock that reports as unavailable (simulates model download failure).
    #[allow(dead_code)]
    pub fn unavailable() -> Self {
        Self {
            embeddings: Mutex::new(HashMap::new()),
            available: false,
            store: None,
            embed_todo_calls: Mutex::new(Vec::new()),
            embed_query_calls: Mutex::new(Vec::new()),
        }
    }

    /// Create a mock with pre-loaded embeddings for specific todo IDs.
    #[allow(dead_code)]
    pub fn with_embeddings(map: HashMap<String, Vec<f32>>) -> Self {
        Self {
            embeddings: Mutex::new(map),
            available: true,
            store: None,
            embed_todo_calls: Mutex::new(Vec::new()),
            embed_query_calls: Mutex::new(Vec::new()),
        }
    }

    /// Generate a deterministic 384-dim embedding from text.
    /// Delegates to `test_utils::embedding::deterministic_embedding`.
    pub fn deterministic_embedding(text: &str) -> Vec<f32> {
        deterministic_embedding(text)
    }

    /// Get the number of embed_todo calls made.
    #[allow(dead_code)]
    pub fn embed_todo_call_count(&self) -> usize {
        self.embed_todo_calls.lock().unwrap().len()
    }

    /// Get the number of embed_query calls made.
    #[allow(dead_code)]
    pub fn embed_query_call_count(&self) -> usize {
        self.embed_query_calls.lock().unwrap().len()
    }
}

#[async_trait]
impl EmbeddingHandler for MockEmbeddingHandler {
    async fn embed_todo(&self, todo: &Todo) -> Result<Option<EmbeddingRecord>> {
        self.embed_todo_calls.lock().unwrap().push(todo.id.clone());

        if !self.available {
            return Ok(None);
        }

        // Compose text the same way the real engine does: title + description + tags
        let text = format!(
            "{} {} {}",
            todo.title,
            todo.description.as_deref().unwrap_or(""),
            todo.tags.join(" ")
        );
        let embedding = Self::deterministic_embedding(&text);

        // Store for later retrieval (in-memory)
        self.embeddings
            .lock()
            .unwrap()
            .insert(todo.id.clone(), embedding.clone());

        let record = EmbeddingRecord {
            id: todo.id.clone(),
            embedding,
            model: "mock-model".to_string(),
            embedded_at: Utc::now(),
        };

        // Persist to EmbeddingStore if available (mirrors real EmbeddingEngineImpl)
        if let Some(ref store) = self.store {
            let mut store = store.write().await;
            store.upsert(record.clone()).await?;
        }

        Ok(Some(record))
    }

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.embed_query_calls
            .lock()
            .unwrap()
            .push(query.to_string());

        if !self.available {
            return Err(common::ToolError::ExecutionFailed(
                "Mock embedding handler unavailable".to_string(),
            )
            .into());
        }

        Ok(Self::deterministic_embedding(query))
    }

    fn is_available(&self) -> bool {
        self.available
    }
}
