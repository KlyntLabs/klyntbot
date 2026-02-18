use std::path::PathBuf;
use tokio::sync::RwLock;

/// A chunk of memory content with metadata.
#[derive(Debug, Clone)]
pub struct MemoryChunk {
    pub content: String,
    pub source: MemorySource,
    pub token_estimate: usize,
}

/// Where a memory chunk originated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorySource {
    /// From MEMORY.md (long-term knowledge)
    LongTerm,
    /// From past conversation history
    Conversation,
}

/// Retrieves relevant memory chunks using embedding similarity, respecting token budgets.
pub struct MemoryRetriever {
    memory_path: PathBuf,
    cached_chunks: RwLock<Vec<(MemoryChunk, Vec<f32>)>>,
}

impl MemoryRetriever {
    /// Create a new retriever pointing at a MEMORY.md file.
    pub fn new(memory_path: PathBuf) -> Self {
        Self {
            memory_path,
            cached_chunks: RwLock::new(Vec::new()),
        }
    }

    /// Retrieve memory chunks similar to the query, fitting within the token budget.
    ///
    /// Returns chunks sorted by similarity (highest first), stopping when
    /// the cumulative token count would exceed `budget_tokens`.
    pub async fn retrieve(
        &self,
        query_embedding: &[f32],
        budget_tokens: usize,
        threshold: f32,
    ) -> Vec<MemoryChunk> {
        let chunks = self.cached_chunks.read().await;

        // Score each chunk by cosine similarity
        let mut scored: Vec<(&MemoryChunk, f32)> = chunks
            .iter()
            .map(|(chunk, emb)| (chunk, Self::cosine_similarity(query_embedding, emb)))
            .filter(|(_, score)| *score >= threshold)
            .collect();

        // Sort by similarity descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Greedily select chunks within budget
        let mut result = Vec::new();
        let mut tokens_used = 0;
        for (chunk, _score) in scored {
            if tokens_used + chunk.token_estimate > budget_tokens {
                continue; // skip this chunk, try smaller ones
            }
            tokens_used += chunk.token_estimate;
            result.push(chunk.clone());
        }

        result
    }

    /// Load and cache chunks from the MEMORY.md file.
    ///
    /// `embed_fn` is called for each chunk to produce its embedding vector.
    /// Pass a no-op function if embeddings aren't available yet.
    pub async fn load_memory<F>(&self, embed_fn: F) -> std::io::Result<usize>
    where
        F: Fn(&str) -> Vec<f32>,
    {
        let content = tokio::fs::read_to_string(&self.memory_path).await?;
        let texts = Self::chunk_memory_file(&content);
        let mut cached = self.cached_chunks.write().await;
        cached.clear();

        for text in &texts {
            let embedding = embed_fn(text);
            cached.push((
                MemoryChunk {
                    content: text.clone(),
                    source: MemorySource::LongTerm,
                    token_estimate: Self::estimate_tokens(text),
                },
                embedding,
            ));
        }

        Ok(texts.len())
    }

    /// Inject pre-embedded chunks (e.g., conversation memories from external store).
    pub async fn add_chunks(&self, chunks: Vec<(MemoryChunk, Vec<f32>)>) {
        let mut cached = self.cached_chunks.write().await;
        cached.extend(chunks);
    }

    /// Cosine similarity between two vectors. Returns 0.0 for zero-length vectors.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }

