//! Signal enrichment stage — wraps the existing ContextualQueryRewriter
//! heuristic logic as a QueryStage.

use async_trait::async_trait;
use context_engine::enhancement::{EnhancementBudget, QueryBundle, QuerySource, QueryStage};
use context_engine::rewriter::{QueryRewriter, RetrievalContext};

use super::query_rewriter::ContextualQueryRewriter;

pub struct SignalEnrichmentStage {
    rewriter: ContextualQueryRewriter,
}

impl SignalEnrichmentStage {
    pub fn new(rewriter: ContextualQueryRewriter) -> Self {
        Self { rewriter }
    }
}

#[async_trait]
impl QueryStage for SignalEnrichmentStage {
    fn name(&self) -> QuerySource {
        QuerySource::SignalEnrichment
    }

    async fn transform(
        &self,
        input: QueryBundle,
        context: &RetrievalContext,
        _budget: &EnhancementBudget,
    ) -> common::Result<QueryBundle> {
        // Use the existing heuristic rewrite logic via the QueryRewriter trait
        let result = self.rewriter.rewrite(&input.original, context).await;
        match result {
            Some(r) => Ok(QueryBundle {
                original: input.original,
                primary: r.enriched_query,
                variants: input.variants,
                confidence: r.confidence,
                sources: {
                    let mut s = input.sources;
                    s.push(QuerySource::SignalEnrichment);
                    s
                },
            }),
            None => Ok(input),
        }
    }
}
