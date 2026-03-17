use std::sync::Arc;

use async_trait::async_trait;

use super::DomainSearcher;
use crate::memory_retriever::{MemoryEntry, MemorySource};
use crate::operators::OperatorContext;
use crate::retrieval_planner::{QueryCategory, RetrievalPlanner};

/// BookRAGSearcher: a DomainSearcher that uses hierarchical retrieval via BookIndex.
pub struct BookRAGSearcher {
    planner: Arc<RetrievalPlanner>,
    max_nodes: usize,
    max_map_nodes: usize,
    operator_timeout_ms: u64,
}

impl BookRAGSearcher {
    pub fn new(
        planner: Arc<RetrievalPlanner>,
        max_nodes: usize,
        max_map_nodes: usize,
        operator_timeout_ms: u64,
    ) -> Self {
        Self {
            planner,
            max_nodes,
            max_map_nodes,
            operator_timeout_ms,
        }
    }
}

#[async_trait]
impl DomainSearcher for BookRAGSearcher {
    fn domain_name(&self) -> &str {
        "book_index"
    }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        if !self.planner.book_index.has_content() {
            return vec![];
        }

        // 1. Plan: classify query + generate operators
        let plan = match self.planner.plan(query).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("BookRAG planning failed: {e}");
                return vec![];
            }
        };
        if plan.category == QueryCategory::PassThrough {
            return vec![];
        }

        // 2. Execute: run operator pipeline with per-operator timeout
        let mut ctx = OperatorContext::new(
            query,
            self.planner.book_index.clone(),
            self.planner.llm(),
            self.max_nodes,
            self.max_map_nodes,
            self.operator_timeout_ms,
        );

        for op in &plan.operators {
            match tokio::time::timeout(ctx.operator_timeout, op.execute(&mut ctx)).await {
                Ok(Ok(_)) => {}
                Ok(Err(e)) => {
                    tracing::warn!("BookRAG operator '{}' failed: {e}", op.name());
                    break; // Return partial results
                }
                Err(_) => {
                    tracing::warn!("BookRAG operator '{}' timed out", op.name());
                    break; // Return partial results
                }
            }
        }

        // 3. Convert: ScoredNode -> MemoryEntry
        ctx.working_set
            .iter()
            .take(limit)
            .map(|node| MemoryEntry {
                id: node.node.id.clone(),
                content: node.node.content.clone(),
                score: node.combined,
                source: MemorySource::Domain {
                    name: "book_index".into(),
                },
                raw_score: node.combined,
            })
            .collect()
    }
}
