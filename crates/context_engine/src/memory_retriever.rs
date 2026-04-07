use async_trait::async_trait;

/// Where a memory result originated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemorySource {
    /// Extracted/consolidated semantic fact (FSRS-scored).
    CognitiveFact,
    /// Past conversation message (time-decay scored).
    ConversationRecall,
    /// Significant event record (episodic memory).
    EpisodicMemory,
    /// Domain-specific search result (notes, tasks, finance, graph).
    Domain { name: String },
}

/// A single retrieved memory entry.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    /// Unique identifier (e.g., todo ID or memory key).
    pub id: String,
    /// Text content of the memory.
    pub content: String,
    /// Similarity score (0.0–1.0; higher = more relevant).
    pub score: f64,
    /// Where this memory originated.
    pub source: MemorySource,
    /// The raw score before any normalization or blending.
    pub raw_score: f64,
}

/// Trait for embedding-based memory retrieval during context assembly.
///
/// Implement this in higher layers (agent, storage) to plug in actual
/// vector-database lookups. The context engine calls this during assembly
/// and budgets the returned entries under `Priority::RetrievedMemory`.
#[async_trait]
pub trait MemoryRetriever: Send + Sync {
    /// Retrieve up to `limit` memories relevant to the given query text.
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockMemoryRetriever {
        entries: Vec<MemoryEntry>,
    }

    impl MockMemoryRetriever {
        fn new(entries: Vec<MemoryEntry>) -> Self {
            Self { entries }
        }
    }

    #[async_trait]
    impl MemoryRetriever for MockMemoryRetriever {
        async fn retrieve(&self, _query: &str, limit: usize) -> Vec<MemoryEntry> {
            self.entries
                .iter()
                .take(limit)
                .map(|e| MemoryEntry {
                    id: e.id.clone(),
                    content: e.content.clone(),
                    score: e.score,
                    source: e.source.clone(),
                    raw_score: e.raw_score,
                })
                .collect()
        }
    }

    #[tokio::test]
    async fn test_memory_retriever_limit() {
        let retriever = MockMemoryRetriever::new(vec![
            MemoryEntry {
                id: "1".into(),
                content: "First memory".into(),
                score: 0.9,
                source: MemorySource::CognitiveFact,
                raw_score: 0.9,
            },
            MemoryEntry {
                id: "2".into(),
                content: "Second memory".into(),
                score: 0.8,
                source: MemorySource::CognitiveFact,
                raw_score: 0.8,
            },
            MemoryEntry {
                id: "3".into(),
                content: "Third memory".into(),
                score: 0.7,
                source: MemorySource::CognitiveFact,
                raw_score: 0.7,
            },
        ]);

        let results = retriever.retrieve("query", 2).await;
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "1");
        assert_eq!(results[1].id, "2");
    }

    #[tokio::test]
    async fn test_memory_retriever_empty() {
        let retriever = MockMemoryRetriever::new(vec![]);
        let results = retriever.retrieve("anything", 5).await;
        assert!(results.is_empty());
    }
}
