//! Embedding engine for semantic search.
//!
//! Supports two providers:
//! - **Local** (default): `bge-small-en-v1.5-Q` via fastembed ONNX runtime.
//!   Lazy-loaded, auto-unloaded after idle timeout.
//! - **OpenAI**: `text-embedding-3-small` via API. Requires an API key.
//!   Requests 384 dimensions (Matryoshka) for LanceDB compatibility.
//!
//! Both produce 384-dimensional vectors — no schema migration when switching.

use async_trait::async_trait;
use chrono::Utc;
#[cfg(feature = "semantic-search")]
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, info, warn};

use crate::embedding_store::EmbeddingRecord;
use crate::todo_types::Todo;
use common::Result;

/// Expected embedding dimensionality (both local and OpenAI produce this).
pub const EMBEDDING_DIM: usize = 384;

/// Idle timeout before the ONNX model is unloaded from memory (15 seconds).
const EMBEDDING_IDLE_SECS: u64 = 15;

/// Which embedding provider to use.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum EmbeddingProvider {
    /// Local ONNX model via fastembed (bge-small-en-v1.5-Q, 384 dims).
    #[default]
    Local,
    /// OpenAI text-embedding-3-small API (384 dims via Matryoshka).
    OpenAi {
        api_key: String,
        api_base: Option<String>,
    },
}

/// Core embedding engine with local and API provider support.
///
/// **Local mode** (default): the model (~420MB) is downloaded on first use,
/// not at construction time. Auto-unloaded after [`EMBEDDING_IDLE_SECS`].
///
/// **OpenAI mode**: calls the embeddings API with `dimensions: 384`.
///
/// When compiled without the `semantic-search` feature, local mode returns
/// errors but OpenAI mode still works.
pub struct EmbeddingEngine {
    provider: EmbeddingProvider,
    #[cfg(feature = "semantic-search")]
    model: Mutex<Option<TextEmbedding>>,
    last_used: Mutex<Instant>,
    http_client: reqwest::Client,
    #[cfg(not(feature = "semantic-search"))]
    _phantom: (),
}

impl Default for EmbeddingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl EmbeddingEngine {
    /// Create a new engine with the local provider (default).
    pub fn new() -> Self {
        Self {
            provider: EmbeddingProvider::Local,
            #[cfg(feature = "semantic-search")]
            model: Mutex::new(None),
            last_used: Mutex::new(Instant::now()),
            http_client: reqwest::Client::new(),
            #[cfg(not(feature = "semantic-search"))]
            _phantom: (),
        }
    }

    /// Create a new engine with a specific provider.
    pub fn with_provider(provider: EmbeddingProvider) -> Self {
        Self {
            provider,
            #[cfg(feature = "semantic-search")]
            model: Mutex::new(None),
            last_used: Mutex::new(Instant::now()),
            http_client: reqwest::Client::new(),
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
        *self.last_used.lock().unwrap() = Instant::now();

        if guard.is_none() {
            info!("Initializing embedding model (first use)...");
            let model = TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::BGESmallENV15Q).with_show_download_progress(true),
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

    /// Unload the ONNX model if it has been idle for [`EMBEDDING_IDLE_SECS`].
    ///
    /// Returns `true` if the model was unloaded. Call periodically from a
    /// maintenance timer.
    #[cfg(feature = "semantic-search")]
    pub fn unload_if_idle(&self) -> bool {
        let last = *self.last_used.lock().unwrap();
        if last.elapsed() >= std::time::Duration::from_secs(EMBEDDING_IDLE_SECS) {
            let mut guard = self.model.lock().unwrap();
            if guard.is_some() {
                *guard = None;
                info!("Embedding model unloaded after idle timeout");
                return true;
            }
        }
        false
    }

    /// Stub when `semantic-search` feature is disabled.
    #[cfg(not(feature = "semantic-search"))]
    pub fn unload_if_idle(&self) -> bool {
        false
    }

    /// Generate embedding for a single text. Returns `Vec<f32>` of [`EMBEDDING_DIM`] length.
    ///
    /// Dispatches to local fastembed or OpenAI API based on the configured provider.
    #[cfg(feature = "semantic-search")]
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        match &self.provider {
            EmbeddingProvider::Local => self.embed_local(text),
            EmbeddingProvider::OpenAi { .. } => self.embed_openai_sync(text),
        }
    }

    /// Stub when `semantic-search` feature is disabled — OpenAI still works.
    #[cfg(not(feature = "semantic-search"))]
    pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
        match &self.provider {
            EmbeddingProvider::OpenAi { .. } => self.embed_openai_sync(text),
            _ => Err(common::ToolError::ExecutionFailed(
                "Semantic search is disabled. Rebuild with `semantic-search` feature or configure OpenAI embeddings.".to_string(),
            ).into()),
        }
    }

