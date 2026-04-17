use std::sync::Arc;
use std::time::Instant;

use tracing::warn;

use crate::memory_retriever::MemoryEntry;
use crate::rewriter::RetrievalContext;

use super::traits::{QueryStage, RankingStage};
use super::types::{
    EnhancementBudget, EnhancementOutput, EnhancementTrace, EnhancementTraceSnapshot,
    LatestEnhancementTrace, QueryBundle,
};

// ---------------------------------------------------------------------------
// QueryPipeline
// ---------------------------------------------------------------------------

/// Runs a sequence of [`QueryStage`]s to transform a raw query into an
/// enhanced [`QueryBundle`] ready for retrieval.
pub struct QueryPipeline {
    stages: Vec<Arc<dyn QueryStage>>,
    latest_trace: Option<Arc<LatestEnhancementTrace>>,
}

impl QueryPipeline {
    /// Create a new pipeline with the given stages (run in order).
    pub fn new(stages: Vec<Arc<dyn QueryStage>>) -> Self {
        Self {
            stages,
            latest_trace: None,
        }
    }

    /// Attach a shared latest-trace store. Each `enhance()` call publishes
    /// its trace snapshot to the store, allowing external inspection (e.g.
    /// the `memory` MCP tool's `get_last_enhancement_trace` action).
    pub fn with_latest_trace_store(mut self, store: Arc<LatestEnhancementTrace>) -> Self {
        self.latest_trace = Some(store);
        self
    }

    /// Enhance the query by running all registered stages sequentially.
    ///
    /// Each stage is given the output of the previous stage. On failure the
    /// previous bundle is passed through unchanged and the error is recorded
    /// in the trace.
    pub async fn enhance(
        &self,
        original_query: &str,
        context: &RetrievalContext,
        budget: &EnhancementBudget,
    ) -> EnhancementOutput {
        let mut bundle = QueryBundle::passthrough(original_query);
        let mut trace = EnhancementTrace::default();

        for stage in &self.stages {
            let start = Instant::now();
            let prev_bundle = bundle.clone();

            match stage.transform(bundle, context, budget).await {
                Ok(new_bundle) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    let summary = build_query_summary(&prev_bundle, &new_bundle);
                    trace.record_success(stage.name(), elapsed_ms, 0, 0, summary);
                    bundle = new_bundle;
                }
                Err(err) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    let err_str = err.to_string();
                    warn!(stage = %stage.name(), error = %err_str, "query pipeline stage failed, passing through previous bundle");
                    trace.record_failure(stage.name(), elapsed_ms, err_str);
                    bundle = prev_bundle;
                }
            }
        }

        if let Some(store) = &self.latest_trace {
            if !trace.stages.is_empty() {
                store.set(EnhancementTraceSnapshot {
                    query: bundle.clone(),
                    trace: trace.clone(),
                    captured_at: jiff::Timestamp::now(),
                });
            }
        }

        EnhancementOutput {
            query: bundle,
            trace,
        }
    }
}

/// Build a human-readable summary describing what changed in the bundle.
fn build_query_summary(prev: &QueryBundle, next: &QueryBundle) -> String {
    let mut parts = Vec::new();

    // Collect new sources (those in next but not in prev).
    for src in &next.sources {
        if !prev.sources.contains(src) {
            parts.push(format!("+{src}"));
        }
    }

    let new_variants = next.variants.len().saturating_sub(prev.variants.len());
    if new_variants > 0 {
        parts.push(format!("{new_variants} variants"));
    }

    if parts.is_empty() {
        "no change".to_string()
    } else {
        parts.join(", ")
    }
}

// ---------------------------------------------------------------------------
// RankingPipeline
// ---------------------------------------------------------------------------

/// Runs a sequence of [`RankingStage`]s to re-order retrieval candidates.
pub struct RankingPipeline {
    stages: Vec<Arc<dyn RankingStage>>,
}

impl RankingPipeline {
    /// Create a new ranking pipeline with the given stages (run in order).
    pub fn new(stages: Vec<Arc<dyn RankingStage>>) -> Self {
        Self { stages }
    }