        dot / (norm_a * norm_b)
    }

    /// Split markdown content into chunks by headers (## or #).
    /// Each chunk includes its header line for context.
    pub fn chunk_memory_file(content: &str) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current_chunk = String::new();

        for line in content.lines() {
            if line.starts_with('#') && !current_chunk.trim().is_empty() {
                chunks.push(current_chunk.trim().to_string());
                current_chunk = String::new();
            }
            current_chunk.push_str(line);
            current_chunk.push('\n');
        }

        // Don't forget the last chunk
        if !current_chunk.trim().is_empty() {
            chunks.push(current_chunk.trim().to_string());
        }

        chunks
    }

    /// Rough token estimate: ~4 chars per token.
    pub fn estimate_tokens(text: &str) -> usize {
        (text.len() + 3) / 4 // ceiling division
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let v = vec![1.0, 2.0, 3.0];
        let sim = MemoryRetriever::cosine_similarity(&v, &v);
        assert!(
            (sim - 1.0).abs() < 1e-6,
            "Identical vectors should have similarity 1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = MemoryRetriever::cosine_similarity(&a, &b);
        assert!(
            sim.abs() < 1e-6,
            "Orthogonal vectors should have similarity 0.0"
        );
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = MemoryRetriever::cosine_similarity(&a, &b);
        assert!(
            (sim - (-1.0)).abs() < 1e-6,
            "Opposite vectors should have similarity -1.0"
        );
    }

    #[test]
    fn test_cosine_similarity_empty_vectors() {
        let sim = MemoryRetriever::cosine_similarity(&[], &[]);
        assert_eq!(sim, 0.0, "Empty vectors should return 0.0");
    }

    #[test]
    fn test_cosine_similarity_mismatched_lengths() {
        let a = vec![1.0, 2.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = MemoryRetriever::cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0, "Mismatched lengths should return 0.0");
    }

    #[test]
    fn test_chunk_memory_file() {
        let content = "# Section One\nHello world\n\n## Section Two\nSome content\nMore content\n\n# Section Three\nFinal part";
        let chunks = MemoryRetriever::chunk_memory_file(content);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].starts_with("# Section One"));
        assert!(chunks[1].starts_with("## Section Two"));
        assert!(chunks[2].starts_with("# Section Three"));
    }

    #[test]
    fn test_chunk_memory_file_no_headers() {
        let content = "Just plain text\nNo headers here";
        let chunks = MemoryRetriever::chunk_memory_file(content);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].contains("Just plain text"));
    }

    #[test]
    fn test_chunk_memory_file_empty() {
        let chunks = MemoryRetriever::chunk_memory_file("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_estimate_tokens() {
        // 12 chars -> 3 tokens
        assert_eq!(MemoryRetriever::estimate_tokens("hello world!"), 3);
        // Empty -> 0 (well, (0+3)/4 = 0 via integer div)
        assert_eq!(MemoryRetriever::estimate_tokens(""), 0);
        // 4 chars -> 1 token
        assert_eq!(MemoryRetriever::estimate_tokens("abcd"), 1);
    }

    #[tokio::test]
    async fn test_retrieve_respects_budget() {
        let retriever = MemoryRetriever::new(PathBuf::from("/tmp/nonexistent"));

        // Manually inject chunks with embeddings
        let query = vec![1.0, 0.0, 0.0];
        let chunks = vec![
            (
                MemoryChunk {
                    content: "A".repeat(400), // ~100 tokens
                    source: MemorySource::LongTerm,
                    token_estimate: 100,
                },
                vec![0.9, 0.1, 0.0], // high similarity
            ),
            (
                MemoryChunk {
                    content: "B".repeat(400),
                    source: MemorySource::LongTerm,
                    token_estimate: 100,
                },
                vec![0.8, 0.2, 0.0], // medium similarity
            ),
            (
                MemoryChunk {
                    content: "C".repeat(400),
                    source: MemorySource::LongTerm,
                    token_estimate: 100,
                },
                vec![0.7, 0.3, 0.0], // lower similarity
            ),
        ];
        retriever.add_chunks(chunks).await;

        // Budget for only 150 tokens -> should get at most 1 chunk (100 tokens each)
        // Actually 150 can fit 1 chunk of 100 tokens
        let results = retriever.retrieve(&query, 150, 0.0).await;
        assert!(results.len() <= 2, "Should respect token budget");
        assert!(!results.is_empty(), "Should return at least one chunk");
        // Total tokens should not exceed budget
        let total: usize = results.iter().map(|c| c.token_estimate).sum();
        assert!(
            total <= 150,
            "Total tokens {} should not exceed budget 150",
            total
        );
    }

    #[tokio::test]
    async fn test_retrieve_filters_by_threshold() {
        let retriever = MemoryRetriever::new(PathBuf::from("/tmp/nonexistent"));

        let chunks = vec![
            (
                MemoryChunk {
                    content: "Relevant".to_string(),
                    source: MemorySource::LongTerm,
                    token_estimate: 10,
                },
                vec![1.0, 0.0], // identical to query
            ),
            (
                MemoryChunk {
                    content: "Irrelevant".to_string(),
                    source: MemorySource::LongTerm,
                    token_estimate: 10,
                },
                vec![0.0, 1.0], // orthogonal to query
            ),
        ];
        retriever.add_chunks(chunks).await;

        let query = vec![1.0, 0.0];
        let results = retriever.retrieve(&query, 1000, 0.5).await;
        assert_eq!(results.len(), 1, "Should filter out low-similarity chunk");
        assert_eq!(results[0].content, "Relevant");
    }

    #[tokio::test]
    async fn test_load_memory_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("MEMORY.md");
        tokio::fs::write(&path, b"# Section A\nContent A\n\n# Section B\nContent B")
            .await
            .unwrap();

        let retriever = MemoryRetriever::new(path);
        // Use a stub embed_fn that returns empty vecs
        let count = retriever.load_memory(|_| vec![]).await.unwrap();
        assert_eq!(count, 2, "Should load 2 chunks from 2 markdown sections");
    }
}
