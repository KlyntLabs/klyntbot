//! Embedding engine for semantic search.
//!
//! Wraps `fastembed::TextEmbedding` with lazy initialization and provides
//! cosine similarity, batch embedding, and the `EmbeddingHandler` trait
//! for dependency inversion (testability without loading the 420MB model).

use async_trait::async_trait;
use chrono::Utc;
#[cfg(feature = "semantic-search")]
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::{Arc, Mutex};
use tracing::{debug, info};

use crate::embedding_store::EmbeddingRecord;
use crate::todo_types::Todo;
use common::Result;

/// Expected embedding dimensionality for paraphrase-multilingual-MiniLM-L12-v2.
pub const EMBEDDING_DIM: usize = 384;

/// Core embedding engine wrapping fastembed with lazy model initialization.
///
/// The model (~420MB) is downloaded on first use, not at construction time.
/// When compiled without the `semantic-search` feature, `embed()` always
/// returns an error — the struct still exists for API compatibility.
pub struct EmbeddingEngine {
    #[cfg(feature = "semantic-search")]
    model: Mutex<Option<TextEmbedding>>,
    #[cfg(not(feature = "semantic-search"))]
    _phantom: (),
}

impl Default for EmbeddingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingEngine {
    /// Create a new engine. The model is NOT loaded until the first embed call.
    pub fn new() -> Self {
        Self {
            #[cfg(feature = "semantic-search")]
            model: Mutex::new(None),
            #[cfg(not(feature = "semantic-search"))]
            _phantom: (),
        }
    }

    /// Lazily initialize the model (downloads ~420MB on first call).
    #[cfg(feature = "semantic-search")]
    fn ensure_model(&self) -> Result<()> {
        let mut guard = self.model.lock().map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Embedding model lock poisoned: {}", e))
        })?;

        if guard.is_none() {
            info!("Initializing embedding model (first use — may download ~420MB)...");
            let model = TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::ParaphraseMLMiniLML12V2)
                    .with_show_download_progress(true),
            )
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!(
                    "Failed to initialize embedding model: {}",
                    e
                ))
            })?;
            *guard = Some(model);
            info!("Embedding model ready.");
        }

        Ok(())
    }

    /// Generate embedding for a single text. Returns `Vec<f32>` of `EMBEDDING_DIM` length.
    #[cfg(feature = "semantic-search")]
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.ensure_model()?;
        let mut guard = self.model.lock().map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Embedding model lock poisoned: {}", e))
        })?;
        let model = guard.as_mut().unwrap();
        let results = model.embed(vec![text], None).map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Embedding generation failed: {}", e))
        })?;
        results.into_iter().next().ok_or_else(|| {
            common::ToolError::ExecutionFailed("Embedding returned empty results".to_string())
                .into()
        })
    }

    /// Stub when `semantic-search` feature is disabled.
    #[cfg(not(feature = "semantic-search"))]
    pub fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        Err(common::ToolError::ExecutionFailed(
            "Semantic search is disabled. Rebuild with the `semantic-search` feature to enable embeddings.".to_string(),
        )
        .into())
    }

    /// Batch embed multiple texts (more efficient than individual calls).
    #[cfg(feature = "semantic-search")]
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        self.ensure_model()?;
        let mut guard = self.model.lock().map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Embedding model lock poisoned: {}", e))
        })?;
        let model = guard.as_mut().unwrap();
        let results = model.embed(texts, None).map_err(|e| {
            common::ToolError::ExecutionFailed(format!("Batch embedding generation failed: {}", e))
        })?;
        Ok(results)
    }

    /// Stub when `semantic-search` feature is disabled.
    #[cfg(not(feature = "semantic-search"))]
    pub fn embed_batch(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        Err(common::ToolError::ExecutionFailed(
            "Semantic search is disabled. Rebuild with the `semantic-search` feature to enable embeddings.".to_string(),
        )
        .into())
    }

    /// Check if the embedding model is initialized and available.
    #[cfg(feature = "semantic-search")]
    pub fn is_available(&self) -> bool {
        self.model
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Always returns false when `semantic-search` is disabled.
    #[cfg(not(feature = "semantic-search"))]
    pub fn is_available(&self) -> bool {
        false
    }

    /// Get the name of the embedding model being used.
    pub fn model_name(&self) -> &str {
        "paraphrase-multilingual-MiniLM-L12-v2"
    }

    /// Cosine similarity between two vectors.
    ///
    /// Returns 0.0 for zero-norm vectors (avoids NaN).
    /// Handles NaN values by treating them as 0.0.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        let mut dot = 0.0f64;
        let mut norm_a = 0.0f64;
        let mut norm_b = 0.0f64;

        for (va, vb) in a.iter().zip(b.iter()) {
            let fa = if va.is_nan() { 0.0f64 } else { *va as f64 };
            let fb = if vb.is_nan() { 0.0f64 } else { *vb as f64 };
            dot += fa * fb;
            norm_a += fa * fa;
            norm_b += fb * fb;
        }

        let denom = norm_a.sqrt() * norm_b.sqrt();
        if denom == 0.0 {
            0.0
        } else {
            dot / denom
        }
    }
}

