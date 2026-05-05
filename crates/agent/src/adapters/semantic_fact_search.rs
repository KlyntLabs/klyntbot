//! SemanticFactSearchHandlerImpl — production handler for memory.search_all.
//!
//! Implements the `SemanticFactSearchHandler` trait defined in tools crate (L4)
//! by delegating to `cognitive::services::retrieval::retrieve_relevant_facts` —
//! the same retriever used by pre-injection. T2.2.

use std::sync::Arc;

use async_trait::async_trait;
use cognitive::repos::SemanticFactRepo;
use cognitive::services::retrieval::{retrieve_relevant_facts, RetrievalParams};
use cognitive::SemanticFactEmbedder;
use tools::semantic_fact_search::{FactSearchResult, SemanticFactSearchHandler};

const ALL_DOMAINS: &[&str] = &[
    "personal", "work", "health", "finance", "learning", "general", "coding",
];

pub struct SemanticFactSearchHandlerImpl {
    fact_repo: SemanticFactRepo,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,
    entity_repo: Option<cognitive::repos::EntityRepo>,
}

impl SemanticFactSearchHandlerImpl {
    pub fn new(
        fact_repo: SemanticFactRepo,
        embedder: Option<Arc<dyn SemanticFactEmbedder>>,
        entity_repo: Option<cognitive::repos::EntityRepo>,
    ) -> Self {
        Self {
            fact_repo,
            embedder,
            entity_repo,
        }
    }
}

#[async_trait]
impl SemanticFactSearchHandler for SemanticFactSearchHandlerImpl {
    async fn search_facts(
        &self,
        query: &str,
        limit: usize,
        widen: bool,
    ) -> common::Result<Vec<FactSearchResult>> {
        // T2.2 v3: widen *recall* (more candidates) but keep *quality bar* high
        // (similarity floor). Earlier widen mode dropped min_similarity to 0.2 —
        // at 384-dim BGE that's essentially random, and the resulting 20-fact
        // dump confused the model into empty output. Now: more candidates (top_k
        // doubles) but min_similarity stays at the default. A post-hoc score
        // filter caps the response at MAX_WIDEN_RESULTS so synthesis isn't drowned.
        const MAX_WIDEN_RESULTS: usize = 6;
        const MIN_WIDEN_SCORE: f64 = 0.30;

        let mut params = RetrievalParams::new(limit);
        if widen {
            params.vector_top_k = (limit * 2).max(60);
            // Deliberately do NOT lower min_similarity — see comment above.
        }
        let scored = retrieve_relevant_facts(
            &self.fact_repo,
            self.embedder.as_deref(),
            query,
            ALL_DOMAINS,
            &params,
            None,
            None,
            None,
            self.entity_repo.as_ref(),
        )
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("fact search failed: {e}")))?;

        let filtered: Vec<_> = if widen {
            scored
                .into_iter()
                .filter(|s| s.score >= MIN_WIDEN_SCORE)
                .take(MAX_WIDEN_RESULTS)
                .collect()
        } else {
            scored
        };

        Ok(filtered
            .into_iter()
            .map(|s| FactSearchResult {
                id: s.fact.id.clone(),
                subject: s.fact.subject.clone(),
                predicate: s.fact.predicate.clone(),
                object: s.fact.object.clone(),
                domain: s.fact.domain.clone(),
                confidence: s.fact.confidence,
                score: s.score,
            })
            .collect())
    }
}