    /// Local fastembed embedding for a single text.
    #[cfg(feature = "semantic-search")]
    fn embed_local(&self, text: &str) -> Result<Vec<f32>> {
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

    /// Call OpenAI embeddings API with `dimensions: 384` for LanceDB compatibility.
    async fn embed_openai(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let (api_key, api_base) = match &self.provider {
            EmbeddingProvider::OpenAi { api_key, api_base } => (api_key, api_base),
            _ => {
                return Err(common::ToolError::ExecutionFailed(
                    "embed_openai called without OpenAI provider".to_string(),
                )
                .into())
            }
        };
        let base = api_base.as_deref().unwrap_or("https://api.openai.com/v1");
        let url = format!("{}/embeddings", base.trim_end_matches('/'));

        let body = serde_json::json!({
            "model": "text-embedding-3-small",
            "input": texts,
            "dimensions": EMBEDDING_DIM,
        });

        let resp = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                common::ToolError::ExecutionFailed(format!("OpenAI embedding request failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(common::ToolError::ExecutionFailed(format!(
                "OpenAI embedding API error {status}: {body}"
            ))
            .into());
        }

        let json: serde_json::Value = resp.json().await.map_err(|e| {
            common::ToolError::ExecutionFailed(format!("OpenAI embedding parse failed: {e}"))
        })?;

        let data = json["data"].as_array().ok_or_else(|| {
            common::ToolError::ExecutionFailed("OpenAI response missing 'data' array".to_string())
        })?;

        let mut embeddings = Vec::with_capacity(data.len());
        for item in data {
            let vec: Vec<f32> = item["embedding"]
                .as_array()
                .ok_or_else(|| {
                    common::ToolError::ExecutionFailed(
                        "OpenAI embedding item missing 'embedding' array".to_string(),
                    )
                })?
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            embeddings.push(vec);
        }