/// Trait for embedding operations, enabling dependency inversion.
///
/// Defined in the tools crate (Layer 3) for testability — tests can mock
/// this trait without loading the 420MB fastembed model.
#[async_trait]
pub trait EmbeddingHandler: Send + Sync {
    /// Generate embedding for a todo's searchable text (title + description + tags).
    async fn embed_todo(&self, todo: &Todo) -> Result<Option<EmbeddingRecord>>;

    /// Generate embedding for a query string.
    async fn embed_query(&self, query: &str) -> Result<Vec<f32>>;

    /// Check if the engine is available.
    fn is_available(&self) -> bool;
}

/// Production implementation of `EmbeddingHandler`.
///
/// Wraps `EmbeddingEngine` (for vector generation) and `EmbeddingRepo`
/// (for SQL persistence). Constructed in the agent crate and injected into `TodoTool`.
pub struct EmbeddingEngineImpl {
    engine: Arc<EmbeddingEngine>,
    repo: storage::EmbeddingRepo,
}

impl EmbeddingEngineImpl {
    pub fn new(engine: Arc<EmbeddingEngine>, repo: storage::EmbeddingRepo) -> Self {
        Self { engine, repo }
    }

    /// Compose the searchable text for a todo: "{title} {description} {tags}".
    fn compose_text(todo: &Todo) -> String {
        format!(
            "{} {} {}",
            todo.title,
            todo.description.as_deref().unwrap_or(""),
            todo.tags.join(" ")
        )
    }
}