    /// Re-rank the candidates by running all registered stages sequentially.
    ///
    /// Each stage receives the output of the previous stage. On failure the
    /// previous ordering is preserved and the error is recorded in the trace.
    pub async fn rerank(
        &self,
        query: &QueryBundle,
        candidates: Vec<MemoryEntry>,
        budget: &EnhancementBudget,
        trace: &mut EnhancementTrace,
    ) -> Vec<MemoryEntry> {
        let mut current = candidates;

        for stage in &self.stages {
            let start = Instant::now();
            let prev = current.clone();

            match stage.rerank(query, current, budget).await {
                Ok(reranked) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    let summary = format!("{} candidates reranked", reranked.len());
                    trace.record_success(stage.name(), elapsed_ms, 0, 0, summary);
                    current = reranked;
                }
                Err(err) => {
                    let elapsed_ms = start.elapsed().as_millis() as u64;
                    let err_str = err.to_string();
                    warn!(stage = %stage.name(), error = %err_str, "ranking pipeline stage failed, preserving previous order");
                    trace.record_failure(stage.name(), elapsed_ms, err_str);
                    current = prev;
                }
            }
        }

        current
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    use crate::enhancement::types::{QuerySource, StageStatus};
    use crate::memory_retriever::{MemoryEntry, MemorySource};
    use crate::rewriter::RetrievalContext;

    // ---- Helpers -----------------------------------------------------------

    fn make_context() -> RetrievalContext {
        RetrievalContext::default()
    }

