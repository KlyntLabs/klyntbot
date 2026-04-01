use async_trait::async_trait;
use cognitive::SemanticFactRepo;
use context_engine::memory_retriever::{MemoryEntry, MemoryRetriever, MemorySource};

/// Simple FTS-based memory retriever for simulation.
///
/// Queries the semantic fact repo directly without embeddings, using SQLite
/// full-text search. This is sufficient for measuring whether extracted facts
/// are retrievable and computing precision/recall when ground-truth annotations
/// include `relevant_facts`.
pub struct FtsMemoryRetriever {
    repo: SemanticFactRepo,
}

impl FtsMemoryRetriever {
    pub fn new(repo: SemanticFactRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl MemoryRetriever for FtsMemoryRetriever {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        match self.repo.search_fts(query, None, limit).await {
            Ok(facts) => facts
                .into_iter()
                .enumerate()
                .map(|(rank, fact)| MemoryEntry {
                    id: fact.id,
                    content: format!("{} {} {}", fact.subject, fact.predicate, fact.object),
                    score: 1.0 / (rank as f64 + 1.0),
                    source: MemorySource::CognitiveFact,
                    raw_score: 1.0 / (rank as f64 + 1.0),
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}