        Ok(embeddings)
    }

    /// Synchronous OpenAI embed — for callers that use the sync `embed()` path.
    fn embed_openai_sync(&self, text: &str) -> Result<Vec<f32>> {
        tokio::task::block_in_place(|| {
            let mut results =
                tokio::runtime::Handle::current().block_on(self.embed_openai(&[text]))?;
            results.pop().ok_or_else(|| {
                common::ToolError::ExecutionFailed("OpenAI returned empty results".to_string())
                    .into()
            })
        })
    }

    /// Batch embed multiple texts (more efficient than individual calls).
    #[cfg(feature = "semantic-search")]
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        match &self.provider {
            EmbeddingProvider::Local => {
                self.ensure_model()?;
                let mut guard = self.model.lock().map_err(|e| {
                    common::ToolError::ExecutionFailed(format!(
                        "Embedding model lock poisoned: {}",
                        e
                    ))
                })?;
                let model = guard.as_mut().unwrap();
                model.embed(texts, None).map_err(|e| {
                    common::ToolError::ExecutionFailed(format!(
                        "Batch embedding generation failed: {}",
                        e
                    ))
                    .into()
                })
            }
            EmbeddingProvider::OpenAi { .. } => tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(self.embed_openai(texts))
            }),
        }
    }

    /// Stub when `semantic-search` feature is disabled.
    #[cfg(not(feature = "semantic-search"))]
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        match &self.provider {
            EmbeddingProvider::OpenAi { .. } => {
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(self.embed_openai(texts))
                })
            }
            _ => Err(common::ToolError::ExecutionFailed(
                "Semantic search is disabled. Rebuild with `semantic-search` feature or configure OpenAI embeddings.".to_string(),
            ).into()),
        }
    }

    /// Async embed — dispatches to local (spawn_blocking) or OpenAI (async HTTP).
    pub async fn embed_async(self: Arc<Self>, text: String) -> Result<Vec<f32>> {
        match &self.provider {
            EmbeddingProvider::Local => tokio::task::spawn_blocking(move || self.embed(&text))
                .await
                .map_err(|e| {
                    common::KlyntbotError::from(common::ToolError::ExecutionFailed(format!(
                        "Embedding task panicked: {e}"
                    )))
                })?,
            EmbeddingProvider::OpenAi { .. } => {
                self.embed_openai(&[&text]).await.and_then(|mut v| {
                    v.pop().ok_or_else(|| {
                        common::ToolError::ExecutionFailed(
                            "OpenAI returned empty results".to_string(),
                        )
                        .into()
                    })
                })
            }
        }
    }

    /// Check if the embedding engine is available.
    pub fn is_available(&self) -> bool {
        match &self.provider {
            EmbeddingProvider::OpenAi { api_key, .. } => !api_key.is_empty(),
            EmbeddingProvider::Local => {
                #[cfg(feature = "semantic-search")]
                {
                    // Lazy init means we're always available until the first call fails.
                    true
                }
                #[cfg(not(feature = "semantic-search"))]
                false
            }
        }
    }

    /// Get the name of the embedding model being used.
    pub fn model_name(&self) -> &str {
        match &self.provider {
            EmbeddingProvider::Local => "bge-small-en-v1.5-Q",
            EmbeddingProvider::OpenAi { .. } => "text-embedding-3-small",
        }
    }

    /// Cosine similarity between two vectors.
    ///
    /// Returns 0.0 for zero-norm vectors (avoids NaN).
    /// Handles NaN values by treating them as 0.0.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
        common::helpers::cosine_similarity(a, b)
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
/// Wraps `EmbeddingEngine` (for vector generation) and `VectorStore`
/// (for LanceDB persistence). Constructed in the agent crate and injected into `TodoTool`.
pub struct EmbeddingEngineImpl {
    engine: Arc<EmbeddingEngine>,
    store: storage::VectorStore,
}

impl EmbeddingEngineImpl {
    pub fn new(engine: Arc<EmbeddingEngine>, store: storage::VectorStore) -> Self {
        Self { engine, store }
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

        debug!(todo_id = %todo_id, text_len = text.len(), "Generating embedding for todo");
        let start = std::time::Instant::now();

        let embedding = self.engine.clone().embed_async(text).await?;

        debug!(
            todo_id = %todo_id,
            elapsed_ms = start.elapsed().as_millis() as u64,
            "Embedding generated for todo"
        );

        let model_name = self.engine.model_name();

        self.store
            .upsert_embedding(
                "todo_embeddings",
                &todo_id,
                &embedding,
                &[("model", model_name)],
            )
            .await
            .map_err(|e| common::ToolError::ExecutionFailed(e.to_string()))?;

        let record = EmbeddingRecord {
            id: todo_id,
            embedding,
            model: model_name.to_string(),
            embedded_at: Utc::now(),
        };

        Ok(Some(record))
    }

    async fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        debug!(query_len = query.len(), "Generating query embedding");
        let start = std::time::Instant::now();

        let result = self.engine.clone().embed_async(query.to_string()).await;

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
