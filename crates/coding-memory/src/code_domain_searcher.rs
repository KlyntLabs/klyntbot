//! `CodeDomainSearcher` — coding-memory registered in `InsightForge` (Tier B4).
//!
//! Implements `DomainSearcher` so that `InsightForge::search_all` includes
//! coding facts and episodes in its blended results.

use async_trait::async_trait;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use context_engine::insight_forge::DomainSearcher;
use context_engine::memory_retriever::MemoryEntry;
use context_engine::MemorySource;

/// Searches coding-domain semantic facts and episodic memories.
pub struct CodeDomainSearcher {
    facts: SemanticFactRepo,
    episodes: EpisodicMemoryRepo,
}

impl CodeDomainSearcher {
    /// Construct from cognitive repos.
    #[must_use]
    pub fn new(facts: SemanticFactRepo, episodes: EpisodicMemoryRepo) -> Self {
        Self { facts, episodes }
    }
}

#[async_trait]
impl DomainSearcher for CodeDomainSearcher {
    fn domain_name(&self) -> &str {
        "coding"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        let mut out = Vec::new();

        // Search semantic facts via FTS5
        match self.facts.search_fts(query, Some("coding"), limit).await {
            Ok(facts) => {
                for f in facts {
                    out.push(MemoryEntry {
                        id: f.id,
                        source: MemorySource::CognitiveFact,
                        content: format!("{} {} {}", f.subject, f.predicate, f.object),
                        score: f.confidence,
                        raw_score: f.confidence,
                    });
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "CodeDomainSearcher semantic search failed");
            }
        }

        // Search episodic memories via FTS5
        match self.episodes.search_fts(query, Some("coding"), limit).await {
            Ok(memories) => {
                for m in memories {
                    out.push(MemoryEntry {
                        id: m.id,
                        source: MemorySource::EpisodicMemory,
                        content: m.content,
                        score: m.importance,
                        raw_score: m.importance,
                    });
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "CodeDomainSearcher episodic search failed");
            }
        }

        // Sort by relevance and truncate
        out.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        out.truncate(limit);
        out
    }
}