    fn make_entry(id: &str, score: f64) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            content: format!("content for {id}"),
            score,
            source: MemorySource::CognitiveFact,
            raw_score: score,
        }
    }

    // ---- Mock stages -------------------------------------------------------

    /// Appends " enriched" to primary, sets confidence 0.8, adds SignalEnrichment.
    struct MockEnrichStage;

    #[async_trait]
    impl QueryStage for MockEnrichStage {
        fn name(&self) -> QuerySource {
            QuerySource::SignalEnrichment
        }

        async fn transform(
            &self,
            mut input: QueryBundle,
            _context: &RetrievalContext,
            _budget: &EnhancementBudget,
        ) -> common::Result<QueryBundle> {
            input.primary = format!("{} enriched", input.primary);
            input.confidence = 0.8;
            input.sources.push(QuerySource::SignalEnrichment);
            Ok(input)
        }
    }

    /// Always returns an error.
    struct FailingStage;

    #[async_trait]
    impl QueryStage for FailingStage {
        fn name(&self) -> QuerySource {
            QuerySource::PseudoRelevanceFeedback
        }

        async fn transform(
            &self,
            _input: QueryBundle,
            _context: &RetrievalContext,
            _budget: &EnhancementBudget,
        ) -> common::Result<QueryBundle> {
            Err(common::KlyntbotError::Bus("test failure".into()))
        }
    }

    /// Pushes "variant query" to variants, adds MultiQuery source.
    struct MockVariantStage;

    #[async_trait]
    impl QueryStage for MockVariantStage {
        fn name(&self) -> QuerySource {
            QuerySource::MultiQuery
        }

        async fn transform(
            &self,
            mut input: QueryBundle,
            _context: &RetrievalContext,
            _budget: &EnhancementBudget,
        ) -> common::Result<QueryBundle> {
            input.variants.push("variant query".to_string());
            input.sources.push(QuerySource::MultiQuery);
            Ok(input)
        }
    }

    /// Reverses the candidate list.
    struct MockReverseRankStage;

    #[async_trait]
    impl RankingStage for MockReverseRankStage {
        fn name(&self) -> QuerySource {
            QuerySource::HeuristicRerank
        }

        async fn rerank(
            &self,
            _query: &QueryBundle,
            mut candidates: Vec<MemoryEntry>,
            _budget: &EnhancementBudget,
        ) -> common::Result<Vec<MemoryEntry>> {
            candidates.reverse();
            Ok(candidates)
        }
    }

    // ---- Tests -------------------------------------------------------------

    #[tokio::test]
    async fn test_pipeline_runs_all_stages() {
        let pipeline =
            QueryPipeline::new(vec![Arc::new(MockEnrichStage), Arc::new(MockVariantStage)]);
        let budget = EnhancementBudget::normal();
        let ctx = make_context();

        let output = pipeline.enhance("hello", &ctx, &budget).await;

        // Primary should have been enriched.
        assert!(
            output.query.primary.contains("enriched"),
            "expected primary to contain 'enriched', got: {}",
            output.query.primary
        );
        assert_eq!(output.query.confidence, 0.8);

        // Variants should contain the mock variant.
        assert!(
            output.query.variants.contains(&"variant query".to_string()),
            "expected 'variant query' in variants"
        );

        // Sources should include both new stages.
        assert!(output
            .query
            .sources
            .contains(&QuerySource::SignalEnrichment));
        assert!(output.query.sources.contains(&QuerySource::MultiQuery));

        // Trace should have exactly 2 successful entries.
        assert_eq!(output.trace.stages.len(), 2);
        for stage_trace in &output.trace.stages {
            assert!(
                matches!(stage_trace.status, StageStatus::Ran),
                "expected Ran, got: {:?}",
                stage_trace.status
            );
        }
    }

    #[tokio::test]
    async fn test_pipeline_degrades_gracefully_on_failure() {
        let pipeline = QueryPipeline::new(vec![
            Arc::new(MockEnrichStage),  // stage 1: success
            Arc::new(FailingStage),     // stage 2: fail
            Arc::new(MockVariantStage), // stage 3: success
        ]);
        let budget = EnhancementBudget::normal();
        let ctx = make_context();

        let output = pipeline.enhance("hello", &ctx, &budget).await;

        // Stage 1 output should be preserved (primary was enriched).
        assert!(
            output.query.primary.contains("enriched"),
            "stage 1 enrichment should survive the failure: {}",
            output.query.primary
        );

        // Stage 3 should still have run (variant added).
        assert!(
            output.query.variants.contains(&"variant query".to_string()),
            "stage 3 should have run after stage 2 failure"
        );

        // Trace should have 3 entries: Ran, Failed, Ran.
        assert_eq!(output.trace.stages.len(), 3);
        assert!(matches!(output.trace.stages[0].status, StageStatus::Ran));
        assert!(matches!(
            output.trace.stages[1].status,
            StageStatus::Failed(_)
        ));
        assert!(matches!(output.trace.stages[2].status, StageStatus::Ran));
    }

    #[tokio::test]
    async fn test_pipeline_passthrough_when_empty() {
        let pipeline = QueryPipeline::new(vec![]);
        let budget = EnhancementBudget::normal();
        let ctx = make_context();

        let output = pipeline.enhance("test query", &ctx, &budget).await;

        assert_eq!(output.query.original, "test query");
        assert_eq!(output.query.primary, "test query");
        assert!(output.query.variants.is_empty());
        assert_eq!(output.query.sources, vec![QuerySource::Passthrough]);
        assert!(output.trace.stages.is_empty());
    }

    #[tokio::test]
    async fn test_ranking_pipeline_reorders() {
        let pipeline = RankingPipeline::new(vec![Arc::new(MockReverseRankStage)]);
        let budget = EnhancementBudget::normal();
        let query = QueryBundle::passthrough("test");
        let mut trace = EnhancementTrace::default();

        let candidates = vec![
            make_entry("a", 0.9),
            make_entry("b", 0.7),
            make_entry("c", 0.5),
        ];

        let result = pipeline
            .rerank(&query, candidates, &budget, &mut trace)
            .await;

        // Reversed: c, b, a.
        assert_eq!(result[0].id, "c");
        assert_eq!(result[1].id, "b");
        assert_eq!(result[2].id, "a");

        // Trace should record the stage.
        assert_eq!(trace.stages.len(), 1);
        assert!(matches!(trace.stages[0].status, StageStatus::Ran));
    }
}