#[async_trait]
impl EmbeddingHandler for EmbeddingEngineImpl {
    async fn embed_todo(&self, todo: &Todo) -> Result<Option<EmbeddingRecord>> {
        let text = Self::compose_text(todo);
        let todo_id = todo.id.clone();
        let engine = self.engine.clone();

        debug!(todo_id = %todo_id, text_len = text.len(), "Generating embedding for todo");
        let start = std::time::Instant::now();

        // CPU-bound work — run in blocking thread
        let embedding = tokio::task::spawn_blocking(move || engine.embed(&text))
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Embedding task panicked: {}", e))
            })??;

        debug!(
            todo_id = %todo_id,
            elapsed_ms = start.elapsed().as_millis() as u64,
            "Embedding generated for todo"
        );

        let model_name = "paraphrase-multilingual-MiniLM-L12-v2";

        // Persist to SQL via repo
        self.repo
            .upsert_vec(&todo_id, &embedding, model_name)
            .await?;

        let record = EmbeddingRecord {
            id: todo_id,
            embedding,
            model: model_name.to_string(),
            embedded_at: Utc::now(),
        };

        Ok(Some(record))
    }

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let query = query.to_string();
        let engine = self.engine.clone();

        debug!(query_len = query.len(), "Generating query embedding");
        let start = std::time::Instant::now();

        let result = tokio::task::spawn_blocking(move || engine.embed(&query))
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("Embedding task panicked: {}", e))
            })?;

        debug!(
            elapsed_ms = start.elapsed().as_millis() as u64,
            "Query embedding generated"
        );
        result
    }

    fn is_available(&self) -> bool {
        // Lazy init means we're always "available" until the first call fails.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Cosine Similarity Tests ──────────────────────────────────

    #[test]
    fn test_cosine_similarity_identical_vectors() {
        let v = vec![1.0, 0.0, 0.0];
        let sim = EmbeddingEngine::cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = EmbeddingEngine::cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let sim = EmbeddingEngine::cosine_similarity(&a, &b);
        assert!((sim + 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_norm_vectors() {
        let zero = vec![0.0, 0.0, 0.0];
        let v = vec![1.0, 0.0, 0.0];
        assert_eq!(EmbeddingEngine::cosine_similarity(&zero, &v), 0.0);
        assert_eq!(EmbeddingEngine::cosine_similarity(&v, &zero), 0.0);
        assert_eq!(EmbeddingEngine::cosine_similarity(&zero, &zero), 0.0);
    }

    #[test]
    fn test_cosine_similarity_nan_handling() {
        let a = vec![f32::NAN, 1.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = EmbeddingEngine::cosine_similarity(&a, &b);
        assert!(!sim.is_nan(), "cosine_similarity should handle NaN inputs");
    }

    #[test]
    fn test_cosine_similarity_384_dimensions() {
        let a: Vec<f32> = (0..384).map(|i| (i as f32 / 384.0).sin()).collect();
        let b: Vec<f32> = (0..384).map(|i| (i as f32 / 384.0).sin()).collect();
        let sim = EmbeddingEngine::cosine_similarity(&a, &b);
        assert!(
            (sim - 1.0).abs() < 1e-5,
            "Identical 384-dim vectors should be ~1.0"
        );
    }

    // ─── Embed Tests (require model — slow/ignored) ──────────────

    #[test]
    #[ignore] // Requires model download (~420MB)
    fn test_embed_returns_384_dim_vector() {
        let engine = EmbeddingEngine::new();
        let result = engine.embed("Fix authentication bug").unwrap();
        assert_eq!(result.len(), EMBEDDING_DIM);
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_batch_returns_correct_count() {
        let engine = EmbeddingEngine::new();
        let texts = vec!["text one", "text two", "text three"];
        let results = engine.embed_batch(&texts).unwrap();
        assert_eq!(results.len(), 3);
        for result in &results {
            assert_eq!(result.len(), EMBEDDING_DIM);
        }
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_empty_string() {
        let engine = EmbeddingEngine::new();
        let result = engine.embed("").unwrap();
        assert_eq!(result.len(), EMBEDDING_DIM);
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_deterministic() {
        let engine = EmbeddingEngine::new();
        let v1 = engine.embed("Fix authentication bug").unwrap();
        let v2 = engine.embed("Fix authentication bug").unwrap();
        assert_eq!(v1, v2, "Embedding should be deterministic");
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_similar_texts_high_similarity() {
        let engine = EmbeddingEngine::new();
        let v1 = engine.embed("Fix login bug").unwrap();
        let v2 = engine.embed("Fix authentication issue").unwrap();
        let sim = EmbeddingEngine::cosine_similarity(&v1, &v2);
        assert!(
            sim > 0.5,
            "Similar texts should have similarity > 0.5, got {}",
            sim
        );
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_dissimilar_texts_low_similarity() {
        let engine = EmbeddingEngine::new();
        let v1 = engine.embed("Fix login bug").unwrap();
        let v2 = engine.embed("Buy groceries from the supermarket").unwrap();
        let sim = EmbeddingEngine::cosine_similarity(&v1, &v2);
        assert!(
            sim < 0.5,
            "Dissimilar texts should have similarity < 0.5, got {}",
            sim
        );
    }
}
